# Application Snapshot

[简体中文](README.zh-CN.md) · **English**

Captures the current application window to the clipboard, keeps a local snapshot history, polishes prompts through an OpenAI-compatible endpoint, extracts text with the system OCR engine, and puts an animated companion on the desktop.

## Repository layout

The **Tauri 2 app at the repository root is the whole application**. It is one codebase for all three platforms: React front end in [src/](src/), native side in [src-tauri/](src-tauri/). The earlier per-platform implementations (Swift, C#, Rust) were removed once the mainline covered them; they are still in the git history.

## Platform capabilities

System capabilities sit behind a shared interface with one adapter per platform, so the calling code does not change between systems.

| | Windows | macOS | Linux |
|---|---|---|---|
| Window capture | xcap | xcap | xcap |
| Window recording | Windows.Graphics.Capture + Media Foundation | Native ScreenCaptureKit window stream + AVAssetWriter | portal ScreenCast + PipeWire → ffmpeg (Wayland); ffmpeg `x11grab` (X11) |
| Text recognition | `Windows.Media.Ocr` | Vision via a `snapshot-ocr` helper | `tesseract` |
| Snapshot history, companion, prompt polishing | yes | yes | yes |

**Device verification**: Windows is the primary verified platform (adapters, tray, shortcuts, capture/recording on real hardware). macOS and Linux capabilities are mainly validated via code paths, compile checks, and adapters; the macOS `snapshot-ocr` helper and some window adapters (capture, icons, Accessibility unminimize) have seen device testing. macOS recording now uses a native ScreenCaptureKit window stream and writes H.264 MP4 through AVAssetWriter. Linux `cargo check` is restored after fixing compile blockers; X11 paths (xcap / x11grab / tesseract / xdotool / XDG autostart) follow the adapter implementation. Wayland portal ScreenCast, tray click behaviour, and packaged installers may still need local verification.

## Requirements

- **Recording**: Windows uses the built-in Windows.Graphics.Capture + Media Foundation and does not need ffmpeg; macOS uses a bundled ScreenCaptureKit sidecar and does not need external ffmpeg; Linux needs `ffmpeg` on `PATH`. On Linux, still captures that include the cursor also prefer ffmpeg `x11grab` (falling back without the cursor). OCR and polishing do not need ffmpeg.
- **macOS permissions**: window capture and recording need Screen Recording permission; restoring a minimized window before capturing it needs Accessibility permission.
- **Linux OCR** needs `tesseract` plus at least one language pack (`apt install tesseract-ocr tesseract-ocr-chi-sim`).
- **macOS sidecars**: Vision OCR is provided by [src-tauri/snapshot-ocr/](src-tauri/snapshot-ocr/); ScreenCaptureKit recording is provided by [src-tauri/snapshot-recorder/](src-tauri/snapshot-recorder/). Both `npm run tauri dev` and `npm run tauri build` prepare them automatically, and packaged builds place them next to the main executable in `Contents/MacOS/`.
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


## Linux

```bash
# Debian / Ubuntu — build + optional runtime deps
sudo bash scripts/linux/install-deps.sh
bash scripts/linux/check-env.sh

npm install
npm run tauri dev            # development
bash scripts/linux/build.sh  # release binary + deb/AppImage when bundlers succeed
```

| Optional tool | Feature |
|---|---|
| `ffmpeg` | Window recording (X11: `x11grab`; Wayland portal: `rawvideo` encode); cursor stills (falls back without cursor) |
| `tesseract` + language packs | OCR |
| `xdotool` | Restore a minimized target window before capture; scrolling capture paging |
| StatusNotifierHost | System tray (KDE native; GNOME needs an AppIndicator extension) |

Recording picks its backend from the session, not from `$DISPLAY`: a Wayland session always tries portal ScreenCast first, because XWayland leaves `$DISPLAY` set and `x11grab` cannot see native Wayland windows. Only when the portal is unavailable does it fall back to `x11grab`, and the capability line then says that the fallback records the X server's view only. An X11 session goes straight to `x11grab`.

**Autostart** is opt-in via Settings → 开机静默自启动. It writes `~/.config/autostart/com.appsnapshot.prompt-pet-shortcut.desktop`. An **optional** systemd `--user` unit example lives at `scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example` (advanced; never enabled by the app).

More detail: [scripts/linux/README.md](scripts/linux/README.md).

Every push and pull request runs [`.github/workflows/check.yml`](.github/workflows/check.yml): frontend `npm run build`, plus `cargo check` / `cargo test` on Linux (via `scripts/linux/install-deps.sh`), Windows, and macOS.

## Shortcuts

Nothing is bound by default. Snapshot, fullscreen capture, scrolling capture (Linux/X11 only for now), recording, prompt polishing and text extraction can each be given a global shortcut on the Shortcuts page; they fire while the window is minimised.

Post-capture behaviour depends on “after capture” and “auto-save local”: copy to clipboard by default, or open the annotate window / save-as dialog. Local history is written only when auto-save is on. Clipboard auto-clear delay is configurable (about 60 seconds by default; skipped if something else was copied meanwhile). Recordings are written as MP4 to the downloads folder by default; their independent destination can be opened or changed from Shortcuts → Clipboard & saving. Extracted text replaces the clipboard contents.

## Companion assets

Pick a `.zip` full of GIFs on the Companion page, or just pick a single GIF. **GIF is the only accepted format**; anything else inside the archive is skipped.

Video (MP4 / WebM) is not supported for now: playback goes through each platform's embedded WebView, and the three engines accept different codecs — the same pack animates on one machine and shows a blank square on another.

One file per action; a file name containing `idle` becomes the default pose. Assets are read on demand and never copied, so moving the original breaks the companion. Limits: 100MB per archive, 50MB per GIF.

## Release

Pushing a `v*` tag builds all three platforms — the macOS universal app bundle (zipped), the two Windows installers, and Linux `.deb` / `.AppImage` for x86_64 and aarch64 (native runners, no cross-compilation; Linux artifacts resume once the compile blockers fixed in this tree are on the default branch) — writes `SHA256SUMS.txt`, and creates a draft + pre-release. There is no signing and no notarization.

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
