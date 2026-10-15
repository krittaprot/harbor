# Harbor — OpenCode handoff

Updated 2026-10-15 (Asia/Bangkok). The owner asked to stop implementation, document the current work, and continue in OpenCode.

## Read first

Read `../AGENTS.md`, `../APPS.md`, and `docs/PLAN.md` from the project root. This handoff records progress; the plan remains the implementation specification.

The original request was: ask the four questions in plan §10, then implement phases 0 and 1. The questions were asked, but **no answers have been received**. Ask for these answers at continuation; do not treat the displayed suggested choices as selections:

1. Public GitHub fork or private copy? Only a local repository exists so far.
2. Should Harbor start itself at login?
3. Should notifications be enabled by default?
4. Which services should be pinned in the tray? The design suggests crystal-blood, hermes, 3things, fasting.

No GitHub repository was created, nothing was pushed, no PR was opened, and no login/notification settings were installed or changed.

## Repository and toolchain

- Branch: `harbor/main`.
- Remote: `upstream` → `https://github.com/azu/launchd-ui.git`; no `origin`.
- Upstream base: `c98e9b5` (v1.4.2).
- `585cd7c`: preserved the supplied plan and design files.
- `ef0f604`: renamed the app surface, added MIT license/attribution and CSP.
- The subsequent WIP commit preserves the unfinished phase 1 sources and this handoff. It is deliberately not a completion checkpoint.
- Rust/cargo 1.95.0 is installed. The project pins pnpm **10.28.2**, which was used successfully; the plan's pnpm 11.20 note does not describe this checkout's active package-manager version.
- `pnpm install --frozen-lockfile` completed. pnpm reported ignored install scripts for esbuild and msw; the actual frontend test/build commands succeeded without changing approval settings.
- ProVault was verified mounted before setup (different device ID from its parent). Keep the checkout here and stop if the volume is missing.

## Phase 0: implementation in place, visual acceptance unfinished

Completed:

- Initialized the repository in place, adopted upstream history, and retained all supplied docs/designs.
- `src-tauri/tauri.conf.json`: product/window name `Harbor`, identifier `com.ode.harbor`, initial and minimum window size 1100×720.
- CSP: `default-src 'self'; style-src 'self' 'unsafe-inline'`.
- Updated the main heading and HTML title to Harbor.
- Added `LICENSE` with MIT text and both azu and 2026 ode copyright notices; added attribution/development notes above the preserved upstream README.
- Kept the Cargo package/library names `launchd-ui` / `launchd_ui_lib`, as permitted by the plan.
- Upstream already ignored `node_modules/` and `src-tauri/target/`.

Verification completed **before adding phase 1 dependencies/sources**:

| Check | Result |
| --- | --- |
| `cargo test --manifest-path src-tauri/Cargo.toml` | 14 passed |
| `pnpm test` | 46 passed in 5 files |
| `pnpm build` | Passed (TypeScript and Vite) |
| `pnpm tauri dev` | Native executable compiled and launched successfully in a foreground PTY |

Native visual acceptance remains open. The computer-use tool could not resolve the unbundled executable as an app (both executable-path and `Harbor` lookups returned `Invalid app`). That is a tooling limitation, **not proof of a rendered/working UI**. The job list/details, native title, and CSP behavior still need an actual native-window check. A local debug `.app` bundle was considered as the next route but **has not been built**. No owner services were started, stopped, restarted, edited, or adopted to test the upstream actions.

The dev session was stopped with Ctrl-C. A subsequent process/listener check found no Harbor dev executable, task-owned Vite process, or listener on port 5173. There is no retained preview or background dev server.

## Phase 1: incomplete, uncompiled draft

Added dependencies with `cargo add` and fetched them:

- `sysinfo = "0.37"` → lockfile 0.37.2
- `libproc = "0.14"` → lockfile 0.14.11
- `chrono = "0.4"` with serde
- `tokio = "1"` with rt-multi-thread, time, macros, sync
- `libc = "0.2"`

