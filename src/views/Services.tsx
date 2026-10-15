import { useState } from "react"
import {
  ChevronRight,
  ExternalLink,
  RotateCw,
  Square,
  Play,
  Pin,
  Search,
  RefreshCw,
  Plus,
  MoreHorizontal,
  FileText,
} from "lucide-react"
import type { Snapshot, Service, UnmanagedGroup } from "@/types"
import type { Filter } from "@/components/Sidebar"
import { ServiceTile } from "@/components/ServiceTile"
import { matchesService, uptime, serviceUrl } from "@/lib/harbor"

export function HealthPill({ service: s }: { service: Service }) {
  const h = s.health
  const good = ["Ok", "PortOpen"].includes(h.status)
  const failed = h.status === "Failed" || s.state === "Failed"
  const label =
    s.state === "Failed"
      ? `Exit ${s.jobs.find((j) => j.pid == null && j.last_exit)?.last_exit ?? "error"}`
      : h.status === "Ok"
        ? `${h.code ?? "Ready"}${h.ms != null ? ` · ${h.ms} ms` : ""}`
        : h.status === "PortOpen"
          ? "TCP open"
          : h.status === "Failed"
            ? "Unhealthy"
            : s.state
  return (
    <span
      className={`h-pill ${failed ? "h-bad" : good ? "h-ok" : "h-neutral"}`}
      title={h.reason ?? undefined}
    >
      <i className="h-dot" />
      {label}
    </span>
  )
}

function Ports({ ports }: { ports: { port: number; addr: string }[] }) {
  const unique = [...new Set(ports.map((p) => p.port))]
  return (
    <div className="h-wrap">
      {unique.length ? (
        unique.map((port) => (
          <span
            className="h-port"
            key={port}
            title={ports
              .filter((p) => p.port === port)
              .map((p) => p.addr)
              .join(", ")}
          >
            :{port}
          </span>
        ))
      ) : (
        <span className="h-sub">—</span>
      )}
    </div>
  )
}

