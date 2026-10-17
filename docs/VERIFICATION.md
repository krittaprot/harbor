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

### In-app updater — v1.5.2, 2026-10-17

- 42 Rust tests, 4 CLI contract tests, 57 frontend tests, 26 release-script tests and 3
  updater asset-validation tests; formatting, lint, type checks, version consistency and build.
- The real updater library checked a loopback manifest, downloaded and verified a signed
  fixture, and replaced only its temporary fixture app. A tampered archive left the old
  fixture untouched; an older release was not offered.
- Native WebKit/IPC smoke acceptance passed with the Software updates card, correct version,
  check-only smoke restrictions, no CSP errors and no missing port bindings. The native-exported
  Settings DOM was rendered and inspected at the app's minimum width without overflow.
- Release signing was exercised locally with an encrypted key stored outside the repository.
  Public release artifacts must still pass the release workflow's two-architecture manifest checks.

Real future-version upgrade/relaunch and administrator-prompt acceptance on installed Macs
remain open. Existing v1.5.1 installations require one manual upgrade to get the updater.

### Tailscale status publishing checks — 2026-10-17

- 38 Rust unit tests, 4 CLI contract tests, 51 frontend tests and 26 release-script tests.
- rustfmt, all-target/all-feature clippy with warnings denied, oxlint, TypeScript checks,
  version consistency and the Vite production build.
- CLI `status --json` with `TERM` and `TAILSCALE_BE_CLI` removed from the parent environment:
  `ok: true`, `tailscale_up: true`.
- Rebuilt debug smoke bundle; native WebKit/IPC acceptance passed with those same variables
  removed. The exported sidebar says **Tailscale connected**, explicit Rescan returns a newer
  snapshot, and there are no CSP violations or missing bindings in the independent `lsof` check.
- The first native run exposed a smoke-harness assumption that startup always selects Services.
  The harness now selects Services explicitly, allowing a saved restart report to open first.

These are source/debug-build checks, not a new release or an installed login/reboot test.
The existing native harness uses local service fixtures; it is not a portable CI acceptance suite.

### Initial implementation

The checks below describe the initial implementation. Subsequent release checks are recorded
in [HANDOFF.md](HANDOFF.md), including v1.5.1's native startup migration and CLI diagnostics.

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
the owner's desktop check, with the MacBook confirmation below providing additional evidence.

## Owner confirmation — MacBook, 2026-10-15

After the v1.5.1 delivery, the owner reported **“Nicely working!”** and supplied a native
macOS screenshot in the development conversation. The **App Background Activity** notification
reads **“harbor-app” can run in the background**, replacing the earlier **“launchd-ui”** name.

This confirms the updated background executable identity on the owner's MacBook. The app's
display name and startup plist label are **Harbor**; macOS's notification uses the executable
name **harbor-app**. It should not be documented as displaying the exact text “Harbor” there.

The owner accepted this result. This screenshot is separate from the mocked diagnostic UI
fixtures and does not independently verify a login/reboot cycle, Harbor's service-health
notifications, or repairs to the Teams/Java installation files.

## Reproduce

Run from the project root, with the local service fixtures available. No background dev server is needed.

```bash
pnpm build
pnpm tauri build --debug --features smoke-test --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
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
pnpm tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
pnpm tauri build --debug --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
open "src-tauri/target/release/bundle/macos/Harbor.app"
```

The normal debug bundle was refreshed too, so the earlier debug launch command now opens
the redesigned interface. Runtime config, login-path cache and diagnostic DOM exports were
written under `~/Library/Application Support/Harbor/`; the normal release launch also wrote
an automatic snapshot there. Fixture logs live under `~/Library/Logs/`.

## Owner acceptance remaining

1. Finish native app/tray checks in both appearances. Owner MacBook use and the updated
   background notification are confirmed above; full appearance coverage remains open.
2. Exercise a login toggle and archive on disposable items; verify macOS Automation permission
   handling, installed start-at-login and notifications. These commands are implemented, but
   real owner items were deliberately not changed for acceptance.
3. Open the login inventory before saving a pre-restart snapshot (login comparison uses the
   cached inventory). At a convenient time, perform the real restart test and inspect the report.

Public MIT source and Apple Silicon DMG releases are published:
https://github.com/krittaprot/harbor/releases. v1.5.1 includes the startup-name and scan-coverage fixes.

Known v1 limitations: non-root visibility; extension inventory is read-only command output;
system-daemon running state is Unknown rather than inferred from the user domain; a fixed-port
adoption can fail readiness while the original owns the port; the restart report is held for
the current app session, while raw snapshots persist. Current process counts can differ from
the design's historical fixture; mixed-parent unmanaged groups are Unknown rather than Orphaned.
