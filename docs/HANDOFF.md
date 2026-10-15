# Harbor — continuation handoff

Updated 2026-10-15. Harbor's redesigned app and backend features are implemented through phase 10.

## MacBook scan coverage and background name: v1.5.1

Owner screenshot shows Teams' system plist unreadable (OS permission denied) and Java's helper
plist missing at read time. These paths are absent on the development Mac, so the MacBook's
underlying files were not changed or asserted repaired. Inventory now preserves typed OS errors
and emits `diagnostics.inventory_issues` with permission/missing/broken-link/invalid/read-failure
codes, explanations, guidance and raw details. The UI uses **Scan coverage**, **Copy scan report**
and expandable details; CLI has the same notes and can diagnose skipped files by path or stem.
Other files and loaded-job fallback remain visible. Old snapshots default to no inventory issues.

Startup plugin explicitly uses `Harbor`; app metadata sets display/name to Harbor and Tauri's
main binary is now `harbor-app` (CLI stays `harbor`). An installed app archives its user-owned,
regular `launchd-ui.plist` only if it has the exact old label/one executable argument for the
same bundle directory. Symlinks, unrelated apps and custom commands are preserved. The old
loaded job is not booted out, because that could terminate the app itself; macOS may retain its
old entry/notification until logout/login. Same-location upgrades avoid duplicate future startup.

Verified: 51 frontend tests; 37 Rust unit + 4 CLI contract tests; 26 release tests; lint, clippy,
format/version/build checks. Real temporary-HOME CLI fixtures reproduced permission denial,
dangling symlinks and malformed files. An installed-layout native app in a temporary HOME
created Harbor.plist pointing to harbor-app and archived the legacy file byte-for-byte; real
user startup files were unchanged. Built UI dark-mode fixtures verified copy/details and narrow
layouts; `harbor-scan-coverage.png` uses mocked screenshot-derived diagnostics, not native capture.
Release notes: `docs/releases/v1.5.1.md`.
Release/debug bundles rebuilt and local CLI refreshed to 1.5.1. DMG verified and mounted
read-only; its signed app has Harbor display metadata, only `harbor-app`/`harbor` executables,
and working CLI inventory. DMG size 8,099,412 bytes; SHA-256
`37bb1a848171c6f62d7879c3facb341af31fbb78467ee606b2c1f9a4c75f6cac`.

**Owner acceptance (2026-10-15):** After v1.5.1 delivery, the owner said “Nicely working!”
and provided a native MacBook screenshot. The macOS App Background Activity notification
now reads **“harbor-app” can run in the background**, not `launchd-ui`. This is the executable
name; the app display name and LaunchAgent label are **Harbor**. Record the observed spelling
accurately rather than claiming the notification says exactly “Harbor”. The owner accepted
the result. Login/reboot recovery and Harbor-generated service alerts remain separate checks;
see `docs/VERIFICATION.md`.

## First packaged release: v1.5.0 (2026-10-15)

Version bumped consistently across package.json, Cargo.toml, Cargo.lock and tauri.conf.json.
Release notes: `docs/releases/v1.5.0.md`; README now documents DMG installation, the bundled
CLI, Apple Silicon scope and macOS's first-launch approval for ad-hoc/non-notarized builds.
This release includes the CLI/adoption work, health wording and monochrome tray icon.

Release checks passed: 49 frontend tests, 33 Rust unit tests, 4 CLI contract tests, 26 release
script tests, lint, version consistency, formatting and all-target/all-feature clippy.
The packaged app and CLI both report 1.5.0 and are arm64 with system-library dependencies.
Package with `CI=true pnpm tauri build --bundles dmg` to avoid interactive Finder styling.
Publish the checked DMG and `SHA256SUMS` together at tag `v1.5.0`; source on `harbor/main`.
The 8,086,520-byte DMG passed `hdiutil verify`; mounted read-only, its bundle passed deep/strict
signature verification and contains the Applications link and both arm64 executables. The
mounted CLI passed version and capabilities checks. DMG SHA-256:
`dec635c069b907ea9599bfa8dc5b99d6223fa690e33db7ce47e27040233b1c21`.

## Monochrome menu-bar icon (2026-10-15)

