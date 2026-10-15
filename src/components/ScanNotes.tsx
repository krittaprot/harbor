import { useState } from "react"
import { Copy } from "lucide-react"
import { copyText } from "@/lib/harbor"
import type { Snapshot } from "@/types"

export function ScanNotes({ diagnostics, takenAt }: {
  diagnostics: Snapshot["diagnostics"]
  takenAt: string
}) {
  const [copyStatus, setCopyStatus] = useState("")
  const issues = diagnostics.inventory_issues ?? []
  const warnings = diagnostics.warnings
  if (!issues.length && !warnings.length) return null

  async function copyReport() {
    setCopyStatus("Copying…")
    try {
      await copyText(JSON.stringify({ report: "Harbor scan coverage", taken_at: takenAt, diagnostics }, null, 2))
      setCopyStatus("Copied")
    } catch (error) {
      setCopyStatus(`Copy failed: ${String(error)}`)
    }
  }

  return (
    <details className="h-card h-extensions h-scan-notes">
      <summary>
        Scan coverage{issues.length > 0 && ` · ${issues.length} ${issues.length === 1 ? "file" : "files"} skipped`}
        {warnings.length > 0 && ` · ${warnings.length} ${warnings.length === 1 ? "warning" : "warnings"}`}
      </summary>
      {issues.length > 0 && <p className="h-sub">
        Inventory is partial. Harbor continued scanning, but could not inspect these files.
        These are file-access or configuration issues, not confirmed service failures.
      </p>}
      <div className="h-wrap">
        <button className="h-button h-small" disabled={copyStatus === "Copying…"} onClick={() => void copyReport()}>
          <Copy size={13} /> Copy scan report
        </button>
        {copyStatus && <span className="h-sub" role="status">{copyStatus}</span>}
      </div>
      {issues.map((issue, index) => (
        <section className="h-scan-note" key={`${issue.path}:${issue.code}:${index}`}>
          <strong>{issue.message}</strong>
          <p className="h-mono h-sub">{issue.path}</p>
          <p>{issue.guidance}</p>
          <details>
            <summary>Technical details</summary>
            <pre>{issue.code}{"\n"}{issue.detail}</pre>
          </details>
        </section>
      ))}
      {warnings.length > 0 && <pre>{warnings.join("\n")}</pre>}
    </details>
  )
}
