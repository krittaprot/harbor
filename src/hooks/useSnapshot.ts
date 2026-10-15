import { useCallback, useEffect, useRef, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { getSnapshot } from "@/lib/invoke"
import { rescanNow } from "@/lib/harbor"
import type { Snapshot } from "@/types"

export function useSnapshot() {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const alive = useRef(true)
  const accept = useCallback((next: Snapshot) => {
    if (!alive.current) return
    setSnapshot((old) => (!old || next.taken_at >= old.taken_at ? next : old))
    setError(null)
  }, [])
  useEffect(() => {
    alive.current = true
    let cancelled = false
    let unlisten: (() => void) | undefined
    let timer: ReturnType<typeof setTimeout> | undefined
    async function initial() {
      try {
        accept(await getSnapshot())
      } catch (e) {
        if (cancelled) return
        if (String(e).includes("initial scan in progress"))
          timer = setTimeout(initial, 500)
        else setError(String(e))
      }
    }
    void listen<Snapshot>("harbor://snapshot", (e) => {
      if (!cancelled) accept(e.payload)
    })
      .then((stop) => {
        if (cancelled) {
          stop()
          return
        }
        unlisten = stop
        void initial()
      })
      .catch((e) => {
        if (!cancelled) setError(String(e))
      })
    return () => {
      cancelled = true
      alive.current = false
      clearTimeout(timer)
      unlisten?.()
    }
  }, [accept])
  const refresh = useCallback(async () => {
    setBusy(true)
    try {
      accept(await rescanNow())
    } catch (e) {
      if (alive.current) setError(String(e))
    } finally {
      if (alive.current) setBusy(false)
    }
  }, [accept])
  return { snapshot, error, busy, refresh }
}
