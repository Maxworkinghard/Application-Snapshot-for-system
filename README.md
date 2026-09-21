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
| Window recording | ffmpeg `gdigrab` | ffmpeg `avfoundation` (main screen, cropped to the window) | ffmpeg `x11grab` |
| Text recognition | `Windows.Media.Ocr` | Vision via a `snapshot-ocr` helper | `tesseract` |
| Snapshot history, companion, prompt polishing | yes | yes | yes |

The Windows adapters, the macOS `snapshot-ocr` helper, and the macOS window adapters (capture, previous-app icon via `NSRunningApplication`, minimized-window restore via the Accessibility API) have been exercised on a real machine. macOS recording is wired via ffmpeg `avfoundation` and compile-tested, but has not been live-tested end to end. The Linux adapter has not been compiled or run yet.

## Requirements

- **Recording** needs `ffmpeg` on `PATH`. Capture, OCR and polishing do not.
- **macOS permissions**: window capture and recording need Screen Recording permission; restoring a minimized window before capturing it needs Accessibility permission.
- **Linux OCR** needs `tesseract` plus at least one language pack (`apt install tesseract-ocr tesseract-ocr-chi-sim`).
- **macOS OCR** needs a `snapshot-ocr` helper on `PATH`. Vision has no built-in command line entry point, so the mainline shells out to a small Swift bridge ([src-tauri/snapshot-ocr/](src-tauri/snapshot-ocr/)): reads a PNG on stdin and writes one line of text per recognised line to stdout; `--probe` reports the recognition language. Build with `cd src-tauri/snapshot-ocr && swift build -c release`, then copy the binary onto `PATH` (e.g. `~/.local/bin`).
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

## Shortcuts

Nothing is bound by default. Snapshot, recording, prompt polishing and text extraction can each be given a global shortcut on the Shortcuts page; they fire while the window is minimised.

A snapshot goes to the clipboard and into the local history. The clipboard entry is cleared after about 60 seconds, unless something else was copied in the meantime. Recordings are written to the downloads folder as MP4. Extracted text replaces the clipboard contents.

## Companion asset packs

The companion reads a `.zip` chosen on the Companion page. GIF, WebP, APNG and PNG are accepted, as are MP4 and WebM. A video has to be H.264, HEVC, AV1 or VP9 (H.264 or HEVC on macOS — WKWebView's codec support varies by system version) — the codec is checked on import, and a package whose clips cannot be decoded is rejected by name rather than silently showing a blank companion.

One file per action; a file name containing `idle` becomes the default pose. The archive is read in place and never unpacked, so moving it breaks the companion. Limits: 100MB per package, 50MB per file.

## Release

Pushing a `v*` tag packages on the three systems, writes `SHA256SUMS.txt`, and creates a draft + pre-release. There is no signing and no notarization.

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
