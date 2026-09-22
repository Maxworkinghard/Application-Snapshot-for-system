# Linux build helpers (Tauri mainline)

These scripts target the **repository-root Tauri app**, not `linux/windowsnap`.

| Script | Purpose |
|---|---|
| `install-deps.sh` | Debian/Ubuntu packages for compiling + optional runtime (`ffmpeg`, `tesseract`, `xdotool`); also used by `.github/workflows/check.yml` / `release.yml` on ubuntu-24.04 |
| `check-env.sh` | Non-destructive environment report |
| `build.sh` | `npm install` + `npm run tauri build` |
| `com.appsnapshot.prompt-pet-shortcut.service.example` | **Optional** systemd `--user` unit (advanced; not enabled by default) |

```bash
sudo bash scripts/linux/install-deps.sh   # once
bash scripts/linux/check-env.sh
bash scripts/linux/build.sh
```

Dev loop from the repo root:

```bash
npm install
npm run tauri dev
```

## Autostart（开机自启）

**Supported / primary:** The Settings toggle **开机静默自启动** writes an XDG desktop entry under
`~/.config/autostart/` when enabled, and removes it when disabled.
Nothing is installed by default (same opt-in pattern as the Windows UI).

### Optional: systemd `--user` unit (advanced)

Only if you deliberately prefer a user service instead of (or in addition to carefully avoiding double-start with) XDG. The app **never** enables this for you.

1. Copy the example and set `ExecStart=` to your real binary:

```bash
mkdir -p ~/.config/systemd/user
cp scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example \
   ~/.config/systemd/user/com.appsnapshot.prompt-pet-shortcut.service
# edit ExecStart= to e.g. /usr/bin/snapshot or …/src-tauri/target/release/snapshot
systemctl --user daemon-reload
systemctl --user enable --now com.appsnapshot.prompt-pet-shortcut.service
```

2. Disable later:

```bash
systemctl --user disable --now com.appsnapshot.prompt-pet-shortcut.service
```

If Settings XDG autostart is already on, leave the unit disabled so the app does not start twice.

## Wayland vs X11

| Capability | X11 / XWayland (`$DISPLAY` set) | Pure Wayland (no `$DISPLAY`) |
|---|---|---|
| Window capture (`xcap`) | yes | compositor / portal dependent |
| Recording | ffmpeg `x11grab` (window rect; honors `includeCursor`) | **portal ScreenCast** (xdg-desktop-portal) → PipeWire frames → ffmpeg `rawvideo`; user picks source in the portal UI |
| Global shortcuts (Tauri plugin) | yes | compositor-dependent |
| System tray | StatusNotifierHost | same; GNOME needs AppIndicator extension |
| OCR (`tesseract`) | yes | yes |

Portal recording needs a working session bus, `xdg-desktop-portal` + a desktop backend (gtk/gnome/kde/wlr), and PipeWire. Without a real graphical Wayland session the portal path feature-detects and returns a clear bilingual error. `saveDir` is honored on both paths. Portal `includeCursor` follows the compositor/portal default (xcap’s ScreenCast helper does not expose `cursor_mode` yet).

## Reference client

`linux/` still packages the older Rust `windowsnap` binary for GitHub Releases.
New features belong in the Tauri mainline. See `linux/README.md`.
