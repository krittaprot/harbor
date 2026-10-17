import assert from "node:assert/strict"
import { execFileSync } from "node:child_process"
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import path from "node:path"
import { test } from "node:test"

const script = path.resolve(import.meta.dirname, "prepare-release-branch.sh")

test("prepares a branch, preserves maintainer commits on rerun, and surfaces real push errors", () => {
  const root = mkdtempSync(path.join(tmpdir(), "harbor-release-"))
  const remote = path.join(root, "remote.git")
  const local = path.join(root, "checkout")
  mkdirSync(local)
  const git = (...args: string[]) => execFileSync("git", args, { cwd: local, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).trim()
  const files = ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"]
  const env = { ...process.env, VERSION: "1.5.3", GITHUB_REPOSITORY: "example/harbor", GITHUB_STEP_SUMMARY: path.join(root, "summary"), RUNNER_TEMP: root }
  const prepare = () => execFileSync("bash", [script], { cwd: local, env, stdio: "pipe" })
  try {
    git("init", "--bare", remote)
    git("init", "-b", "harbor/main")
    git("config", "user.name", "Release test")
    git("config", "user.email", "test@example.invalid")
    git("remote", "add", "origin", remote)
    mkdirSync(path.join(local, "src-tauri"))
    for (const file of files) writeFileSync(path.join(local, file), "old version\n")
    git("add", ".")
    git("commit", "-m", "Initial")
    git("push", "origin", "harbor/main")
    for (const file of files) writeFileSync(path.join(local, file), "new version\n")
    writeFileSync(path.join(root, "release-notes.md"), "Release notes fixture\n")
    prepare()
    assert.equal(git("branch", "--show-current"), "release/v1.5.3")
    assert.match(readFileSync(env.GITHUB_STEP_SUMMARY, "utf8"), /compare\/harbor\/main\.\.\.release\/v1\.5\.3/)
    assert.match(readFileSync(env.GITHUB_STEP_SUMMARY, "utf8"), /Type: Release/)
    writeFileSync(path.join(local, "maintainer.txt"), "Keep this work")
    git("add", ".")
    git("commit", "-m", "Maintainer work")
    git("push", "origin", "HEAD")
    const before = git("ls-remote", "origin", "refs/heads/release/v1.5.3")
    prepare()
    assert.equal(git("ls-remote", "origin", "refs/heads/release/v1.5.3"), before)
    git("switch", "harbor/main")
    env.VERSION = "1.5.4"
    for (const file of files) writeFileSync(path.join(local, file), "another version\n")
    writeFileSync(path.join(remote, "hooks/pre-receive"), "#!/bin/sh\nexit 1\n", { mode: 0o755 })
    assert.throws(prepare, /Command failed/)
    assert.equal(git("ls-remote", "origin", "refs/heads/release/v1.5.4"), "")
    git("remote", "set-url", "origin", path.join(root, "missing.git"))
    assert.throws(prepare, /Command failed/)
  } finally {
    rmSync(root, { recursive: true, force: true })
  }
})
