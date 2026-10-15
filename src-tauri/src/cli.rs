//! Headless, single-shot API for scripts and agents. Never initializes the GUI runtime.
use crate::{
    health::HealthScanner,
    model::{Service, Snapshot},
    operations::{self, ServiceAction},
    scanner,
};
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{Value, json};
use std::os::unix::fs::OpenOptionsExt;
use std::{
    ffi::OsString,
    io::{IsTerminal, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

const SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Parser)]
#[command(
    name = "harbor",
    version,
    about = "macOS service inventory and controls for people and agents",
    disable_help_subcommand = true
)]
struct Cli {
    /// Force JSON output (automatic when stdout is piped or captured).
    #[arg(long, global = true, conflicts_with = "text")]
    json: bool,
    /// Force readable text when piping or capturing output.
    #[arg(long, global = true)]
    text: bool,
    /// Run HTTP/TCP health probes. Otherwise health stays NotChecked; `health` always probes.
    #[arg(long, global = true)]
    probe: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Discover commands, JSON schema version and exit codes without scanning.
    Capabilities,
    /// Summarize services, listener counts and system readiness.
    Status,
    /// List grouped services; Apple/system jobs are hidden unless --all is supplied.
    Services {
        #[arg(long)]
        all: bool,
        #[arg(long, value_enum)]
        kind: Option<Kind>,
        /// Case-insensitive match against name, ID, label, port or process command.
        #[arg(long)]
        search: Option<String>,
    },
    /// Inspect one exact service ID (get IDs from `services`).
    #[command(alias = "service")]
    Show { id: String },
    /// List listening TCP bindings and their process owners.
    Ports {
        #[arg(long)]
        port: Option<u16>,
        #[arg(long)]
        pid: Option<u32>,
    },
    /// List unmanaged listener groups, optionally including application-owned listeners.
    Unmanaged {
        #[arg(long)]
        include_apps: bool,
    },
    /// Remember an unmanaged command group as temporary (same as the app checkbox).
    Temporary {
        /// Group name or exact key from `unmanaged`. Applies to current/future copies.
        key: String,
        /// Restore review warnings; also works for remembered groups no longer running.
        #[arg(long)]
        undo: bool,
        /// Preview the preference change without saving it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Undo a temporary choice and include the group in review warnings again.
    Review { group: String },
    /// Preview a preference or service action without changing anything.
    Preview {
        #[command(subcommand)]
        action: PreviewAction,
    },
    /// Probe service health. Exits 1 if any selected service is failed or unhealthy.
    Health { id: Option<String> },
    /// Read login apps through System Events (may need macOS Automation permission).
    Login,
    /// Report restart prerequisites, failed jobs and volume dependencies.
    Readiness,
    /// Collect a job/service/group diagnostic report, log tails and last adoption attempt.
    Diagnose { target: String },
    /// Adopt an unmanaged group as a LaunchAgent. Use `preview adopt` first.
    Adopt(AdoptArgs),
    /// Archive an unloaded user job's plist. Requires --yes or `preview archive`.
    Archive(ActionArgs),
    /// Print a fresh full snapshot; --save explicitly persists a baseline for comparison.
    Snapshot {
        #[arg(long)]
        save: bool,
        /// Include login apps; otherwise no Automation access is attempted.
        #[arg(long)]
        include_login: bool,
    },
    /// Compare a saved snapshot with this boot. Never restarts the Mac.
    Compare {
        path: PathBuf,
        #[arg(long)]
        include_login: bool,
    },
    /// Tail a job's configured stdout or stderr log (bounded to the last 1 MiB).
    Logs {
        label: String,
        #[arg(long, value_enum, default_value = "stderr")]
        stream: LogStream,
        #[arg(long, default_value_t = 100, value_parser = clap::value_parser!(u32).range(1..=10_000))]
        lines: u32,
    },
    /// Load unloaded user jobs. Requires --yes or --dry-run.
    Start(ActionArgs),
    /// Unload all user jobs in a service. Requires --yes or --dry-run.
    Stop(ActionArgs),
    /// Stop then start a service's user jobs. Requires --yes or --dry-run.
    Restart(ActionArgs),
}

#[derive(Debug, Subcommand)]
enum PreviewAction {
    /// Review a generated LaunchAgent without writing or loading it.
    Adopt(AdoptSetup),
    /// Review the destination for an unloaded user job's plist.
    Archive { id: String },
    /// Preview marking a group temporary.
    Temporary { group: String },
    /// Preview undoing a temporary choice.
    Review { group: String },
    /// Preview starting a service.
    Start { id: String },
    /// Preview stopping a service.
    Stop { id: String },
    /// Preview restarting a service.
    Restart { id: String },
}

fn normalize_command(command: Command) -> Command {
    match command {
        Command::Review { group } => Command::Temporary {
            key: group,
            undo: true,
            dry_run: false,
        },
        Command::Preview { action } => match action {
            PreviewAction::Adopt(setup) => Command::Adopt(AdoptArgs {
                setup,
                yes: false,
                dry_run: true,
            }),
            PreviewAction::Archive { id } => Command::Archive(ActionArgs {
                id,
                yes: false,
                dry_run: true,
            }),
            PreviewAction::Temporary { group } => Command::Temporary {
                key: group,
                undo: false,
                dry_run: true,
            },
            PreviewAction::Review { group } => Command::Temporary {
                key: group,
                undo: true,
                dry_run: true,
            },
            PreviewAction::Start { id } => Command::Start(ActionArgs {
                id,
                yes: false,
                dry_run: true,
            }),
            PreviewAction::Stop { id } => Command::Stop(ActionArgs {
                id,
                yes: false,
                dry_run: true,
            }),
            PreviewAction::Restart { id } => Command::Restart(ActionArgs {
                id,
                yes: false,
                dry_run: true,
            }),
        },
        other => other,
    }
}

fn resolve_group(
    snapshot: &Snapshot,
    saved: &[String],
    target: &str,
    undo: bool,
) -> Result<String, CliError> {
    if snapshot.unmanaged.iter().any(|g| g.key == target)
        || (undo && saved.iter().any(|k| k == target))
    {
        return Ok(target.into());
    }
    let matches: Vec<_> = snapshot
        .unmanaged
        .iter()
        .filter(|g| g.name.eq_ignore_ascii_case(target))
        .collect();
    match matches.as_slice() {
        [group] => Ok(group.key.clone()),
        [] => Err(CliError::new(
            "not_found",
            "No matching unmanaged group. Run `harbor unmanaged` for names and keys. Stopped groups require their saved key from temporary_keys.",
            3,
        )),
        _ => Err(CliError::new(
            "ambiguous_target",
            format!(
                "More than one group is named '{target}'. Use an exact key: {}",
                matches
                    .iter()
                    .map(|g| g.key.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            2,
        )),
    }
}

#[derive(Debug, Args)]
struct AdoptSetup {
    /// Exact group name or key from `harbor unmanaged`.
    group: String,
    #[arg(value_parser = ["as"], value_name = "as")]
    as_word: String,
    /// New user LaunchAgent label.
    label: String,
    /// Stop original copies only after the new agent becomes ready (SIGTERM only).
    #[arg(long)]
    stop_originals: bool,
    #[arg(long)]
    no_keep_alive: bool,
    #[arg(long)]
    no_login: bool,
    #[arg(long)]
    no_logs: bool,
}

#[derive(Debug, Args)]
struct AdoptArgs {
    #[command(flatten)]
    setup: AdoptSetup,
    #[arg(long, conflicts_with = "dry_run")]
    yes: bool,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct ActionArgs {
    id: String,
    /// Explicitly authorize the service mutation. There is no interactive prompt.
    #[arg(long, conflicts_with = "dry_run")]
    yes: bool,
    /// Validate and print the affected jobs without making changes.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Kind {
    Web,
    Ai,
    Data,
    Infra,
    Other,
}
impl Kind {
    fn name(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Ai => "ai",
            Self::Data => "data",
            Self::Infra => "infra",
            Self::Other => "other",
        }
    }
}
#[derive(Debug, Clone, Copy, ValueEnum)]
enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Serialize)]
struct CliError {
    code: &'static str,
    message: String,
    #[serde(skip)]
    exit: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}
impl CliError {
    fn new(code: &'static str, message: impl Into<String>, exit: u8) -> Self {
        Self {
            code,
            message: message.into(),
            exit,
            details: None,
        }
    }
    fn operation(message: String) -> Self {
        Self::new("operation_failed", message, 1)
    }
}

struct Output {
    data: Value,
    text: String,
    exit: u8,
}
impl Output {
    fn new(data: Value, text: String) -> Self {
        Self {
            data,
            text,
            exit: 0,
        }
    }
    fn detail(data: Value) -> Self {
        Self::new(
            data.clone(),
            serde_json::to_string_pretty(&data).expect("JSON value"),
        )
    }
}

fn find_service<'a>(snapshot: &'a Snapshot, id: &str) -> Result<&'a Service, CliError> {
    snapshot
        .services
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| {
            CliError::new(
                "not_found",
                format!("No service with ID '{id}'. Use `harbor services --json` to discover IDs."),
                3,
            )
        })
}

