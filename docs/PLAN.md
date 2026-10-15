# Harbor — build plan (handoff for Codex)

Harbor is a macOS app for the Mac mini (`mini`) that shows **everything running on the machine**
in one place: launchd services, processes listening on ports that have *no* launch agent,
apps that open at login, and whether it is safe to restart. It is a fork of
[azu/launchd-ui](https://github.com/azu/launchd-ui) (Tauri 2 + React + Rust, MIT).

This file is self-contained. Read it top to bottom before writing code.

## Continuation progress (2026-10-15)

- [ ] Phase 0 visual acceptance: native title, rendered list/details, CSP and tests verified;
  owner visual inspection remains. Implementation is in place.
- [x] Phase 1 process/socket collectors, persistent scanner, snapshot IPC and native smoke check.

See `PHASE-1.md` for the incremental contract, measured results and owner decisions.

---

## 0. Ground rules (from the owner — do not break these)

1. **Never start a dev server in the background or add one to a startup script.** Run
   `pnpm tauri dev` only in the foreground while working and stop it when done.
2. **Killing processes:** plain `kill` (SIGTERM) first, verify identity (pid + command line)
   *before*, verify the process is gone *after*. Never `kill -9` without asking.
   Never kill a process not owned by the current user.
3. **`/Volumes/ProVault`** holds crystal-blood's Postgres + MLflow data. Never work around
   it being missing; Harbor only *reports* it.
4. Harbor itself is user-level. No root, no privileged helper in v1. System daemons are
   **read-only** in the UI.
5. Commit in small steps on a branch. Do not push or open PRs unless the owner asks.
6. **All code lives on the SSD:** `/Volumes/ProVault/projects/smollab-app/harbor/` (this folder).
   Never create a working copy in `~/Developer` or elsewhere. Follow the shared rules in
   `../AGENTS.md` (relative paths only in scripts and docs, keep the project movable) and keep
   Harbor's row in `../APPS.md` current. If `/Volumes/ProVault` is not mounted, stop and tell the owner.
7. Harbor's own data (config, snapshots) goes in `~/Library/Application Support/Harbor/`, never in
   the project folder.

---

## 1. Inputs you have

| What | Where |
|---|---|
| Upstream source | `https://github.com/azu/launchd-ui` (v1.4.2, Tauri 2.10, Rust 2024 edition, pnpm, shadcn/ui, vitest) |
| Project folder (code goes here) | `/Volumes/ProVault/projects/smollab-app/harbor/` |
| Design source (HTML mockups, real data) | `docs/design/*.dc.html` in this folder — open `Main.dc.html` in a text editor; all layout, colours, copy and sample data live in it. `MenuBar.dc.html` = tray popover. |
| Design canvas (owner can view; you cannot) | https://claude.ai/artifact/Mh39FuW5NcBt13Za8ojb8m |
| Toolchain on this Mac | cargo/rustc 1.95, pnpm 11.20 |

The `.dc.html` files are a custom mockup format: the markup inside `<x-dc>` is HTML with
`{{holes}}`, `<sc-for>` loops and `<sc-if>` branches; the sample data is in the
`renderVals()` function at the bottom. Treat them as a **visual + data spec**, not code to port.

---

## 2. Setup (phase 0)

The folder already holds `docs/` (this plan + design). Pull upstream into it in place — no
GitHub fork needed yet (whether to publish a fork is an open question for the owner, §10):

```bash
cd /Volumes/ProVault/projects/smollab-app/harbor
git init -b harbor/main
git remote add upstream https://github.com/azu/launchd-ui.git
git fetch upstream main
git reset upstream/main          # adopt upstream history; docs/ stays as untracked new files
git checkout -- .                # materialise upstream files in the working tree
git add docs && git commit -m "Add Harbor plan and design"
pnpm install
pnpm tauri dev                   # foreground only; Ctrl-C when done
```
Add `src-tauri/target/` and `node_modules/` to `.gitignore` if upstream doesn't already.
If the owner later wants a GitHub repo: `gh repo create krittaprot/harbor --private --source . --push`
(ask first).

Then:
- Rename: `productName` → `Harbor`, `identifier` → `com.ode.harbor`, window title `Harbor`,
  min size 1100×720 (`src-tauri/tauri.conf.json`). Keep `Cargo.toml` package name or rename to
  `harbor`; update `lib.rs` crate name references accordingly.
- Upstream has `license = "MIT"` in Cargo.toml but **no LICENSE file**. Add `LICENSE` (MIT,
  "Copyright (c) azu" + "Copyright (c) 2026 ode") and keep attribution in README.
- Set a real CSP in `tauri.conf.json` (`"csp": "default-src 'self'; style-src 'self' 'unsafe-inline'"`).
- `cargo test` and `pnpm test` must pass before you change anything else.

**Done when:** app launches as "Harbor", upstream features still work, tests green.

---

## 3. Existing code you build on

```
src-tauri/src/
  launchctl.rs   list / bootstrap / bootout / kickstart / enable / disable (shells out to launchctl, gui/<uid>)
  plist_util.rs  scan plist dirs, parse_plist → PlistConfig, write_plist, raw xml read/write
  commands.rs    #[tauri::command]s: list_jobs, get_job_detail, start/stop/restart/kickstart,
                 enable/disable, save_job, create_job, save_raw_plist, delete_job, logs, reveal_in_finder
  types.rs       JobSource, JobStatus, JobListEntry, PlistConfig, LaunchdJob
src/
  App.tsx, hooks/useJobs.ts, components/JobList|JobRow|JobDetail|JobForm|LogViewer, lib/invoke.ts
```

Keep all of it. New code goes in new modules; the old job list becomes the detail layer
under a **Service**.

---

## 4. Target architecture

```
                ┌──────────────── Rust (src-tauri) ────────────────┐
 tokio interval │ scanner::run() every 10 s  ──► Snapshot (in Arc<RwLock>) ──► emit "harbor://snapshot"
                │   ├─ launchd.rs   (existing launchctl + plist scan)
                │   ├─ procs.rs     (sysinfo: pid, ppid, uid, argv, cwd, cpu, mem, start time)
                │   ├─ ports.rs     (libproc: listening TCP sockets per pid)
                │   ├─ health.rs    (HTTP/TCP probes, cloudflared /ready)
                │   ├─ grouping.rs  (jobs+procs+ports → Service)
                │   ├─ login.rs     (login items, background items)
                │   └─ system.rs    (boot time, FileVault, ProVault, Tailscale)
                │ adopt.rs, snapshots.rs, tray.rs, config.rs
                └───────────────────────────────────────────────────┘
 React: listens to "harbor://snapshot", renders Services / Open at Login / Restart Readiness, tray popover
```

### New Cargo dependencies
```toml
tokio = { version = "1", features = ["rt-multi-thread", "time", "macros", "sync"] }
sysinfo = "0.37"          # processes, cpu, mem, parent pid, cmd, cwd
libproc = "0.14"          # listening sockets per pid (no lsof parsing)
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
chrono = { version = "0.4", features = ["serde"] }
toml = "0.9"
tauri = { version = "2.10", features = ["tray-icon", "image-png"] }
tauri-plugin-notification = "2"
tauri-plugin-positioner = { version = "2", features = ["tray-icon"] }
tauri-plugin-autostart = "2"
```
(Check the latest compatible versions with `cargo add`; adjust if the API differs.)

---

## 5. Data model (`src-tauri/src/model.rs`, mirror in `src/types.ts`)

```rust
pub struct Snapshot {
    pub taken_at: DateTime<Utc>,
    pub system: SystemInfo,
    pub services: Vec<Service>,          // managed: has ≥1 launchd job
    pub unmanaged: Vec<UnmanagedGroup>,  // listening, no launchd ancestor, not app-owned
    pub app_owned: Vec<AppOwned>,        // listening, owned by an app bundle / login item
    pub not_loaded: Vec<NotLoadedPlist>, // plists in ~/Library/LaunchAgents not loaded
    pub counts: Counts,                  // healthy, attention, unmanaged, not_loaded, ports
}

pub struct Service {
    pub id: String,              // "crystal-blood", "3things"
    pub name: String,
    pub kind: ServiceKind,       // WebApp | AiAgent | DataStack | Infra | Other
    pub icon: String, pub tint: String,   // from config, else defaults per kind
    pub jobs: Vec<JobRef>,       // label, role ("server","tunnel"…), pid, last_exit, status
    pub processes: Vec<ProcInfo>,// all pids under those jobs (descendants included)
    pub ports: Vec<PortInfo>,    // port, addr, pid, is_tunnel_metrics
    pub health: Health,          // Ok{code, ms} | PortOpen | Failed{reason} | NotChecked
    pub state: ServiceState,     // Running | Restarted{since} | Failed{exit} | Stopped
    pub uptime_secs: Option<u64>,
    pub depends_on: Vec<Dependency>,  // e.g. Volume("/Volumes/ProVault", mounted)
}

pub struct ProcInfo { pub pid: u32, pub ppid: u32, pub uid: u32, pub name: String,
    pub argv: Vec<String>, pub cwd: Option<String>, pub cpu: f32, pub mem_bytes: u64,
    pub started_at: DateTime<Utc>, pub launchd_label: Option<String> }

pub struct PortInfo { pub port: u16, pub addr: String, pub pid: u32 }

pub struct UnmanagedGroup {   // same executable+script collapsed into one row
    pub key: String, pub name: String, pub copies: Vec<ProcInfo>, pub ports: Vec<PortInfo>,
    pub verdict: Verdict,     // Orphaned (ppid==1) | Expected (user-ignored / known list) | Unknown
    pub hint: Option<String>, // e.g. "Same install as openchamber"
}

pub struct SystemInfo { pub hostname: String, pub boot_time: DateTime<Utc>,
    pub filevault_on: bool, pub provault_mounted: bool, pub tailscale_up: bool,
    pub uid: u32 }
```

---

## 6. Backend modules — what each must do

### 6.1 `procs.rs`
- `sysinfo::System::refresh_processes_specifics` each tick (cpu needs two samples → keep the
  `System` alive across ticks inside the scanner).
- For each pid: ppid, uid, name, `cmd()`, `cwd()`, cpu, memory, `start_time()`.
- Build `pid → ProcInfo` map and a `ppid` tree for descendant lookups.

### 6.2 `ports.rs`
- For each pid owned by the current uid: `libproc::proc_pid::listpidinfo::<ListFDs>` → for
  socket fds `pidfdinfo::<SocketFDInfo>` → keep TCP sockets in state LISTEN; record local
  port + address. Dedupe IPv4/IPv6.
- Processes owned by other users (root daemons) can't be read: fall back to one
  `lsof -nP -iTCP -sTCP:LISTEN -F pcn` call per tick and parse (`p`=pid, `c`=command, `n`=addr).
  Unit-test the parser with captured output.

### 6.3 `launchd.rs` (extend existing `launchctl.rs`)
- Keep `launchctl list` (pid, last exit, label) + plist scan.
- Map `pid → label` for every running job; a process is **managed** if it or any ancestor
  (walk ppid until 1) is a job pid.
- `not_loaded`: plists in `~/Library/LaunchAgents` whose label is absent from `launchctl list`.
- Hide Apple jobs (`com.apple.*`, plists under `/System`) unless the "Apple & System" filter is on.

### 6.4 `grouping.rs` (the heart of the Services view)
Rules, applied in order:
1. **Config override** `~/Library/Application Support/Harbor/services.toml`:
   ```toml
   [service.crystal-blood]
   name = "crystal-blood"; kind = "data"; icon = "drop"; tint = "red"
   labels = ["com.ode.crystalblood.*"]
   depends_on = ["/Volumes/ProVault"]
   health = "http://127.0.0.1:5050/"
   pinned = true
   ```
2. **Label pattern:** `com.<owner>.<service>.<role>` → service `<service>`, role `<role>`
   (e.g. `com.krittaprot.fasting.push` → fasting / push). `ai.hermes.*` → hermes.
3. **Homebrew:** `homebrew.mxcl.<name>` → service `<name>`, kind Infra.
4. Otherwise one service per label.
- Kind default: has a `tunnel` role → WebApp; name matches hermes|codex|deepseek|ollama|asr → AiAgent;
  depends on a volume / runs postgres → DataStack; homebrew/rustdesk/caddy → Infra.
- Seed a `services.toml` on first run from the current grouping so the owner can edit it.

### 6.5 `health.rs`
- Per service, probe at most every 30 s, 2 s timeout, concurrently (`futures::join_all`):
  - configured URL, else `GET http://127.0.0.1:<first non-tunnel port>/`.
  - Any HTTP response < 500 = healthy (401/404/302 are fine — record code + ms).
  - Non-HTTP port (postgres 5432, rustdesk 21118): TCP connect = `PortOpen`.
  - **cloudflared tunnels:** their 127.0.0.1:202xx ports are metrics servers; `GET /ready`
    returns `{"status":200,"readyConnections":4}`. Healthy when status 200 and readyConnections > 0.
- `ServiceState::Restarted` when a job's pid start time is newer than the last snapshot's and
  last exit ≠ 0 (e.g. `homespaces` last exit -15).
- `Failed` when a job is not running and last exit ≠ 0 (e.g. `com.ode.postreboot.restore-check` exit 1).

### 6.6 `login.rs`
- **Login items (apps):** no public API lists other apps' login items. Use
  `osascript -e 'tell application "System Events" to get {name, path} of every login item'`
  (first call triggers the macOS Automation permission prompt — handle the denial with a clear
  message). Toggle with `make login item at end with properties {path:…, hidden:false}` /
  `delete login item "<name>"`. Running state = a process whose bundle path matches.
- **Background items:** plists in `~/Library/LaunchAgents`, `/Library/LaunchAgents`,
  `/Library/LaunchDaemons` that are not Apple, grouped by vendor prefix
  (`com.google.*` → Google Updater, `com.carriez.*` → RustDesk, `com.docker.*` → Docker).
  Flag **Leftover** when the plist points to an app/binary that no longer exists
  (e.g. `com.docker.socket` while `/Applications/Docker.app` is missing).
- **Network/system extensions:** `systemextensionsctl list` parse (Tailscale). Read-only.
- `sfltool dumpbtm` needs root — do not use.

### 6.7 `system.rs`
- Boot time: `sysctl kern.boottime` (or `libc::sysctl`).
- FileVault: `fdesetup status` → "FileVault is On."
- ProVault: `Path::new("/Volumes/ProVault").exists()` and is a mount point (compare `st_dev` with `/Volumes`).
- Tailscale: `tailscale status --json` if the CLI exists, else "unknown".

### 6.8 `adopt.rs` — turn an unmanaged process into a LaunchAgent
1. Input: `UnmanagedGroup.key` + chosen label + options {run_at_load, keep_alive, logs, stop_copies}.
2. Build `PlistConfig` from the *first* copy: `ProgramArguments = argv` (absolute path for argv[0]:
   resolve via `/proc`-equivalent `proc_pidpath`), `WorkingDirectory = cwd`, `RunAtLoad`,
   `KeepAlive` + `ThrottleInterval 10`, log paths `~/Library/Logs/<label>/{out,err}.log`
   (create the dir). Optionally capture `PATH` from the process environment (sysinfo `environ()`).
3. Return the XML preview to the UI first (`adopt_preview`). Only on confirm (`adopt_apply`):
   write to `~/Library/LaunchAgents/<label>.plist`, `plutil -lint`, `launchctl bootstrap gui/<uid>`.
4. Wait up to 15 s for the new job to have a pid and (if it had ports) a listening port.
5. If `stop_copies`: for each old copy, re-read pid's argv and confirm it matches → `kill -TERM`
   → poll up to 5 s → report which exited. Never SIGKILL automatically.
6. Refuse labels that already exist; refuse paths outside `~/Library/LaunchAgents`.
- Also provide `stop_unmanaged(key)` (same safe kill path) and `ignore_unmanaged(key)` (persist
  to config → verdict Expected).

### 6.9 `snapshots.rs` — restart peace of mind
- `save_snapshot()` → `~/Library/Application Support/Harbor/snapshots/<ts>.json` with the list of
  running labels, login apps, unmanaged groups, ports.
- Auto-save one snapshot every hour and right before the user clicks "Restart mini…".
- On launch: if `boot_time` > newest snapshot's `taken_at`, run **compare**: came back /
  missing / new, time from unlock to all-up (unlock ≈ start time of the earliest user
  LaunchAgent process after boot). Open the app on the Restart Readiness view with that report
  and send a notification ("mini restarted · 34/34 came back").
- `readiness()` returns the checklist: FileVault unlock needed (info), ProVault mounted,
  N jobs with RunAtLoad/KeepAlive, unmanaged that won't come back, failed jobs, login apps.
- "Restart mini…" button: confirmation dialog listing warnings, then
  `osascript -e 'tell app "System Events" to restart'`. Never restart without the dialog.

### 6.10 `tray.rs` — menu-bar extra
- Tauri `TrayIconBuilder` with a template anchor icon (black PNG, `icon_as_template(true)`) plus a
  coloured status dot variant (green/orange/red PNGs, non-template) chosen from `counts`.
- Click → toggle a second window `popover` (360×600, no decorations, transparent, always on top,
  hides on blur) positioned under the icon with `tauri-plugin-positioner` (`TrayCenter`).
- Notifications (`tauri-plugin-notification`) when a service goes Running → Failed/Stopped, or a
  health check fails twice in a row. Toggle stored in config.
- Optional: start Harbor at login via `tauri-plugin-autostart` (user setting, default off —
  ask the owner).

### 6.11 Tauri commands to add (`commands.rs`)
```
get_snapshot() -> Snapshot                 rescan_now() -> Snapshot
restart_service(id), stop_service(id), start_service(id)   // loop over the service's jobs
adopt_preview(key, label, opts) -> String  adopt_apply(key, label, opts) -> AdoptResult
stop_unmanaged(key) -> StopResult          ignore_unmanaged(key)
list_login_items() -> LoginItems           set_login_item(path, enabled)
archive_plist(path)   // move to ~/Library/LaunchAgents/.harbor-archive/, never delete
readiness() -> Readiness                   save_snapshot() -> SnapshotMeta
last_restart_report() -> Option<RestartReport>
restart_machine()     // only after UI confirm
open_url(url)  open_log(label)
```
Event: `harbor://snapshot` emitted after every scan. Every command that changes state must
validate inputs (labels `[A-Za-z0-9._-]+`, paths under the allowed dirs).

---

## 7. Frontend (React) — map the design

Replace the single job table with a sidebar layout. Files in `src/`:

```
views/Services.tsx        summary cards, search, segmented filter, unmanaged card,
                          grouped service cards, "Started by apps", expandable process tree
views/OpenAtLogin.tsx     login apps (switches), background items, agents that won't start
views/RestartReadiness.tsx hero + stacked bar, checklist, last-restart timeline
components/AdoptSheet.tsx right-side floating sheet with 3 sections + live plist preview
components/Sidebar.tsx    app icon, Manage (Services/Open at Login/Restart), Smart filters, status card
components/ServiceTile.tsx coloured gradient tile + SVG icon
popover/Popover.tsx       tray popover (separate Vite entry popover.html)
hooks/useSnapshot.ts      listen("harbor://snapshot") + initial get_snapshot()
theme.css                 CSS variables, light + dark (prefers-color-scheme)
```

**Visual tokens** (copy exactly; full set in `docs/design/Main.dc.html` `<helmet><style>`):
- Font: `-apple-system, "SF Pro Text"`; numbers `ui-rounded`; mono `"SF Mono"`.
- Light: bg `#F5F5F7`, card `#FFF` radius 16, separator `#F0F0F3`, text `#1D1D1F`, muted `#6E6E73`,
  primary button `#0071E3`, switch on `#2FB24F`.
- Dark: bg `#161618`, card `#232326`, separator `rgba(255,255,255,.07)`, muted `#9A9AA1`,
  primary `#0A84FF`, switch on `#30D158`.
- Pills: ok `#E2F6E7/#17692F`, warn `#FFEFD6/#8F4B00`, bad `#FFE5E3/#B0201A` (dark variants in file).
- Service tiles: 180° gradients — blue `#4BA3FF→#0A6CFF`, orange `#FFB340→#FF8A00`,
  indigo `#7A78FF→#4F4CE0`, green `#5EDB7D→#28B04A`, teal `#5AD2E6→#1AA5C2`,
  yellow `#FFD84D→#F0B400`, pink `#FF6F91→#F0306A`, purple `#CF7DF2→#A445D8`,
  graphite `#9A9AA0→#6E6E75`, red `#FF6F61→#E8382B`, cyan `#64C8FF→#1E9BF0`.
  Icon SVG paths are in the `I` object inside `Main.dc.html` `renderVals()`.
- **App icon:** white rounded tile (`#FFFFFF→#ECEDF1`) with a dark navy (`#14213A`) anchor
  (circle at top, vertical shaft, crossbar, bottom arc). Generate the `.icns` with the
  `codex-imagegen` skill or draw it as SVG → `pnpm tauri icon icon.png`.
- Use real `<button>`s with `aria-label` on icon buttons; text contrast ≥ 4.5:1.

---

## 8. Phases and acceptance checks

| # | Phase | Done when |
|---|---|---|
| 0 | Fork, rename, LICENSE, CSP | App opens as "Harbor"; `cargo test` + `pnpm test` pass |
| 1 | `procs.rs` + `ports.rs` + scanner loop + `get_snapshot` | Unit tests for lsof parser; `get_snapshot` lists ≈41 listening ports on mini incl. 3216, 5432, 9230 |
| 2 | `grouping.rs` + `services.toml` | crystal-blood shows 4 jobs + postgres/uvicorn/next children; fasting shows server/push/sync/tunnel |
| 3 | Services view UI (light) | Matches `docs/design/Main.dc.html` default view; search + filters work |
| 4 | `health.rs` | 3things shows `200 · ~7 ms`; tunnels healthy via `/ready`; restore-check shows Failed exit 1 |
| 5 | Unmanaged + Adopt | figma-console-mcp ×4 shows as one Orphaned row; Adopt preview XML matches design; apply works on a throwaway test process (e.g. `python3 -m http.server 8999` started by you in the foreground) — **not** on the owner's real processes without asking |
| 6 | Open at Login | Lists OrbStack, Lane, Ollama, Claude, FigmaAgent; toggling works; Docker plist flagged Leftover; 7 not-loaded plists listed; Archive moves, never deletes |
| 7 | Restart readiness + snapshots | Checklist matches design; snapshot JSON written; compare report shown after a reboot (owner tests the reboot) |
| 8 | Tray + popover + notifications | Anchor icon with status dot; popover matches `docs/design/MenuBar.dc.html` |
| 9 | Dark mode + app icon + polish | Follows system appearance; matches `ServicesDark`/`RestartDark` |
| 10 | Build | `pnpm tauri build` → `Harbor.app`; owner installs it by hand |

Write Rust unit tests for: lsof parser, label → service grouping, health classification,
adopt plist generation (snapshot the XML), snapshot compare. Use captured fixtures, not live calls.

---

## 9. Facts about this machine (for test fixtures and sanity checks)

- uid 501, user `ode`, host `mini`, FileVault on, no auto-login, last boot Mon Oct 12 10:18.
- ~34 user launchd jobs. Pattern `com.krittaprot.<app>.{server,tunnel,push,sync}`, `com.ode.*`,
  `ai.hermes.{gateway,hermes-server}`, `homebrew.mxcl.caddy`, `com.carriez.RustDesk_*`.
- Web apps: 3things :3216, fasting :3219/:3223/:3228, homespaces :8787, nest :3217, signal :3213,
  yolk :3218, kindaimai :3225, examer :3230, openchamber :3235 (+opencode :4747),
  hermes dashboard :9119, hermes-webui :8788, deepseek-harness :3080 (401 = OK), codex-asr :8790 (404 = OK).
- crystal-blood: postgres :5432 (data on ProVault), uvicorn :5050, python :2024, next :3000, cloudflared :20242.
- Tunnels: cloudflared metrics on 127.0.0.1:20242–20270, `/ready` returns JSON.
- Unmanaged at time of design: figma-console-mcp ×4 (:9225 :9230–9232, ppid 1, cwd ~/Developer/research),
  opencode serve (:49374 :61585, ppid 1), VS Code Server (:64949).
- App-owned: OrbStack (:80 :8080 :2283 immich :32222), Ollama :11434, Figma Agent :44950/:44960,
  Codex app children :9223/:9224/:9227, Chrome remote debugging :9222.
- Not-loaded plists: com.crystalblood.{stack,postgres,cloudflared,logrotate} (old names),
  ai.hermes.serve, com.krittaprot.postreboot-check, com.ode.stock-dip-radar-preview.
- `com.ode.postreboot.restore-check` last exit 1 — a real issue, report it, don't fix it unasked.

---

## 10. Open questions for the owner (ask, don't guess)

1. Fork publicly on GitHub (`gh repo fork`) or keep a private copy?
2. Should Harbor start itself at login?
3. Should notifications be on by default?
4. Which services are "pinned" in the tray popover (design shows crystal-blood, hermes, 3things, fasting)?
