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
    io::{Read, Seek, SeekFrom, Write},
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
    /// Emit exactly one versioned JSON response, including on errors (except --help/--version).
    #[arg(long, global = true)]
    json: bool,
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
    /// Probe service health. Exits 1 if any selected service is failed or unhealthy.
    Health { id: Option<String> },
    /// Read login apps through System Events (may need macOS Automation permission).
    Login,
    /// Report restart prerequisites, failed jobs and volume dependencies.
    Readiness,
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
}
impl CliError {
    fn new(code: &'static str, message: impl Into<String>, exit: u8) -> Self {
        Self {
            code,
            message: message.into(),
            exit,
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
    if let Some((args, _)) = action_args(command)
        && !args.yes
        && !args.dry_run
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
        "gui_required":false,"commands":commands,"mutating_commands":["start --yes","stop --yes","restart --yes","snapshot --save"],
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
        Command::Unmanaged { include_apps } => Ok(Output::detail(
            json!({"unmanaged":snapshot.unmanaged,"app_owned":if *include_apps { Some(&snapshot.app_owned) } else { None }}),
        )),
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
    let json_output = args
        .iter()
        .take_while(|a| *a != "--")
        .any(|a| a == "--json");
    let cli = match Cli::try_parse_from(args) {
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