fn matches_service(service: &Service, query: &str) -> bool {
    let query = query.to_lowercase();
    service.name.to_lowercase().contains(&query)
        || service.id.to_lowercase().contains(&query)
        || service
            .jobs
            .iter()
            .any(|j| j.label.to_lowercase().contains(&query))
        || service
            .ports
            .iter()
            .any(|p| p.port.to_string().contains(&query))
        || service.processes.iter().any(|p| {
            p.argv
                .iter()
                .chain(p.cwd.iter())
                .any(|s| s.to_lowercase().contains(&query))
        })
}

fn service_lines(services: &[&Service]) -> String {
    let mut lines = vec![format!(
        "{:<30} {:<11} {:<12} PORTS",
        "SERVICE ID", "STATE", "HEALTH"
    )];
    for service in services {
        let ports = service
            .ports
            .iter()
            .map(|p| p.port)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(",");
        lines.push(format!(
            "{:<30} {:<11} {:<12} {}",
            service.id, service.state, service.health.status, ports
        ));
    }
    lines.join("\n")
}

fn action_args(command: &Command) -> Option<(&ActionArgs, ServiceAction)> {
    match command {
        Command::Start(a) => Some((a, ServiceAction::Start)),
        Command::Stop(a) => Some((a, ServiceAction::Stop)),
        Command::Restart(a) => Some((a, ServiceAction::Restart)),
        _ => None,
    }
}