export function ServiceRow({
  service: s,
  onJob,
  action,
  open,
  pin,
}: {
  service: Service
  onJob: (path: string) => void
  action: (s: Service, a: "start" | "stop" | "restart") => void
  open: (url: string) => void
  pin: (s: Service) => void
}) {
  const [expanded, setExpanded] = useState(s.id === "crystal-blood")
  const [menu, setMenu] = useState(false)
  const editable = s.jobs.every((j) => j.editable)
  const url = serviceUrl(s)
  return (
    <article className="h-service" data-service={s.id}>
      <div className="h-service-row">
        <button
          className="h-service-name"
          onClick={() => setExpanded((v) => !v)}
          aria-expanded={expanded}
        >
          <ChevronRight size={12} className={expanded ? "h-expanded" : ""} />
          <ServiceTile icon={s.icon} tint={s.tint} />
          <span>
            <strong>{s.name}</strong>
            <small>
              {s.processes.length} processes · {s.jobs.length}{" "}
              {s.jobs.length === 1 ? "job" : "jobs"}
              {s.state === "Restarted" ? " · restarted" : ""}
            </small>
          </span>
        </button>
        <div className="h-wrap h-jobs">
          {s.jobs.map((j) => (
            <button
              className="h-job"
              disabled={!j.plist_path}
              key={j.label}
              onClick={() => j.plist_path && onJob(j.plist_path)}
              title={`${j.label}${j.last_exit != null ? ` · exit ${j.last_exit}` : ""}`}
            >
              <i
                className={`h-dot ${j.pid ? "h-green" : j.last_exit ? "h-red" : "h-gray"}`}
              />
              {j.role}
            </button>
          ))}
        </div>
        <Ports ports={s.ports} />
        <div>
          <HealthPill service={s} />
          {s.health.tunnels.length > 0 && (
            <small className="h-sub h-tunnel">
              {s.health.tunnels.filter((t) => t.ready).length}/
              {s.health.tunnels.length} tunnels ready
            </small>
          )}
        </div>
        <span className="h-uptime">{uptime(s.uptime_secs)}</span>
        <div className="h-actions">
          <button
            className="h-icon-button"
            disabled={!url}
            onClick={() => url && open(url)}
            aria-label={`Open ${s.name}`}
          >
            <ExternalLink size={15} />
          </button>
          <button
            className="h-icon-button"
            disabled={!editable}
            onClick={() => action(s, "restart")}
            aria-label={`Restart ${s.name}`}
          >
            <RotateCw size={15} />
          </button>
          <button
            className="h-icon-button"
            onClick={() => setMenu((v) => !v)}
            aria-expanded={menu}
            aria-label={`More actions for ${s.name}`}
          >
            <MoreHorizontal size={16} />
          </button>
        </div>
      </div>
      {menu && (
        <div className="h-inline-menu">
          <button
            className="h-button h-small"
            onClick={() => {
              pin(s)
              setMenu(false)
            }}
          >
            <Pin size={13} />
            {s.pinned ? "Unpin" : "Pin in menu bar"}
          </button>
          <button
            className="h-button h-small"
            disabled={!editable}
            onClick={() => action(s, "start")}
          >
            <Play size={13} />
            Start
          </button>
          <button
            className="h-button h-small"
            disabled={!editable}
            onClick={() => action(s, "stop")}
          >
            <Square size={13} />
            Stop
          </button>
          {!editable && (
            <span className="h-sub">System and unknown jobs are read-only</span>
          )}
        </div>
      )}
      {expanded && (
        <div className="h-process-tree">
          <div className="h-process-row h-tree-head">
            <span>Job → process</span>
            <span>Ports</span>
            <span>PID</span>
            <span>Memory / CPU</span>
          </div>
          {s.jobs.map((j) => (
            <div key={j.label}>
              <div className="h-process-row h-job-line">
                <button
                  disabled={!j.plist_path}
                  onClick={() => j.plist_path && onJob(j.plist_path)}
                >
                  <FileText size={12} />
                  <span>{j.label}</span>
                </button>
                <span />
                <span className="h-mono">{j.pid ?? "—"}</span>
                <span className="h-sub">
                  {j.status}
                  {j.last_exit ? ` · exit ${j.last_exit}` : ""}
                </span>
              </div>
              {s.processes
                .filter((p) => p.launchd_label === j.label)
                .map((p) => (
                  <div className="h-process-row" key={p.pid}>
                    <span
                      className="h-process-command h-mono"
                      title={p.argv.join(" ")}
                    >
                      {p.pid !== j.pid ? "↳ " : ""}
                      {p.name} {p.argv.slice(1).join(" ")}
                    </span>
                    <Ports
                      ports={s.ports.filter((port) => port.pid === p.pid)}
                    />
                    <span className="h-mono h-sub">{p.pid}</span>
                    <span className="h-mono h-sub">
                      {(p.mem_bytes / 1048576).toFixed(0)} MB ·{" "}
                      {p.cpu?.toFixed(1) ?? "—"}%
                    </span>
                  </div>
                ))}
            </div>
          ))}
          {s.depends_on.map((d) => (
            <span
              key={d.path}
              className={`h-pill ${d.mounted ? "h-ok" : "h-bad"}`}
            >
              Needs {d.path} · {d.mounted ? "mounted" : "missing"}
            </span>
          ))}
        </div>
      )}
    </article>
  )
}

