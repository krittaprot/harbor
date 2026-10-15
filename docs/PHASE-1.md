# Phase 1 inventory contract and verification

## Owner decisions (2026-10-15)

Eventual public GitHub fork; start Harbor at login; notifications on by default;
tray pins crystal-blood, hermes, 3things, fasting. Publication is a separate step.
Startup/notification preferences apply when those features are implemented.

## Contract

`get_snapshot` returns the last completed in-memory `Snapshot`, without triggering a scan.
Before initialization it rejects with `initial scan in progress; wait for harbor://snapshot`.
Consumers should subscribe before invoking and ignore older `taken_at` values if events race
the initial request. No snapshot files are written.

Rust `model.rs` and TypeScript `src/types.ts` mirror:

- `taken_at`: UTC RFC3339 completion time; `uid`: current user.
- `processes`: PID/parent/UID/name/argv/cwd/CPU/memory/start-time/launchd-label records.
  Unknown UID, cwd and start time remain null. CPU is percentage of one core (can exceed 100),
  null until the same PID/start-time identity has two samples. Missing UID never implies root.
- `ports`: TCP LISTEN `{ pid, port, addr }` tuples, sorted and deduplicated. Wildcard IPv4/IPv6
  collapse to `*`; mapped IPv6 normalizes to IPv4. Concrete IPv4/IPv6 bindings remain distinct.
- `listening_port_count`: distinct numeric ports, not bindings or file descriptors.
- `diagnostics`: duration, libproc binding count, extra lsof bindings, inaccessible PIDs,
  warnings and `visibility_limited` (always true for this non-root app).

This increment of PLAN §5 exposes raw inventory, not fabricated service/health/system results.
Later phases extend it. An inaccessible process or failed fallback is reported without discarding
the rest of the scan. Other-user listeners can be hidden even when warnings are empty.

## Lifecycle

One dedicated worker owns the retained `sysinfo::System`, scans immediately, then every ten
seconds (scan time deducted), updates `Arc<RwLock<Option<Snapshot>>>`, then emits the event.
The blocking worker serves the plan's interval role without enumeration on the UI or async
executor. Shutdown wakes the interval and joins the current scan. `rescan_now` is deferred.

libproc reads current-user sockets. One non-root lsof call merges **all** visible bindings,
including those missed by apparently successful libproc calls. A read-only `launchctl list`
supplies nearest-job ancestry labels; grouping remains phase 2.

Scanner subprocesses have a three-second deadline, nonblocking pipe draining and 8 MiB limits
per stream. On failure only the owned, unreaped child receives SIGTERM, with a one-second exit
check. If it ignores TERM, a reaper retains ownership and the scan reports failure; no SIGKILL.
Native sysinfo/libproc calls do not have a user-space cancellation deadline.

## Native acceptance harness

Run from the project root, in the foreground:

```sh
pnpm build
RUSTUP_HOME="$HOME/.rustup" cargo build --manifest-path src-tauri/Cargo.toml --features smoke-test,tauri/custom-protocol
src-tauri/target/debug/launchd-ui
```

The opt-in feature opens native WebKit with bundled assets, invokes the registered command,
checks events and CPU after another scan, opens read-only job details, checks DOM/CSP, compares
listeners to independent lsof and exits through Tauri's normal shutdown. It prints no argv/cwd.
Assertions/missing lsof bindings fail the process. A 40-second watchdog exits if IPC fails.
Live listeners can change between comparison samples. Normal builds exclude the harness.

Observed 2026-10-15 05:22 UTC:

- Native title and heading Harbor; 45 jobs; details opened for `ai.hermes.gateway`.
- 741 process records; 70 distinct ports / 72 bindings; exact lsof match, no missing or extras.
- 3216, 5432, 9230 present. The plan's approximate 41 ports is historical.
- First scan zero CPU samples; next scan about ten seconds later had 551 CPU samples.
- Event/cache timestamps matched. Second scan 146 ms; no inaccessible PIDs or warnings.
- No CSP violations after adding `connect-src 'self' ipc: http://ipc.localhost` to the
  specified policy. Before that, WebKit blocked `ipc://localhost/plugin%3Aevent%7Clisten`.
- 20 Rust and 46 frontend tests passed; rustfmt, clippy (all targets/features), oxlint and
  TypeScript/Vite build passed.

Native DOM/IPC behavior is verified; **visual appearance still needs owner confirmation**.
Desktop-control tools were unavailable. No real service mutations were exercised.
