# Harbor CLI

`harbor` gives shell-based agents the same process inventory, grouping, health probes and
service controls as the desktop app. It runs independently: no app window, tray, daemon or
local HTTP API is required. It is a macOS CLI; run it as your normal logged-in user.

## Build and install

From the project root, with the same Rust prerequisites as the desktop app:

```bash
pnpm cli:build
./src-tauri/target/release/harbor --help

pnpm cli:install
harbor capabilities --json
```

The installer copies the release executable to `~/.local/bin/harbor`. Add `~/.local/bin` to
PATH if necessary. It refuses to replace an existing command unless explicitly invoked with
`pnpm cli:install --force`. Set `HARBOR_INSTALL_PREFIX` to choose a different install prefix.
The source checkout stays where it is. The installer does not start a service or register login startup.

New desktop app builds also include the CLI. For an app installed in `/Applications`:

```bash
"/Applications/Harbor.app/Contents/MacOS/harbor" status --json
```

Cargo can also build the CLI directly:

```bash
cargo build --manifest-path src-tauri/Cargo.toml --release --locked --bin harbor
```

## Agent quick start

```bash
harbor capabilities
harbor status
harbor services
harbor services --kind web
harbor show crystal-blood
harbor ports --port 5432
harbor unmanaged
harbor health crystal-blood
harbor readiness
```

Output is readable text in an interactive terminal and automatically versioned JSON when
stdout is piped or captured by an agent. No output flag is needed for normal use. `--json`
forces JSON in a terminal; `--text` forces readable output in a pipe. Help/version stay text.

Use the exact service `id` from `services` for subsequent commands. `services --search TEXT`
matches names, IDs, labels, ports and process command lines; it is discovery, not an exact ID
selector. `services --all` includes normally hidden Apple/system jobs. `show` also accepts
`service` as an alias. `harbor COMMAND --help` lists that command's options.

Normal inventory does **not** issue HTTP health probes. Health remains `NotChecked`, and
`counts.healthy` is therefore not a statement that no services are healthy. Use `health` or
add `--probe` to an inventory command when you need a fresh network check. `health ID` probes
only that service; `health` probes all visible services. Each probe has the existing two-second
timeout, with bounded concurrency across services.

Each invocation takes a fresh single-shot scan. CPU is `null` because utilization requires
two samples. `Restarted` transitions also require the desktop's historical samples; the CLI
reports current job state and exit status. Snapshots include timestamps and diagnostics;
non-root macOS process visibility can be incomplete. IDs, PIDs, ports and counts may change
between calls. No configuration, login cache or automatic snapshot is written by inventory.

In the desktop app, **Services → Running without a launch agent → Mark temporary** remembers
that a command group is intentionally short-lived. It applies to all current and future copies
of that group; **Undo temporary** restores review warnings. These processes stay visible but
are excluded from unmanaged warning counts and, in newly saved baselines, missing-process/port
restart findings. Existing “Mark expected” choices use the same preference. In CLI JSON,
they retain `verdict: "Expected"` for compatibility, with a `hint` explaining the temporary
choice. This preference does not create a LaunchAgent or stop any process.

Agents can make the same choice through the CLI:

```bash
harbor unmanaged
harbor preview temporary figma-console-mcp
harbor temporary figma-console-mcp
harbor review figma-console-mcp
harbor preview review figma-console-mcp
```

Use the group name from `unmanaged` (case-insensitive exact match). If multiple groups have
that name, the command returns `ambiguous_target` with exit 2 and asks for an exact key;
it never chooses one arbitrarily. Exact keys still work; quote them because they often contain
`|` and paths. `review` undoes the temporary choice; `preview` never saves it.
This reversible preference needs no `--yes`; the command explicitly sets it, rather than
toggling it, so repeating it is safe. Output reports `temporary`, `previous_temporary`,
`changed`, `would_change`, `dry_run`, and the number of `running_copies`. The app reflects the
choice on its next scan. `data.temporary_keys` lists saved choices, including stopped groups;
`review` accepts those exact keys too. Unknown targets return `not_found` with exit 3. Marking a new
group requires it to be present in unmanaged inventory. No health probe runs unless requested.
Existing `temporary KEY --undo`, `--dry-run` and `--json` invocations remain supported.

## Logs and login apps

```bash
harbor logs com.krittaprot.3things.server --stream stderr --lines 100 --json
harbor login --json
```

Logs use an exact **job label**, not a service ID. The path comes from that job's plist.
The reader returns at most the last 1 MiB and up to the requested number of lines (1–10,000).
`truncated` records when earlier content was omitted. Log files are never modified.

