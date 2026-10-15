# Harbor implementation and verification

2026-10-15. Implementation extends through phase 10. Owner acceptance is distinguished
from automated/native checks; a real reboot was not requested or performed.

## Delivered

- Services: live grouped jobs, descendants, ports, HTTP/TCP/tunnel health, search, filters,
  job details/logs/editor, service actions, persistent pins and ignore preferences.
- Unmanaged: executable/script grouping, owner/argv/start-time validation, XML preview,
  no-overwrite adoption, readiness gate before stopping originals, SIGTERM-only termination.
- Login: live System Events inventory, add/toggle commands, vendor helpers, Docker leftover
  detection, unloaded user agents and archive action, read-only extension output.
- Restart: readiness checklist, manual/hourly/pre-restart snapshots, post-boot comparison,
  explicit confirmation before asking macOS to restart.
- Tray: status icon, pinned services, restart confirmation in the main window, notifications,
  settings synchronization, hide-on-blur popover and main-window close-to-tray behavior.
- System light/dark appearance, Harbor anchor app icon, release and normal debug bundles.

## Checks passed

- 26 Rust tests; 46 frontend tests; rustfmt; clippy with all targets/features and warnings denied;
  oxlint; TypeScript/Vite production build; `git diff --check`.
- Native WebKit/IPC: title, Services, job detail open/close, search by port, restart/login
  navigation, adoption preview, snapshot events/cache, repeated CPU measurements, no CSP errors.
- Final inventory: **69 distinct ports / 71 bindings**, exactly matching independent `lsof`.
  Ports 3216 and 5432 were present. The design's old 9230 listener is no longer running.
- crystal-blood: four jobs, Postgres and application descendants, ports 5432/5050/2024/3000/20242.
  fasting: server/push/sync/tunnel jobs. HTTP 200, 302, 401 and 404 responses and tunnel `/ready`
  responses classified as expected. restore-check is shown failed with exit 1; it was not changed.
- Live login inventory: Claude, Ollama, OrbStack, Lane and FigmaAgent. Docker's surviving
  privileged helper is flagged because its associated Docker app is absent.
- Adoption: two foreground-owned Python listeners were grouped, previewed, adopted into one
  temporary LaunchAgent, and stopped only after the new agent owned a listener. Both originals
  exited with SIGTERM. The new managed service was visible. The fixture agent and plist were
  unloaded/removed afterward. No owner's services or unmanaged processes were mutated.
- Layout overflow checks at 320, 375, 414, 768, 1100 and 1440 px; popover reviewed at 360×600.
- Both normal bundles pass `codesign --verify --deep --strict` with local ad-hoc signatures.
  The release bundle was launched as an owned foreground test: its native window appeared,
  it remained running without the smoke harness, and it wrote an automatic runtime snapshot.
  The test process was then identity-checked and stopped with SIGTERM.

## Visual evidence

`verification/harbor-{light,dark,restart,adopt,login,popover}.png` are **Chromium renders of
DOM and CSS exported from the running native WebKit app**, with live data. They are not native
desktop screenshots. macOS window capture returned “could not create image from window”.
These renders were inspected and used to fix overlay cleanup, adoption-sheet positioning and
popover footer clipping. Native title-bar, tray positioning and system interaction still need
the owner's desktop check.

## Reproduce

Run from the project root, with ProVault mounted. No background dev server is needed.

```bash
pnpm build
pnpm tauri build --debug --features smoke-test --bundles app
src-tauri/target/debug/harbor-app
uv run scripts/render-smoke.py
```

For the state-changing test **only on harness-owned fixtures**:

```bash
python3 scripts/adopt-smoke.py
```

The harness isolates its exact owned fixture from OpenChamber's launchd ancestry only in
smoke builds. Real adoption uses the normal production commands and fresh PID validation.
An early file-backed fixture could not obtain a listener under launchd; its originals remained
running and the new agent was unloaded. The passing fixture uses an inline Python program
and the home directory, so it does not depend on launchd access to the project volume.

Normal bundles must be built **without** `smoke-test` (the smoke executable exits itself):

```bash
pnpm tauri build --bundles app
pnpm tauri build --debug --bundles app
open "src-tauri/target/release/bundle/macos/Harbor.app"
```

The normal debug bundle was refreshed too, so the earlier debug launch command now opens
the redesigned interface. Runtime config, login-path cache and diagnostic DOM exports were
written under `~/Library/Application Support/Harbor/`; the normal release launch also wrote
an automatic snapshot there. Fixture logs live under `~/Library/Logs/`.

## Owner acceptance remaining

1. Inspect the native release app and tray in both appearances. Install the bundle by hand.
2. Exercise a login toggle and archive on disposable items; verify macOS Automation permission
   handling, installed start-at-login and notifications. These commands are implemented, but
   real owner items were deliberately not changed for acceptance.
3. Open the login inventory before saving a pre-restart snapshot (login comparison uses the
   cached inventory). At a convenient time, perform the real restart test and inspect the report.
4. Public MIT source publication was approved on 2026-10-15: https://github.com/krittaprot/harbor.
   A packaged binary release remains a separate step.

Known v1 limitations: non-root visibility; extension inventory is read-only command output;
system-daemon running state is Unknown rather than inferred from the user domain; a fixed-port
adoption can fail readiness while the original owns the port; the restart report is held for
the current app session, while raw snapshots persist. Current process counts can differ from
the design's historical fixture; mixed-parent unmanaged groups are Unknown rather than Orphaned.
