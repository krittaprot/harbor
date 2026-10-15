use crate::{
    config,
    model::{ProcInfo, UnmanagedGroup},
    procs::ProcessScanner,
    scanner::Scanner,
};
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::Path,
    time::{Duration, Instant},
};
use tauri::State;

#[derive(Debug, Clone, Serialize)]
pub struct AdoptionProgress {
    stage: &'static str,
    message: String,
    warnings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Options {
    pub run_at_load: bool,
    pub keep_alive: bool,
    pub logs: bool,
    pub stop_copies: bool,
}

fn verify(expected: &ProcInfo) -> Result<ProcInfo, String> {
    let mut scanner = ProcessScanner::default();
    let table = scanner.scan();
    let fresh = table
        .by_pid
        .get(&expected.pid)
        .ok_or("Process exited; rescan")?;
    let uid = unsafe { libc::getuid() };
    if fresh.uid != Some(uid)
        || expected.uid != Some(uid)
        || expected.started_at.is_none()
        || fresh.started_at != expected.started_at
        || fresh.argv != expected.argv
        || fresh.argv.is_empty()
    {
        return Err("Process identity or owner changed; rescan before acting".into());
    }
    Ok(fresh.clone())
}

fn group(key: &str, scanner: &Scanner) -> Result<UnmanagedGroup, String> {
    scanner
        .snapshot()?
        .unmanaged
        .into_iter()
        .find(|g| g.key == key)
        .ok_or_else(|| "Unmanaged group no longer exists".into())
}

pub fn xml(
    label: &str,
    argv: &[String],
    cwd: &str,
    logs: &str,
    options: &Options,
) -> Result<String, String> {
    if !config::label_valid(label) {
        return Err(
            "Use a label containing only letters, numbers, dots, underscores and hyphens".into(),
        );
    }
    if argv.is_empty() || !Path::new(&argv[0]).is_absolute() || !Path::new(cwd).is_absolute() {
        return Err("An absolute executable and working directory are required".into());
    }
    let mut dict = plist::Dictionary::new();
    dict.insert("Label".into(), label.into());
    dict.insert(
        "ProgramArguments".into(),
        plist::Value::Array(argv.iter().map(|a| a.as_str().into()).collect()),
    );
    dict.insert("WorkingDirectory".into(), cwd.into());
    dict.insert("RunAtLoad".into(), options.run_at_load.into());
    dict.insert("KeepAlive".into(), options.keep_alive.into());
    dict.insert("ThrottleInterval".into(), 10u64.into());
    if options.logs {
        dict.insert("StandardOutPath".into(), format!("{logs}/out.log").into());
        dict.insert("StandardErrorPath".into(), format!("{logs}/err.log").into());
    }
    let mut bytes = Vec::new();
    plist::Value::Dictionary(dict)
        .to_writer_xml(&mut bytes)
        .map_err(|e| e.to_string())?;
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn preview(group: &UnmanagedGroup, label: &str, options: &Options) -> Result<String, String> {
    let process = verify(group.copies.first().ok_or("No process to adopt")?)?;
    let executable = libproc::libproc::proc_pid::pidpath(process.pid as i32)
        .map_err(|e| format!("Cannot resolve executable: {e}"))?;
    let mut argv = process.argv;
    argv[0] = executable;
    let cwd = process.cwd.ok_or("Working directory could not be read")?;
    let logs = dirs::home_dir()
        .ok_or("No home directory")?
        .join("Library/Logs")
        .join(label);
    xml(label, &argv, &cwd, &logs.to_string_lossy(), options)
}

#[tauri::command]
pub async fn adopt_preview(
    key: String,
    label: String,
    options: Options,
    scanner: State<'_, Scanner>,
) -> Result<String, String> {
    let group = group(&key, &scanner)?;
    tauri::async_runtime::spawn_blocking(move || preview(&group, &label, &options))
        .await
        .map_err(|e| e.to_string())?
}

fn terminate(process: &ProcInfo) -> Result<String, String> {
    verify(process)?;
    // Identity, argv and UID were re-read immediately above. Never SIGKILL.
    if unsafe { libc::kill(process.pid as i32, libc::SIGTERM) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let start = Instant::now();
    loop {
        let mut scanner = ProcessScanner::default();
        let table = scanner.scan();
        if table
            .by_pid
            .get(&process.pid)
            .is_none_or(|p| p.started_at != process.started_at)
        {
            return Ok(format!("PID {} exited", process.pid));
        }
        if start.elapsed() >= Duration::from_secs(5) {
            return Err(format!(
                "PID {} is still running after SIGTERM; no force kill was sent",
                process.pid
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[tauri::command]
pub async fn stop_unmanaged(
    key: String,
    scanner: State<'_, Scanner>,
) -> Result<Vec<String>, String> {
    let group = group(&key, &scanner)?;
    tauri::async_runtime::spawn_blocking(move || {
        for p in &group.copies {
            verify(p)?;
        }
        Ok(group
            .copies
            .iter()
            .map(|p| terminate(p).unwrap_or_else(|e| e))
            .collect())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn adopt_apply(
    key: String,
    label: String,
    options: Options,
    expected_xml: String,
    on_progress: tauri::ipc::Channel<AdoptionProgress>,
    scanner: State<'_, Scanner>,
) -> Result<Vec<String>, String> {
    let group = group(&key, &scanner)?;
    tauri::async_runtime::spawn_blocking(move || {
        let report = |stage, message: String, warnings| {
            let _ = on_progress.send(AdoptionProgress { stage, message, warnings });
        };
        report("verify", "Checking process identity and the reviewed configuration…".into(), false);
        let contents = preview(&group, &label, &options)?;
        if contents != expected_xml { return Err("Process/options changed since preview; review a new preview".into()); }
        if crate::launchctl::list_loaded().map_err(|e| e.to_string())?.iter().any(|j| j.label == label) { return Err("That label is already loaded".into()); }
        let base = crate::plist_util::get_user_agents_dir();
        std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
        let path = base.join(format!("{label}.plist"));
        let path_string = path.to_string_lossy().into_owned();
        config::user_plist(&path_string)?;
        report("write", "Writing and validating the LaunchAgent plist…".into(), false);
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(|e| e.to_string())?;
        file.write_all(contents.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        let lint = crate::scan_command::output("/usr/bin/plutil", &["-lint", &path_string]).map_err(|e| e.to_string())?;
        if !lint.status.success() { return Err("Generated plist failed plutil validation; not loaded".into()); }
        if options.logs { std::fs::create_dir_all(dirs::home_dir().unwrap().join("Library/Logs").join(&label)).map_err(|e| e.to_string())?; }
        report("load", "Asking launchd to load the new agent…".into(), false);
        crate::launchctl::bootstrap(&path_string).map_err(|e| e.to_string())?;
        let readiness_started = Instant::now();
        report("ready", "Waiting for the new agent to own a listening port (up to 15s)…".into(), false);
        let deadline = readiness_started + Duration::from_secs(15);
        let mut ready = false;
        let mut observation = "Waiting for launchd to report a process".to_string();
        while Instant::now() < deadline {
            let loaded = crate::launchctl::list_loaded().map_err(|e| e.to_string())?;
            let job = loaded.iter().find(|j| j.label == label);
            if let Some(pid) = job.and_then(|j| j.pid) {
                let mut ps = ProcessScanner::default();
                let table = ps.scan();
                let mut pids = table.descendants(pid); pids.push(pid);
                let (ports, _) = crate::ports::scan(&table, unsafe { libc::getuid() });
                if group.ports.is_empty() || ports.iter().any(|p| pids.contains(&p.pid)) { ready = true; break; }
                observation = format!("PID {pid} is running; waiting for it or a child to own a listening port");
            } else {
                observation = match job.and_then(|j| j.last_exit_code) {
                    Some(code) => format!("No running PID; last exit code {code}"),
                    None => "Waiting for launchd to report a process".into(),
                };
            }
            report("ready", format!("{observation}. Checking for up to 15s ({}s elapsed).", readiness_started.elapsed().as_secs()), false);
            std::thread::sleep(Duration::from_millis(300));
        }
        if !ready {
            report("rollback", "Readiness timed out. Unloading the new agent; originals have not been stopped…".into(), false);
            let target = format!("gui/{}/{label}", unsafe { libc::getuid() });
            let diagnostic = crate::scan_command::output("/bin/launchctl", &["print", &target]).ok().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
            let rollback = crate::launchctl::bootout(&path_string);
            let unload = match rollback { Ok(()) => "The new agent was unloaded.".to_string(), Err(e) => format!("Could not unload the new agent: {e}. Inspect it in Services.") };
            return Err(format!("The new agent did not become ready within 15 seconds.\nOriginal processes were not stopped. {unload}\nLast observation: {observation}.\nPlist retained at {path_string}. Review the command and logs before trying another adoption; this label's plist already exists.\nA fixed port conflict or a command that exits instead of staying in the foreground can prevent readiness.\n\nlaunchctl diagnostics:\n{diagnostic}"));
        }
        let mut results = vec![format!("{label} loaded and ready")];
        let mut warnings = false;
        if options.stop_copies {
            for (index, process) in group.copies.iter().enumerate() {
                report("stop", format!("New agent is ready. Stopping original {}/{} (PID {}); allowing up to 5s for exit.", index + 1, group.copies.len(), process.pid), false);
                match terminate(process) {
                    Ok(message) => results.push(message),
                    Err(error) => { warnings = true; results.push(error); }
                }
            }
        }
        report("complete", if warnings { "Agent ready, but some original copies could not be stopped. Review the results below." } else { "The new launch agent is ready." }.into(), warnings);
        Ok(results)
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plist_escapes_arguments_and_enforces_paths() {
        let opts = Options {
            run_at_load: true,
            keep_alive: true,
            logs: true,
            stop_copies: false,
        };
        let text = xml(
            "com.test.harbor",
            &["/usr/bin/python3".into(), "a&b.py".into()],
            "/some folder",
            "/logs/test",
            &opts,
        )
        .unwrap();
        let value = plist::Value::from_reader_xml(text.as_bytes()).unwrap();
        let d = value.as_dictionary().unwrap();
        assert_eq!(d["ThrottleInterval"].as_unsigned_integer(), Some(10));
        assert_eq!(
            d["ProgramArguments"].as_array().unwrap()[1].as_string(),
            Some("a&b.py")
        );
        assert!(xml("../escape", &["/bin/true".into()], "/", "/logs", &opts).is_err());
        assert!(xml("valid", &["python".into()], "/", "/logs", &opts).is_err());
    }
}
