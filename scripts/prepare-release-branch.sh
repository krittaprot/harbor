#!/usr/bin/env bash
set -euo pipefail

: "${VERSION:?}" "${GITHUB_REPOSITORY:?}" "${GITHUB_STEP_SUMMARY:?}" "${RUNNER_TEMP:?}"
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo 'Invalid release version' >&2; exit 1; }
base="${RELEASE_BASE:-harbor/main}"
branch="release/v$VERSION"

# Exit 2 means no matching ref. Authentication/network failures must stay failures.
if git ls-remote --exit-code --heads origin "refs/heads/$branch"; then
  echo "Reusing $branch without changing its commits."
else
  status=$?
  if [ "$status" -ne 2 ]; then exit "$status"; fi
  git switch -c "$branch"
  git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock
  git -c user.name='github-actions[bot]' \
    -c user.email='41898282+github-actions[bot]@users.noreply.github.com' \
    commit -m "chore(release): v$VERSION"
  # Use the runner's ephemeral token via gh; never embed credentials in a URL.
  git -c credential.helper= -c 'credential.helper=!gh auth git-credential' \
    push origin "HEAD:refs/heads/$branch"
fi

{
  echo "## Release branch ready: v$VERSION"
  echo
  echo "[Open the release PR](https://github.com/$GITHUB_REPOSITORY/compare/$base...$branch?expand=1&labels=Type%3A%20Release)"
  echo
  echo 'A maintainer must open the PR and apply **Type: Release**, then wait for CI before merging.'
  echo 'This respects repository permissions and ensures PR CI is triggered. Existing branch commits are never overwritten.'
  echo
  echo '### Suggested release notes'
  cat "$RUNNER_TEMP/release-notes.md"
} >> "$GITHUB_STEP_SUMMARY"
