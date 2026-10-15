import { useEffect, useState } from "react"
import { Save, Power } from "lucide-react"
import { lastRestartReport, saveSnapshot } from "@/lib/harbor"
import type { Snapshot, RestartReport } from "@/types"
import { ServiceTile } from "@/components/ServiceTile"
export function RestartReadiness({
  snapshot: s,
  restart,
  run,
  navigate,
}: {
  snapshot: Snapshot
  restart: () => void
  run: (f: () => Promise<unknown>) => Promise<void>
  navigate: (view: "services" | "login") => void
}) {
  const [report, setReport] = useState<RestartReport | null>(null)
  const [saved, setSaved] = useState(false)
  useEffect(() => {
    void lastRestartReport()
      .then(setReport)
      .catch(() => {})
  }, [s.taken_at])
  const jobs = s.services.filter((x) => !x.apple).flatMap((x) => x.jobs)
  const auto = jobs.filter((j) => j.autostart && j.status !== "Unloaded").length
  const failed = jobs.filter((j) => !j.pid && j.last_exit).length
  const concerns =
    Number(!s.system.provault_mounted) +
    Number(s.counts.unmanaged > 0) +
    Number(failed > 0)
  const checks = [
    {
      title: s.system.filevault_on
        ? "Unlock required after restart"
        : s.system.filevault_on === false
          ? "FileVault is off"
          : "FileVault status unknown",
      detail:
        "User agents start after login. Harbor does not assume auto-login is enabled.",
      tone: "orange",
      icon: "restart",
    },
    {
      title: s.system.provault_mounted
        ? "ProVault is mounted"
        : "ProVault is not mounted",
      detail:
        "Data services depend on this volume. Harbor only reports its state.",
      tone: s.system.provault_mounted ? "green" : "red",
      icon: "shield",
    },
    {
      title: `${auto} jobs configured to start automatically`,
      detail:
        "Loaded jobs with RunAtLoad or KeepAlive. This is configuration, not a guarantee of successful recovery.",
      tone: "green",
      icon: "services",
    },
    {
      title: `${s.counts.unmanaged} processes have no launch agent`,
      detail: "Review them before restarting. They may need a manual start.",
      tone: s.counts.unmanaged ? "orange" : "green",
      icon: "warning",
      action: "Review",
      view: "services" as const,
    },
    {
      title: `${failed} jobs last exited with an error`,
      detail:
        "Inspect the job’s logs to understand the failure before relying on it.",
      tone: failed ? "red" : "green",
      icon: "warning",
      action: "Inspect",
      view: "services" as const,
    },
    {
      title: "Review login apps",
      detail:
        "Read the current login items before saving a snapshot to include their recovery state.",
      tone: "purple",
      icon: "login",
      action: "Open at Login",
      view: "login" as const,
    },
  ]
  return (
    <>
      <header className="h-page-header">
        <div>
          <h1>Restart Readiness</h1>
          <p>
            What comes back, what won’t, and what needs you at the keyboard.
          </p>
        </div>
        <button
          className="h-button"
          onClick={() =>
            void run(async () => {
              await saveSnapshot()
              setSaved(true)
            })
          }
        >
          <Save size={15} />
          {saved ? "Snapshot saved" : "Save Snapshot"}
        </button>
        <button className="h-button h-primary" onClick={restart}>
          <Power size={15} />
          Restart {s.system.hostname}…
        </button>
      </header>
      <div className="h-page-content">
        <section className="h-card h-readiness-hero">
          <div className="h-hero-heading">
            <ServiceTile
              icon="shield"
              tint={concerns ? "orange" : "green"}
              size="large"
            />
            <div>
              <h2>
                {concerns
                  ? `${concerns} things to review before restarting`
                  : "Restart checklist looks clear"}
              </h2>
              <p>
                {auto} jobs are configured to return after login.{" "}
                {s.counts.unmanaged} processes have no launch agent.
              </p>
            </div>
          </div>
          <div
            className="h-readiness-bar"
            role="img"
            aria-label={`${auto} automatic jobs, ${s.counts.unmanaged} unmanaged processes, ${failed} failed jobs`}
          >
            <span className="h-bar-ok" style={{ flex: Math.max(1, auto) }} />
            <span className="h-bar-warn" style={{ flex: s.counts.unmanaged }} />
            <span className="h-bar-bad" style={{ flex: failed }} />
          </div>
          <div className="h-wrap h-legend">
            <span>
              <i className="h-dot h-green" />
              {auto} automatic jobs
            </span>
            <span>
              <i className="h-dot h-amber" />
              {s.counts.unmanaged} no agent
            </span>
            <span>
              <i className="h-dot h-red" />
              {failed} failed
            </span>
          </div>
        </section>
        <section>
          <h2 className="h-section-title">Before you restart</h2>
          <div className="h-card">
            {checks.map((c) => (
              <div className="h-list-row" key={c.title}>
                <ServiceTile icon={c.icon} tint={c.tone} />
                <div className="h-grow">
                  <strong>{c.title}</strong>
                  <p className="h-sub">{c.detail}</p>
                </div>
                {c.action && c.view && (
                  <button
                    className="h-button h-small h-tinted"
                    onClick={() => navigate(c.view!)}
                  >
                    {c.action}
                  </button>
                )}
              </div>
            ))}
          </div>
        </section>
        <section>
          <h2 className="h-section-title">Last restart</h2>
          <div className="h-card h-report">
            {report ? (
              <>
                <p className="h-sub">
                  Booted {new Date(report.boot_time).toLocaleString()} ·
                  compared with {new Date(report.before).toLocaleString()}
                </p>
                <div className="h-report-stats">
                  <div>
                    <b>{report.came_back.length}</b>
                    <span>jobs came back</span>
                  </div>
                  <div>
                    <b>{report.missing.length}</b>
                    <span>jobs missing</span>
                  </div>
                  <div>
                    <b>{report.new.length}</b>
                    <span>new jobs</span>
                  </div>
                  <div>
                    <b>
                      {report.seconds_after_first_agent == null
                        ? "—"
                        : `${report.seconds_after_first_agent}s`}
                    </b>
                    <span>observed after first user agent</span>
                  </div>
                </div>
                {report.missing.length > 0 && (
                  <p className="h-error">
                    Missing: {report.missing.join(", ")}
                  </p>
                )}
                <p className="h-sub">
                  Login apps:{" "}
                  {report.login_returned == null
                    ? "not compared — read login items to update"
                    : `${report.login_returned} returned; ${report.login_missing?.length ?? 0} missing`}
                  . {report.unmanaged_missing.length} unmanaged groups and{" "}
                  {report.ports_missing.length} ports did not return.
                </p>
              </>
            ) : (
              <div className="h-empty">
                <ServiceTile icon="restart" tint="graphite" />
                <h3>No restart comparison yet</h3>
                <p>
                  Save a snapshot before your next restart. Harbor will compare
                  it with what returns.
                </p>
              </div>
            )}
          </div>
        </section>
      </div>
    </>
  )
}
