#!/bin/sh
set -eu

# Install the CLI only. The source checkout stays on its original volume.
cd "$(dirname "$0")/.."
prefix="${HARBOR_INSTALL_PREFIX:-$HOME/.local}"
destination="$prefix/bin/harbor"
force=false
case "${1:-}" in
  --force) force=true ;;
  "") ;;
  *) echo "Usage: sh scripts/install-cli.sh [--force]" >&2; exit 2 ;;
esac
if [ -e "$destination" ] || [ -L "$destination" ]; then
  if [ "$force" != true ]; then
    echo "$destination already exists. Use --force to replace it." >&2
    exit 1
  fi
  # Never follow an existing symlink and overwrite an unrelated executable.
  if [ -d "$destination" ]; then
    echo "Refusing to replace a directory: $destination" >&2
    exit 1
  fi
fi
cargo build --manifest-path src-tauri/Cargo.toml --release --locked --bin harbor
mkdir -p "$prefix/bin"
temporary=$(mktemp "$prefix/bin/.harbor-install.XXXXXX")
trap 'rm -f "$temporary"' EXIT HUP INT TERM
install -m 755 "${CARGO_TARGET_DIR:-src-tauri/target}/release/harbor" "$temporary"
mv -f "$temporary" "$destination"
echo "Installed $destination"
case ":$PATH:" in
  *":$prefix/bin:"*) ;;
  *) echo "Add $prefix/bin to PATH to use harbor by name." ;;
esac
