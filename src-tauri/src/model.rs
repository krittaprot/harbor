use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Phase 1 inventory. Service grouping and health are added in later phases.
/// Unknown metadata stays null; an unreadable process is never assumed to be root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub uid: Option<u32>,
    pub name: String,
    pub argv: Vec<String>,
    pub cwd: Option<String>,
    /// Percentage of one core; null until two samples exist for this process.
    pub cpu: Option<f32>,
    pub mem_bytes: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub launchd_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct PortInfo {
    pub pid: u32,
    pub port: u16,
    pub addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub taken_at: DateTime<Utc>,
    pub uid: u32,
    pub processes: Vec<ProcInfo>,
    pub ports: Vec<PortInfo>,
    /// Unique port numbers, not the number of address bindings or socket FDs.
    pub listening_port_count: usize,
    pub diagnostics: ScanDiagnostics,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanDiagnostics {
    pub duration_ms: u64,
    pub libproc_listener_count: usize,
    pub lsof_listener_count: usize,
    pub inaccessible_pids: Vec<u32>,
    /// Always true: without root, macOS can hide other users' listeners.
    pub visibility_limited: bool,
    pub warnings: Vec<String>,
}
