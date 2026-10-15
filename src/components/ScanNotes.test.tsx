import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, expect, it, vi } from "vitest"
import { ScanNotes } from "./ScanNotes"
import { copyText } from "@/lib/harbor"
import type { Snapshot } from "@/types"

vi.mock("@/lib/harbor", () => ({ copyText: vi.fn() }))
afterEach(() => { cleanup(); vi.clearAllMocks() })
const diagnostics: Snapshot["diagnostics"] = {
  duration_ms: 10, libproc_listener_count: 2, lsof_listener_count: 0,
  inaccessible_pids: [], visibility_limited: true, warnings: [],
  inventory_issues: [{ code: "permission_denied", path: "/Library/LaunchDaemons/test.plist",
    message: "macOS denied access to this plist.", guidance: "The job's configuration is unknown.", detail: "Permission denied (os error 13)" }],
}

it("explains partial coverage and copies complete diagnostics for agents", async () => {
  vi.mocked(copyText).mockResolvedValue(undefined)
  render(<ScanNotes diagnostics={diagnostics} takenAt="2026-10-15T15:00:00Z" />)
  fireEvent.click(screen.getByText("Scan coverage · 1 file skipped"))
  expect(screen.getByText(/not confirmed service failures/)).toBeVisible()
  expect(screen.getByText("macOS denied access to this plist.")).toBeVisible()
  expect(screen.getByText(/os error 13/)).not.toBeVisible()
  fireEvent.click(screen.getByRole("button", { name: "Copy scan report" }))
  expect(await screen.findByText("Copied")).toBeVisible()
  expect(JSON.parse(vi.mocked(copyText).mock.calls[0][0]).diagnostics).toEqual(diagnostics)
})

it("keeps older snapshots and genuine scan warnings visible", () => {
  render(<ScanNotes diagnostics={{ ...diagnostics, inventory_issues: undefined, warnings: ["launchctl unavailable"] }} takenAt="now" />)
  fireEvent.click(screen.getByText("Scan coverage · 1 warning"))
  expect(screen.getByText("launchctl unavailable")).toBeVisible()
  expect(screen.queryByText(/files skipped/)).not.toBeInTheDocument()
})
