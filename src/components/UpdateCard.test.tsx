import { useState } from "react"
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, beforeEach, expect, it } from "vitest"
import { useUpdates } from "@/hooks/useUpdates"
import { Channel, resetFakeHandlers, setFakeHandler } from "@/test-utils/tauri-mock"
import type { UpdateProgress } from "@/lib/updates"
import { UpdateCard } from "./UpdateCard"

function Harness() {
  const updater = useUpdates()
  const [visible, show] = useState(true)
  return <><button onClick={() => show(v => !v)}>Switch view</button>{visible && <UpdateCard updater={updater} />}</>
}

beforeEach(() => {
  resetFakeHandlers()
  setFakeHandler("update_status", () => ({ current_version: "1.5.2", can_install: true }))
  setFakeHandler("check_update", () => ({ version: "1.5.3", notes: "A signed test update." }))
})
afterEach(cleanup)

it("checks only on request and distinguishes up-to-date from an offline error", async () => {
  let checks = 0
  setFakeHandler("check_update", () => { checks++; return null })
  render(<Harness />)
  await screen.findByText("Harbor 1.5.2 · GitHub Releases")
  expect(checks).toBe(0)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  expect(await screen.findByText("You’re all set!")).toBeVisible()
  expect(screen.getByRole("status")).toHaveTextContent("Harbor 1.5.2 is the latest version")
  expect(screen.getByRole("status").querySelector("time")).toHaveAttribute("datetime")
  setFakeHandler("check_update", () => { throw new Error("Network unavailable") })
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  expect(await screen.findByRole("alert")).toHaveTextContent("Network unavailable")
  expect(screen.queryByText("You’re all set!")).not.toBeInTheDocument()
  expect(screen.queryByText(/Checked at/)).not.toBeInTheDocument()
})

it("requires an explicit install, rejects duplicate clicks and keeps progress across navigation", async () => {
  let installs = 0
  let complete!: () => void
  let channel!: Channel<UpdateProgress>
  setFakeHandler("install_update", args => {
    installs++
    expect(args.version).toBe("1.5.3")
    channel = args.onProgress as Channel<UpdateProgress>
    return new Promise<void>(resolve => { complete = resolve })
  })
  render(<Harness />)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  const install = await screen.findByRole("button", { name: "Download & Install 1.5.3" })
  expect(screen.getByRole("status")).toHaveTextContent("A new update is ready")
  expect(installs).toBe(0)
  fireEvent.click(install)
  fireEvent.click(install)
  expect(installs).toBe(1)
  expect(screen.getByRole("button", { name: "Check for Updates" })).toBeDisabled()
  fireEvent.click(screen.getByText("Switch view"))
  act(() => channel.onmessage({ stage: "downloading", downloaded: 50, total: 100 }))
  fireEvent.click(screen.getByText("Switch view"))
  expect(screen.getByRole("status")).toHaveTextContent("50%")
  act(() => channel.onmessage({ stage: "verifying", downloaded: 0, total: null }))
  expect(screen.getByRole("status")).toHaveTextContent("Verifying")
  await act(async () => complete())
  expect(screen.getByRole("button", { name: "Restart Harbor" })).toBeEnabled()
  expect(screen.queryByRole("button", { name: /Download & Install/ })).not.toBeInTheDocument()
})

it("shows checking feedback until GitHub responds, without claiming success early", async () => {
  let resolve!: (value: null) => void
  setFakeHandler("check_update", () => new Promise<null>(done => { resolve = done }))
  render(<Harness />)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  await waitFor(() => expect(resolve).toBeTypeOf("function"))
  expect(screen.getByRole("button", { name: "Checking…" })).toBeDisabled()
  expect(screen.getByRole("status")).toHaveTextContent("Looking for something new…")
  expect(screen.queryByText("You’re all set!")).not.toBeInTheDocument()
  expect(screen.queryByText(/Checked at/)).not.toBeInTheDocument()
  await act(async () => resolve(null))
  expect(screen.getByRole("status")).toHaveTextContent("You’re all set!")
  expect(screen.getByRole("button", { name: "Check for Updates" })).toBeEnabled()
})

it("keeps failed signature verification retryable without offering a restart", async () => {
  setFakeHandler("install_update", () => { throw new Error("Signature verification failed") })
  render(<Harness />)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  fireEvent.click(await screen.findByRole("button", { name: "Download & Install 1.5.3" }))
  expect(await screen.findByRole("alert")).toHaveTextContent("Signature verification failed")
  expect(screen.getByRole("button", { name: "Download & Install 1.5.3" })).toBeEnabled()
  expect(screen.queryByRole("button", { name: "Restart Harbor" })).not.toBeInTheDocument()
})

it("clears stale update information when a later check fails", async () => {
  render(<Harness />)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  await screen.findByRole("button", { name: "Download & Install 1.5.3" })
  setFakeHandler("check_update", () => { throw new Error("GitHub unavailable") })
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  await screen.findByRole("alert")
  expect(screen.queryByRole("button", { name: /Download & Install/ })).not.toBeInTheDocument()
})

it("allows checks but blocks installation outside a supported release app location", async () => {
  setFakeHandler("update_status", () => ({ current_version: "1.5.2", can_install: false }))
  render(<Harness />)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  expect(await screen.findByRole("button", { name: "Download & Install 1.5.3" })).toBeDisabled()
  expect(screen.getByText(/Development builds and disk images can check only/)).toBeVisible()
})

it("restarts only after successful installation and allows retry if restart fails", async () => {
  let restarts = 0
  setFakeHandler("install_update", () => undefined)
  setFakeHandler("restart_after_update", () => { restarts++; throw new Error("Restart failed") })
  render(<Harness />)
  fireEvent.click(screen.getByRole("button", { name: "Check for Updates" }))
  fireEvent.click(await screen.findByRole("button", { name: "Download & Install 1.5.3" }))
  const restart = await screen.findByRole("button", { name: "Restart Harbor" })
  expect(restarts).toBe(0)
  fireEvent.click(restart)
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Restart failed"))
  expect(screen.getByRole("button", { name: "Restart Harbor" })).toBeEnabled()
  expect(screen.queryByRole("button", { name: /Download & Install/ })).not.toBeInTheDocument()
})