export function Services({
  snapshot: s,
  filter,
  setFilter,
  busy,
  refresh,
  onNew,
  onJob,
  action,
  open,
  pin,
  adopt,
  stop,
  ignore,
}: {
  snapshot: Snapshot
  filter: Filter
  setFilter: (f: Filter) => void
  busy: boolean
  refresh: () => void
  onNew: () => void
  onJob: (path: string) => void
  action: (s: Service, a: "start" | "stop" | "restart") => void
  open: (url: string) => void
  pin: (s: Service) => void
  adopt: (g: UnmanagedGroup) => void
  stop: (g: UnmanagedGroup) => void
  ignore: (g: UnmanagedGroup) => void
}) {
  const [search, setSearch] = useState("")
  const all = s.services.filter((x) => !x.apple)
  const visible = s.services.filter(
    (x) =>
      matchesService(x, search) &&
      (filter === "apple"
        ? x.apple || x.jobs.some((j) => !j.editable)
        : !x.apple &&
          (filter === "all" ||
            filter === x.kind ||
            (filter === "attention" &&
              (x.state === "Failed" ||
                x.state === "Restarted" ||
                x.health.status === "Failed"))))
  )
  const unmanaged = s.unmanaged.filter((g) =>
    [
      g.name,
      ...g.copies.flatMap((p) => p.argv),
      ...g.ports.map((p) => String(p.port)),
    ]
      .join(" ")
      .toLowerCase()
      .includes(search.toLowerCase())
  )
  const filters: [Filter, string][] = [
    ["all", "All"],
    ["web", "Web apps"],
    ["ai", "AI & agents"],
    ["data", "Data"],
    ["infra", "Infra"],
  ]
  const kinds = [
    ["web", "Web apps"],
    ["ai", "AI & agents"],
    ["data", "Data & stacks"],
    ["infra", "Infrastructure"],
    ["other", "Other services"],
  ]
  return (
    <>
      <header className="h-page-header">
        <div>
          <h1>Services</h1>
          <p>
            {all.length} services · {all.reduce((n, x) => n + x.jobs.length, 0)}{" "}
            launchd jobs · {s.listening_port_count} listening ports
          </p>
        </div>
        <button className="h-button" disabled={busy} onClick={refresh}>
          <RefreshCw size={15} className={busy ? "h-spin" : ""} />
          {busy ? "Scanning…" : "Rescan"}
        </button>
        <button className="h-button h-primary" onClick={onNew}>
          <Plus size={15} />
          New Service
        </button>
      </header>
      <div className="h-page-content">
        <div className="h-stats">
          <div className="h-card h-stat">
            <svg className="h-ring" viewBox="0 0 52 52" aria-hidden="true">
              <circle cx="26" cy="26" r="21" />
              <circle
                cx="26"
                cy="26"
                r="21"
                strokeDasharray={`${(132 * s.counts.healthy) / Math.max(1, all.length)} 132`}
              />
            </svg>
            <div>
              <b>
                {s.counts.healthy}
                <small>/{all.length}</small>
              </b>
              <span>Healthy services</span>
            </div>
          </div>
          <button
            className="h-card h-stat"
            onClick={() => setFilter("unmanaged")}
          >
            <ServiceTile icon="warning" tint="yellow" size="large" />
            <div>
              <b>{s.counts.unmanaged}</b>
              <span>Unmanaged to review</span>
            </div>
          </button>
          <button
            className="h-card h-stat"
            onClick={() => setFilter("attention")}
          >
            <ServiceTile icon="warning" tint="red" size="large" />
            <div>
              <b>{s.counts.attention}</b>
              <span>Need attention</span>
            </div>
          </button>
          <div className="h-card h-stat">
            <ServiceTile icon="unloaded" tint="graphite" size="large" />
            <div>
              <b>{s.counts.not_loaded}</b>
              <span>Agents not loaded</span>
            </div>
          </div>
        </div>
        <div className="h-toolbar">
          <label className="h-search">
            <Search size={14} />
            <input
              aria-label="Search services"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search name, port, label or path"
            />
          </label>
          <div className="h-segments" aria-label="Service filter">
            {filters.map(([value, name]) => (
              <button
                key={value}
                aria-pressed={filter === value}
                className={filter === value ? "is-active" : ""}
                onClick={() => setFilter(value)}
              >
                {name}
                <small>
                  {value === "all"
                    ? all.length
                    : all.filter((x) => x.kind === value).length}
                </small>
              </button>
            ))}
          </div>
        </div>
        {["all", "unmanaged"].includes(filter) && unmanaged.length > 0 && (
          <section className="h-card h-unmanaged">
            <div className="h-section-card-header">
              <ServiceTile icon="warning" tint="orange" />
              <div>
                <h2>Running without a launch agent</h2>
                <p>
                  Mark intentionally short-lived processes as temporary to
                  exclude them from missing-agent warnings. This is remembered
                  for all copies of the same command group, including future runs.
                </p>
              </div>
            </div>
            {unmanaged.map((g) => (
              <div className="h-unmanaged-row" key={g.key}>
                <ServiceTile
                  icon="terminal"
                  tint={g.verdict === "Expected" ? "graphite" : "pink"}
                />
                <div className="h-grow">
                  <strong>
                    {g.name}{" "}
                    <span
                      className={`h-pill ${g.verdict === "Expected" ? "h-neutral" : "h-warn"}`}
                    >
                      {g.verdict === "Expected" ? "Temporary" : g.verdict}
                      {g.copies.length > 1
                        ? ` · ${g.copies.length} copies`
                        : ""}
                    </span>
                  </strong>
                  <p
                    className="h-sub h-mono h-ellipsis"
                    title={g.copies[0]?.argv.join(" ")}
                  >
                    {g.copies[0]?.argv.join(" ") || "Command unavailable"}
                  </p>
                  {g.verdict === "Expected" && (
                    <p className="h-sub">
                      Intentionally temporary · not expected after restart
                    </p>
                  )}
                </div>
                <Ports ports={g.ports} />
                <div className="h-wrap">
                  <button
                    className="h-button h-small h-tinted"
                    onClick={() => adopt(g)}
                  >
                    Adopt…
                  </button>
                  <button className="h-button h-small" onClick={() => stop(g)}>
                    Stop
                  </button>
                  <button
                    className="h-button h-small"
                    disabled={busy}
                    onClick={() => ignore(g)}
                    title={
                      g.verdict === "Expected"
                        ? "Include this command group in missing-agent warnings again"
                        : "Remember that this command group does not need a launch agent"
                    }
                    aria-label={`${g.verdict === "Expected" ? "Undo temporary" : "Mark temporary"} for ${g.name}`}
                  >
                    {g.verdict === "Expected"
                      ? "Undo temporary"
                      : "Mark temporary"}
                  </button>
                </div>
              </div>
            ))}
          </section>
        )}
        {kinds.map(([kind, title]) => {
          const rows = visible.filter(
            (x) =>
              x.kind === kind ||
              (kind === "other" &&
                !["web", "ai", "data", "infra"].includes(x.kind))
          )
          return (
            rows.length > 0 && (
              <section key={kind} className="h-service-section">
                <h2>
                  <i className={`h-dot h-kind-${kind}`} />
                  {filter === "apple" ? `${title} · system` : title}
                  <small>{rows.length}</small>
                </h2>
                <div className="h-card h-table-card">
                  <div className="h-service-row h-table-head">
                    <span>Service</span>
                    <span>Launchd jobs</span>
                    <span>Ports</span>
                    <span>Health</span>
                    <span>Uptime</span>
                    <span>Actions</span>
                  </div>
                  {rows.map((service) => (
                    <ServiceRow
                      key={service.id}
                      service={service}
                      onJob={onJob}
                      action={action}
                      open={open}
                      pin={pin}
                    />
                  ))}
                </div>
              </section>
            )
          )
        })}
        {filter === "all" && s.app_owned.length > 0 && (
          <section>
            <h2 className="h-section-title">
              Started by apps{" "}
              <small>They manage themselves — every port has an owner.</small>
            </h2>
            <div className="h-app-grid">
              {s.app_owned
                .filter((g) =>
                  [g.name, ...g.ports.map((p) => String(p.port))]
                    .join(" ")
                    .toLowerCase()
                    .includes(search.toLowerCase())
                )
                .map((g) => (
                  <div className="h-card h-app-card" key={g.key}>
                    <ServiceTile icon="cpu" tint="graphite" />
                    <div>
                      <strong>{g.name}</strong>
                      <Ports ports={g.ports} />
                    </div>
                  </div>
                ))}
            </div>
          </section>
        )}
        {!visible.length &&
          !(unmanaged.length && ["all", "unmanaged"].includes(filter)) && (
            <div className="h-empty">
              <Search size={26} />
              <h2>No matching services</h2>
              <p>Try a different name, port or filter.</p>
            </div>
          )}
        {s.diagnostics.warnings.length > 0 && <details className="h-card h-extensions"><summary>Scan diagnostics · {s.diagnostics.warnings.length} warnings</summary><pre>{s.diagnostics.warnings.join("\n")}</pre></details>}
        <p className="h-footer-note">
          {s.counts.hidden} Apple and application jobs hidden by default.{" "}
          <button onClick={() => setFilter("apple")}>
            Show Apple & System
          </button>
          <br />
          Non-root inventory: macOS may hide other users’ processes and sockets.
        </p>
      </div>
    </>
  )
}