The tray now keeps its native template anchor instead of replacing it with green/orange/red
icons after every scan. macOS renders it white against a dark menu bar and adjusts contrast
for light backgrounds. Status still appears in the tooltip/popover. Release/debug bundles
rebuilt, formatting checked and release signature verified. Quit/reopen to apply.

## Health wording (2026-10-15)

Replaced the Services healthy/total ratio and progress ring with **Checks passing** and an
explicit unchecked-service count. The summary distinguishes failed checks, no detected check
failures, and no results yet. Counts describe probe outcomes; failed jobs/recent restarts remain
in Needs Attention independently. Unchecked rows show **Health not checked** under lifecycle
status. Stopped/unloaded jobs are explained as not necessarily failures. Zero-count attention
and process-review tiles use neutral icons, and the empty attention filter explains its result.

Verified lint, build and all 49 frontend tests. Chromium checked the built UI at 375/768/1100px,
plus failure, all-unchecked and empty fixtures via mocked read-only IPC. The captured live scan
showed 17 passing and 10 unchecked services. `docs/verification/harbor-health-wording.png`
shows that built preview, not a native screenshot. Release/debug app bundles rebuilt; release
signature verified. Quit/reopen Harbor to see the change.

## CLI addition (2026-10-15)

Implemented on local branch `harbor/cli`; publication branch remains `harbor/main`.
`harbor` is installed at `~/.local/bin/harbor`. Run `harbor capabilities --json` to discover
commands; see `docs/CLI.md` for JSON/exit-code contracts, controls, tests and installation.
The CLI works without the GUI and shares scanner/grouping/health/operations code with it.
28 Rust unit tests, three process-contract tests and owned-fixture CLI smoke checks passed.
Source changes are committed locally; this feature branch has not been pushed.

## Temporary unmanaged processes (2026-10-15)

Services now exposes **Mark temporary / Undo temporary** on each unmanaged command group,
replacing the subtle “Mark expected” icon. The persisted choice covers all copies and future
runs of that group. Processes remain visible with a Temporary badge, excluded from warning
counts and missing-process/port restart comparisons when marked in the saved baseline.
The existing config `ignored` list and JSON `Expected` verdict remain compatible; CLI output
adds an explanatory hint. No real processes were marked or stopped during implementation.
Verified: 29 Rust unit + 3 CLI contract tests, lint and frontend build, and a built-UI browser
check of mark/undo and responsive overflow with mocked IPC (not native runtime acceptance).
Clippy passed with all targets/features and warnings denied. Normal release/debug bundles
were rebuilt, the release signature verified, and the installed CLI refreshed.

Agents can now set that same preference with `harbor temporary "<exact group key>" --json`,
preview it with `--dry-run`, or clear it with `--undo`. This reversible preference does not
require `--yes`. The desktop and CLI share `config::set_temporary`; `unmanaged --json` also
returns `temporary_keys` so stopped groups can be unmarked. Repeated sets are idempotent.
Installed-CLI smoke checks with an isolated HOME verified dry-run, persistence, repeated set,
inventory visibility, undo (including a stopped group), unknown-key errors and preservation of
unrelated settings. Real preferences remained byte-for-byte unchanged. Rust tests and clippy
passed again; installed CLI and both normal app bundles were refreshed.

## Natural CLI syntax (2026-10-15)

`temporary NAME` accepts case-insensitive exact group names or full keys; `review NAME`
undoes the preference. Ambiguous names return `ambiguous_target` (exit 2) with candidate keys.
`preview temporary/review NAME` and `preview start/stop/restart ID` replace dry-run flags in
the common workflow. Existing flags remain supported; actual service controls still need
`--yes`. Output defaults to text on a terminal and versioned JSON when piped/captured;
`--text`/`--json` override it. Human unmanaged inventory is now a readable list.
Verified 30 Rust unit + 4 process-contract tests, clippy, and installed-CLI smoke checks for
natural syntax, exact-key compatibility, previews, undo, automatic JSON and terminal text.
Smoke writes used an isolated HOME; actual user preferences were unchanged. Installed CLI
and both normal app bundles refreshed. Work remains on local `harbor/cli`, not pushed.

## Adoption progress and failure UX (2026-10-15)

