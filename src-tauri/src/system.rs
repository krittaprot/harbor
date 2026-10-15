use crate::{model::SystemInfo, scan_command};
use std::{os::unix::fs::MetadataExt, path::Path};

pub fn mounted(path: &str) -> bool {
    let path = Path::new(path);
    path.metadata()
        .ok()
        .zip(path.parent().and_then(|p| p.metadata().ok()))
        .is_some_and(|(here, parent)| here.dev() != parent.dev())
}

pub fn scan(uid: u32) -> SystemInfo {
    let filevault_on = scan_command::output("/usr/bin/fdesetup", &["status"])
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout);
            if text.contains("FileVault is On") {
                Some(true)
            } else if text.contains("FileVault is Off") {
                Some(false)
            } else {
                None
            }
        });
    let tailscale = [
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "/opt/homebrew/bin/tailscale",
        "/usr/local/bin/tailscale",
    ]
    .into_iter()
    .find(|p| Path::new(p).exists());
    let tailscale_up = tailscale
        .and_then(|bin| scan_command::output(bin, &["status", "--json"]).ok())
        .filter(|o| o.status.success())
        .and_then(|o| serde_json::from_slice::<serde_json::Value>(&o.stdout).ok())
        .and_then(|v| v["BackendState"].as_str().map(|s| s == "Running"));
    SystemInfo {
        hostname: sysinfo::System::host_name().unwrap_or_else(|| "Mac".into()),
        boot_time: chrono::DateTime::from_timestamp(sysinfo::System::boot_time() as i64, 0),
        filevault_on,
        provault_mounted: mounted("/Volumes/ProVault"),
        tailscale_up,
        uid,
    }
}
