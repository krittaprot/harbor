use crate::{
    config,
    login::{LoginCache, LoginItems},
    model::Snapshot,
    scanner::Scanner,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, sync::Mutex};
use tauri::{Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedSnapshot {
    pub snapshot: Snapshot,
    pub login: Option<LoginItems>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestartReport {
    pub before: String,
    pub boot_time: String,
    pub observed_at: String,
    pub came_back: Vec<String>,
    pub missing: Vec<String>,
    pub new: Vec<String>,
    pub login_returned: Option<usize>,
    pub login_missing: Option<Vec<String>>,
    pub unmanaged_missing: Vec<String>,
    pub ports_missing: Vec<u16>,
    pub seconds_after_first_agent: Option<u64>,
}
#[derive(Default)]
pub struct SnapshotState {
    pub report: Mutex<Option<RestartReport>>,
}

fn running(s: &Snapshot) -> BTreeSet<String> {
    s.services
        .iter()
        .filter(|s| !s.apple)
        .flat_map(|s| &s.jobs)
        .filter(|j| j.pid.is_some())
        .map(|j| j.label.clone())
        .collect()
}

pub fn compare(
    before: &SavedSnapshot,
    after: &Snapshot,
    login: Option<&LoginItems>,
) -> Option<RestartReport> {
    let boot = after.system.boot_time?;
    if boot <= before.snapshot.taken_at {
        return None;
    }
    let old = running(&before.snapshot);
    let now = running(after);
    let login_missing = before.login.as_ref().zip(login).map(|(old, now)| {
        old.apps
            .iter()
            .filter(|a| a.enabled && !now.apps.iter().any(|b| b.path == a.path && b.running))
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
    });
    let login_returned = before.login.as_ref().zip(login).map(|(old, now)| {
        old.apps
            .iter()
            .filter(|a| a.enabled && now.apps.iter().any(|b| b.path == a.path && b.running))
            .count()
    });
    let first = after
        .services
        .iter()
        .flat_map(|s| &s.processes)
        .filter_map(|p| p.started_at)
        .filter(|t| *t >= boot)
        .min();
    let temporary_pids: BTreeSet<_> = before
        .snapshot
        .unmanaged
        .iter()
        .filter(|g| g.verdict == "Expected")
        .flat_map(|g| g.copies.iter().map(|p| p.pid))
        .collect();
    Some(RestartReport {
        before: before.snapshot.taken_at.to_rfc3339(),
        boot_time: boot.to_rfc3339(),
        observed_at: after.taken_at.to_rfc3339(),
        came_back: old.intersection(&now).cloned().collect(),
        missing: old.difference(&now).cloned().collect(),
        new: now.difference(&old).cloned().collect(),
        login_returned,
        login_missing,
        unmanaged_missing: before
            .snapshot
            .unmanaged
            .iter()
            .filter(|g| g.verdict != "Expected")
            .filter(|g| !after.unmanaged.iter().any(|n| n.key == g.key))
            .map(|g| g.name.clone())
            .collect(),
        ports_missing: before
            .snapshot
            .ports
            .iter()
            .filter(|p| !temporary_pids.contains(&p.pid))
            .map(|p| p.port)
            .collect::<BTreeSet<_>>()
            .difference(&after.ports.iter().map(|p| p.port).collect())
            .copied()
            .collect(),
        seconds_after_first_agent: if old.is_subset(&now) {
            first.map(|t| (after.taken_at - t).num_seconds().max(0) as u64)
        } else {
            None
        },
    })
}

pub fn latest() -> Result<Option<SavedSnapshot>, String> {
    let directory = config::directory().join("snapshots");
    if !directory.exists() {
        return Ok(None);
    }
    let mut paths: Vec<_> = fs::read_dir(directory)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    paths.sort();
    for path in paths.iter().rev() {
        if let Ok(bytes) = fs::read(path)
            && let Ok(snapshot) = serde_json::from_slice(&bytes)
        {
            return Ok(Some(snapshot));
        }
    }
    Ok(None)
}

pub fn save(snapshot: Snapshot, login: Option<LoginItems>) -> Result<String, String> {
    let directory = config::directory().join("snapshots");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path = directory.join(format!(
        "{}.json",
        chrono::Utc::now().format("%Y%m%dT%H%M%S%.9fZ")
    ));
    let temp = path.with_extension("tmp");
    let data = SavedSnapshot { snapshot, login };
    fs::write(
        &temp,
        serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::rename(temp, &path).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn save_snapshot(
    scanner: State<'_, Scanner>,
    login: State<'_, LoginCache>,
) -> Result<String, String> {
    save(
        scanner.snapshot()?,
        login.0.lock().map_err(|e| e.to_string())?.clone(),
    )
}

#[tauri::command]
pub fn last_restart_report(
    state: State<'_, SnapshotState>,
) -> Result<Option<RestartReport>, String> {
    Ok(state.report.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
pub async fn restart_machine(confirmed: bool, app: tauri::AppHandle) -> Result<(), String> {
    if !confirmed {
        return Err("Restart requires explicit confirmation".into());
    }
    let snapshot = app.state::<Scanner>().rescan().await?;
    let login = app
        .state::<LoginCache>()
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .clone();
    save(snapshot, login)?;
    tauri::async_runtime::spawn_blocking(|| {
        let output = std::process::Command::new("/usr/bin/osascript")
            .args(["-e", "tell application \"System Events\" to restart"])
            .output()
            .map_err(|e| e.to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn temporary_processes_and_their_ports_are_not_missing_after_restart() {
        use crate::model::{PortInfo, ProcInfo, UnmanagedGroup};
        let t = chrono::DateTime::from_timestamp(1000, 0).unwrap();
        let mut before = Snapshot {
            taken_at: t,
            ..Default::default()
        };
        for (pid, port, verdict) in [
            (10, 9000, "Expected"),
            (11, 9001, "Orphaned"),
            (12, 9002, "Expected"),
            (13, 9002, "Unknown"),
        ] {
            let binding = PortInfo {
                pid,
                port,
                addr: "127.0.0.1".into(),
            };
            before.ports.push(binding.clone());
            before.unmanaged.push(UnmanagedGroup {
                key: format!("process-{pid}"),
                name: format!("process-{pid}"),
                copies: vec![ProcInfo {
                    pid,
                    ppid: 1,
                    uid: Some(501),
                    name: "fixture".into(),
                    argv: vec![],
                    cwd: None,
                    cpu: None,
                    mem_bytes: 0,
                    started_at: Some(t),
                    launchd_label: None,
                }],
                ports: vec![binding],
                verdict: verdict.into(),
                app_path: None,
                hint: None,
            });
        }
        let saved = SavedSnapshot {
            snapshot: before,
            login: None,
        };
        let mut after = Snapshot {
            taken_at: t + chrono::Duration::seconds(120),
            ..Default::default()
        };
        after.system.boot_time = Some(t + chrono::Duration::seconds(60));
        let report = compare(&saved, &after, None).unwrap();
        assert_eq!(report.unmanaged_missing, vec!["process-11", "process-13"]);
        // A port shared with a non-temporary process still needs to be reported.
        assert_eq!(report.ports_missing, vec![9001, 9002]);
    }

    #[test]
    fn restart_comparison_requires_a_new_boot_and_reports_missing_ports() {
        let t = chrono::DateTime::from_timestamp(1000, 0).unwrap();
        let mut before = Snapshot {
            taken_at: t,
            ..Default::default()
        };
        before.ports.push(crate::model::PortInfo {
            pid: 10,
            port: 5432,
            addr: "*".into(),
        });
        let saved = SavedSnapshot {
            snapshot: before,
            login: Some(LoginItems {
                apps: vec![crate::login::LoginApp {
                    name: "Example".into(),
                    path: "/Applications/Example.app".into(),
                    enabled: true,
                    running: true,
                }],
                extensions: String::new(),
            }),
        };
        let mut after = Snapshot {
            taken_at: t + chrono::Duration::seconds(120),
            ..Default::default()
        };
        after.system.boot_time = Some(t - chrono::Duration::seconds(1));
        assert!(compare(&saved, &after, None).is_none());
        after.system.boot_time = Some(t + chrono::Duration::seconds(60));
        let report = compare(&saved, &after, None).unwrap();
        assert_eq!(report.ports_missing, vec![5432]);
        assert_eq!(report.login_missing, None);
        let report = compare(
            &saved,
            &after,
            Some(&LoginItems {
                apps: vec![],
                extensions: String::new(),
            }),
        )
        .unwrap();
        assert_eq!(report.login_missing, Some(vec!["Example".into()]));
        assert_eq!(report.login_returned, Some(0));
    }
}
