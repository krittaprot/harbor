import { useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { RotateCw, ExternalLink, Settings } from "lucide-react"
import type { Snapshot, HarborSettings } from "@/types"
import { AnchorMark, ServiceTile } from "@/components/ServiceTile"
import * as api from "@/lib/harbor"
export function Popover({
  snapshot: s,
  refresh,
  busy,
}: {
  snapshot: Snapshot | null
  refresh: () => void
  busy: boolean
}) {
  const [settings, setSettings] = useState<HarborSettings | null>(null)
  const [error, setError] = useState<string | null>(null)
  useEffect(() => {
    void api
      .getSettings()
      .then(setSettings)
      .catch((e) => setError(String(e)))
    const listener = listen<HarborSettings>("harbor://settings", (e) =>
      setSettings(e.payload)
    )
    return () => {
      void listener.then((stop) => stop())
    }
  }, [])
  const attempt = (fn: () => Promise<unknown>) => {
    setError(null)
    void fn().catch((e) => setError(String(e)))
  }
  return (
    <div className="h-popover">
      <div className="h-brand">
        <AnchorMark />
        <div>
          <strong>
            {s?.system.hostname ?? "Harbor"} · {s?.counts.attention ?? "—"} need
            a look
          </strong>
          <span>
            {s
              ? `Scanned ${new Date(s.taken_at).toLocaleTimeString()}`
              : "Scanning…"}
          </span>
        </div>
      </div>
      {error && <p className="h-error">{error}</p>}
      {s && (
        <>
          <div className="h-pop-stats">
            <div className="h-ok">
              <b>{s.counts.running}</b>
              <span>running</span>
            </div>
            <div className="h-warn">
              <b>{s.counts.unmanaged}</b>
              <span>unmanaged to review</span>
            </div>
            <div className="h-bad">
              <b>
                {
                  s.services.filter((x) => !x.apple && x.state === "Failed")
                    .length
                }
              </b>
              <span>failed</span>
            </div>
          </div>
          <h3>Needs a look</h3>
          {s.services
            .filter(
              (x) =>
                !x.apple &&
                (x.state === "Failed" || x.health.status === "Failed")
            )
            .slice(0, 2)
            .map((x) => (
              <button
                className="h-pop-row"
                key={x.id}
                onClick={() => attempt(() => api.showMain("services"))}
              >
                <ServiceTile icon="warning" tint="red" size="small" />
                <span className="h-grow">
                  <strong>{x.name}</strong>
                  <small>
                    {x.state === "Failed"
                      ? "Job failed"
                      : "Health check failed"}
                  </small>
                </span>
                <span className="h-sub">Review</span>
              </button>
            ))}
          {s.counts.unmanaged > 0 && (
            <button
              className="h-pop-row"
              onClick={() => attempt(() => api.showMain("services"))}
            >
              <ServiceTile icon="warning" tint="orange" size="small" />
              <span className="h-grow">
                {s.counts.unmanaged} unmanaged processes to review
              </span>
            </button>
          )}
          <h3>Pinned</h3>
          {s.services
            .filter((x) => x.pinned && !x.apple)
            .map((x) => (
              <div className="h-pop-row" key={x.id}>
                <ServiceTile icon={x.icon} tint={x.tint} size="small" />
                <button
                  className="h-grow h-text-button"
                  onClick={() => attempt(() => api.showMain("services"))}
                >
                  <strong>{x.name}</strong>
                  <small className="h-mono">
                    {[...new Set(x.ports.map((p) => `:${p.port}`))].join(" ")}
                  </small>
                </button>
                <i
                  className={`h-dot ${x.state === "Failed" || x.health.status === "Failed" ? "h-red" : x.state === "Running" ? "h-green" : "h-gray"}`}
                />
                <button
                  className="h-icon-button"
                  aria-label={`Review restart for ${x.name}`}
                  disabled={!x.jobs.every(j => j.editable)}
                  onClick={() => attempt(() => api.showMain(`restart-service:${x.id}`))}
                >
                  <RotateCw size={14} />
                </button>
                <button
                  className="h-icon-button"
                  disabled={!api.serviceUrl(x)}
                  aria-label={`Open ${x.name}`}
                  onClick={() => attempt(() => api.openUrl(api.serviceUrl(x)!))}
                >
                  <ExternalLink size={14} />
                </button>
              </div>
            ))}
          <button
            className="h-pop-row h-pop-divider"
            onClick={() => attempt(() => api.showMain("restart"))}
          >
            <ServiceTile icon="restart" tint="orange" size="small" />
            <div className="h-grow">
              <strong>Restart readiness</strong>
              <small>
                {s.system.filevault_on
                  ? "Needs keyboard unlock"
                  : "Review before restarting"}
              </small>
            </div>
          </button>
          {settings && (
            <div className="h-pop-row">
              <span className="h-grow">Notify me when something stops</span>
              <button
                role="switch"
                aria-checked={settings.notifications}
                aria-label="Notifications"
                className={`h-switch ${settings.notifications ? "is-on" : ""}`}
                onClick={() =>
                  attempt(async () => {
                    await api.setSettings(
                      !settings.notifications,
                      settings.start_at_login
                    )
                    setSettings({
                      ...settings,
                      notifications: !settings.notifications,
                    })
                  })
                }
              >
                <span />
              </button>
            </div>
          )}
        </>
      )}
      <footer>
        <button
          className="h-button h-primary h-small"
          onClick={() => attempt(() => api.showMain())}
        >
          Open Harbor
        </button>
        <button className="h-button h-small" disabled={busy} onClick={refresh}>
          {busy ? "Scanning…" : "Rescan"}
        </button>
        <button
          className="h-icon-button"
          aria-label="Settings"
          onClick={() => attempt(() => api.showMain("settings"))}
        >
          <Settings size={15} />
        </button>
      </footer>
    </div>
  )
}
