export type JobSource = "UserAgent" | "SystemAgent" | "SystemDaemon"

/** Phase 1 raw inventory; service grouping and health arrive in later phases. */
export type Snapshot = {
  taken_at: string
  uid: number
  processes: ProcInfo[]
  ports: PortInfo[]
  listening_port_count: number
  diagnostics: {
    duration_ms: number
    libproc_listener_count: number
    lsof_listener_count: number
    inaccessible_pids: number[]
    visibility_limited: boolean
    warnings: string[]
  }
}

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
