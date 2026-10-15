import { act, fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react"
import { afterEach, beforeEach, expect, it, vi } from "vitest"
import { AdoptSheet } from "./AdoptSheet"
import { adoptApply, adoptPreview, copyText, diagnoseTarget, type AdoptionProgress } from "@/lib/harbor"
import type { UnmanagedGroup } from "@/types"

vi.mock("@/lib/harbor", () => ({ adoptApply: vi.fn(), adoptPreview: vi.fn(), copyText: vi.fn(), diagnoseTarget: vi.fn() }))
const group: UnmanagedGroup = { key: "fixture", name: "fixture", copies: [], ports: [], verdict: "Unknown", app_path: null, hint: null }
beforeEach(() => {
  vi.clearAllMocks()
  HTMLElement.prototype.scrollTo = vi.fn()
  vi.mocked(adoptPreview).mockResolvedValue("<plist>fixture</plist>")
  vi.mocked(diagnoseTarget).mockResolvedValue({ findings: ["test finding"] })
  vi.mocked(copyText).mockResolvedValue(undefined)
})
afterEach(cleanup)

it("keeps progress visible, prevents duplicate submission, and summarizes rollback errors", async () => {
  let send!: (p: AdoptionProgress) => void
  let fail!: (error: string) => void
  vi.mocked(adoptApply).mockImplementation((_key, _label, _options, _xml, progress) => {
    send = progress!
    return new Promise((_resolve, reject) => { fail = reject })
  })
  const done = vi.fn()
  render(<AdoptSheet group={group} close={vi.fn()} done={done} />)
  const button = screen.getByRole("button", { name: "Adopt & Load" })
  await waitFor(() => expect(button).toBeEnabled())
  fireEvent.click(button)
  expect(screen.getByRole("button", { name: "Adopting…" })).toBeDisabled()
  expect(screen.queryByRole("button", { name: "Close" })).not.toBeInTheDocument()
  act(() => send({ stage: "ready", message: "Waiting for a listening port (up to 15s)", warnings: false }))
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "3")
  expect(screen.getByText("Waiting for a listening port (up to 15s)")).toBeVisible()
  act(() => send({ stage: "rollback", message: "Unloading the new agent…", warnings: false }))
  await act(async () => fail("The new agent did not become ready within 15 seconds.\nOriginal processes were not stopped. The new agent was unloaded.\nPlist retained for review.\nlaunchctl diagnostics: fixture"))
  expect(screen.getByText("Adoption did not finish")).toBeVisible()
  expect(screen.getByRole("alert")).toHaveTextContent("Original processes were not stopped")
  expect(screen.getByText("Technical details & next steps").closest("details")).not.toHaveAttribute("open")
  expect(screen.getByRole("button", { name: "Cancel" })).toBeEnabled()
  expect(screen.getByRole("button", { name: "Retry adoption" })).toBeEnabled()
  expect(adoptApply).toHaveBeenCalledTimes(1)
  expect(done).toHaveBeenCalledTimes(1)
  fireEvent.click(screen.getByRole("button", { name: "Copy report" }))
  expect(await screen.findByText("Copied", { exact: true })).toBeVisible()
  const copied = vi.mocked(copyText).mock.calls[0][0]
  expect(copied).toContain("launchctl diagnostics: fixture")
  expect(copied).toContain("test finding")
  expect(copied).toContain("<plist>fixture</plist>")
})

it("blocks adoption when the preview detects a different existing plist", async () => {
  vi.mocked(adoptPreview).mockRejectedValue("A different LaunchAgent already exists for this label.\nNothing was overwritten. Choose another label.")
  render(<AdoptSheet group={group} close={vi.fn()} done={vi.fn()} />)
  expect(await screen.findByRole("alert")).toHaveTextContent("Nothing was overwritten")
  expect(screen.getByRole("button", { name: "Adopt & Load" })).toBeDisabled()
  expect(adoptApply).not.toHaveBeenCalled()
})

it("distinguishes a ready agent with original-process cleanup warnings from full success", async () => {
  vi.mocked(adoptApply).mockImplementation(async (_key, _label, _options, _xml, progress) => {
    progress?.({ stage: "complete", message: "Some originals could not be stopped.", warnings: true })
    return ["fixture loaded and ready", "PID 123 is still running after SIGTERM; no force kill was sent"]
  })
  render(<AdoptSheet group={group} close={vi.fn()} done={vi.fn()} />)
  const button = screen.getByRole("button", { name: "Adopt & Load" })
  await waitFor(() => expect(button).toBeEnabled())
  fireEvent.click(button)
  expect(await screen.findByText("Adopted · needs review")).toBeVisible()
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "4")
  expect(screen.getByRole("button", { name: "Done" })).toBeEnabled()
})
