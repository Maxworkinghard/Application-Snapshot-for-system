#!/usr/bin/env bash
# Install build + optional runtime dependencies for the Tauri mainline on Debian/Ubuntu.
# Idempotent. Does NOT enable any autostart.
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
  tesseract-ocr \
  tesseract-ocr-eng \
  tesseract-ocr-chi-sim \
  librsvg2-bin

echo ""
echo "==> System packages ready."
echo "Also need: Node.js 20+ (npm) and Rust (https://rustup.rs)."
echo "Then: npm install && npm run tauri dev"
