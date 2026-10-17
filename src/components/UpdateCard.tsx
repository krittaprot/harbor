import type { useUpdates } from "@/hooks/useUpdates"
import { openUrl } from "@/lib/harbor"
import { Anchor, ArrowDownToLine, Check, CircleAlert, RefreshCw, ShieldCheck } from "lucide-react"

export function UpdateCard({ updater }: { updater: ReturnType<typeof useUpdates> }) {
  const { status, available, phase, progress, error, checkedAt, check, install, restart } = updater
  const busy = ["checking", "downloading", "verifying", "installing", "restarting"].includes(phase)
  const installed = phase === "installed" || phase === "restarting"
  const percent = progress?.total && progress.stage === "downloading"
    ? Math.min(100, Math.floor(progress.downloaded / progress.total * 100)) : undefined
  const [title, detail] = {
    idle: ["Ready when you are", "Check for the latest features and fixes from GitHub."],
    checking: ["Looking for something new…", "Checking GitHub for the latest Harbor release."],
    current: ["You’re all set!", `Harbor ${status?.current_version ?? ""} is the latest version. Enjoy a little peace of mind.`],
    available: ["A new update is ready", `Harbor ${available?.version ?? ""} is available. Take a look at what’s new below.`],
    downloading: ["Your update is on its way", percent !== undefined ? `Downloaded ${percent}% · Keep Harbor open.` : "Downloading the signed update. Keep Harbor open."],
    verifying: ["Verifying your update", "Making sure this download is a trusted Harbor release."],
    installing: ["Giving Harbor a fresh start", "Installing the update. Keep Harbor open for a moment."],
    installed: ["All installed. One last step.", `Restart Harbor to start using ${available?.version ?? "the new version"}. Your settings will be right here.`],
    restarting: ["See you in a moment", "Restarting Harbor with your new update…"],
  }[phase]
  const tone = error ? "error" : phase === "current" || installed ? "success" : phase === "idle" ? "idle" : "active"
  const Icon = error ? CircleAlert : phase === "current" || installed ? Check
    : phase === "available" || phase === "downloading" ? ArrowDownToLine
    : phase === "verifying" || phase === "installing" ? ShieldCheck : Anchor
  const errorTitle = installed ? "Harbor couldn’t restart" : available ? "Let’s try that update again" : "Couldn’t check this time"
  const errorDetail = installed ? "The update is installed. Quit and reopen Harbor, or try Restart again."
    : available ? "The update didn’t finish. Try again, or get the DMG from GitHub."
    : "We couldn’t confirm the latest version. Please try again in a moment."
  return (
    <section className="h-card h-update-card" aria-labelledby="harbor-updates-title" data-update-state={error ? "error" : phase}>
      <div className="h-list-row">
        <div className="h-grow">
          <h2 id="harbor-updates-title">Software updates</h2>
          <p className="h-sub">{status ? `Harbor ${status.current_version}` : "Harbor"} · GitHub Releases</p>
        </div>
        {!installed && <button className="h-button h-update-check" disabled={busy} onClick={() => void check()}>
          <RefreshCw size={14} aria-hidden="true" />
          {phase === "checking" ? "Checking…" : "Check for Updates"}
        </button>}
      </div>
      <div className="h-update-body">
        <div className={`h-update-status h-update-status--${tone}`} role={error ? "alert" : "status"} aria-live={error ? "assertive" : "polite"} aria-atomic="true">
          <span key={error ? `error-${phase}` : phase} className={`h-update-symbol ${phase === "checking" && !error ? "is-checking" : ""}`} aria-hidden="true">
            <Icon size={24} strokeWidth={2} />
            {phase === "checking" && !error && <span className="h-update-orbit" />}
          </span>
          <div className="h-update-message">
            <strong>{error ? errorTitle : title}</strong>
            <p>{error ? errorDetail : detail}</p>
            {!error && checkedAt && ["current", "available"].includes(phase) && <time dateTime={checkedAt}>
              Checked at {new Date(checkedAt).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}
            </time>}
            {error && <details className="h-update-error-details"><summary>Technical details</summary><p>{error}</p></details>}
          </div>
        </div>
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
        <a href="https://github.com/krittaprot/harbor/releases/latest" className="h-text-button"
          onClick={event => {
            event.preventDefault()
            void openUrl("https://github.com/krittaprot/harbor/releases/latest").catch(() => {})
          }}>View releases on GitHub ↗</a>
      </div>
    </section>
  )
}
