import { useEffect, useRef, useState } from "react"
import * as api from "@/lib/updates"

type Phase = "idle" | "checking" | "current" | "available" | "downloading" | "verifying" | "installing" | "installed" | "restarting"

// Owned by App, so changing views never discards an active download or install.
export function useUpdates() {
  const [status, setStatus] = useState<api.UpdateStatus | null>(null)
  const [available, setAvailable] = useState<api.AvailableUpdate | null>(null)
  const [phase, setPhase] = useState<Phase>("idle")
  const [progress, setProgress] = useState<api.UpdateProgress | null>(null)
  const [error, setError] = useState<string | null>(null)
  const running = useRef(false)
  useEffect(() => {
    let active = true
    void api.updateStatus().then(value => { if (active) setStatus(value) })
      .catch(e => { if (active) setError(String(e)) })
    return () => { active = false }
  }, [])

  async function check() {
    if (running.current || phase === "installed" || phase === "restarting") return
    running.current = true
    setError(null)
    setAvailable(null)
    setPhase("checking")
    try {
      setStatus(await api.updateStatus())
      const next = await api.checkUpdate()
      setAvailable(next)
      setPhase(next ? "available" : "current")
    } catch (e) {
      setError(String(e))
      setPhase("idle")
    } finally {
      running.current = false
    }
  }

  async function install() {
    if (running.current || phase !== "available" || !available || !status?.can_install) return
    running.current = true
    setError(null)
    setProgress(null)
    setPhase("downloading")
    try {
      await api.installUpdate(available.version, event => {
        setProgress(event)
        setPhase(event.stage)
      })
      setPhase("installed")
    } catch (e) {
      setError(String(e))
      setPhase("available")
    } finally {
      running.current = false
    }
  }

  async function restart() {
    if (running.current || phase !== "installed") return
    running.current = true
    setError(null)
    setPhase("restarting")
    try {
      await api.restartAfterUpdate()
    } catch (e) {
      setError(String(e))
      setPhase("installed")
    } finally {
      running.current = false
    }
  }

  return { status, available, phase, progress, error, check, install, restart }
}
