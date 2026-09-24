#!/usr/bin/env bash
# Report whether this Linux box can build/run the Tauri mainline.
# Safe to run without installing anything.
set -euo pipefail

ok() { printf '  [OK]  %s\n' "$*"; }
miss() { printf '  [!!]  %s\n' "$*"; }
info() { printf '  [--]  %s\n' "$*"; }

echo "Application Snapshot — Linux environment check"
echo "=============================================="

echo "Display"
info "DISPLAY=${DISPLAY:-<unset>}"
info "WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-<unset>}"
info "XDG_SESSION_TYPE=${XDG_SESSION_TYPE:-<unset>}"
if [[ -n "${DISPLAY:-}" ]]; then ok "X11 display present (x11grab recording possible)"
elif [[ -n "${WAYLAND_DISPLAY:-}" ]]; then info "Wayland without DISPLAY — will try portal ScreenCast (needs xdg-desktop-portal + PipeWire)"
else miss "No DISPLAY / WAYLAND_DISPLAY"
fi

echo "Build tools"
command -v rustc >/dev/null && ok "rustc $(rustc --version | awk '{print $2}')" || miss "rustc missing (rustup.rs)"
command -v cargo >/dev/null && ok "cargo" || miss "cargo missing"
command -v npm >/dev/null && ok "npm $(npm -v)" || miss "npm missing"
command -v pkg-config >/dev/null && ok "pkg-config" || miss "pkg-config missing"
if command -v pkg-config >/dev/null && pkg-config --exists webkit2gtk-4.1; then
  ok "webkit2gtk-4.1"
else
  miss "webkit2gtk-4.1 (scripts/linux/install-deps.sh)"
fi

echo "Optional runtime"
command -v ffmpeg >/dev/null && ok "ffmpeg (recording)" || miss "ffmpeg (recording disabled until installed)"
command -v xdotool >/dev/null && ok "xdotool (restore minimized windows)" || miss "xdotool (optional)"
command -v rsvg-convert >/dev/null && ok "rsvg-convert (SVG app icons)" || info "rsvg-convert optional"

echo "Session services"
if dbus-send --session --dest=org.freedesktop.DBus --type=method_call --print-reply /org/freedesktop/DBus org.freedesktop.DBus.ListNames >/dev/null 2>&1; then
  ok "session D-Bus reachable"
else
  miss "session D-Bus weak/unavailable — tray / notifications / portal may fail; app should still start"
fi
if gdbus introspect --session --dest org.freedesktop.portal.Desktop --object-path /org/freedesktop/portal/desktop 2>/dev/null | grep -q ScreenCast; then
  ok "xdg-desktop-portal ScreenCast interface present"
else
  info "ScreenCast portal not visible here (install xdg-desktop-portal + desktop backend for pure Wayland recording)"
fi
if pgrep -x pipewire >/dev/null 2>&1 || pgrep -x pipewire-pulse >/dev/null 2>&1; then
  ok "PipeWire process seen"
else
  info "PipeWire not running in this environment (needed for portal recording)"
fi

echo ""
echo "Mainline entry: npm run tauri dev   (repo root)"
echo "Reference client (linux/windowsnap) is NOT the product path — see README."