fn validate_action_request(command: &Command) -> Result<(), CliError> {
    let authorization = match command {
        Command::Adopt(args) => Some((args.yes, args.dry_run)),
        Command::Archive(args) => Some((args.yes, args.dry_run)),
        _ => action_args(command).map(|(args, _)| (args.yes, args.dry_run)),
    };
    if let Some((yes, dry_run)) = authorization
        && !yes
        && !dry_run
    {
        return Err(CliError::new(
            "confirmation_required",
            "Use --dry-run to review the affected jobs, or --yes to authorize this action. Nothing was changed.",
            2,
        ));
    }
    Ok(())
}

fn capabilities() -> Value {
    let commands: Vec<_> = Cli::command()
        .get_subcommands()
        .map(|c| json!({"name":c.get_name(),"description":c.get_about().map(|a| a.to_string())}))
        .collect();
    json!({"name":"harbor","version":env!("CARGO_PKG_VERSION"),"schema_version":SCHEMA_VERSION,"platform":"macos",
        "gui_required":false,"commands":commands,"mutating_commands":["start --yes","stop --yes","restart --yes","adopt --yes","archive --yes","snapshot --save","temporary","review"],
        "output_default":"text in a terminal; JSON when piped or captured","text_flags":["--text"],
        "examples":["harbor unmanaged","harbor temporary figma-console-mcp","harbor review figma-console-mcp","harbor preview temporary figma-console-mcp","harbor preview restart 3things"],
        "json_flags":["--json"],"exit_codes":{"0":"success","1":"operation failed or unhealthy health result","2":"usage or confirmation required","3":"target not found","4":"operation refused"},
        "notes":["Process CPU is null in single-shot inventory.","Health is NotChecked unless --probe or the health command is used.","Non-root inventory can be incomplete; inspect diagnostics.","A successful service action means launchctl accepted it, not that the service became healthy."]})
}

