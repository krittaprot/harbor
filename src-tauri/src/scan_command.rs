//! Bounded read-only scanner subprocesses. Never used for service mutations.
use std::io::{self, Read};
use std::os::fd::AsRawFd;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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
    let mut child = Command::new(program)
        .args(args)
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
            if started.elapsed() >= Duration::from_secs(3) {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("{program} exceeded 3 seconds"),
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
    fn timeout_terminates_and_reaps_our_child() {
        let started = Instant::now();
        let error = output("/bin/sleep", &["10"]).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
