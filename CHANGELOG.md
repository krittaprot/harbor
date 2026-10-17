# Changelog

## Unreleased

### Changed

- Launch Harbor in the menu bar only, without a Dock/Command-Tab entry or an
  automatically opened dashboard. Open Harbor and Settings remain available from
  the menu-bar icon; closing the dashboard keeps monitoring active.
- Friendlier update feedback: an animated checking indicator, a green “You’re all set!”
  confirmation, and a distinct update-ready state with version and installation action.
  Successful checks show their time; errors cannot appear as a successful check.
  Animations respect Reduce Motion.

### Release tooling

- Normalize tauri-action's floating updater URLs to version-pinned GitHub URLs and
  include the release notes in the updater manifest before publishing.

## [1.5.2](docs/releases/v1.5.2.md) — 2026-10-17

### Added

- Settings → Software updates: check GitHub Releases, review release notes, download
  and install a signed update, then restart Harbor when ready.
- Download progress, signature verification, retryable errors, installation-location
  checks, and protection against duplicate installs or quitting during app replacement.
- Signed Apple Silicon and Intel updater archives, a validated `latest.json`, and
  SHA-256 checksums in the release workflow. Install v1.5.2 manually once to enable
  in-app updates for future releases.

### Fixed

- Detect Tailscale status when Harbor is launched from Finder or at login by explicitly
  selecting CLI mode for the Tailscale subprocess. Previously, the bundled Tailscale
  executable could return a GUI startup error instead of JSON, leaving status Unknown.
- Refresh system status on explicit Rescan requests instead of waiting for the
  background system-status cache to expire (up to 60 seconds).

### Internal

- Share bounded subprocess capture and its default timeout between ordinary helpers
  and helpers with child-only environment overrides.
- Cover child environment isolation with a regression test.
- Exercise explicit Rescan in native smoke acceptance, and select Services even when
  a saved restart report opens Restart Readiness at startup.

## Previous releases

- [v1.5.1](docs/releases/v1.5.1.md)
- [v1.5.0](docs/releases/v1.5.0.md)
