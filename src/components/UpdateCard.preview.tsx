// Development-only state gallery. Not an app route or production build entry.
import { useState } from "react"
import { createRoot } from "react-dom/client"
import { UpdateCard } from "./UpdateCard"
import type { useUpdates } from "@/hooks/useUpdates"
import "@/theme.css"

type Model = ReturnType<typeof useUpdates>
const noop = async () => {}
const sample: Model = {
  status: { current_version: "1.5.2", can_install: true },
  available: null, phase: "idle", progress: null, error: null, checkedAt: null,
  check: noop, install: noop, restart: noop,
}

function Preview() {
  const [phase, setPhase] = useState<Model["phase"]>("idle")
  const [outcome, setOutcome] = useState("current")
  const [dark, setDark] = useState(false)
  const [checkedAt, setCheckedAt] = useState<string | null>(null)
  const offer = { version: "1.5.3", notes: "Example release notes: menu-bar-only launch and friendlier update feedback." }
  const error = phase === "idle" && checkedAt === "error" ? "Example: GitHub could not be reached." : null
  const check = async () => {
    setCheckedAt(null)
    setPhase("checking")
    await new Promise(resolve => setTimeout(resolve, 1400)) // Demo-only latency.
    setPhase(outcome === "error" ? "idle" : outcome as Model["phase"])
    setCheckedAt(outcome === "error" ? "error" : new Date().toISOString())
  }
  const states: [string, Partial<Model>][] = [
    ["Default", {}], ["Hover", {}], ["Focus", {}], ["Active", {}],
    ["Disabled", { phase: "installing", available: offer }],
    ["Loading", { phase: "checking" }],
    ["Error", { error: "Example: GitHub could not be reached." }],
    ["Success", { phase: "current", checkedAt: new Date().toISOString() }],
    ["Update available", { phase: "available", available: offer, checkedAt: new Date().toISOString() }],
  ]
  return <div className={dark ? "dark" : ""}>
    <style>{`
      .update-preview { padding: 24px 16px; min-height: 100vh; background: var(--h-bg); color: var(--h-text); font: 13px/1.4 var(--font-harbor); }
      .update-preview main { max-width: 900px; margin: auto; }
      .update-preview header { display: flex; flex-wrap: wrap; align-items: center; gap: 12px; margin-bottom: 20px; }
      .update-preview select { background: var(--h-card); color: var(--h-text); padding: 8px; }
      .update-preview .h-card { margin-bottom: 20px; }
      .preview-hover .h-update-check { background: var(--h-hover); }
      .preview-focus .h-update-check { outline: 2px solid var(--h-blue); outline-offset: 3px; }
      .preview-active .h-update-check { transform: translateY(1px); }
      .update-preview h1 { font-size: 20px; margin: 0; }
    `}</style>
    <div className="update-preview"><main>
      <header><h1>Update feedback</h1><span>Development fixtures · no downloads or installs</span>
        <button className="h-button" onClick={() => setDark(v => !v)}>{dark ? "Light appearance" : "Dark appearance"}</button>
        <label>Demo result <select value={outcome} onChange={event => setOutcome(event.target.value)}>
          <option value="current">Up to date</option><option value="available">Update available</option><option value="error">Check failed</option>
        </select></label>
      </header>
      <div data-preview="interactive"><UpdateCard updater={{ ...sample, phase, available: phase === "available" ? offer : null, error, checkedAt: error ? null : checkedAt, check }} /></div>
      {states.map(([label, overrides]) => <section key={label} data-preview={label.toLowerCase().replaceAll(" ", "-")} className={`preview-${label.toLowerCase()}`}>
        <h2>{label}</h2><UpdateCard updater={{ ...sample, ...overrides }} />
      </section>)}
    </main></div>
  </div>
}

createRoot(document.getElementById("root")!).render(<Preview />)
