use crate::{config, model::Snapshot, ownership, plist_util};
use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};

pub fn save_adoption(label: &str, record: &Value) -> Result<(), String> {
    if !config::label_valid(label) {
        return Err("Invalid label".into());
    }
    let dir = config::directory().join("adoptions");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{label}.json"));
    let temp = dir.join(format!("{label}.{}.tmp", std::process::id()));
    fs::write(
        &temp,
        serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, path).map_err(|e| e.to_string())
}

fn last_adoption(label: &str) -> Option<Value> {
    if !config::label_valid(label) {
        return None;
    }
    let path = config::directory()
        .join("adoptions")
        .join(format!("{label}.json"));
    if fs::metadata(&path).ok()?.len() > 2 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice(&fs::read(path).ok()?).ok()
}

fn log_tail(path: Option<&str>) -> Value {
    let Some(path) = path else {
        return json!({"configured":false});
    };
    let read = || -> Result<(String, bool), String> {
        let mut file = fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .map_err(|e| e.to_string())?;
        let meta = file.metadata().map_err(|e| e.to_string())?;
        if !meta.is_file() {
            return Err("Not a regular log file".into());
        }
        let start = meta.len().saturating_sub(32768);
        file.seek(SeekFrom::Start(start))
            .map_err(|e| e.to_string())?;
        let mut bytes = Vec::new();
        file.take(32768)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        Ok((String::from_utf8_lossy(&bytes).into_owned(), start > 0))
    };
    match read() {
        Ok((text, truncated)) => json!({"path":path,"text":text,"truncated":truncated}),
        Err(error) => json!({"path":path,"error":error}),
    }
}

fn findings(argv: &[String]) -> Vec<Value> {
    if ownership::opencode_service(argv) {
        vec![
            json!({"code":"self_managed_singleton","message":ownership::OPENCODE_GUIDANCE,
            "next_steps":["opencode service status","opencode api get /api/info"],
            "registration":ownership::opencode_registration()}),
        ]
    } else {
        vec![]
    }
}

fn job_report(snapshot: &Snapshot, label: &str, path: &str) -> Result<Value, String> {
    let plist = plist_util::parse_plist(path).map_err(|e| e.to_string())?;
    if plist.label != label {
        return Err("Plist label does not match the requested job".into());
    }
    let argv = plist
        .program_arguments
        .clone()
        .unwrap_or_else(|| plist.program.clone().into_iter().collect());
    let observed = snapshot
        .services
        .iter()
        .flat_map(|s| &s.jobs)
        .find(|j| j.label == label);
    let mut issues = findings(&argv);
    if let Some(executable) = plist.program.as_ref().or(argv.first())
        && !Path::new(executable).exists()
    {
        issues.push(json!({"code":"missing_executable","message":format!("Executable does not exist: {executable}")}));
    }
    if let Some(cwd) = &plist.working_directory
        && !Path::new(cwd).is_dir()
    {
        issues.push(json!({"code":"missing_working_directory","message":format!("Working directory is unavailable: {cwd}")}));
    }
    if observed.is_some_and(|j| j.pid.is_none() && j.last_exit == Some(0)) {
        issues.push(json!({"code":"exited_successfully","message":"No running PID and last exit 0. A successful exit is not readiness; check for singleton ownership or a command that exits/daemonizes."}));
    }
    let history = last_adoption(label);
    Ok(
        json!({"label":label,"plist_path":path,"observed_job":observed,"command":argv,
        "working_directory":plist.working_directory,"run_at_load":plist.run_at_load,"keep_alive":plist.keep_alive,
        "findings":issues,"last_adoption":history,
        "logs":{"stdout":log_tail(plist.standard_out_path.as_deref()),"stderr":log_tail(plist.standard_error_path.as_deref())}}),
    )
}

pub fn report(snapshot: &Snapshot, target: &str) -> Result<Value, String> {
    let mut jobs = Vec::new();
    let selected: Vec<_> = snapshot
        .services
        .iter()
        .flat_map(|s| &s.jobs)
        .filter(|j| j.label == target)
        .collect();
    if !selected.is_empty() {
        for job in selected {
            if let Some(path) = &job.plist_path {
                jobs.push(job_report(snapshot, &job.label, path)?);
            }
        }
    } else if let Some(service) = snapshot.services.iter().find(|s| s.id == target) {
        for job in &service.jobs {
            if let Some(path) = &job.plist_path {
                jobs.push(job_report(snapshot, &job.label, path)?);
            }
        }
    } else if config::label_valid(target) {
        for base in [
            plist_util::get_user_agents_dir(),
            plist_util::get_user_agents_dir().join(".harbor-archive"),
        ] {
            let path = base.join(format!("{target}.plist"));
            if path.is_file() {
                jobs.push(job_report(snapshot, target, &path.to_string_lossy())?);
                break;
            }
        }
    }
    let groups: Vec<_> = snapshot
        .unmanaged
        .iter()
        .filter(|g| g.key == target || g.name.eq_ignore_ascii_case(target))
        .collect();
    if jobs.is_empty() && groups.len() > 1 {
        return Err("Ambiguous group name; use its exact key".into());
    }
    let group = if jobs.is_empty() {
        groups.first().copied()
    } else {
        None
    };
    let history = last_adoption(target);
    if jobs.is_empty() && group.is_none() && history.is_none() {
        return Err(format!(
            "No service, job, group or adoption report named '{target}'"
        ));
    }
    let group_findings = group
        .and_then(|g| g.copies.first())
        .map(|p| findings(&p.argv))
        .unwrap_or_default();
    Ok(
        json!({"version":env!("CARGO_PKG_VERSION"),"target":target,"taken_at":snapshot.taken_at,"jobs":jobs,"group":group,"findings":group_findings,
        "last_adoption":history,"scan_diagnostics":snapshot.diagnostics}),
    )
}

#[tauri::command]
pub async fn diagnose_target(target: String) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || report(&crate::scanner::scan_once()?, &target))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn copy_text(text: String) -> Result<(), String> {
    if text.len() > 4 * 1024 * 1024 {
        return Err("Report is too large to copy".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = std::process::Command::new("/usr/bin/pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        let written = child
            .stdin
            .take()
            .ok_or("Clipboard input unavailable")?
            .write_all(text.as_bytes());
        let status = child.wait().map_err(|e| e.to_string())?;
        written.map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("Could not write to the clipboard".into())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
