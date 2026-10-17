//! Bounded subprocess capture. Deadlines terminate only the owned helper;
//! callers must not interpret a timeout as proof that a requested mutation rolled back.
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

fn nonblocking(pipe: &impl AsRawFd) -> io::Result<()> {
    // SAFETY: the borrowed pipe owns a valid descriptor for the entire call.
    let result = unsafe { libc::fcntl(pipe.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn drain(pipe: &mut impl Read, bytes: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer = [0; 8192];
    // Bound each turn so a noisy child cannot prevent the deadline check.
    for _ in 0..128 {
        match pipe.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                if bytes.len() + n > 8 * 1024 * 1024 {
                    return Err(io::Error::other("scanner output exceeded 8 MiB"));
                }
                bytes.extend_from_slice(&buffer[..n]);
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

pub fn output(program: &str, args: &[&str]) -> io::Result<Output> {
    output_with_timeout(program, args, DEFAULT_TIMEOUT)
}

pub fn output_with_timeout(program: &str, args: &[&str], timeout: Duration) -> io::Result<Output> {
    capture(Command::new(program).args(args), timeout)
}

/// Override only this helper's environment, without changing the app or other workers.
pub fn output_with_env(program: &str, args: &[&str], env: &[(&str, &str)]) -> io::Result<Output> {
    capture(
        Command::new(program).args(args).envs(env.iter().copied()),
        DEFAULT_TIMEOUT,
    )
}

fn capture(command: &mut Command, timeout: Duration) -> io::Result<Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("piped stdout");
    let mut stderr = child.stderr.take().expect("piped stderr");
    let result = (|| {
        nonblocking(&stdout)?;
        nonblocking(&stderr)?;
        let started = Instant::now();
        let mut out = Vec::new();
        let mut err = Vec::new();
        loop {
            drain(&mut stdout, &mut out)?;
            drain(&mut stderr, &mut err)?;
            if let Some(status) = child.try_wait()? {
                drain(&mut stdout, &mut out)?;
                drain(&mut stderr, &mut err)?;
                return Ok(Output {
                    status,
                    stdout: out,
                    stderr: err,
                });
            }
            if started.elapsed() >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{program} exceeded {} seconds", timeout.as_secs()),
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    })();
    if result.is_err() && child.try_wait()?.is_none() {
        // This is our own direct, unreaped child of the exact command above;
        // its PID cannot be reused. Only SIGTERM, never SIGKILL.
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(1);
        while child.try_wait()?.is_none() {
            if Instant::now() >= deadline {
                // Keep ownership and reap eventual exit without blocking scans/UI.
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                return Err(io::Error::other(format!(
                    "{program} did not exit within one second of SIGTERM"
                )));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drains_output_and_preserves_status() {
        let result = output("/usr/bin/printf", &["p12\\nn*:5432\\n"]).unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, b"p12\nn*:5432\n");
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn helper_environment_is_explicit_and_scoped_to_the_child() {
        let before = std::env::var_os("TAILSCALE_BE_CLI");
        let result = output_with_env(
            "/usr/bin/printenv",
            &["TAILSCALE_BE_CLI"],
            &[("TAILSCALE_BE_CLI", "1")],
        )
        .unwrap();
        assert!(result.status.success());
        assert_eq!(result.stdout, b"1\n");
        assert_eq!(std::env::var_os("TAILSCALE_BE_CLI"), before);
    }

    #[test]
    fn timeout_terminates_and_reaps_our_child() {
        let started = Instant::now();
        let error = output("/bin/sleep", &["10"]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
