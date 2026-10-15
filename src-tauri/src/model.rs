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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub taken_at: DateTime<Utc>,
    pub uid: u32,
    pub processes: Vec<ProcInfo>,
    pub ports: Vec<PortInfo>,
    /// Unique port numbers, not the number of address bindings or socket FDs.
    pub listening_port_count: usize,
    pub diagnostics: ScanDiagnostics,
    pub services: Vec<Service>,
    pub unmanaged: Vec<UnmanagedGroup>,
    pub app_owned: Vec<UnmanagedGroup>,
    pub not_loaded: Vec<JobRef>,
    pub system: SystemInfo,
    pub counts: Counts,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobRef {
    pub label: String,
    pub role: String,
    pub pid: Option<u32>,
    pub last_exit: Option<i32>,
    pub status: String,
    pub plist_path: Option<String>,
    pub editable: bool,
    pub apple: bool,
    pub autostart: bool,
    pub scheduled: bool,
    pub missing_executable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub icon: String,
    pub tint: String,
    pub jobs: Vec<JobRef>,
    pub processes: Vec<ProcInfo>,
    pub ports: Vec<PortInfo>,
    pub health: Health,
    pub state: String,
    pub uptime_secs: Option<u64>,
    pub depends_on: Vec<Dependency>,
    pub pinned: bool,
    pub url: Option<String>,
    pub apple: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Health {
    pub status: String,
    pub code: Option<u16>,
    pub ms: Option<u64>,
    pub reason: Option<String>,
    pub checked_at: Option<DateTime<Utc>>,
    pub tunnels: Vec<TunnelHealth>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelHealth {
    pub port: u16,
    pub ready: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub path: String,
    pub mounted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnmanagedGroup {
    pub key: String,
    pub name: String,
    pub copies: Vec<ProcInfo>,
    pub ports: Vec<PortInfo>,
    pub verdict: String,
    pub app_path: Option<String>,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SystemInfo {
    pub hostname: String,
    pub boot_time: Option<DateTime<Utc>>,
    pub filevault_on: Option<bool>,
    pub provault_mounted: bool,
    pub tailscale_up: Option<bool>,
    pub uid: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Counts {
    pub healthy: usize,
    pub attention: usize,
    pub running: usize,
    pub unmanaged: usize,
    pub not_loaded: usize,
    pub ports: usize,
    pub hidden: usize,
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
    /// Individual plist files skipped during inventory, not service-health failures.
    #[serde(default)]
    pub inventory_issues: Vec<InventoryIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub guidance: String,
    pub detail: String,
}
