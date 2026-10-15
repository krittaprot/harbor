use crate::{
    config::{Config, ServiceConfig},
    launchctl::LoadedService,
    model::*,
    plist_util,
    types::JobSource,
};
use std::collections::{BTreeMap, BTreeSet};

fn matches(pattern: &str, label: &str) -> bool {
    pattern
        .strip_suffix('*')
        .map_or(pattern == label, |p| label.starts_with(p))
}

pub fn label_group(label: &str) -> (String, String) {
    if let Some(name) = label.strip_prefix("homebrew.mxcl.") {
        return (name.into(), "homebrew".into());
    }
    if let Some(role) = label.strip_prefix("ai.hermes.") {
        return ("hermes".into(), role.into());
    }
    let parts: Vec<_> = label.split('.').collect();
    if parts.len() >= 4 && parts[0] == "com" {
        return (parts[2].into(), parts[3..].join("."));
    }
    (label.into(), "agent".into())
}

pub fn jobs(loaded: &[LoadedService], warnings: &mut Vec<String>) -> Vec<JobRef> {
    let mut result = BTreeMap::new();
    for (path, source) in plist_util::scan_plist_files() {
        match plist_util::parse_plist(&path) {
            Ok(p) => {
                let user = source == JobSource::UserAgent;
                // launchctl list describes our GUI domain, not system daemon state.
                let live = if source != JobSource::SystemDaemon {
                    loaded.iter().find(|j| j.label == p.label)
                } else {
                    None
                };
                let status = if source == JobSource::SystemDaemon {
                    "Unknown"
                } else if live.is_some_and(|j| j.pid.is_some()) {
                    "Running"
                } else if live.is_some() {
                    "Loaded"
                } else {
                    "Unloaded"
                };
                let executable = p.program.as_deref().or_else(|| {
                    p.program_arguments
                        .as_ref()
                        .and_then(|a| a.first().map(String::as_str))
                });
                let raw = plist::Value::from_reader_xml(p.raw_xml.as_bytes()).ok();
                let dictionary = raw.as_ref().and_then(|v| v.as_dictionary());
                let docker_helper_without_app = dictionary
                    .and_then(|d| d.get("AssociatedBundleIdentifiers"))
                    .and_then(|v| v.as_array())
                    .is_some_and(|ids| {
                        ids.iter()
                            .any(|v| v.as_string() == Some("com.docker.docker"))
                    })
                    && !std::path::Path::new("/Applications/Docker.app").exists()
                    && !dirs::home_dir()
                        .is_some_and(|p| p.join("Applications/Docker.app").exists());
                let missing_executable = executable
                    .is_some_and(|p| p.starts_with('/') && !std::path::Path::new(p).exists())
                    || docker_helper_without_app;
                let keep_alive = p.keep_alive == Some(true)
                    || dictionary
                        .and_then(|d| d.get("KeepAlive"))
                        .is_some_and(|v| v.as_dictionary().is_some());
                let job = JobRef {
                    label: p.label.clone(),
                    role: label_group(&p.label).1,
                    pid: live.and_then(|j| j.pid),
                    last_exit: live.and_then(|j| j.last_exit_code),
                    status: status.into(),
                    plist_path: Some(path),
                    editable: user,
                    apple: p.label.starts_with("com.apple.")
                        || source != JobSource::UserAgent
                        || executable
                            .is_some_and(|p| p.contains(".app/") && !p.contains("/Python.app/")),
                    autostart: p.run_at_load == Some(true) || keep_alive,
                    scheduled: p.start_interval.is_some() || p.start_calendar_interval.is_some(),
                    missing_executable,
                };
                result.insert(p.label, job);
            }
            Err(e) => warnings.push(format!("plist inventory: {e}")),
        }
    }
    for live in loaded {
        result.entry(live.label.clone()).or_insert_with(|| JobRef {
            label: live.label.clone(),
            role: label_group(&live.label).1,
            pid: live.pid,
            last_exit: live.last_exit_code,
            status: if live.pid.is_some() {
                "Running"
            } else {
                "Loaded"
            }
            .into(),
            plist_path: None,
            editable: false,
            apple: live.label.starts_with("com.apple.") || live.label.starts_with("application."),
            autostart: false,
            scheduled: false,
            missing_executable: false,
        });
    }
    result.into_values().collect()
}

