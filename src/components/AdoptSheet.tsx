import { useEffect, useRef, useState } from "react"
import { Check, Copy, LoaderCircle, TriangleAlert } from "lucide-react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
import { adoptPreview, adoptApply, copyText, diagnoseTarget, type AdoptOptions, type AdoptionProgress } from "@/lib/harbor"
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
  const [progress, setProgress] = useState<AdoptionProgress | null>(null)
  const [elapsed, setElapsed] = useState(0)
  const started = useRef(0)
  const applying = useRef(false)
  const scroll = useRef<HTMLDivElement>(null)
  const [applyFailed, setApplyFailed] = useState(false)
  const [copyStatus, setCopyStatus] = useState("")
  async function copyReport() {
    setCopyStatus("Copying…")
    try {
      let diagnostics: unknown
      try { diagnostics = await diagnoseTarget(label) }
      catch { diagnostics = group ? await diagnoseTarget(group.key).catch(String) : null }
      await copyText([
        "Harbor adoption report", `Time: ${new Date().toISOString()}`,
        `Group: ${group?.name}`, `Key: ${group?.key}`, `Label: ${label}`,
        `Stage: ${progress?.stage ?? "preview"}`, `Elapsed: ${elapsed}s`,
        `Options: ${JSON.stringify(options)}`, "", error ?? result?.join("\n") ?? "Preview",
        "", "Diagnostics:", JSON.stringify(diagnostics, null, 2), "", "Reviewed plist:", xml,
      ].join("\n"))
      setCopyStatus("Copied")
    } catch (e) { setCopyStatus(`Copy failed: ${String(e)}`) }
  }
  useEffect(() => {
    if (busy || error || result) scroll.current?.scrollTo({ top: 0 })
  }, [busy, error, result])
  useEffect(() => {
    if (!busy) return
    const timer = setInterval(() => setElapsed(Math.floor((Date.now() - started.current) / 1000)), 1000)
    return () => clearInterval(timer)
  }, [busy])
  useEffect(() => {
    if (group) {
      setLabel(
        `com.harbor.${group.name.toLowerCase().replace(/[^a-z0-9._-]/g, "-")}`
      )
      setResult(null)
      setProgress(null)
      setApplyFailed(false)
      setError(null)
      setCopyStatus("")
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
    setCopyStatus("")
    setProgress(null)
    setApplyFailed(false)
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
    if (!group || applying.current) return
    applying.current = true
    started.current = Date.now()
    setElapsed(0)
    setError(null)
    setApplyFailed(false)
    setProgress({ stage: "verify", message: "Checking process identity and the reviewed configuration…", warnings: false })
    setBusy(true)
    try {
      setResult(await adoptApply(group.key, label, options, xml, setProgress))
      done()
    } catch (e) {
      setError(String(e))
      setApplyFailed(true)
      done()
    } finally {
      applying.current = false
      setBusy(false)
    }
  }
  const steps = [
    ["verify", "Verify"], ["write", "Save plist"], ["load", "Load"],
    ["ready", "Readiness"], ...(options.stop_copies ? [["stop", "Stop originals"]] : []),
  ]
  const completed = result ? steps.length : Math.max(0, steps.findIndex(([stage]) => stage === (progress?.stage === "rollback" ? "ready" : progress?.stage)))
  const failure = error?.split("\n")
  return (
    <Dialog
      open={!!group}
      onOpenChange={(open) => {
        if (!open && !busy) close()
      }}
    >
      <DialogContent
        className="h-adopt-sheet"
        showCloseButton={!busy}
      >
        <DialogHeader>
          <div className="h-adopt-heading">
            <ServiceTile icon="terminal" tint="pink" size="large" />
            <div>
              <p className="h-sub">Adopt as launch agent</p>
              <DialogTitle>{group?.name}</DialogTitle>
            </div>
          </div>
          <DialogDescription>
            Review the command and generated plist before creating a user
            LaunchAgent.
          </DialogDescription>
        </DialogHeader>
        {progress && (
          <section className={`h-adopt-progress ${applyFailed ? "is-error" : result && progress.warnings ? "is-warning" : ""}`} aria-label="Adoption progress" aria-busy={busy}>
            <div className="h-adopt-progress-title" role="status" aria-live="polite">
              {busy ? <LoaderCircle size={17} className="h-spin" /> : applyFailed || progress.warnings ? <TriangleAlert size={17} /> : <Check size={17} />}
              <strong>{applyFailed ? "Adoption did not finish" : result ? (progress.warnings ? "Adopted · needs review" : "Launch agent ready") : progress.stage === "rollback" ? "Unloading new agent…" : "Adopting…"}</strong>
              <span className="h-sub">{elapsed}s elapsed</span>
            </div>
            <div className="h-adopt-track" role="progressbar" aria-label="Adoption stages completed" aria-valuemin={0} aria-valuemax={steps.length} aria-valuenow={completed} aria-valuetext={`${completed} of ${steps.length} stages completed${applyFailed ? "; adoption failed" : ""}`}>
              <span style={{ width: `${completed / steps.length * 100}%` }} />
            </div>
            <ol className="h-adopt-steps">
              {steps.map(([stage, title], index) => <li key={stage} className={index < completed ? "is-done" : index === completed ? "is-current" : ""} aria-current={busy && index === completed ? "step" : undefined}>{index < completed ? "✓ " : `${index + 1}. `}{title}</li>)}
            </ol>
            <p className="h-sub">{applyFailed ? "Review the failure below before trying again." : progress.message}</p>
          </section>
        )}
        <div className="h-sheet-scroll" ref={scroll}>
          {error && (
            <div role="alert" className="h-error h-adopt-error">
              <button className="h-button h-small" onClick={() => void copyReport()} disabled={copyStatus === "Copying…"}><Copy size={13} /> Copy report</button>
              {copyStatus && <p role="status">{copyStatus}</p>}
              <strong>{failure?.[0]}</strong>
              {failure && failure.length > 1 && <>
                <p>{failure[1]}</p>
                {failure.some((line) => line.startsWith("Plist retained")) && <p>The plist was kept for review. Close this dialog and use Launch job inspector to check the command and logs before trying again.</p>}
                <details><summary>Technical details & next steps</summary><pre>{failure.slice(2).join("\n")}</pre></details>
              </>}
            </div>
          )}
          {result && (
            <div className="h-notice">
              <button className="h-button h-small" onClick={() => void copyReport()} disabled={copyStatus === "Copying…"}><Copy size={13} /> Copy report</button>
              {copyStatus && <p role="status">{copyStatus}</p>}
              {result.map((line, i) => <p key={i}>{line}</p>)}
            </div>
          )}
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
          <button className="h-button h-small" disabled={!xml} onClick={() => void copyText(xml).then(() => setCopyStatus("Plist copied")).catch((e) => setCopyStatus(`Copy failed: ${String(e)}`))}><Copy size={13} /> Copy plist</button>
          {!error && !result && copyStatus && <p role="status" className="h-sub">{copyStatus}</p>}
          <p className="h-sub h-mono">~/Library/LaunchAgents/{label}.plist</p>
          <pre className="h-code">{xml || "Preparing preview…"}</pre>
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
              disabled={busy || !xml || (!!error && !applyFailed)}
              onClick={apply}
            >
              {busy ? <><LoaderCircle size={14} className="h-spin" /> Adopting…</> : applyFailed ? "Retry adoption" : "Adopt & Load"}
            </button>
          )}
        </footer>
      </DialogContent>
    </Dialog>
  )
}
