# Linux build helpers

Scripts for building and running the Tauri app at the repository root on Linux.

| File | Purpose |
|---|---|
| `install-deps.sh` | Installs the build dependencies and the optional runtime tools (`ffmpeg`, `xdotool`) on Debian / Ubuntu. CI (`check.yml`, `release.yml`) uses it too. |
| `check-env.sh` | Reports what this machine can build and run. Changes nothing. |
| `build.sh` | `npm install` + `npm run tauri build` |
| `com.appsnapshot.snapshot.service.example` | Optional systemd `--user` unit (advanced; never enabled by the app) |

```bash
sudo bash scripts/linux/install-deps.sh   # once
bash scripts/linux/check-env.sh
bash scripts/linux/build.sh               # release binary, plus deb / AppImage when the bundlers succeed
```

For day-to-day development, run from the repository root:

```bash
npm install
npm run tauri dev
```

On Fedora, Arch and other distributions, install the equivalents of the [Tauri Linux prerequisites](https://v2.tauri.app/start/prerequisites/#linux), plus the runtime tools below.

## Runtime tools

| Tool | Used for |
|---|---|
| `ffmpeg` | Window recording (both backends below); screenshots that include the mouse pointer |
| `pactl` (`pulseaudio-utils`; also works with PipeWire-Pulse) | Recording system audio and the microphone; ffmpeg must be built with PulseAudio support |
| `xdotool` | Restoring a minimized window before capture; scrolling capture |
| StatusNotifierHost | System tray (built into KDE; GNOME needs the AppIndicator extension) |

## X11 vs Wayland

The app goes by the **session type**, not by whether `$DISPLAY` is set: a session counts as Wayland when `WAYLAND_DISPLAY` is set or `XDG_SESSION_TYPE=wayland`. GNOME and KDE Wayland sessions usually run XWayland, so `$DISPLAY` is set there as well, but `x11grab` cannot see native Wayland windows.

| Capability | X11 session | Wayland session |
|---|---|---|
| Window capture (xcap) | yes | depends on the compositor / portal |
| Recording | ffmpeg `x11grab` on the window's rectangle; follows the pointer setting | portal ScreenCast → PipeWire → ffmpeg; you pick the window or screen in the portal dialog, and the compositor decides whether the pointer is drawn |
| Recording without a portal | — | falls back to `x11grab` if XWayland is running, which only sees X11 windows (native Wayland windows come out black) |
| Restore a minimized window | `xdotool` | X11 windows only (`xdotool`); restore native Wayland windows by hand |
| Scrolling capture | yes (`xdotool`) | no |
| Global shortcuts | yes | depends on the compositor |
| System tray | StatusNotifierHost | StatusNotifierHost |

Portal recording needs a session D-Bus, `xdg-desktop-portal` with a desktop backend (gtk / gnome / kde / wlr), and PipeWire. Without them, and without XWayland to fall back to, recording fails with an error saying what is missing. Both backends save to the folder set under Shortcuts → Recording (快捷操作 → 录制).

## Launch at login

Turn on 开机时静默启动 in Preferences (偏好设置). The app then writes `~/.config/autostart/com.appsnapshot.snapshot.desktop`, and removes it when the option is turned off. Nothing is installed by default.

### Optional: systemd `--user` unit

Only if you deliberately prefer a user service. The app never enables it for you. If XDG autostart is already on, leave the unit disabled so the app does not start twice.

```bash
mkdir -p ~/.config/systemd/user
cp scripts/linux/com.appsnapshot.snapshot.service.example \
   ~/.config/systemd/user/com.appsnapshot.snapshot.service
# edit ExecStart= to the real binary, e.g. /usr/bin/snapshot or …/src-tauri/target/release/snapshot
systemctl --user daemon-reload
systemctl --user enable --now com.appsnapshot.snapshot.service
```

To disable it later:

```bash
systemctl --user disable --now com.appsnapshot.snapshot.service
```
