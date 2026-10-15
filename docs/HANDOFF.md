# Harbor — continuation handoff

Updated 2026-10-15. Phases 0 and 1 continued in OpenCode.

## Read first

Read `../AGENTS.md`, `../APPS.md`, `docs/PLAN.md` and `docs/PHASE-1.md`.
The latter records owner decisions, the snapshot contract, lifecycle and acceptance evidence.

## Owner decisions received

Eventual **public fork**, **start at login**, **notifications on**, pins **crystal-blood, hermes,
3things, fasting**. Publication remains separate; nothing created or pushed to GitHub.
Startup/notification preferences are recorded for later phases, not installed now.

## Implementation

- Local branch `harbor/main`, upstream azu/launchd-ui v1.4.2 (`c98e9b5`); no origin or PR.
- Phase 0 rename, MIT attribution/license, sizing and CSP implemented. Native verification caught
  a blocked IPC connection; CSP now permits the specific Tauri IPC endpoints.
- Phase 1 modules wired, compiled and tested: `model.rs`, `procs.rs`, `ports.rs`,
  `scan_command.rs`, `scanner.rs`; `get_snapshot` registered; TS mirror/invoke helper present.
- Dedicated worker retains CPU state, caches scans and emits `harbor://snapshot` every ten
  seconds. Shutdown wakes and joins it. Enumeration stays off UI/async threads.
- libproc scans current-user sockets; bounded lsof merges all visible bindings. Unknown
  metadata stays null and non-root visibility is explicit.
- Rust minimum 1.88 for sysinfo 0.37.2. Installed Rust 1.95; project pnpm pin 10.28.2.
- `smoke-test` feature adds a native WebKit/IPC acceptance harness, excluded from normal builds.
  See `PHASE-1.md` for commands and the incremental snapshot contract.

## Acceptance

Passed: 20 Rust tests, 46 frontend tests, rustfmt, clippy (all targets/features), oxlint,
TypeScript/Vite build, native smoke. The native check verified Harbor title, rendered job list
and read-only details, snapshot/event/cache, two-sample CPU, CSP and normal exit.
70 distinct ports / 72 bindings exactly matched independent lsof, including 3216, 5432, 9230.

**Remaining phase 0 check:** owner visual inspection of the native window. DOM assertions
are not visual proof; desktop-control tools were unavailable. Real service mutation actions
have not been exercised against owner services.

## Continue

1. Owner opens the normal debug app and confirms title/list/details appearance.
   Built at `src-tauri/target/debug/bundle/macos/Harbor.app` with
   `pnpm tauri build --debug --bundles app` (without the smoke feature).
2. Phase 2 grouping/configuration, following the plan and preserved design.
3. Publish the public fork only on a separate publication request.

Runtime data belongs in `~/Library/Application Support/Harbor/`; none was written by this phase.
Do not alter the failing restore-check job, real unmanaged processes, Postgres/MLflow data or
other services while testing. Keep dev commands foreground-only and stop them when done.
All native smoke processes exited; no task-owned Harbor/Vite process remains.
