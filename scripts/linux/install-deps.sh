#!/usr/bin/env bash
# Install build + optional runtime dependencies for the Tauri mainline on Debian/Ubuntu.
# Idempotent. Does NOT enable any autostart.
#
# Aimed at Ubuntu 22.04+ / Debian 12 (bookworm)+ (webkit2gtk 4.1 + soup3); CI runs it on both 22.04 and 24.04,
# and releases are built on 22.04 so the packages also run there. Explicitly lists
# libsoup-3.0-dev / libjavascriptcoregtk-4.1-dev even though webkit often pulls them,
# so a minimal CI image does not miss pkg-config files during cargo check.
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "error: this script is for Linux only" >&2
  exit 1
fi

if ! command -v apt-get >/dev/null 2>&1; then
  echo "error: apt-get not found. On Fedora/Arch see scripts/linux/README.md for package names." >&2
  exit 1
fi

export DEBIAN_FRONTEND=noninteractive

sudo apt-get update
sudo apt-get install -y -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold \
  build-essential \
  clang \
  libclang-dev \
  curl \
  wget \
  file \
  pkg-config \
  libwebkit2gtk-4.1-dev \
  libjavascriptcoregtk-4.1-dev \
  libsoup-3.0-dev \
  libgtk-3-dev \
  librsvg2-dev \
  patchelf \
  libssl-dev \
  libayatana-appindicator3-1 \
  libayatana-appindicator3-dev \
  libpipewire-0.3-dev \
  libgbm-dev \
  libwayland-dev \
  xdotool \
  ffmpeg \
  librsvg2-bin

echo ""
echo "==> System packages ready."
echo "Also need: Node.js 22+ (npm; distro packages are often 18/20 — too old for npm test) and Rust (https://rustup.rs)."
echo "Then: npm install && npm run tauri dev"