`login` explicitly invokes System Events. macOS may request Automation permission for the
calling terminal/agent; denial or a 30-second helper timeout produces an error. Other commands
do not access login apps unless `--include-login` is requested. The CLI does not update the
desktop's saved login-path cache.

## Service controls

```bash
harbor preview restart 3things
harbor restart 3things --yes --json
harbor stop 3things --yes --json
harbor start 3things --yes --json
```

These commands never prompt on stdin. Without `--yes` or `--dry-run`, they return
`confirmation_required` with exit code 2 before scanning or changing anything.
`--yes` and `--dry-run` are mutually exclusive. Dry runs validate user-agent paths/labels
and return the affected jobs. Actual actions revalidate through the same implementation
used by the desktop's confirmation dialog. Services containing read-only jobs are refused.
`harbor preview start NAME`, `preview stop NAME`, and `preview restart NAME` are natural
equivalents of the corresponding `--dry-run` commands. Actual start/stop/restart still require
`--yes`.

Jobs start in dependency order (Postgres, other jobs, tunnels) and stop in reverse order.
`start` bootstraps unloaded jobs; already-loaded jobs are skipped. `restart` unloads then
bootstraps the group. `accepted: true` means launchctl accepted the commands, **not** that the
service is healthy. Follow with `show` and `health`. A failure can leave some jobs changed;
rescan before deciding whether to retry. There is no CLI Mac-restart command.

## Snapshots and recovery

```bash
harbor snapshot --json
harbor snapshot --save --include-login --json
harbor compare "/path/from/saved_path.json" --include-login --json
```

`snapshot` prints current inventory. Only `--save` writes a timestamped baseline under
`~/Library/Application Support/Harbor/snapshots/`; the result includes its `saved_path`.
`compare` reads that saved file and compares it with the current boot. Within the same boot,
it returns `new_boot: false` and `report: null`. Include login apps on both calls to compare
their recovery. No command here restarts the machine.

## JSON contract and exit codes

When stdout is piped/captured, or with `--json` (before or after the command), stdout contains
exactly one JSON object. No
progress messages or ANSI formatting are mixed into it. Help/version requests remain text.

Success:

```json
{"schema_version":1,"ok":true,"data":{},"diagnostics":null}
```

`data` contains the selected result; `diagnostics` is the scan diagnostics object, or `null`
for `capabilities`. Successful inventory can still have `diagnostics.warnings` and
`visibility_limited: true`. Inspect those rather than assuming complete visibility.

Error:

```json
{"schema_version":1,"ok":false,"error":{"code":"confirmation_required","message":"..."}}
```

- **0:** command completed.
- **1:** scan/operation failed, or `health` found a failed job/unhealthy probe.
- **2:** invalid arguments, missing confirmation or invalid snapshot format.
- **3:** requested service/job or configured log stream not found.
- **4:** refused operation (root invocation, read-only service, invalid mutation target).

A `health` result with exit 1 still has `ok: true` when the checks completed successfully;
`data.unhealthy` is true. `NotChecked` services are not asserted healthy. Process errors have
`ok: false` and an `error` object. Human-mode errors/warnings go to stderr; JSON errors go to stdout.

## Verification

Rust tests include process-level JSON/exit-code/confirmation contracts and bounded log-tail
tests. `scripts/cli-smoke.py` creates one uniquely named temporary LaunchAgent and exercises
dry-run, confirmation refusal, start, show, ports, health, log tail, restart and stop, then
removes its own fixture. It does not change existing services.

```bash
cargo test --manifest-path src-tauri/Cargo.toml
python3 scripts/cli-smoke.py                         # debug CLI
python3 scripts/cli-smoke.py "$HOME/.local/bin/harbor" # installed CLI
```

The desktop and CLI share `scanner.rs`, `grouping.rs`, `health.rs`, `operations.rs`, plist
validation and snapshot comparison. The CLI entry point is `src-tauri/src/bin/harbor.rs`;
`src-tauri/src/cli.rs` owns argument parsing and the versioned response contract.

Verified 2026-10-15: 28 Rust unit tests and three CLI process-contract tests passed; clippy
with all targets/features and warnings denied passed. The owned-fixture smoke passed against
both debug and installed release binaries. Installed-CLI checks also verified saved snapshots,
same-boot comparison, read-only service refusal, targeted and aggregate health exit semantics,
and restart readiness. No existing service was changed. The desktop app still bundles with
`launchd-ui` as its main executable and `harbor` as the additional CLI binary.
