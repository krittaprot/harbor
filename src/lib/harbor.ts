import { Channel, invoke } from "@tauri-apps/api/core"
import type {
  Service,
  Snapshot,
  LoginItems,
  RestartReport,
  HarborSettings,
} from "@/types"

export const rescanNow = () => invoke<Snapshot>("rescan_now")
export const serviceAction = (
  id: string,
  action: "start" | "stop" | "restart"
) => invoke<void>("service_action", { id, action })
export const pinService = (id: string, pinned: boolean) =>
  invoke<void>("pin_service", { id, pinned })
export const ignoreUnmanaged = (key: string, ignored: boolean) =>
  invoke<void>("ignore_unmanaged", { key, ignored })
export const stopUnmanaged = (key: string) =>
  invoke<string[]>("stop_unmanaged", { key })
export const archivePlist = (path: string) =>
  invoke<void>("archive_plist", { path })
export const listLoginItems = () => invoke<LoginItems>("list_login_items")
export const setLoginItem = (path: string, enabled: boolean) =>
  invoke<void>("set_login_item", { path, enabled })
export const saveSnapshot = () => invoke<string>("save_snapshot")
export const lastRestartReport = () =>
  invoke<RestartReport | null>("last_restart_report")
export const restartMachine = () =>
  invoke<void>("restart_machine", { confirmed: true })
export const getSettings = () => invoke<HarborSettings>("get_settings")
export const setSettings = (notifications: boolean, startAtLogin: boolean) =>
  invoke<void>("set_settings", { notifications, startAtLogin })
export const openUrl = (url: string) => invoke<void>("open_url", { url })
export const showMain = (view?: string) => invoke<void>("show_main", { view })
export const quitHarbor = () => invoke<void>("quit_harbor")
export const diagnoseTarget = (target: string) => invoke<unknown>("diagnose_target", { target })
export const copyText = (text: string) => invoke<void>("copy_text", { text })
export type AdoptOptions = {
  run_at_load: boolean
  keep_alive: boolean
  logs: boolean
  stop_copies: boolean
}
export const adoptPreview = (
  key: string,
  label: string,
  options: AdoptOptions
) => invoke<string>("adopt_preview", { key, label, options })
export type AdoptionProgress = {
  stage: "verify" | "write" | "load" | "ready" | "stop" | "rollback" | "complete"
  message: string
  warnings: boolean
}
export const adoptApply = (
  key: string,
  label: string,
  options: AdoptOptions,
  expectedXml: string,
  progress?: (event: AdoptionProgress) => void
) => {
  const onProgress = new Channel<AdoptionProgress>()
  onProgress.onmessage = (event) => progress?.(event)
  return invoke<string[]>("adopt_apply", { key, label, options, expectedXml, onProgress })
}

export function uptime(seconds: number | null) {
  if (seconds === null) return "—"
  if (seconds < 60) return `${Math.floor(seconds)}s`
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m`
  if (seconds < 86400)
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`
  return `${Math.floor(seconds / 86400)}d ${Math.floor((seconds % 86400) / 3600)}h`
}
export function serviceUrl(s: Service) {
  return (
    s.url ??
    (s.ports.find(
      (p) =>
        !s.processes.some(
          (proc) => proc.pid === p.pid && proc.name.includes("cloudflared")
        ) && ![5432, 21118].includes(p.port)
    )
      ? `http://127.0.0.1:${s.ports.find((p) => !s.processes.some((proc) => proc.pid === p.pid && proc.name.includes("cloudflared")) && ![5432, 21118].includes(p.port))!.port}/`
      : null)
  )
}
export function matchesService(s: Service, search: string) {
  const q = search.toLowerCase().trim()
  return (
    !q ||
    [
      s.name,
      s.id,
      ...s.jobs.map((j) => j.label),
      ...s.ports.map((p) => String(p.port)),
      ...s.processes.flatMap((p) => [...p.argv, p.cwd ?? ""]),
    ].some((v) => v.toLowerCase().includes(q))
  )
}
