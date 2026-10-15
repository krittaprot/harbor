const paths: Record<string, string> = {
  checklist: "M9 6h11M9 12h11M9 18h11M4 6l1 1 2-2M4 12l1 1 2-2M4 18l1 1 2-2",
  timer: "M12 9v4l2.5 2M12 21a8 8 0 1 0 0-16 8 8 0 0 0 0 16zM10 2h4",
  house: "M4 11l8-7 8 7v9H4zM10 20v-5h4v5",
  leaf: "M5 19c0-8 5-14 15-15-1 10-7 15-15 15zM5 19l7-7",
  bubble: "M21 12a8 8 0 0 1-11.6 7.1L4 20l1-4.6A8 8 0 1 1 21 12z",
  egg: "M12 3c3.6 0 6 5.3 6 10a6 6 0 0 1-12 0c0-4.7 2.4-10 6-10z",
  bowl: "M3 11h18a9 9 0 0 1-18 0zM8 7c0-1.5 1-2 1-3.5M12 7c0-1.5 1-2 1-3.5M16 7c0-1.5 1-2 1-3.5",
  book: "M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2zM4 21V5M8 7h7",
  terminal: "M4 5h16v14H4zM8 10l3 2-3 2M13 15h3",
  sparkles:
    "M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8zM19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z",
  cpu: "M7 7h10v10H7zM10 3v4M14 3v4M10 17v4M14 17v4M3 10h4M3 14h4M17 10h4M17 14h4",
  drop: "M12 3c3.5 4.2 6 7.6 6 11a6 6 0 0 1-12 0c0-3.4 2.5-6.8 6-11z",
  globe:
    "M12 3a9 9 0 1 0 0 18 9 9 0 1 0 0-18M3 12h18M12 3c2.5 2.6 3.8 5.6 3.8 9s-1.3 6.4-3.8 9c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3",
  services: "M4 4h16v6H4zM4 14h16v6H4zM8 7h.01M8 17h.01",
  login: "M3 3h7v7H3zM14 3h7v7h-7zM3 14h7v7H3zM17.5 14v7M14 17.5h7",
  restart: "M12 3v8M6.3 6.8a8 8 0 1 0 11.4 0",
  warning: "M12 3l9.5 17h-19zM12 10v4M12 17h.01",
  unloaded: "M6 6l12 12M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18",
  shield: "M12 3l8 3v6c0 4.5-3.4 8.2-8 9-4.6-.8-8-4.5-8-9V6zM8 12l3 3 5-6",
}
export function ServiceTile({
  icon = "terminal",
  tint = "graphite",
  size = "normal",
}: {
  icon?: string
  tint?: string
  size?: "small" | "normal" | "large"
}) {
  return (
    <span className={`h-tile h-tint-${tint} h-tile-${size}`} aria-hidden="true">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.1"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <path d={paths[icon] ?? paths.terminal} />
      </svg>
    </span>
  )
}
export function AnchorMark() {
  return (
    <span className="h-anchor" aria-hidden="true">
      <svg
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
      >
        <circle cx="12" cy="5" r="2.2" />
        <path d="M12 7.2V21M8 11h8M4.5 13.5a7.5 7.5 0 0 0 15 0" />
      </svg>
    </span>
  )
}
