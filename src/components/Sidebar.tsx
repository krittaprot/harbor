import { Settings, SlidersHorizontal } from "lucide-react"
import { ServiceTile, AnchorMark } from "./ServiceTile"
import { uptime } from "@/lib/harbor"
import type { Snapshot } from "@/types"
export type View = "services" | "login" | "restart" | "settings" | "jobs"
export type Filter =
  "all" | "web" | "ai" | "data" | "infra" | "unmanaged" | "attention" | "apple"
export function Sidebar({
  snapshot: s,
  view,
  filter,
  navigate,
  selectFilter,
}: {
  snapshot: Snapshot | null
  view: View
  filter: Filter
  navigate: (v: View) => void
  selectFilter: (f: Filter) => void
}) {
  const age = s?.system.boot_time
    ? Math.max(0, (Date.now() - Date.parse(s.system.boot_time)) / 1000)
    : null
  const row = (
    label: string,
    icon: string,
    tint: string,
    count: string | number,
    active: boolean,
    click: () => void
  ) => (
    <button className={`h-nav ${active ? "is-active" : ""}`} onClick={click}>
      <ServiceTile icon={icon} tint={tint} size="small" />
      <span>{label}</span>
      <small>{count}</small>
    </button>
  )
  return (
    <aside className="h-sidebar">
      <div className="h-brand">
        <AnchorMark />
        <div>
          <strong>Harbor</strong>
          <span>
            {s?.system.hostname ?? "Your Mac"} · up {uptime(age)}
          </span>
        </div>
      </div>
      <div className="h-nav-label">Manage</div>
      {row(
        "Services",
        "services",
        "blue",
        s?.services.filter((x) => !x.apple).length ?? "—",
        view === "services" &&
          !["unmanaged", "attention", "apple"].includes(filter),
        () => {
          navigate("services")
          selectFilter("all")
        }
      )}
      {row("Open at Login", "login", "purple", "", view === "login", () =>
        navigate("login")
      )}
      {row(
        "Restart Readiness",
        "restart",
        "orange",
        s?.counts.attention ?? "",
        view === "restart",
        () => navigate("restart")
      )}
      <div className="h-nav-label">Smart filters</div>
      {row(
        "No Launch Agent",
        "warning",
        "yellow",
        s?.counts.unmanaged ?? "—",
        view === "services" && filter === "unmanaged",
        () => selectFilter("unmanaged")
      )}
      {row(
        "Needs Attention",
        "warning",
        "red",
        s?.counts.attention ?? "—",
        view === "services" && filter === "attention",
        () => selectFilter("attention")
      )}
      {row(
        "Not Loaded",
        "unloaded",
        "graphite",
        s?.counts.not_loaded ?? "—",
        false,
        () => navigate("login")
      )}
      {row(
        "Apple & System",
        "cpu",
        "graphite",
        filter === "apple" ? "shown" : "hidden",
        view === "services" && filter === "apple",
        () => selectFilter(filter === "apple" ? "all" : "apple")
      )}
      <div className="h-sidebar-bottom">
        <div className="h-system-status">
          <p>
            <i
              className={`h-dot ${s?.system.provault_mounted ? "h-green" : "h-amber"}`}
            />
            ProVault{" "}
            {s
              ? s.system.provault_mounted
                ? "mounted"
                : "not mounted"
              : "checking…"}
          </p>
          <p>
            <i
              className={`h-dot ${s?.system.tailscale_up ? "h-green" : "h-gray"}`}
            />
            Tailscale{" "}
            {s?.system.tailscale_up == null
              ? "unknown"
              : s.system.tailscale_up
                ? "connected"
                : "disconnected"}
          </p>
          <p>
            <i className="h-dot h-amber" />
            FileVault{" "}
            {s?.system.filevault_on == null
              ? "unknown"
              : s.system.filevault_on
                ? "on · unlock after restart"
                : "off"}
          </p>
          <span className="h-sub">
            {s
              ? `Scanned ${new Date(s.taken_at).toLocaleTimeString()}`
              : "Reading this Mac…"}
          </span>
        </div>
        <button className="h-nav" onClick={() => navigate("settings")}>
          <Settings size={16} />
          Settings
        </button>
        <button className="h-nav" onClick={() => navigate("jobs")}>
          <SlidersHorizontal size={16} />
          Launchd job inspector
        </button>
      </div>
    </aside>
  )
}
