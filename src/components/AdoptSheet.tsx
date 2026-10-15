import { useEffect, useState } from "react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
import { adoptPreview, adoptApply, type AdoptOptions } from "@/lib/harbor"
import type { UnmanagedGroup } from "@/types"
import { ServiceTile } from "./ServiceTile"
export function AdoptSheet({
  group,
  close,
  done,
}: {
  group: UnmanagedGroup | null
  close: () => void
  done: () => void
}) {
  const [label, setLabel] = useState("")
  const [options, setOptions] = useState<AdoptOptions>({
    run_at_load: true,
    keep_alive: true,
    logs: true,
    stop_copies: false,
  })
  const [xml, setXml] = useState("")
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<string[] | null>(null)
  useEffect(() => {
    if (group) {
      setLabel(
        `com.harbor.${group.name.toLowerCase().replace(/[^a-z0-9._-]/g, "-")}`
      )
      setResult(null)
      setOptions({
        run_at_load: true,
        keep_alive: true,
        logs: true,
        stop_copies: false,
      })
    }
  }, [group])
  useEffect(() => {
    if (!group || !label) return
    let cancelled = false
    setXml("")
    const timer = setTimeout(() => {
      void adoptPreview(group.key, label, options)
        .then((value) => {
          if (!cancelled) {
            setXml(value)
            setError(null)
          }
        })
        .catch((e) => {
          if (!cancelled) setError(String(e))
        })
    }, 300)
    return () => {
      cancelled = true
      clearTimeout(timer)
    }
  }, [group, label, options])
  async function apply() {
    if (!group) return
    setBusy(true)
    try {
      setResult(await adoptApply(group.key, label, options, xml))
      done()
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }
  return (
    <Dialog
      open={!!group}
      onOpenChange={(open) => {
        if (!open && !busy) close()
      }}
    >
      <DialogContent
        className="h-adopt-sheet"
        aria-describedby="adopt-description"
      >
        <DialogHeader>
          <div className="h-adopt-heading">
            <ServiceTile icon="terminal" tint="pink" size="large" />
            <div>
              <p className="h-sub">Adopt as launch agent</p>
              <DialogTitle>{group?.name}</DialogTitle>
            </div>
          </div>
          <DialogDescription id="adopt-description">
            Review the command and generated plist before creating a user
            LaunchAgent.
          </DialogDescription>
        </DialogHeader>
        <div className="h-sheet-scroll">
          <h3>What’s running now</h3>
          <div className="h-card h-adopt-current">
            <div className="h-wrap">
              <span className="h-pill h-warn">
                {group?.verdict} · {group?.copies.length} copies
              </span>
              {[...new Set(group?.ports.map((p) => p.port))].map((p) => (
                <span className="h-port" key={p}>
                  :{p}
                </span>
              ))}
            </div>
            <dl>
              <dt>Command</dt>
              <dd className="h-mono">
                {group?.copies[0]?.argv.join(" ") || "Unavailable"}
              </dd>
              <dt>Folder</dt>
              <dd className="h-mono">
                {group?.copies[0]?.cwd ?? "Unavailable"}
              </dd>
              <dt>Parent</dt>
              <dd>
                {group?.copies[0]?.ppid === 1
                  ? "launchd — original session exited"
                  : `PID ${group?.copies[0]?.ppid}`}
              </dd>
            </dl>
          </div>
          <h3>Set it up</h3>
          <div className="h-card">
            <label className="h-list-row">
              <span>Label</span>
              <input
                className="h-input h-mono"
                value={label}
                disabled={busy || !!result}
                onChange={(e) => setLabel(e.target.value)}
              />
            </label>
            {(
              [
                ["run_at_load", "Start when I log in", "RunAtLoad"],
                [
                  "keep_alive",
                  "Keep it running",
                  "Restart if the process exits",
                ],
                ["logs", "Save output to logs", "~/Library/Logs/<label>/"],
                [
                  "stop_copies",
                  "Stop original copies after readiness",
                  "SIGTERM only, after the new agent owns a listening port",
                ],
              ] as const
            ).map(([key, title, detail]) => (
              <div className="h-list-row" key={key}>
                <div className="h-grow">
                  <strong>{title}</strong>
                  <p className="h-sub">{detail}</p>
                </div>
                <button
                  role="switch"
                  aria-checked={options[key]}
                  aria-label={title}
                  className={`h-switch ${options[key] ? "is-on" : ""}`}
                  disabled={busy || !!result}
                  onClick={() => setOptions((o) => ({ ...o, [key]: !o[key] }))}
                >
                  <span />
                </button>
              </div>
            ))}
          </div>
          <h3>Preview</h3>
          <p className="h-sub h-mono">~/Library/LaunchAgents/{label}.plist</p>
          <pre className="h-code">{xml || "Preparing preview…"}</pre>
          {error && (
            <p role="alert" className="h-error">
              {error}
            </p>
          )}
          {result && (
            <div className="h-notice">
              {result.map((line, i) => (
                <p key={i}>{line}</p>
              ))}
            </div>
          )}
        </div>
        <footer className="h-sheet-footer">
          <p className="h-sub">
            The new agent must become ready before originals are stopped. A
            fixed port already in use can prevent readiness.
          </p>
          <button className="h-button" disabled={busy} onClick={close}>
            {result ? "Done" : "Cancel"}
          </button>
          {!result && (
            <button
              className="h-button h-primary"
              disabled={busy || !xml || !!error}
              onClick={apply}
            >
              {busy ? "Loading & checking…" : "Adopt & Load"}
            </button>
          )}
        </footer>
      </DialogContent>
    </Dialog>
  )
}