fn tail(path: &Path, lines: usize) -> Result<(String, bool), CliError> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| CliError::operation(format!("{}: {e}", path.display())))?;
    let metadata = file
        .metadata()
        .map_err(|e| CliError::operation(e.to_string()))?;
    if !metadata.is_file() {
        return Err(CliError::new(
            "refused",
            "Log path must be a regular file",
            4,
        ));
    }
    let start = metadata.len().saturating_sub(1024 * 1024);
    file.seek(SeekFrom::Start(start))
        .map_err(|e| CliError::operation(e.to_string()))?;
    let mut bytes = vec![];
    file.take(1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| CliError::operation(e.to_string()))?;
    // Drop the partial first line when starting in the middle of a large file.
    let offset = if start > 0 {
        bytes
            .iter()
            .position(|b| *b == b'\n')
            .map_or(bytes.len(), |n| n + 1)
    } else {
        0
    };
    let text = String::from_utf8_lossy(&bytes[offset..]);
    let all: Vec<_> = text.lines().collect();
    Ok((
        all[all.len().saturating_sub(lines)..].join("\n"),
        start > 0 || all.len() > lines,
    ))
}

fn execute(cli: &Cli, snapshot: &Snapshot) -> Result<Output, CliError> {
    match &cli.command {
        Command::Capabilities => Ok(Output::detail(capabilities())),
        Command::Status => Ok(Output::new(
            json!({"taken_at":snapshot.taken_at,"system":snapshot.system,"counts":snapshot.counts,
            "services":snapshot.services.iter().filter(|s| !s.apple).count(),"health_probed":cli.probe}),
            format!(
                "{}\n{} services · {} running jobs · {} listening ports\n{} need attention · {} unmanaged listeners · {} unloaded agents\nHealth probes: {}",
                snapshot.system.hostname,
                snapshot.services.iter().filter(|s| !s.apple).count(),
                snapshot.counts.running,
                snapshot.counts.ports,
                snapshot.counts.attention,
                snapshot.counts.unmanaged,
                snapshot.counts.not_loaded,
                if cli.probe {
                    "enabled"
                } else {
                    "not requested (use --probe)"
                }
            ),
        )),
        Command::Services { all, kind, search } => {
            let selected: Vec<_> = snapshot
                .services
                .iter()
                .filter(|s| {
                    (*all || !s.apple)
                        && kind.is_none_or(|k| s.kind == k.name())
                        && search.as_ref().is_none_or(|q| matches_service(s, q))
                })
                .collect();
            Ok(Output::new(json!(selected), service_lines(&selected)))
        }
        Command::Show { id } => Ok(Output::detail(json!(find_service(snapshot, id)?))),
        Command::Ports { port, pid } => {
            let ports: Vec<_> = snapshot.ports.iter().filter(|p| port.is_none_or(|n| n == p.port) && pid.is_none_or(|n| n == p.pid))
                .map(|p| json!({"port":p.port,"addr":p.addr,"pid":p.pid,"process":snapshot.processes.iter().find(|process| process.pid == p.pid)})).collect();
            Ok(Output::detail(json!(ports)))
        }
        Command::Unmanaged { include_apps } => {
            let saved = crate::config::load().map_err(CliError::operation)?.ignored;
            let mut lines = vec![format!(
                "{:<28} {:<12} {:<8} PORTS",
                "GROUP", "STATUS", "COPIES"
            )];
            for group in snapshot
                .unmanaged
                .iter()
                .chain(snapshot.app_owned.iter().filter(|_| *include_apps))
            {
                let status = if group.app_path.is_some() {
                    "App-owned"
                } else if group.verdict == "Expected" {
                    "Temporary"
                } else {
                    &group.verdict
                };
                let ports = group
                    .ports
                    .iter()
                    .map(|p| p.port)
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                lines.push(format!(
                    "{:<28} {:<12} {:<8} {}\n  key: {}",
                    group.name,
                    status,
                    group.copies.len(),
                    ports,
                    group.key
                ));
                if let Some(hint) = &group.hint {
                    lines.push(format!("  {hint}"));
                }
            }
            for key in saved
                .iter()
                .filter(|key| !snapshot.unmanaged.iter().any(|g| &g.key == *key))
            {
                lines.push(format!("Saved temporary (not running): {key}"));
            }
            lines.push("\nUse: harbor temporary <group> · harbor review <group> · harbor preview temporary <group>".into());
            Ok(Output::new(
                json!({"unmanaged":snapshot.unmanaged,"app_owned":if *include_apps { Some(&snapshot.app_owned) } else { None },"temporary_keys":saved}),
                lines.join("\n"),
            ))
        }
        Command::Temporary { key, undo, dry_run } => {
            let config = crate::config::load().map_err(CliError::operation)?;
            let key = resolve_group(snapshot, &config.ignored, key, *undo)?;
            let group = snapshot.unmanaged.iter().find(|g| g.key == key);
            if !undo && group.is_some_and(|g| g.verdict == "Managed") {
                return Err(CliError::new(
                    "refused",
                    "This group is already managed by its application and excluded from missing-agent warnings. Use diagnose to inspect its lifecycle.",
                    4,
                ));
            }
            let temporary = !undo;
            let changed = config.ignored.contains(&key) != temporary;
            if !dry_run && changed {
                crate::config::set_temporary(&key, temporary).map_err(CliError::operation)?;
            }
            let name = group.map_or(key.as_str(), |g| g.name.as_str());
            let state = if temporary {
                "temporary"
            } else {
                "included in review warnings"
            };
            let text = if *dry_run {
                format!("Preview: {name} would be {state}. Nothing saved.")
            } else if changed {
                format!("{name} is now {state}.")
            } else {
                format!("{name} is already {state}.")
            };
            Ok(Output::new(
                json!({
                    "key":key,"temporary":temporary,"previous_temporary":config.ignored.contains(&key),
                    "dry_run":dry_run,"changed":!dry_run && changed,"would_change":changed,
                    "running_copies":group.map_or(0, |g| g.copies.len()),
                    "scope":"all current and future copies of this command group"
                }),
                format!("{text}\nApplies to all current and future copies of this command group."),
            ))
        }
        Command::Review { .. } | Command::Preview { .. } => {
            unreachable!("commands normalized before execution")
        }
        Command::Health { id } => {
            let selected: Vec<_> = if let Some(id) = id {
                vec![find_service(snapshot, id)?]
            } else {
                snapshot.services.iter().filter(|s| !s.apple).collect()
            };
            let unhealthy = selected
                .iter()
                .any(|s| s.state == "Failed" || s.health.status == "Failed");
            let mut output = Output::new(
                json!({"unhealthy":unhealthy,"services":selected.iter().map(|s| json!({"id":s.id,"state":s.state,"health":s.health})).collect::<Vec<_>>()}),
                service_lines(&selected),
            );
            output.exit = u8::from(unhealthy);
            Ok(output)
        }
        Command::Login => Ok(Output::detail(json!(
            crate::login::inspect(&snapshot.processes).map_err(CliError::operation)?
        ))),
        Command::Readiness => Ok(Output::detail(readiness(snapshot))),
        Command::Diagnose { target } => Ok(Output::detail(
            crate::diagnostics::report(snapshot, target)
                .map_err(|e| CliError::new("diagnosis_failed", e, 1))?,
        )),
        Command::Archive(args) => {
            let job = snapshot
                .services
                .iter()
                .flat_map(|s| &s.jobs)
                .find(|j| j.label == args.id)
                .ok_or_else(|| {
                    CliError::new(
                        "not_found",
                        "Use an exact job label from services or show",
                        3,
                    )
                })?;
            let path = job
                .plist_path
                .as_ref()
                .ok_or_else(|| CliError::new("not_found", "Job has no plist", 3))?;
            let destination = operations::archive_plist(path, args.dry_run)
                .map_err(|e| CliError::new("refused", e, 4))?;
            Ok(Output::detail(
                json!({"label":args.id,"source":path,"destination":destination,"dry_run":args.dry_run,"archived":!args.dry_run}),
            ))
        }
        Command::Adopt(args) => {
            let setup = &args.setup;
            let key = resolve_group(snapshot, &[], &setup.group, false)?;
            let group = snapshot.unmanaged.iter().find(|g| g.key == key).unwrap();
            let options = crate::adopt::Options {
                run_at_load: !setup.no_login,
                keep_alive: !setup.no_keep_alive,
                logs: !setup.no_logs,
                stop_copies: setup.stop_originals,
            };
            let xml = crate::adopt::preview_group(group, &setup.label, &options)
                .map_err(|e| CliError::new("refused", e, 4))?;
            if args.dry_run {
                return Ok(Output::detail(
                    json!({"key":key,"label":setup.label,"options":options,"xml":xml,"dry_run":true}),
                ));
            }
            let events = std::cell::RefCell::new(Vec::new());
            let result = crate::adopt::apply_group(group, &setup.label, &options, &xml, |event| {
                if !cli.json {
                    eprintln!("harbor: {}: {}", event.stage, event.message);
                }
                events.borrow_mut().push(event);
            });
            match result {
                Ok(results) => Ok(Output::detail(
                    json!({"key":key,"label":setup.label,"ready":true,"results":results,"progress":events.into_inner(),"dry_run":false}),
                )),
                Err(message) => {
                    let mut error = CliError::new("adoption_failed", message, 1);
                    let fresh = scanner::scan_once().unwrap_or_else(|_| snapshot.clone());
                    error.details = Some(
                        json!({"progress":events.into_inner(),"diagnostics":crate::diagnostics::report(&fresh, &setup.label).ok()}),
                    );
                    Err(error)
                }
            }
        }
        Command::Snapshot {
            save,
            include_login,
        } => {
            let login = if *include_login {
                Some(crate::login::inspect(&snapshot.processes).map_err(CliError::operation)?)
            } else {
                None
            };
            let path = if *save {
                Some(
                    crate::snapshots::save(snapshot.clone(), login.clone())
                        .map_err(CliError::operation)?,
                )
            } else {
                None
            };
            Ok(Output::detail(
                json!({"snapshot":snapshot,"login":login,"saved_path":path}),
            ))
        }
        Command::Compare {
            path,
            include_login,
        } => {
            let before = std::fs::read(path)
                .map_err(|e| CliError::operation(format!("{}: {e}", path.display())))?;
            let before: crate::snapshots::SavedSnapshot = serde_json::from_slice(&before)
                .map_err(|e| CliError::new("invalid_snapshot", e.to_string(), 2))?;
            let login = if *include_login {
                Some(crate::login::inspect(&snapshot.processes).map_err(CliError::operation)?)
            } else {
                None
            };
            Ok(Output::detail(
                json!({"report":crate::snapshots::compare(&before,snapshot,login.as_ref()),"new_boot":snapshot.system.boot_time.is_some_and(|t| t > before.snapshot.taken_at)}),
            ))
        }
        Command::Logs {
            label,
            stream,
            lines,
        } => {
            let job = snapshot
                .services
                .iter()
                .flat_map(|s| &s.jobs)
                .find(|j| &j.label == label)
                .ok_or_else(|| CliError::new("not_found", format!("No job '{label}'"), 3))?;
            let plist = job
                .plist_path
                .as_ref()
                .ok_or_else(|| CliError::new("not_found", "No plist available for this job", 3))?;
            let config = crate::plist_util::parse_plist(plist)
                .map_err(|e| CliError::operation(e.to_string()))?;
            let path = match stream {
                LogStream::Stdout => config.standard_out_path,
                LogStream::Stderr => config.standard_error_path,
            }
            .ok_or_else(|| {
                CliError::new(
                    "not_found",
                    "This job has no configured log for that stream",
                    3,
                )
            })?;
            let (text, truncated) = tail(Path::new(&path), *lines as usize)?;
            Ok(Output::new(
                json!({"label":label,"path":path,"text":text,"truncated":truncated}),
                text,
            ))
        }
        Command::Start(_) | Command::Stop(_) | Command::Restart(_) => {
            let (args, action) = action_args(&cli.command).unwrap();
            find_service(snapshot, &args.id)?;
            let plan = operations::plan(snapshot, &args.id, action)
                .map_err(|e| CliError::new("refused", e, 4))?;
            if !args.dry_run {
                operations::apply(snapshot, &args.id, action).map_err(CliError::operation)?;
            }
            Ok(Output::detail(
                json!({"dry_run":args.dry_run,"accepted":!args.dry_run,"plan":plan}),
            ))
        }
    }
}