fn style(id: &str, kind: &str) -> (&'static str, &'static str) {
    match id {
        "3things" | "3-things" => ("checklist", "blue"),
        "fasting" => ("timer", "orange"),
        "homespaces" => ("house", "indigo"),
        "nest" => ("leaf", "green"),
        "signal" => ("bubble", "cyan"),
        "yolk" => ("egg", "yellow"),
        "kindaimai" => ("bowl", "pink"),
        "examer" => ("book", "purple"),
        "crystal-blood" => ("drop", "red"),
        "hermes" => ("sparkles", "purple"),
        _ => match kind {
            "web" => ("globe", "blue"),
            "ai" => ("cpu", "purple"),
            "data" => ("drop", "red"),
            _ => ("terminal", "graphite"),
        },
    }
}

pub fn group(
    jobs: Vec<JobRef>,
    processes: &[ProcInfo],
    ports: &[PortInfo],
    config: &Config,
) -> Vec<Service> {
    let mut grouped: BTreeMap<String, (Vec<JobRef>, Option<&ServiceConfig>)> = BTreeMap::new();
    for job in jobs {
        let override_ = config
            .service
            .iter()
            .find(|(_, c)| c.labels.iter().any(|p| matches(p, &job.label)));
        let id = override_
            .map(|(id, _)| id.clone())
            .unwrap_or_else(|| label_group(&job.label).0);
        let entry = grouped
            .entry(id)
            .or_insert_with(|| (vec![], override_.map(|(_, c)| c)));
        entry.0.push(job);
    }
    grouped
        .into_iter()
        .map(|(id, (jobs, cfg))| {
            let ps: Vec<_> = processes
                .iter()
                .filter(|p| {
                    p.launchd_label
                        .as_ref()
                        .is_some_and(|l| jobs.iter().any(|j| &j.label == l))
                })
                .cloned()
                .collect();
            let pids: BTreeSet<_> = ps.iter().map(|p| p.pid).collect();
            let sockets: Vec<_> = ports
                .iter()
                .filter(|p| pids.contains(&p.pid))
                .cloned()
                .collect();
            let dependencies: Vec<_> = cfg
                .map(|c| {
                    c.depends_on
                        .iter()
                        .map(|p| Dependency {
                            path: p.clone(),
                            mounted: crate::system::mounted(p),
                        })
                        .collect()
                })
                .unwrap_or_default();
            let kind = cfg.and_then(|c| c.kind.clone()).unwrap_or_else(|| {
                if !dependencies.is_empty() || ps.iter().any(|p| p.name.contains("postgres")) {
                    "data"
                } else if ["hermes", "codex", "deepseek", "ollama", "asr"]
                    .iter()
                    .any(|n| id.contains(n))
                {
                    "ai"
                } else if jobs
                    .iter()
                    .any(|j| j.role.contains("tunnel") || j.role.contains("cloudflared"))
                {
                    "web"
                } else if jobs.iter().any(|j| j.label.starts_with("homebrew."))
                    || id.to_lowercase().contains("rustdesk")
                    || id == "caddy"
                {
                    "infra"
                } else {
                    "other"
                }
                .into()
            });
            let (icon, tint) = style(&id, &kind);
            let state = if jobs
                .iter()
                .any(|j| j.pid.is_none() && j.last_exit.is_some_and(|c| c != 0))
            {
                "Failed"
            } else if jobs.iter().any(|j| j.pid.is_some()) {
                "Running"
            } else if jobs.iter().any(|j| j.scheduled && j.status == "Loaded") {
                "Scheduled"
            } else if jobs.iter().all(|j| j.status == "Unknown") {
                "Unknown"
            } else {
                "Stopped"
            };
            let uptime_secs = ps
                .iter()
                .filter_map(|p| p.started_at)
                .min()
                .map(|s| (chrono::Utc::now() - s).num_seconds().max(0) as u64);
            Service {
                name: cfg
                    .and_then(|c| c.name.clone())
                    .unwrap_or_else(|| id.clone()),
                icon: cfg
                    .and_then(|c| c.icon.clone())
                    .unwrap_or_else(|| icon.into()),
                tint: cfg
                    .and_then(|c| c.tint.clone())
                    .unwrap_or_else(|| tint.into()),
                pinned: cfg.map_or(
                    ["crystal-blood", "hermes", "3things", "fasting"].contains(&id.as_str()),
                    |c| c.pinned,
                ),
                apple: jobs.iter().all(|j| j.apple),
                id,
                kind,
                jobs,
                processes: ps,
                ports: sockets,
                health: Health {
                    status: "NotChecked".into(),
                    ..Default::default()
                },
                state: state.into(),
                uptime_secs,
                depends_on: dependencies,
                url: cfg.and_then(|c| c.health.clone()),
            }
        })
        .collect()
}

