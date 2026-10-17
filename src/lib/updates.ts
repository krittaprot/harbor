import { Channel, invoke } from "@tauri-apps/api/core"

export type UpdateStatus = { current_version: string; can_install: boolean }
export type AvailableUpdate = { version: string; notes: string }
export type UpdateProgress = {
  stage: "downloading" | "verifying" | "installing"
  downloaded: number
  total: number | null
}

export const updateStatus = () => invoke<UpdateStatus>("update_status")
export const checkUpdate = () => invoke<AvailableUpdate | null>("check_update")
export const restartAfterUpdate = () => invoke<void>("restart_after_update")
export const installUpdate = (version: string, progress: (event: UpdateProgress) => void) => {
  const onProgress = new Channel<UpdateProgress>()
  onProgress.onmessage = progress
  return invoke<void>("install_update", { version, onProgress })
}
