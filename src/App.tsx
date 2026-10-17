import { useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"
import { Sidebar, type View, type Filter } from "@/components/Sidebar"
import { useSnapshot } from "@/hooks/useSnapshot"
import { useUpdates } from "@/hooks/useUpdates"
import { UpdateCard } from "@/components/UpdateCard"
import { Services } from "@/views/Services"
import { OpenAtLogin } from "@/views/OpenAtLogin"
import { RestartReadiness } from "@/views/RestartReadiness"
import { AdoptSheet } from "@/components/AdoptSheet"
import { JobDetail } from "@/components/JobDetail"
import { JobForm } from "@/components/JobForm"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from "@/components/ui/dialog"
import { createJob, saveJob } from "@/lib/invoke"
import * as api from "@/lib/harbor"
import type {
  UnmanagedGroup,
  LaunchdJob,
  PlistConfig,
  HarborSettings,
} from "@/types"
import LegacyJobs from "./LegacyJobs"
import { Popover } from "./popover/Popover"
import "./theme.css"

type Confirmation = {
  title: string
  description: string
  label: string
  action: () => Promise<unknown>
}
function App() {
  const { snapshot, error, busy, refresh } = useSnapshot()
  const updater = useUpdates()
  const [view, setView] = useState<View>("services")
  const [filter, setFilter] = useState<Filter>("all")
  const [actionError, setActionError] = useState<string | null>(null)
  const [notice, setNotice] = useState<string | null>(null)
  const [pending, setPending] = useState(false)
  const [confirm, setConfirm] = useState<Confirmation | null>(null)
  const [adopt, setAdopt] = useState<UnmanagedGroup | null>(null)
  const [jobPath, setJobPath] = useState<string | null>(null)
  const [formOpen, setFormOpen] = useState(false)
  const [editing, setEditing] = useState<LaunchdJob | null>(null)
  const [settings, setSettings] = useState<HarborSettings | null>(null)
  const [restartRequest, setRestartRequest] = useState<string | null>(null)
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)")
    const update = () =>
      document.documentElement.classList.toggle("dark", media.matches)
    update()
    media.addEventListener("change", update)
    const listening = listen<string>("harbor://navigate", (e) => {
      if (["services", "login", "restart", "settings"].includes(e.payload))
        setView(e.payload as View)
      else if (e.payload.startsWith("restart-service:")) {
        setView("services")
        setRestartRequest(e.payload.slice(16))
      }
    })
    const settingsListener = listen<HarborSettings>("harbor://settings", (e) =>
      setSettings(e.payload)
    )
    void api
      .getSettings()
      .then(setSettings)
      .catch((e) => setActionError(String(e)))
    void api
      .lastRestartReport()
      .then((r) => {
        if (r) setView("restart")
      })
      .catch(() => {})
    return () => {
      media.removeEventListener("change", update)
      void listening.then((stop) => stop())
      void settingsListener.then((stop) => stop())
    }
  }, [])
  useEffect(() => {
    if (!snapshot || !restartRequest) return
    const service = snapshot.services.find((s) => s.id === restartRequest)
    setRestartRequest(null)
    if (service)
      setConfirm({
        title: `Restart ${service.name}?`,
        description: `Restart ${service.jobs.length} launchd jobs. Active connections may be interrupted.`,
        label: "Restart",
        action: () => api.serviceAction(service.id, "restart"),
      })
  }, [snapshot, restartRequest])
  async function run(action: () => Promise<unknown>) {
    setPending(true)
    setActionError(null)
    setNotice(null)
    try {
      const result = await action()
      if (Array.isArray(result)) setNotice(result.join(" · "))
      await refresh()
    } catch (e) {
      setActionError(String(e))
    } finally {
      setPending(false)
    }
  }
  const selectFilter = (f: Filter) => {
    setFilter(f)
    setView("services")
  }
  async function save(config: PlistConfig, path?: string) {
    if (path) await saveJob(path, config)
    else await createJob(config.label, config)
    await refresh()
  }
  if (new URLSearchParams(location.search).has("popover"))
    return <Popover snapshot={snapshot} refresh={refresh} busy={busy} />
  return (
    <div className="harbor">
      <Sidebar
        snapshot={snapshot}
        view={view}
        filter={filter}
        navigate={setView}
        selectFilter={selectFilter}
      />
      <main className="h-main" aria-busy={pending}>
        {(error || actionError) && (
          <div className="h-error h-global-message" role="alert">
            {error || actionError}
            <button
              className="h-button h-small"
              onClick={() => {
                setActionError(null)
                void refresh()
              }}
            >
              Retry scan
            </button>
          </div>
        )}
        {notice && (
          <div className="h-notice h-global-message" role="status">
            {notice}
            <button className="h-text-button" onClick={() => setNotice(null)}>
              Dismiss
            </button>
          </div>
        )}
        {!snapshot && view !== "jobs" && view !== "settings" && (
          <div className="h-empty h-initial">
            <h1>Harbor</h1>
            <p>Reading services, processes and listening ports…</p>
            <span className="h-sub">
              The first scan includes health and system checks.
            </span>
          </div>
        )}
        {snapshot && view === "services" && (
          <Services
            snapshot={snapshot}
            filter={filter}
            setFilter={selectFilter}
            busy={busy || pending}
            refresh={refresh}
            onNew={() => {
              setEditing(null)
              setFormOpen(true)
            }}
            onJob={setJobPath}
            open={(url) => void run(() => api.openUrl(url))}
            pin={(s) => void run(() => api.pinService(s.id, !s.pinned))}
            action={(s, a) =>
              setConfirm({
                title: `${a[0].toUpperCase() + a.slice(1)} ${s.name}?`,
                description: `This affects ${s.jobs.length} launchd jobs: ${s.jobs.map((j) => j.role).join(", ")}. ${a !== "start" ? "Active connections may be interrupted." : ""}`,
                label: a[0].toUpperCase() + a.slice(1),
                action: () => api.serviceAction(s.id, a),
              })
            }
            adopt={setAdopt}
            copyReport={(g) => void run(async () => {
              const report = await api.diagnoseTarget(g.key)
              await api.copyText(JSON.stringify(report, null, 2))
              return ["Diagnostic report copied"]
            })}
            stop={(g) =>
              setConfirm({
                title: `Stop ${g.name}?`,
                description: `Send SIGTERM to ${g.copies.length} verified, current-user processes. Harbor will check that they exit and will never force kill.`,
                label: "Stop processes",
                action: () => api.stopUnmanaged(g.key),
              })
            }
            ignore={(g) =>
              void run(() =>
                api.ignoreUnmanaged(g.key, g.verdict !== "Expected")
              )
            }
          />
        )}
        {snapshot && view === "login" && (
          <OpenAtLogin
            snapshot={snapshot}
            onJob={setJobPath}
            run={run}
            archive={(j) =>
              setConfirm({
                title: `Archive ${j.label}?`,
                description:
                  "Move this unloaded plist into ~/Library/LaunchAgents/.harbor-archive/. It will not be deleted.",
                label: "Archive",
                action: () => api.archivePlist(j.plist_path!),
              })
            }
          />
        )}
        {snapshot && view === "restart" && (
          <RestartReadiness
            snapshot={snapshot}
            run={run}
            navigate={setView}
            restart={() =>
              setConfirm({
                title: `Restart ${snapshot.system.hostname}?`,
                description: `${snapshot.counts.unmanaged} unmanaged processes need review (temporary processes excluded). ${snapshot.counts.attention} services need attention. ${snapshot.system.filevault_on ? "FileVault requires someone to unlock this Mac." : "Review login and volume readiness first."} Harbor saves a fresh snapshot before requesting macOS restart.`,
                label: "Restart Mac",
                action: api.restartMachine,
              })
            }
          />
        )}
        {view === "jobs" && (
          <div className="h-legacy">
            <LegacyJobs />
          </div>
        )}
        {view === "settings" && (
          <>
            <header className="h-page-header">
              <div>
                <h1>Settings</h1>
                <p>Harbor lives in your menu bar. Settings stay on this Mac.</p>
              </div>
            </header>
            <div className="h-page-content">
              <UpdateCard updater={updater} />
              <div className="h-card">
                {settings &&
                  (
                    [
                      [
                        "notifications",
                        "Notify me when something stops",
                        "A service stops or fails, or a health probe fails twice.",
                      ],
                      [
                        "start_at_login",
                        "Open Harbor at login",
                        "Applied to this installed application bundle.",
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
                        aria-label={title}
                        aria-checked={settings[key]}
                        disabled={pending}
                        className={`h-switch ${settings[key] ? "is-on" : ""}`}
                        onClick={() =>
                          void run(async () => {
                            const next = { ...settings, [key]: !settings[key] }
                            await api.setSettings(
                              next.notifications,
                              next.start_at_login
                            )
                            setSettings(next)
                          })
                        }
                      >
                        <span />
                      </button>
                    </div>
                  ))}
              </div>
              <div className="h-card h-settings-info">
                <h2>Service overrides</h2>
                <p>
                  Edit{" "}
                  <code>
                    ~/Library/Application Support/Harbor/services.toml
                  </code>{" "}
                  to change grouping, health URLs, volume dependencies and pins.
                  Changes are read on the next scan.
                </p>
                <h2>About Harbor</h2>
                <p>
                  A native macOS service monitor, built on azu/launchd-ui. MIT
                  licensed. User-level only; system daemons remain read-only.
                </p>
                <button
                  className="h-button"
                  onClick={() => void api.quitHarbor()}
                >
                  Quit Harbor
                </button>
              </div>
            </div>
          </>
        )}
      </main>
      {jobPath && (
        <JobDetail
          plistPath={jobPath}
          open
          onClose={() => setJobPath(null)}
          onEdit={(job) => {
            setEditing(job)
            setJobPath(null)
            setFormOpen(true)
          }}
        />
      )}
      <JobForm
        key={editing?.plist_path ?? "new"}
        open={formOpen}
        onClose={() => setFormOpen(false)}
        onSave={save}
        editingJob={editing}
      />
      {adopt && <AdoptSheet
        group={adopt}
        close={() => setAdopt(null)}
        done={() => void refresh()}
      />}
      <Dialog
        open={!!confirm}
        onOpenChange={(open) => {
          if (!open && !pending) setConfirm(null)
        }}
      >
        <DialogContent>
          <DialogHeader>
            <DialogTitle>{confirm?.title}</DialogTitle>
            <DialogDescription>{confirm?.description}</DialogDescription>
          </DialogHeader>
          {actionError && <p className="h-error">{actionError}</p>}
          <DialogFooter>
            <button
              className="h-button"
              disabled={pending}
              onClick={() => setConfirm(null)}
            >
              Cancel
            </button>
            <button
              className="h-button h-primary"
              disabled={pending}
              onClick={async () => {
                if (!confirm) return
                await run(confirm.action)
                setConfirm(null)
              }}
            >
              {pending ? "Working…" : confirm?.label}
            </button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
export default App