**Known metadata mismatch:** sysinfo 0.37.2 requires Rust 1.88, while upstream Cargo.toml still declares `rust-version = "1.85.0"`. The installed compiler is new enough. Update the declared minimum (or deliberately select compatible dependencies) before claiming the dependency change is ready.

Draft files:

| File | Current intent |
| --- | --- |
| `src-tauri/src/model.rs` | Phase 1 raw snapshot, process and port records, diagnostics. Uses nullable UID/start time and CPU until two samples are available. |
| `src-tauri/src/procs.rs` | Retains sysinfo System across scans; PID map and parent tree; cycle-safe descendants and nearest launchd ancestor labeling; one fixture-based unit test. |
| `src-tauri/src/ports.rs` | libproc TCP LISTEN enumeration for current-user processes; one non-root lsof fallback; address normalization/deduplication; parser and socket-decoding unit tests. |
| `src-tauri/src/fixtures/listeners.lsof` | Real field-mode listener capture from mini on 2026-10-15, using `lsof -nP -iTCP -sTCP:LISTEN -F pcn`. Contains PID/command/address/port records, no argv/environment. |

**These modules are not declared in lib.rs yet. They have not been compiled, linted, or tested.** A normal cargo test currently would not exercise these new files. Review them as drafts, including API calls and unsafe union decoding.

Still missing:

- `scanner.rs`, persistent scanner ownership, ten-second loop, cached shared snapshot, startup/shutdown lifecycle, and `harbor://snapshot` events.
- Tauri `get_snapshot` command and handler registration; optional `rescan_now` if implemented in this phase.
- React/TypeScript mirror and invoke helper.
- Native IPC verification, repeat-scan/CPU verification, comparison against live lsof, and a final regression/format/lint pass.
- A documented final snapshot contract. The draft uses raw `processes`/`ports` rather than pretending phase 2's grouped services or later health/system checks already exist. Reconcile this incremental contract with plan §5 before wiring the frontend.

Review points before wiring:

- Unknown UID must stay unknown; never infer root or authorize an action from missing metadata.
- Non-root lsof cannot guarantee visibility of other users' sockets; the draft exposes `visibility_limited`.
- The draft fallback only merges other-user, failed-libproc, or newly observed PIDs. Verify it does not omit a legitimate current-user listener when libproc succeeds incompletely.
- Wildcards and IPv4-mapped IPv6 normalize; distinct concrete IPv4 and IPv6 bindings remain distinct. `listening_port_count` is intended to count distinct numeric ports, separately from binding count.
- The lsof subprocess currently uses blocking `Command::output()` without a timeout. Keep all blocking enumeration off the async/UI thread and review timeout/shutdown behavior.
- The parser fixture includes 3216, 5432, and 9230. That only proves those ports existed in the capture, **not that get_snapshot works**. The plan's approximately 41 ports is historical, not an exact assertion for today's machine.
- New tests intentionally use captured/synthetic data; live scanner checks should be an explicit smoke check, not ordinary deterministic unit tests.

## Suggested continuation sequence

1. Resolve the four owner questions and record the answers; do not publish or enable startup speculatively.
2. Read the draft sources and fix the Rust minimum/version metadata.
3. Wire the new modules, compile, and run the parser/process-tree/socket tests; fix issues found.
4. Implement scanner lifecycle, caching, event emission, command registration, and TypeScript contract. Preserve upstream modules and features.
5. Verify the actual `get_snapshot` command against live listeners, including 3216/5432/9230 when still present. Record counts, errors/visibility, and what changed from the historical fixture.
6. Complete the native-window phase 0 check and phase 1 acceptance; stop all task-owned foreground dev processes afterward.
7. Update this handoff and Harbor's row in `../APPS.md`, and commit in small local steps. No push/PR unless requested.

Useful commands, run from the project root:

```sh
pnpm install --frozen-lockfile
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
pnpm test
pnpm lint
pnpm build
pnpm tauri dev
# Ctrl-C when done; never background this command.
```

No Harbor config or snapshot data has been written. Future runtime data belongs in `~/Library/Application Support/Harbor/`, never in the checkout. Do not modify the real failing restore-check job, real unmanaged processes, Postgres/MLflow data, or unrelated services as part of this continuation.
