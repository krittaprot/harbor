import { useEffect, useState } from "react"
import { Plus, RefreshCw, LockKeyhole } from "lucide-react"
import { listLoginItems, setLoginItem } from "@/lib/harbor"
import { ServiceTile } from "@/components/ServiceTile"
import type { Snapshot, LoginItems, JobRef } from "@/types"
export function OpenAtLogin({
  snapshot,
  onJob,
  archive,
  run,
}: {
  snapshot: Snapshot
  onJob: (path: string) => void
  archive: (job: JobRef) => void
  run: (f: () => Promise<unknown>) => Promise<void>
}) {
  const [items, setItems] = useState<LoginItems | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  const [path, setPath] = useState("")
  const [add, setAdd] = useState(false)
  async function load() {
    setBusy(true)
    try {
      setItems(await listLoginItems())
      setError(null)
    } catch (e) {
      setError(String(e))
    } finally {
      setBusy(false)
    }
  }
  useEffect(() => {
    void load()
  }, [])
  const jobs = snapshot.services
    .flatMap((s) => s.jobs)
    .filter((j) => j.plist_path && !j.label.startsWith("com.apple."))
  const vendors = jobs
    .filter(
      (j) =>
        !j.editable ||
        (!j.label.startsWith("com.krittaprot.") &&
          !j.label.startsWith("com.ode.") &&
          !j.label.startsWith("ai.hermes."))
    )
    .reduce<Record<string, JobRef[]>>((groups, j) => {
      const vendor = j.label.startsWith("com.google.")
        ? "Google Updater"
        : j.label.startsWith("com.carriez.")
          ? "RustDesk"
          : j.label.startsWith("com.docker.")
            ? "Docker"
            : j.label.split(".").slice(0, 2).join(".")
      ;(groups[vendor] ??= []).push(j)
      return groups
    }, {})
  return (
    <>
      <header className="h-page-header">
        <div>
          <h1>Open at Login</h1>
          <p>Apps, background helpers and your own agents.</p>
        </div>
        <button className="h-button" disabled={busy} onClick={load}>
          <RefreshCw size={15} />
          Refresh
        </button>
        <button
          className="h-button h-primary"
          onClick={() => setAdd((v) => !v)}
        >
          <Plus size={15} />
          Add App…
        </button>
      </header>
      <div className="h-page-content">
        {snapshot.system.filevault_on && (
          <div className="h-card h-callout">
            <LockKeyhole size={22} />
            <span>
              FileVault is on. After a restart, someone must unlock this Mac
              before user services can start.
            </span>
          </div>
        )}
        {add && (
          <form
            className="h-card h-add-app"
            onSubmit={(e) => {
              e.preventDefault()
              void run(async () => {
                await setLoginItem(path, true)
                await load()
                setAdd(false)
                setPath("")
              })
            }}
          >
            <label>
              Application bundle path
              <input
                className="h-input"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="/Applications/Example.app"
                required
              />
            </label>
            <button className="h-button h-primary" type="submit">
              Add login item
            </button>
          </form>
        )}
        <section>
          <h2 className="h-section-title">
            Apps that open at login <small>Managed through System Events</small>
          </h2>
          {error && (
            <div role="alert" className="h-error">
              {error}
              <button className="h-button h-small" onClick={load}>
                Retry
              </button>
            </div>
          )}
          <div className="h-card">
            {busy && !items && (
              <p className="h-loading">
                Reading login items… macOS may ask for Automation permission.
              </p>
            )}
            {items?.apps.map((app) => (
              <div className="h-list-row" key={app.path}>
                <ServiceTile icon="login" tint="purple" />
                <div className="h-grow">
                  <strong>{app.name}</strong>
                  <p className="h-sub h-ellipsis" title={app.path}>
                    {app.path}
                  </p>
                </div>
                <span
                  className={`h-pill ${app.running ? "h-ok" : "h-neutral"}`}
                >
                  {app.running ? "Running" : "Not running"}
                </span>
                <button
                  className={`h-switch ${app.enabled ? "is-on" : ""}`}
                  role="switch"
                  aria-checked={app.enabled}
                  aria-label={`Open ${app.name} at login`}
                  disabled={busy}
                  onClick={() => {
                    setBusy(true)
                    void run(async () => {
                      await setLoginItem(app.path, !app.enabled)
                      await load()
                    }).finally(() => setBusy(false))
                  }}
                >
                  <span />
                </button>
              </div>
            ))}
            {items && !items.apps.length && (
              <p className="h-loading">
                No login apps found. Add an app using its bundle path.
              </p>
            )}
          </div>
        </section>
        <section>
          <h2 className="h-section-title">
            Background helpers{" "}
            <small>
              Installed agents and daemons · system jobs are read-only
            </small>
          </h2>
          <div className="h-card">
            {Object.entries(vendors).map(([vendor, entries]) => (
              <div className="h-list-row" key={vendor}>
                <ServiceTile icon="cpu" tint="graphite" />
                <div className="h-grow">
                  <strong>{vendor}</strong>
                  <p className="h-sub">
                    {entries?.map((j) => j.label).join(" · ")}
                  </p>
                </div>
                <span
                  className={`h-pill ${entries?.some((j) => j.missing_executable) ? "h-warn" : "h-neutral"}`}
                >
                  {entries?.some((j) => j.missing_executable)
                    ? "Leftover · app or binary missing"
                    : `${entries?.length} helpers`}
                </span>
                {entries?.find((j) => j.plist_path) && (
                  <button
                    className="h-button h-small"
                    onClick={() =>
                      onJob(entries.find((j) => j.plist_path)!.plist_path!)
                    }
                  >
                    Details
                  </button>
                )}
              </div>
            ))}
          </div>
        </section>
        <section>
          <h2 className="h-section-title">
            Your agents that aren’t loaded{" "}
            <small>
              {snapshot.not_loaded.length} plists in ~/Library/LaunchAgents
            </small>
          </h2>
          <div className="h-card">
            {snapshot.not_loaded.map((job) => (
              <div className="h-list-row" key={job.label}>
                <i className="h-dot h-gray" />
                <button
                  className="h-text-button h-mono h-grow"
                  onClick={() => job.plist_path && onJob(job.plist_path)}
                >
                  {job.label}
                </button>
                <span className="h-sub">
                  {job.missing_executable
                    ? "App or executable missing"
                    : "Not in launchctl list"}
                </span>
                <button
                  className="h-button h-small"
                  onClick={() => archive(job)}
                >
                  Archive…
                </button>
              </div>
            ))}
            {!snapshot.not_loaded.length && (
              <p className="h-loading">
                All discovered user agents are loaded.
              </p>
            )}
          </div>
        </section>
        {items && (
          <details className="h-card h-extensions">
            <summary>
              Network & system extensions{" "}
              <span className="h-sub">Read-only</span>
            </summary>
            <pre>{items.extensions || "No extension output"}</pre>
          </details>
        )}
      </div>
    </>
  )
}
