//! Recognize documented self-managed services without mistaking parent PID 1 for an orphan.
use crate::model::UnmanagedGroup;
use serde::{Deserialize, Serialize};

pub fn opencode_service(argv: &[String]) -> bool {
    argv.first()
        .and_then(|p| std::path::Path::new(p).file_name())
        .is_some_and(|n| n == "opencode" || n == "opencode.exe")
        && argv.get(1).is_some_and(|a| a == "serve")
        && argv.iter().skip(2).any(|a| a == "--service")
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Registration {
    pub pid: u32,
    pub url: String,
}

pub fn opencode_registration() -> Option<Registration> {
    // Deserialize only public identity fields, never service credentials.
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))?;
    let path = root.join("opencode/service.json");
    if std::fs::metadata(&path).ok()?.len() > 65536 {
        return None;
    }
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

pub fn registered_group(group: &UnmanagedGroup, record: &Registration) -> bool {
    let Ok(url) = reqwest::Url::parse(&record.url) else {
        return false;
    };
    if !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")) {
        return false;
    }
    group.copies.len() == 1
        && group.copies.iter().all(|p| {
            p.pid == record.pid
                && p.uid == Some(unsafe { libc::getuid() })
                && opencode_service(&p.argv)
        })
        && group
            .ports
            .iter()
            .any(|p| p.pid == record.pid && Some(p.port) == url.port_or_known_default())
}

pub const OPENCODE_GUIDANCE: &str = "OpenCode manages this shared service itself.\nDo not adopt `opencode serve --service` as a second LaunchAgent. OpenCode clients discover or start the shared service automatically. Use `opencode service status` and `opencode api get /api/info` to check it; `opencode service restart` is available if it becomes unhealthy.\nIf a failed Harbor adoption left an unloaded plist, archive that duplicate instead of retrying. A PPID of 1 does not mean this service is abandoned.";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_internal_service_commands_are_recognized() {
        let argv = |args: &[&str]| args.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(opencode_service(&argv(&[
            "/bin/opencode.exe",
            "serve",
            "--service"
        ])));
        assert!(!opencode_service(&argv(&["/bin/opencode.exe", "serve"])));
        assert!(!opencode_service(&argv(&[
            "/bin/node",
            "serve",
            "--service"
        ])));
        assert!(!opencode_service(&argv(&[
            "/bin/opencode.exe",
            "run",
            "--service"
        ])));
    }

    #[test]
    fn registry_identity_requires_matching_process_owner_command_and_port() {
        let mut group = UnmanagedGroup {
            key: "test".into(),
            name: "opencode.exe".into(),
            verdict: "Unknown".into(),
            hint: None,
            app_path: None,
            copies: vec![crate::model::ProcInfo {
                pid: 42,
                ppid: 1,
                uid: Some(unsafe { libc::getuid() }),
                name: "opencode.exe".into(),
                argv: vec![
                    "/bin/opencode.exe".into(),
                    "serve".into(),
                    "--service".into(),
                ],
                cwd: None,
                cpu: None,
                mem_bytes: 0,
                started_at: None,
                launchd_label: None,
            }],
            ports: vec![crate::model::PortInfo {
                pid: 42,
                port: 12345,
                addr: "127.0.0.1".into(),
            }],
        };
        let record = Registration {
            pid: 42,
            url: "http://127.0.0.1:12345".into(),
        };
        assert!(registered_group(&group, &record));
        group.ports[0].port = 12346;
        assert!(!registered_group(&group, &record));
        group.ports[0].port = 12345;
        group.copies[0].uid = None;
        assert!(!registered_group(&group, &record));
        group.copies[0].uid = Some(unsafe { libc::getuid() });
        group.copies[0].pid = 43;
        assert!(!registered_group(&group, &record));
    }
}
