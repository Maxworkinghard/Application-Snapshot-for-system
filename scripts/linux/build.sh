#!/usr/bin/env bash
# Build the Tauri mainline for the current Linux host.
# Produces installers under src-tauri/target/release/bundle/ (deb / appimage when available).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "error: Linux packages must be built on Linux" >&2
  exit 1
fi

need() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "error: missing '$1'. Run scripts/linux/install-deps.sh and install Node/Rust." >&2
    exit 1
  fi
}

need npm
need cargo
need pkg-config

if ! pkg-config --exists webkit2gtk-4.1; then
  echo "error: webkit2gtk-4.1 not found. Run: sudo bash scripts/linux/install-deps.sh" >&2
  exit 1
fi

echo "==> npm install"
npm install

echo "==> tauri build"
npm run tauri build

echo ""
echo "==> Artifacts (if bundlers succeeded):"
find src-tauri/target/release/bundle -maxdepth 3 -type f \( -name '*.deb' -o -name '*.AppImage' -o -name 'snapshot' \) 2>/dev/null | sed 's/^/  /' || true
echo "Binary: src-tauri/target/release/snapshot"
