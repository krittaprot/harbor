use crate::{launchctl, model::Snapshot, ports, procs::ProcessScanner, scan_command};
use chrono::Utc;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, RwLock, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tauri::Emitter;

pub struct Scanner {
    cache: Arc<RwLock<Option<Snapshot>>>,
    stop: mpsc::Sender<()>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

pub fn collect(processes: &mut ProcessScanner) -> Snapshot {
    let started = Instant::now();
    let mut table = processes.scan();
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    let (ports, mut diagnostics) = ports::scan(&table, uid);
    match scan_command::output("/bin/launchctl", &["list"]) {
        Ok(output) if output.status.success() => {
            let jobs: BTreeMap<_, _> =
                launchctl::parse_list_output(&String::from_utf8_lossy(&output.stdout))
                    .into_iter()
                    .filter_map(|job| job.pid.map(|pid| (pid, job.label)))
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
    diagnostics.duration_ms = started.elapsed().as_millis() as u64;
    Snapshot {
        taken_at: Utc::now(),
        uid,
        listening_port_count: ports.iter().map(|p| p.port).collect::<BTreeSet<_>>().len(),
        processes: table.by_pid.into_values().collect(),
        ports,
        diagnostics,
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
            loop {
                let started = Instant::now();
                let snapshot = collect(&mut processes);
                *shared.write().expect("snapshot lock") = Some(snapshot.clone());
                if let Err(error) = app.emit("harbor://snapshot", &snapshot) {
                    eprintln!("Harbor snapshot event: {error}");
                }
                match receiver
                    .recv_timeout(Duration::from_secs(10).saturating_sub(started.elapsed()))
                {
                    Err(mpsc::RecvTimeoutError::Timeout) => (),
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
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.lock().expect("worker lock").take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Scanner {
    fn drop(&mut self) {
        self.stop();
    }
}
