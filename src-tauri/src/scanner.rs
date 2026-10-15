use crate::{
    config, grouping, health, launchctl,
    model::{Counts, Snapshot},
    ports,
    procs::ProcessScanner,
    scan_command, system,
};
use chrono::Utc;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};

pub struct Scanner {
    cache: Arc<RwLock<Option<Snapshot>>>,
    stop: mpsc::Sender<Message>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

enum Message {
    Stop,
    Rescan(tokio::sync::oneshot::Sender<Snapshot>),
}

pub fn collect(processes: &mut ProcessScanner) -> Snapshot {
    let started = Instant::now();
    let mut table = processes.scan();
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    let (ports, mut diagnostics) = ports::scan(&table, uid);
    let mut loaded = vec![];
    match scan_command::output("/bin/launchctl", &["list"]) {
        Ok(output) if output.status.success() => {
            loaded = launchctl::parse_list_output(&String::from_utf8_lossy(&output.stdout));
            let jobs: BTreeMap<_, _> = loaded
                .iter()
                .filter_map(|job| job.pid.map(|pid| (pid, job.label.clone())))
                .collect();
            table.assign_launchd_labels(&jobs);
        }
        Ok(output) => diagnostics.warnings.push(format!(
            "launchctl list: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        Err(error) => diagnostics
            .warnings
            .push(format!("launchctl list unavailable: {error}")),
    }
    let config = config::load().unwrap_or_else(|e| {
        diagnostics.warnings.push(e);
        config::Config::default()
    });
    let processes: Vec<_> = table.by_pid.into_values().collect();
    #[cfg(feature = "smoke-test")]
    let processes = crate::smoke::isolate_fixture(processes, uid);
    let jobs = grouping::jobs(&loaded, &mut diagnostics.warnings);
    let not_loaded = jobs
        .iter()
        .filter(|j| j.editable && j.status == "Unloaded")
        .cloned()
        .collect();
    let services = grouping::group(jobs, &processes, &ports, &config);
    let (unmanaged, app_owned) = grouping::unmanaged(&processes, &ports, &config);
    diagnostics.duration_ms = started.elapsed().as_millis() as u64;
    Snapshot {
        taken_at: Utc::now(),
        uid,
        listening_port_count: ports.iter().map(|p| p.port).collect::<BTreeSet<_>>().len(),
        processes,
        ports,
        diagnostics,
        services,
        unmanaged,
        app_owned,
        not_loaded,
        system: crate::model::SystemInfo::default(),
        counts: Counts::default(),
    }
}

impl Scanner {
    pub fn start(app: tauri::AppHandle) -> Self {
        let cache = Arc::new(RwLock::new(None));
        let shared = Arc::clone(&cache);
        let (stop, receiver) = mpsc::channel();
        // A dedicated worker keeps all sysinfo/libproc/subprocess work off the
        // UI and async executor, and owns the same System for CPU deltas.
        let worker = std::thread::spawn(move || {
            let mut processes = ProcessScanner::default();
            let mut health = health::HealthScanner::default();
            let mut system_cache = None;
            let mut system_time = Instant::now();
            let mut previous: Option<Snapshot> = None;
            let mut reply: Option<tokio::sync::oneshot::Sender<Snapshot>> = None;
            let baseline = crate::snapshots::latest().unwrap_or(None);
            let mut last_save = Instant::now();
            let mut notifications = crate::tray::Notifications::default();
            let mut reported_restart = false;
            loop {
                let started = Instant::now();
                let mut snapshot = collect(&mut processes);
                if system_cache.is_none() || system_time.elapsed() >= Duration::from_secs(60) {
                    system_cache = Some(system::scan(snapshot.uid));
                    system_time = Instant::now();
                }
                snapshot.system = system_cache.clone().unwrap();
                health.update(&mut snapshot.services);
                if let Some(old) = &previous {
                    for service in &mut snapshot.services {
                        if service.state == "Running"
                            && service.jobs.iter().any(|job| {
                                job.last_exit.is_some_and(|e| e != 0)
                                    && job.pid.is_some()
                                    && old
                                        .services
                                        .iter()
                                        .flat_map(|s| &s.jobs)
                                        .any(|j| j.label == job.label && j.pid != job.pid)
                            })
                        {
                            service.state = "Restarted".into();
                        }
                    }
                }
                let visible: Vec<_> = snapshot.services.iter().filter(|s| !s.apple).collect();
                snapshot.counts = Counts {
                    healthy: visible
                        .iter()
                        .filter(|s| {
                            ["Ok", "PortOpen"].contains(&s.health.status.as_str())
                                && s.state != "Failed"
                        })
                        .count(),
                    attention: visible
                        .iter()
                        .filter(|s| {
                            s.state == "Failed"
                                || s.state == "Restarted"
                                || s.health.status == "Failed"
                        })
                        .count(),
                    running: visible
                        .iter()
                        .flat_map(|s| &s.jobs)
                        .filter(|j| j.pid.is_some())
                        .count(),
                    unmanaged: snapshot
                        .unmanaged
                        .iter()
                        .filter(|g| g.verdict != "Expected")
                        .map(|g| g.copies.len())
                        .sum(),
                    not_loaded: snapshot.not_loaded.len(),
                    ports: snapshot.listening_port_count,
                    hidden: snapshot
                        .services
                        .iter()
                        .filter(|s| s.apple)
                        .flat_map(|s| &s.jobs)
                        .count(),
                };
                if let Err(e) = config::seed(&snapshot.services) {
                    snapshot.diagnostics.warnings.push(e);
                }
                snapshot.diagnostics.duration_ms = started.elapsed().as_millis() as u64;
                snapshot.taken_at = Utc::now();
                crate::tray::update(&app, &snapshot);
                notifications.update(&app, &snapshot);
                let login = app
                    .state::<crate::login::LoginCache>()
                    .0
                    .lock()
                    .ok()
                    .and_then(|c| c.clone());
                if let Some(before) = &baseline
                    && let Some(mut report) =
                        crate::snapshots::compare(before, &snapshot, login.as_ref())
                {
                    let state = app.state::<crate::snapshots::SnapshotState>();
                    if let Ok(mut current) = state.report.lock() {
                        if let Some(old) = current.as_ref()
                            && old.missing.is_empty()
                        {
                            report.seconds_after_first_agent = old.seconds_after_first_agent;
                        }
                        *current = Some(report.clone());
                    }
                    if !reported_restart {
                        let _ = app.emit("harbor://navigate", "restart");
                        if config::load().is_ok_and(|c| c.notifications)
                            && !cfg!(feature = "smoke-test")
                        {
                            use tauri_plugin_notification::NotificationExt;
                            let _ = app
                                .notification()
                                .builder()
                                .title("Harbor · restart report")
                                .body(format!(
                                    "{} restarted · {}/{} jobs came back",
                                    snapshot.system.hostname,
                                    report.came_back.len(),
                                    report.came_back.len() + report.missing.len()
                                ))
                                .show();
                        }
                        reported_restart = true;
                    }
                }
                if !cfg!(feature = "smoke-test")
                    && (last_save.elapsed() >= Duration::from_secs(3600)
                        || previous.is_none() && baseline.is_none())
                {
                    if let Err(error) = crate::snapshots::save(snapshot.clone(), login) {
                        snapshot
                            .diagnostics
                            .warnings
                            .push(format!("Automatic snapshot: {error}"));
                    }
                    last_save = Instant::now();
                }
                *shared.write().expect("snapshot lock") = Some(snapshot.clone());
                if let Err(error) = app.emit("harbor://snapshot", &snapshot) {
                    eprintln!("Harbor snapshot event: {error}");
                }
                if let Some(reply) = reply.take() {
                    let _ = reply.send(snapshot.clone());
                }
                previous = Some(snapshot);
                match receiver
                    .recv_timeout(Duration::from_secs(10).saturating_sub(started.elapsed()))
                {
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                    Ok(Message::Rescan(sender)) => reply = Some(sender),
                    _ => break,
                }
            }
        });
        Self {
            cache,
            stop,
            worker: Mutex::new(Some(worker)),
        }
    }

    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.cache
            .read()
            .map_err(|_| "snapshot cache unavailable".to_string())?
            .clone()
            .ok_or_else(|| "initial scan in progress; wait for harbor://snapshot".into())
    }

    pub fn stop(&self) {
        let _ = self.stop.send(Message::Stop);
        if let Some(worker) = self.worker.lock().expect("worker lock").take() {
            let _ = worker.join();
        }
    }

    pub async fn rescan(&self) -> Result<Snapshot, String> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        self.stop
            .send(Message::Rescan(sender))
            .map_err(|e| e.to_string())?;
        receiver.await.map_err(|e| e.to_string())
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        self.stop();
    }
}