fn readiness(snapshot: &Snapshot) -> Value {
    let services: Vec<_> = snapshot.services.iter().filter(|s| !s.apple).collect();
    json!({"system":snapshot.system,"unlock_required":snapshot.system.filevault_on,"auto_start_jobs":services.iter().flat_map(|s| &s.jobs).filter(|j| j.autostart && j.status != "Unloaded").count(),
        "failed_jobs":services.iter().flat_map(|s| &s.jobs).filter(|j| j.pid.is_none() && j.last_exit.is_some_and(|n| n != 0)).collect::<Vec<_>>(),
        "missing_dependencies":services.iter().flat_map(|s| s.depends_on.iter().filter(|d| !d.mounted).map(|d| json!({"service_id":s.id,"path":d.path}))).collect::<Vec<_>>(),
        "unmanaged":snapshot.unmanaged,"not_loaded":snapshot.not_loaded,"login_items_checked":false,
        "note":"Configuration is not a guarantee of recovery. FileVault may require keyboard unlock; use `harbor login` to inspect login apps."})
}

fn write_error(error: &CliError, json_output: bool) {
    if json_output {
        println!(
            "{}",
            json!({"schema_version":SCHEMA_VERSION,"ok":false,"error":error})
        );
    } else {
        eprintln!("harbor: {}: {}", error.code, error.message);
    }
}