Adoption now streams per-invocation Tauri channel updates for verify, plist write, load,
readiness, optional original-process cleanup, rollback and completion. A fixed panel above
the scrolling preview shows completed stages (not estimated time percentages), elapsed time,
readiness observations and per-PID cleanup status. Readiness timeout errors summarize what
happened, retain rollback results and next steps, and put raw launchctl output in expandable
diagnostics. Partial cleanup is shown as “Adopted · needs review.” Duplicate submission and
closing during work are blocked; errors/results scroll into view and refresh inventory.

Verified: 48 frontend tests, 30 Rust unit + 4 CLI contract tests, lint, build and clippy.
Native owned-fixture adoption verified every channel stage and readiness-before-stop ordering;
both original fixture processes exited via SIGTERM and the fixture agent/plist were removed.
Normal release/debug app bundles rebuilt afterward, release signature verified. Browser checks
covered fixed progress while scrolling, failure diagnostics and 375/768/1100px layouts.
`docs/verification/harbor-adoption-{progress,failure}.png` are Chromium renders of the built
UI with mocked adoption progress/failure, not captures of an actual adoption failure.

Owner-reported `com.harbor.opencode.exe` readiness failure was initially unresolved: screenshot shows
last exit 0 and no running PID; both log files were empty. Only read-only log inspection was
performed at that point. The subsequent investigation and resolution are recorded below.

Follow-up: fixed retry failing with “File exists (os error 17).” Preview checks existing plists;
an identical unloaded file is reused without rewriting it. Different contents are rejected with
the path and guidance to change the label or inspect the existing job. Existing loaded jobs are
also rejected with actionable guidance. Symlinks and special files are not reused. Apply failures
now offer **Retry adoption** while preview conflicts still block submission. Unit tests verify
file content/inode/mtime preservation, conflicts and symlink rejection. Native owned-fixture
smoke now loads an agent, unloads it leaving its plist, then successfully retries with that file.
Verified 49 frontend tests, 31 Rust unit + 4 CLI tests, lint and clippy; normal release/debug
bundles rebuilt and signed after smoke. The OpenCode cause was investigated next.

## Copy reports, CLI adoption, and OpenCode resolution (2026-10-15)

The adoption sheet now has native **Copy report** and **Copy plist** actions; process rows can
copy a diagnostic report too. Reports contain context, full errors, log tails, findings and
saved adoption stages/results. CLI commands: `diagnose TARGET`, `preview adopt GROUP as LABEL`,
`adopt GROUP as LABEL --yes`, `preview archive LABEL`, `archive LABEL --yes`. App/CLI share the
same implementation and validations. Attempts persist in Harbor's `adoptions/<label>.json`.

Root cause confirmed for OpenCode V2.0.29: the existing shared service registered at PID 3110,
http://127.0.0.1:49374, was healthy (`opencode api get /api/info`). A duplicate invocation of
the plist's exact `serve --service` command under launchd-like environment exited 0 in 0.08s
with empty stdout/stderr; registration and live PID remained unchanged. OpenCode documents
one shared background service per user, automatically discovered/started by clients:
https://opencode.ai/v2/docs/cli and https://opencode.ai/v2/docs/troubleshooting.

Harbor now corroborates OpenCode registry PID/URL with the user-owned process command and port,
marks the group `Managed`, and removes adoption/stop/temporary actions for that group. Its
own lifecycle is explained, rather than interpreting PPID 1 as proof of abandonment. The shared
adoption core refuses internal `opencode serve --service` commands with actionable guidance.
`Managed` is not a promise of reboot recovery. Other unmanaged groups remain reviewable.

The owner's redundant **unloaded** `com.harbor.opencode.exe` plist was archived using the new
CLI into `~/Library/LaunchAgents/.harbor-archive/com.harbor.opencode.exe.plist`. The original
PID 3110 still answered the API afterward. No live owner service was restarted or stopped.
Its diagnosis remains available by label, including the archived plist and empty logs.
The project investigation todo is completed.

