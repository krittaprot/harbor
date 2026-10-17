import type { useUpdates } from "@/hooks/useUpdates"
import { openUrl } from "@/lib/harbor"

export function UpdateCard({ updater }: { updater: ReturnType<typeof useUpdates> }) {
  const { status, available, phase, progress, error, check, install, restart } = updater
  const busy = ["checking", "downloading", "verifying", "installing", "restarting"].includes(phase)
  const installed = phase === "installed" || phase === "restarting"
  const percent = progress?.total && progress.stage === "downloading"
    ? Math.min(100, Math.floor(progress.downloaded / progress.total * 100)) : undefined
  const message = {
    idle: "Check GitHub for the latest stable release.",
    checking: "Checking GitHub…",
    current: "You’re up to date.",
    available: `Harbor ${available?.version ?? ""} is available.`,
    downloading: `Downloading update${percent !== undefined ? ` · ${percent}%` : "…"}`,
    verifying: "Verifying update signature…",
    installing: "Installing update. Keep Harbor open…",
    installed: "Update installed. Restart Harbor when you’re ready.",
    restarting: "Restarting Harbor…",
  }[phase]
  return (
    <section className="h-card h-update-card" aria-labelledby="harbor-updates-title">
      <div className="h-list-row">
        <div className="h-grow">
          <h2 id="harbor-updates-title">Software updates</h2>
          <p className="h-sub">{status ? `Harbor ${status.current_version}` : "Harbor"} · GitHub Releases</p>
        </div>
        {!installed && <button className="h-button" disabled={busy} onClick={() => void check()}>
          {phase === "checking" ? "Checking…" : "Check for Updates"}
        </button>}
      </div>
      <div className="h-update-body">
        <p role="status" aria-live="polite">{message}</p>
        {["downloading", "verifying", "installing"].includes(phase) &&
          <progress aria-label="Update progress" max={100} value={percent} />}
        {available && !installed && <>
          {available.notes && <details className="h-update-notes">
            <summary>What’s new in {available.version}</summary>
            <p>{available.notes}</p>
          </details>}
          <p className="h-sub">Downloads a signed release and replaces Harbor. Your settings and monitored services stay in place. Restart Harbor afterward.</p>
          <button className="h-button h-primary" disabled={busy || !status?.can_install} onClick={() => void install()}>
            Download &amp; Install {available.version}
          </button>
        </>}
        {installed && <button className="h-button h-primary" disabled={busy} onClick={() => void restart()}>
          {phase === "restarting" ? "Restarting…" : "Restart Harbor"}
        </button>}
        {status && !status.can_install && <p className="h-sub">
          To install updates here, open a release build from Applications or ~/Applications. Development builds and disk images can check only.
        </p>}
        {error && <div role="alert" className="h-error">
          <strong>{installed ? "Harbor could not restart" : "Update did not finish"}</strong>
          <p>{error}</p>
          {installed && <p>You can quit and reopen Harbor to use the installed update.</p>}
        </div>}
        <a href="https://github.com/krittaprot/harbor/releases/latest" className="h-text-button"
          onClick={event => {
            event.preventDefault()
            void openUrl("https://github.com/krittaprot/harbor/releases/latest").catch(() => {})
          }}>View releases on GitHub ↗</a>
      </div>
    </section>
  )
}
