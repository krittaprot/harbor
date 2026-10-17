# Harbor update and release maintenance

## User flow

Settings → Software updates checks the stable release at
`https://github.com/krittaprot/harbor/releases/latest/download/latest.json`.
An available release is shown with its version and plain-text release notes. Installation
and restart are separate, explicit actions. Nothing is downloaded on app startup.

The native updater verifies the archive against the public key embedded in `tauri.conf.json`.
Only a newer version for the current architecture is offered. Checks time out after 30 seconds,
downloads after five minutes. The backend owns the selected update, rejects concurrent
operations and mismatched version requests, and blocks installation in debug/smoke builds or
outside `/Applications` and `~/Applications`. Quit requests are deferred during replacement.
Failed checks clear stale metadata; failed downloads can be retried. An installed update can
be restarted without downloading again. Configuration is outside the app bundle and is preserved.

## Signing key

GitHub Actions uses repository secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. The public key is committed; the private key and password
must stay outside the repository and must be backed up securely. Losing the key prevents
existing installations from trusting future updates. Do not rotate it without a migration plan.
Updater signing is separate from Apple Developer ID signing/notarization.

## Publishing

1. Run **Create Release PR** on `harbor/main` with `patch`, `minor` or `major`.
2. Update the changelog/release notes and wait for CI. The PR body becomes the release notes.
3. Merge the release PR with the **Type: Release** label.
4. The release workflow creates a draft, builds both Mac architectures, signs and uploads the
   updater archives, then checks that both platform entries, signatures, URLs and versions match.
   The validator pins tauri-action's floating `latest` asset URLs to the exact release tag
   (including bundle-specific target aliases) and fills the manifest's release notes from the PR.
   It uploads `SHA256SUMS` and publishes only after all assets pass validation.

If repository settings prohibit Actions from opening a PR, the workflow may still create
`release/vX.Y.Z`. Open that branch's PR with an authenticated maintainer account, targeting
`harbor/main`, and apply **Type: Release**. There is no need to change branch protections.

Never put signing material in workflow YAML, PR descriptions, logs, or fixtures. The fixture
in `src-tauri/src/fixtures/updater.json` uses a separate disposable test key whose private
half was destroyed; it cannot sign production updates.

## Verification

- Frontend tests cover explicit check/install/restart, duplicate clicks, navigation during
  download, offline checks, signature failures, stale metadata and installation restrictions.
- Rust integration tests use a loopback server and temporary fixture bundle to verify a
  signed download and replacement, reject a tampered archive before replacement, and reject
  older releases. Insecure HTTP is enabled only in the isolated test runtime.
- Release asset validation tests reject wrong versions, missing architectures, changed URLs,
  mismatched signatures and missing/empty assets.
- Native smoke acceptance checks the Settings card and update-status IPC; smoke builds cannot
  replace an installed app. A real future-version upgrade should also be checked on an installed
  Mac, including administrator prompts and restart behavior.