fn app_ancestor<'a>(process: &'a ProcInfo, all: &'a [ProcInfo]) -> Option<String> {
    let mut current = Some(process);
    let mut seen = BTreeSet::new();
    while let Some(p) = current {
        if !seen.insert(p.pid) {
            break;
        }
        for arg in p.argv.iter().take(1).chain(p.cwd.iter()) {
            // Homebrew/framework Python uses a Python.app executable even for
            // headless scripts. That wrapper is not a GUI app owning the job.
            if arg.contains("/Python.app/") && p.argv.len() > 1 {
                continue;
            }
            if let Some((base, _)) = arg.split_once(".app/") {
                return Some(format!("{base}.app"));
            }
        }
        current = all.iter().find(|a| a.pid == p.ppid && a.pid > 1);
    }
    None
}

pub fn unmanaged(
    processes: &[ProcInfo],
    ports: &[PortInfo],
    config: &Config,
) -> (Vec<UnmanagedGroup>, Vec<UnmanagedGroup>) {
    let mut groups: BTreeMap<String, UnmanagedGroup> = BTreeMap::new();
    for process in processes
        .iter()
        .filter(|p| ports.iter().any(|s| s.pid == p.pid))
    {
        let app_path = app_ancestor(process, processes);
        if process
            .launchd_label
            .as_ref()
            .is_some_and(|label| !label.starts_with("application."))
        {
            continue;
        }
        let script = process.argv.iter().skip(1).find(|a| {
            !a.starts_with('-') && (a.contains('/') || a.ends_with(".js") || a.ends_with(".py"))
        });
        let key = app_path.clone().unwrap_or_else(|| {
            format!(
                "{}|{}",
                process.argv.first().unwrap_or(&process.name),
                script.map_or("", String::as_str)
            )
        });
        let name = app_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_stem())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if process.argv.iter().any(|a| a.contains("figma-console-mcp")) {
                    "figma-console-mcp".into()
                } else {
                    script
                        .and_then(|s| std::path::Path::new(s).file_name())
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_else(|| process.name.clone())
                }
            });
        let group = groups.entry(key.clone()).or_insert_with(|| UnmanagedGroup {
            key: key.clone(),
            name,
            copies: vec![],
            ports: vec![],
            verdict: "Unknown".into(),
            app_path,
            hint: None,
        });
        group.copies.push(process.clone());
        group
            .ports
            .extend(ports.iter().filter(|s| s.pid == process.pid).cloned());
    }
    let registration = crate::ownership::opencode_registration();
    for group in groups.values_mut() {
        if config.ignored.contains(&group.key) {
            group.hint = Some("Marked temporary by the user; not expected after restart. Applies to all copies of this command group, including future runs.".into());
        }
        group.verdict = if registration.as_ref().is_some_and(|r| crate::ownership::registered_group(group, r)) {
            group.hint = Some("Managed by OpenCode's shared-service lifecycle. Clients start it on demand; use opencode service status. No duplicate LaunchAgent is needed.".into());
            "Managed"
        } else if group.app_path.is_some() || config.ignored.contains(&group.key) {
            "Expected"
        } else if group.copies.iter().all(|p| p.ppid == 1) {
            "Orphaned"
        } else {
            "Unknown"
        }
        .into();
        group.ports.sort();
        group.ports.dedup();
    }
    groups.into_values().partition(|g| g.app_path.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn label_rules_and_override_patterns() {
        assert_eq!(
            label_group("com.krittaprot.fasting.push"),
            ("fasting".into(), "push".into())
        );
        assert_eq!(label_group("ai.hermes.hermes-server").0, "hermes");
        assert_eq!(label_group("homebrew.mxcl.caddy").0, "caddy");
        assert_eq!(label_group("vendor.label").0, "vendor.label");
        assert!(matches(
            "com.ode.crystalblood.*",
            "com.ode.crystalblood.postgres-supervisor"
        ));
        assert!(!matches(
            "com.ode.crystalblood.*",
            "com.ode.crystalblood-other"
        ));
    }

    fn job(label: &str, pid: u32) -> JobRef {
        JobRef {
            label: label.into(),
            role: label_group(label).1,
            pid: Some(pid),
            last_exit: Some(0),
            status: "Running".into(),
            plist_path: None,
            editable: true,
            apple: false,
            autostart: true,
            scheduled: false,
            missing_executable: false,
        }
    }

    #[test]
    fn config_groups_jobs_and_attributes_descendant_ports() {
        let labels = [
            "com.ode.crystalblood.postgres-supervisor",
            "com.ode.crystalblood.stack-supervisor",
            "com.ode.crystalblood.production-cloudflared",
            "com.ode.crystalblood.log-rotation",
        ];
        let jobs = labels
            .iter()
            .enumerate()
            .map(|(i, l)| job(l, 10 + i as u32))
            .collect();
        let process = ProcInfo {
            pid: 20,
            ppid: 10,
            uid: Some(501),
            name: "postgres".into(),
            argv: vec!["/bin/postgres".into()],
            cwd: None,
            cpu: None,
            mem_bytes: 100,
            started_at: None,
            launchd_label: Some(labels[0].into()),
        };
        let ports = vec![PortInfo {
            pid: 20,
            port: 5432,
            addr: "127.0.0.1".into(),
        }];
        let result = group(jobs, &[process], &ports, &Config::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "crystal-blood");
        assert_eq!(result[0].jobs.len(), 4);
        assert_eq!(result[0].ports, ports);
        assert_eq!(result[0].processes[0].ppid, 10);
        assert_eq!(result[0].kind, "data");
    }

    #[test]
    fn unmanaged_deduplicates_copies_but_not_different_scripts() {
        let processes: Vec<_> = [
            (20, "/tools/figma-console-mcp/local.js"),
            (21, "/tools/figma-console-mcp/local.js"),
            (22, "/tools/other.js"),
        ]
        .into_iter()
        .map(|(pid, script)| ProcInfo {
            pid,
            ppid: 1,
            uid: Some(501),
            name: "node".into(),
            argv: vec!["/bin/node".into(), script.into()],
            cwd: None,
            cpu: None,
            mem_bytes: 100,
            started_at: None,
            launchd_label: None,
        })
        .collect();
        let ports: Vec<_> = processes
            .iter()
            .map(|p| PortInfo {
                pid: p.pid,
                port: 9000 + p.pid as u16,
                addr: "*".into(),
            })
            .collect();
        let (groups, apps) = unmanaged(&processes, &ports, &Config::default());
        assert!(apps.is_empty());
        assert_eq!(groups.len(), 2);
        let figma = groups
            .iter()
            .find(|g| g.name == "figma-console-mcp")
            .unwrap();
        assert_eq!(figma.copies.len(), 2);
        assert_eq!(figma.ports.len(), 2);
        assert_eq!(figma.verdict, "Orphaned");

        // A remembered user choice survives config serialization and future PIDs.
        let mut config = Config::default();
        config.ignored.push(figma.key.clone());
        let mut config: Config = toml::from_str(&toml::to_string(&config).unwrap()).unwrap();
        let mut later_processes = processes.clone();
        let mut later_ports = ports.clone();
        for process in &mut later_processes {
            process.pid += 100;
        }
        for port in &mut later_ports {
            port.pid += 100;
        }
        let (groups, _) = unmanaged(&later_processes, &later_ports, &config);
        let expected = groups.iter().find(|g| g.key == figma.key).unwrap();
        assert_eq!(expected.verdict, "Expected");
        assert!(expected.hint.as_ref().unwrap().contains("temporary"));
        let mut snapshot = crate::model::Snapshot {
            unmanaged: groups,
            ..Default::default()
        };
        crate::scanner::update_counts(&mut snapshot);
        assert_eq!(snapshot.counts.unmanaged, 1);
        config.ignored.clear();
        snapshot.unmanaged = unmanaged(&later_processes, &later_ports, &config).0;
        crate::scanner::update_counts(&mut snapshot);
        assert_eq!(snapshot.counts.unmanaged, 3);
        assert!(
            snapshot
                .unmanaged
                .iter()
                .all(|g| g.verdict == "Orphaned" && g.hint.is_none())
        );
    }
}