CLI fixture acceptance covered dry-run, success, deliberate no-autostart readiness failure,
rollback with originals preserved, persisted diagnostics, archive and diagnosis after archive,
then successful adoption with SIGTERM. All test-owned agents/processes/files were cleaned up.
Verification passed: 49 frontend tests, 33 Rust unit + 4 CLI contract tests, lint, clippy,
CLI end-to-end smoke and native WebKit adoption/report smoke. Browser checks verified report
and plist copying, clipboard error feedback, Managed-row actions and narrow layouts with
mocked IPC/clipboard; `docs/verification/harbor-copy-report.png` is illustrative built UI.
Normal release/debug bundles and installed CLI refreshed after testing; release signature verified.

## Read first

Read `../AGENTS.md`, `../APPS.md`, `docs/PLAN.md`, `docs/PHASE-1.md` and `docs/VERIFICATION.md`.
The verification document distinguishes completed checks from owner acceptance still needed.

## Owner decisions received

**Public MIT fork**, **start at login**, **notifications on**, pins **crystal-blood, hermes,
3things, fasting**. The owner approved publication on 2026-10-15:
https://github.com/krittaprot/harbor. Further testing continues; no binary release is published.
Startup/notification preferences are implemented. Startup registration is applied only from an
installed bundle, not from builds inside `target/`. Nothing was installed into Applications.

## Implementation

- Published default branch `harbor/main`, origin `krittaprot/harbor`, upstream
  `azu/launchd-ui` v1.4.2 (`c98e9b5`). Published as a GitHub fork; no publication PR needed.
- Persistent scanner retains CPU state, gathers launchd/process/port inventory, groups services,
  caches health checks and system state, emits snapshots, and services explicit rescan requests.
- New backend modules: `config`, `grouping`, `health`, `system`, `harbor_commands`, `adopt`,
  `login`, `snapshots`, `tray`. Existing job mutations now enforce direct user LaunchAgent paths.
- New frontend: Services, OpenAtLogin, RestartReadiness, AdoptSheet, Sidebar, ServiceTile,
  Popover, useSnapshot, Harbor API helpers and design tokens. Existing App moved to LegacyJobs;
  upstream detail/editor/log components remain available.
- Tray uses a separate popover Vite entry. Light/dark themes follow system appearance. The
  template/colored tray icons and app icon are generated by `scripts/icons.py`.
- Confirmation gates service and Mac restart actions. Adoption validates identity and waits
  for readiness before SIGTERM; system jobs remain read-only.
- Rust minimum 1.88 for sysinfo 0.37.2. Installed Rust 1.95; project pnpm pin 10.28.2.
- `smoke-test` adds native WebKit/IPC acceptance, DOM export and opt-in owned-fixture adoption;
  excluded from both normal app bundles. See `VERIFICATION.md` for commands and provenance.

## Acceptance

Passed: 26 Rust tests, 46 frontend tests, rustfmt, clippy (all targets/features), oxlint,
TypeScript/Vite build, native smoke, owned-fixture adoption and release/debug bundle builds.
Final normal inventory: 69 ports / 71 bindings exactly matched independent lsof; 9230 is no
longer listening. crystal-blood and fasting grouping, HTTP/tunnel health and all five expected
login apps verified live. Visual exports reviewed in both themes and at narrow/wide widths.

Owner checks remain for native window/tray interaction, installed startup/notifications,
login toggle/archive on disposable items, and an actual reboot. Screenshots are headless
renders of native-exported DOM/CSS, not desktop captures. No real owner service was mutated.

## Continue

1. Open `src-tauri/target/release/bundle/macos/Harbor.app`; install by hand if satisfied.
   The normal debug bundle at `src-tauri/target/debug/bundle/macos/Harbor.app` was refreshed too.
2. Complete owner acceptance in `docs/VERIFICATION.md`; preserve the original plan constraints.
3. Continue testing on the public fork. A packaged binary release remains a separate step.

Runtime data belongs in `~/Library/Application Support/Harbor/`; config, login-path cache and
diagnostic DOM exports were written there. Owned adoption fixture logs are in `~/Library/Logs/`.
The signed normal release launch also wrote its first automatic snapshot in Application Support.
Do not alter the failing restore-check job, real unmanaged processes, Postgres/MLflow data or
other services while testing. Keep dev commands foreground-only and stop them when done.
All native smoke and throwaway listener processes exited; their launchd jobs/plists were cleaned
up. No task-owned Harbor/Vite process remains. No Mac restart was performed.
