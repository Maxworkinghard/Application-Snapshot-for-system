# Application Snapshot

[简体中文](README.zh-CN.md) · **English**

Captures the current application window to the clipboard, keeps a local snapshot history, polishes prompts through an OpenAI-compatible endpoint, extracts text with the system OCR engine, and puts an animated companion on the desktop.

## Repository layout

The **Tauri 2 app at the repository root is the mainline**. It is one codebase for all three platforms: React front end in [src/](src/), native side in [src-tauri/](src-tauri/).

[macos/](macos/) (Swift), [windows/](windows/) (C#) and [linux/](linux/) (Rust) are the earlier per-platform implementations. They are **kept as reference only** and no longer receive new features. Where they already wrap a mature system capability, the mainline calls that capability through a platform adapter rather than rewriting it in Rust.

## Platform capabilities

System capabilities sit behind a shared interface with one adapter per platform, so the calling code does not change between systems.

| | Windows | macOS | Linux |
|---|---|---|---|
| Window capture | xcap | xcap | xcap |
| Window recording | ffmpeg `gdigrab` | not wired up yet — ScreenCaptureKit | ffmpeg `x11grab` (X11); portal ScreenCast + PipeWire → ffmpeg (pure Wayland) |
| Text recognition | `Windows.Media.Ocr` | not wired up yet — Vision via a `snapshot-ocr` helper | `tesseract` |
| Snapshot history, companion, prompt polishing | yes | yes | yes |

Only the Windows adapters have been exercised extensively on a real machine. Linux adapters compile on this codebase and cover X11 recording, portal ScreenCast recording for pure Wayland (needs a real graphical session to E2E), tesseract OCR, XDG autostart (opt-in; optional systemd user unit is documented only), and best-effort app icons. macOS recording/OCR helpers are not finished yet.

## Requirements

- **Recording** needs `ffmpeg` on `PATH`. Capture, OCR and polishing do not.
- **Linux OCR** needs `tesseract` plus at least one language pack (`apt install tesseract-ocr tesseract-ocr-chi-sim`).
- **macOS OCR** needs a `snapshot-ocr` helper on `PATH`. Vision has no built-in command line entry point, so the mainline shells out to a small Swift bridge: reads a PNG on stdin and writes one line of text per recognised line to stdout; `--probe` reports the recognition language. The helper is not written yet.
- **Prompt polishing** needs an OpenAI-compatible endpoint, configured under Settings. The API key goes to the OS keychain, never to a config file.

## Run from source

```bash
npm install
npm run tauri dev
```

Production build:

```bash
npm run build          # front end only
npm run tauri build    # installer
```

The reference implementations build on their own:

```bash
./macos/scripts/build-app.sh
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\scripts\build.ps1
cd linux && ./scripts/build-linux.sh
```


## Linux (Tauri mainline)

The product path on Linux is this Tauri app, not `linux/windowsnap` (that directory is the older reference client still used by the current Release workflow tarball).

```bash
# Debian / Ubuntu — build + optional runtime deps
sudo bash scripts/linux/install-deps.sh
bash scripts/linux/check-env.sh

npm install
npm run tauri dev          # development
bash scripts/linux/build.sh  # release binary + deb/AppImage when bundlers succeed
```

| Optional tool | Feature |
|---|---|
| `ffmpeg` | Window recording (`x11grab` when `$DISPLAY` is set; portal path encodes via ffmpeg `rawvideo`) |
| `tesseract` + language packs | OCR |
| `xdotool` | Restore a minimized target window before capture |
| StatusNotifierHost | System tray (KDE native; GNOME needs an AppIndicator extension) |

**Autostart** is opt-in via Settings → 开机静默自启动. It writes `~/.config/autostart/com.appsnapshot.prompt-pet-shortcut.desktop`. An **optional** systemd `--user` unit example lives at `scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example` (advanced; never enabled by the app).

More detail: [scripts/linux/README.md](scripts/linux/README.md).

## Shortcuts

Nothing is bound by default. Snapshot, recording, prompt polishing and text extraction can each be given a global shortcut on the Shortcuts page; they fire while the window is minimised.

A snapshot goes to the clipboard and into the local history. The clipboard entry is cleared after about 60 seconds, unless something else was copied in the meantime. Recordings are written to the downloads folder as MP4. Extracted text replaces the clipboard contents.

## Companion asset packs

The companion reads a `.zip` chosen on the Companion page. GIF, WebP, APNG and PNG are accepted, as are MP4 and WebM. A video has to be H.264, HEVC, AV1 or VP9 — the codec is checked on import, and a package whose clips cannot be decoded is rejected by name rather than silently showing a blank companion.

One file per action; a file name containing `idle` becomes the default pose. The archive is read in place and never unpacked, so moving it breaks the companion. Limits: 100MB per package, 50MB per file.

## Release

Pushing a `v*` tag packages on the three systems, writes `SHA256SUMS.txt`, and creates a draft + pre-release. There is no signing and no notarization.

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
