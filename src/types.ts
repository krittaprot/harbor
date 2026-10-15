export type JobSource = "UserAgent" | "SystemAgent" | "SystemDaemon"

/** Phase 1 raw inventory; service grouping and health arrive in later phases. */
export type Snapshot = {
  taken_at: string
  uid: number
  processes: ProcInfo[]
  ports: PortInfo[]
  listening_port_count: number
  services: Service[]
  unmanaged: UnmanagedGroup[]
  app_owned: UnmanagedGroup[]
  not_loaded: JobRef[]
  system: { hostname: string; boot_time: string | null; filevault_on: boolean | null; provault_mounted: boolean; tailscale_up: boolean | null; uid: number }
  counts: { healthy: number; attention: number; running: number; unmanaged: number; not_loaded: number; ports: number; hidden: number }
  diagnostics: {
    duration_ms: number
    libproc_listener_count: number
    lsof_listener_count: number
    inaccessible_pids: number[]
    visibility_limited: boolean
    warnings: string[]
    inventory_issues?: { code: string; path: string; message: string; guidance: string; detail: string }[]
  }
}

export type JobRef = {
  label: string; role: string; pid: number | null; last_exit: number | null; status: string
  plist_path: string | null; editable: boolean; apple: boolean; autostart: boolean; scheduled: boolean; missing_executable: boolean
}
export type Health = { status: string; code: number | null; ms: number | null; reason: string | null; checked_at: string | null; tunnels: { port: number; ready: boolean; reason: string }[] }
export type Service = {
  id: string; name: string; kind: string; icon: string; tint: string; jobs: JobRef[]; processes: ProcInfo[]; ports: PortInfo[]
  health: Health; state: string; uptime_secs: number | null; depends_on: { path: string; mounted: boolean }[]; pinned: boolean; url: string | null; apple: boolean
}
export type UnmanagedGroup = { key: string; name: string; copies: ProcInfo[]; ports: PortInfo[]; verdict: string; app_path: string | null; hint: string | null }
export type LoginItems = { apps: { name: string; path: string; enabled: boolean; running: boolean }[]; extensions: string }
export type RestartReport = {
  before: string; boot_time: string; observed_at: string; came_back: string[]; missing: string[]; new: string[]
  login_returned: number | null; login_missing: string[] | null; unmanaged_missing: string[]; ports_missing: number[]; seconds_after_first_agent: number | null
}
export type HarborSettings = { notifications: boolean; start_at_login: boolean }

export type ProcInfo = {
  pid: number
  ppid: number
  uid: number | null
  name: string
  argv: string[]
  cwd: string | null
  cpu: number | null
  mem_bytes: number
  started_at: string | null
  launchd_label: string | null
}

export type PortInfo = { pid: number; port: number; addr: string }
export type JobStatus = "Running" | "Loaded" | "Unloaded" | "Unknown"

// Filter values for the source toolbar. "Home" is a virtual filter (a subset of
// UserAgent) matching user-authored automations, driven by JobListEntry.is_home_agent.
export type SourceFilter = JobSource | "All" | "Home"

export type JobListEntry = {
  label: string
  pid: number | null
  last_exit_code: number | null
  plist_path: string
  source: JobSource
  status: JobStatus
  last_run_at: string | null
  is_home_agent: boolean
}

export type CalendarInterval = {
  minute: number | null
  hour: number | null
  day: number | null
  weekday: number | null
  month: number | null
}

export type PlistConfig = {
  label: string
  program: string | null
  program_arguments: string[] | null
  run_at_load: boolean | null
  keep_alive: boolean | null
  start_interval: number | null
  start_calendar_interval: CalendarInterval[] | null
  standard_out_path: string | null
  standard_error_path: string | null
  working_directory: string | null
  environment_variables: Record<string, string> | null
  disabled: boolean | null
  wake_system: boolean | null
  raw_xml: string
}

export type LaunchdJob = {
  label: string
  plist_path: string
  source: JobSource
  status: JobStatus
  pid: number | null
  last_exit_code: number | null
  plist: PlistConfig
  last_run_at: string | null
}