/// Entry point shared by the standalone binary and CLI contract tests.
pub fn run() -> u8 {
    run_args(std::env::args_os())
}

fn run_args(args: impl IntoIterator<Item = OsString>) -> u8 {
    let args: Vec<_> = args.into_iter().collect();
    let flags: Vec<_> = args.iter().take_while(|a| *a != "--").collect();
    let json_output = flags.iter().any(|a| *a == "--json")
        || (!std::io::stdout().is_terminal() && !flags.iter().any(|a| *a == "--text"));
    let mut cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if matches!(
                error.kind(),
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion
            ) {
                print!("{error}");
                return 0;
            }
            write_error(&CliError::new("usage", error.to_string(), 2), json_output);
            return 2;
        }
    };
    cli.json = cli.json || (!cli.text && !std::io::stdout().is_terminal());
    cli.command = normalize_command(cli.command);
    let result = (|| {
        validate_action_request(&cli.command)?;
        if matches!(cli.command, Command::Capabilities) {
            return Ok((Output::detail(capabilities()), None));
        }
        // The CLI has the same user-level boundary as the desktop app.
        if unsafe { libc::geteuid() } == 0 {
            return Err(CliError::new(
                "refused",
                "Run Harbor as your normal user, not root",
                4,
            ));
        }
        let started = std::time::Instant::now();
        let mut snapshot = scanner::scan_once().map_err(|e| CliError::new("scan_failed", e, 1))?;
        if let Command::Health { id: Some(id) } = &cli.command {
            find_service(&snapshot, id)?;
            let service = snapshot.services.iter_mut().find(|s| &s.id == id).unwrap();
            HealthScanner::default().update(std::slice::from_mut(service));
        } else if cli.probe || matches!(cli.command, Command::Health { .. }) {
            HealthScanner::default().update(&mut snapshot.services);
        }
        scanner::update_counts(&mut snapshot);
        snapshot.taken_at = chrono::Utc::now();
        snapshot.diagnostics.duration_ms = started.elapsed().as_millis() as u64;
        let output = execute(&cli, &snapshot)?;
        Ok((output, Some(snapshot.diagnostics)))
    })();
    match result {
        Ok((output, diagnostics)) => {
            let text = if cli.json {
                json!({"schema_version":SCHEMA_VERSION,"ok":true,"data":output.data,"diagnostics":diagnostics}).to_string()
            } else {
                output.text
            };
            if let Err(error) = writeln!(std::io::stdout().lock(), "{text}") {
                // `harbor ... | head` is normal shell usage.
                return if error.kind() == std::io::ErrorKind::BrokenPipe {
                    0
                } else {
                    1
                };
            }
            if !cli.json
                && let Some(d) = diagnostics
            {
                for warning in d.warnings {
                    eprintln!("harbor: warning: {warning}");
                }
            }
            output.exit
        }
        Err(error) => {
            write_error(&error, cli.json);
            error.exit
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn natural_previews_cannot_apply_and_names_must_be_unambiguous() {
        for action in ["start", "stop", "restart"] {
            let cli = Cli::try_parse_from(["harbor", "preview", action, "demo"]).unwrap();
            let command = normalize_command(cli.command);
            let (args, _) = action_args(&command).unwrap();
            assert!(args.dry_run && !args.yes);
            assert!(validate_action_request(&command).is_ok());
            assert!(Cli::try_parse_from(["harbor", "preview", action, "demo", "--yes"]).is_err());
        }
        let group = |key: &str| crate::model::UnmanagedGroup {
            key: key.into(),
            name: "worker".into(),
            copies: vec![],
            ports: vec![],
            verdict: "Unknown".into(),
            app_path: None,
            hint: None,
        };
        let mut snapshot = Snapshot {
            unmanaged: vec![group("/one|worker.js")],
            ..Default::default()
        };
        assert_eq!(
            resolve_group(&snapshot, &[], "WORKER", false).unwrap(),
            "/one|worker.js"
        );
        snapshot.unmanaged.push(group("/two|worker.js"));
        assert_eq!(
            resolve_group(&snapshot, &[], "worker", false)
                .unwrap_err()
                .code,
            "ambiguous_target"
        );
        assert_eq!(
            resolve_group(&snapshot, &[], "/two|worker.js", false).unwrap(),
            "/two|worker.js"
        );
        let saved = vec!["stopped-key".into()];
        assert!(resolve_group(&snapshot, &saved, "stopped-key", true).is_ok());
        assert!(resolve_group(&snapshot, &saved, "stopped-key", false).is_err());
    }

    #[test]
    fn confirmation_is_required_and_dry_run_cannot_be_combined_with_yes() {
        let cli = Cli::try_parse_from(["harbor", "restart", "demo"]).unwrap();
        assert_eq!(
            validate_action_request(&cli.command).unwrap_err().code,
            "confirmation_required"
        );
        for flag in ["--yes", "--dry-run"] {
            let cli = Cli::try_parse_from(["harbor", "--json", "restart", "demo", flag]).unwrap();
            assert!(validate_action_request(&cli.command).is_ok());
        }
        assert!(Cli::try_parse_from(["harbor", "restart", "demo", "--yes", "--dry-run"]).is_err());
        assert!(Cli::try_parse_from(["harbor", "logs", "demo", "--lines", "0"]).is_err());
    }
    #[test]
    fn tail_bounds_large_logs_and_retains_last_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.log");
        std::fs::write(
            &path,
            format!("{}\nfirst\nsecond\n", "x".repeat(2 * 1024 * 1024)),
        )
        .unwrap();
        let (text, truncated) = tail(&path, 1).unwrap();
        assert_eq!(text, "second");
        assert!(truncated);
        std::fs::write(&path, "one\ntwo\n").unwrap();
        assert_eq!(tail(&path, 10).unwrap(), ("one\ntwo".into(), false));
        assert!(tail(dir.path(), 10).is_err());
    }
}
