# Application Snapshot

[![Check](https://github.com/Maxworkinghard/Application-Snapshot-for-system/actions/workflows/check.yml/badge.svg)](https://github.com/Maxworkinghard/Application-Snapshot-for-system/actions/workflows/check.yml)

[简体中文](README.zh-CN.md) · **English**

Captures the current application window to the clipboard, keeps a local snapshot history, polishes prompts through an OpenAI-compatible endpoint, and puts an animated companion on the desktop.

## Repository layout

The **Tauri 2 app at the repository root is the whole application**. It is one codebase for all three platforms: React front end in [src/](src/), native side in [src-tauri/](src-tauri/). The earlier per-platform implementations (Swift, C#, Rust) were removed once the mainline covered them; they are still in the git history.

## Choosing an implementation

**When the platform's own facility is clearly better, implement it separately for that platform rather than bending it to a shared path.** The test is the actual result, not tidy code. Recording went this way: all three platforms once shared ffmpeg, and `gdigrab` recorded hardware-accelerated windows as black frames — a lowest common denominator that was not good enough anywhere. Windows now uses WGC, macOS ScreenCaptureKit, Linux portal/x11grab, and each is better than before.

Conversely, when only the API names differ and the result does not, use a mature cross-platform library or a shared implementation. Window capture uses xcap on all three — which is itself three native implementations, maintained by someone else. Writing our own three would gain almost nothing.

Before adding a capability, ask: **does the OS own this?** If yes, write it per platform. If not (computation, files, network, UI), share it.

The cost is not only the code. Three implementations mean three places a bug can hide, each reproducible only on its own machine, and behaviour drifts (`restore_minimized_window` already means three different things). **Verification, not implementation, is the real bottleneck** — a native path never tried on real hardware is not automatically more reliable than a portable one that has been.

## Platform capabilities

Each platform has its own native implementation behind a shared set of commands. **Capabilities and behaviour differ per platform** — the same button may go through entirely different system APIs, with different edge cases. See the table below, and the "local capabilities" panel in Settings, which each adapter reports at runtime rather than being hard-coded copy.

Only the parts that do not depend on system capabilities are shared: settings, snapshot history, prompt polishing, the UI. Anything the OS itself provides where the platforms genuinely differ — recording, window control — is written separately for each. Picking a lowest-common-denominator implementation for the sake of uniformity produced something that was not good enough anywhere.

| | Windows | macOS | Linux |
|---|---|---|---|
| Window capture | xcap | xcap | xcap |
| Window recording | Windows.Graphics.Capture + Media Foundation | Native ScreenCaptureKit window stream + AVAssetWriter | portal ScreenCast + PipeWire → ffmpeg (Wayland); ffmpeg `x11grab` (X11) |
| Snapshot history, companion, prompt polishing | yes | yes | yes |

**Device verification**: Windows is the primary verified platform (adapters, tray, shortcuts, capture/recording on real hardware). macOS and Linux capabilities are mainly validated via code paths, compile checks, and adapters; some macOS window adapters (capture, icons, Accessibility unminimize) have seen device testing. macOS recording now uses a native ScreenCaptureKit window stream and writes H.264 MP4 through AVAssetWriter. Linux `cargo check` is restored after fixing compile blockers; X11 paths (xcap / x11grab / xdotool / XDG autostart) follow the adapter implementation. Wayland portal ScreenCast, tray click behaviour, and packaged installers may still need local verification.

Items that cannot be verified on the development machine (macOS) are tracked in [docs/pending-device-verification.md](docs/pending-device-verification.md), each with the platform that must check it.

## Requirements

- **Recording**: Windows uses the built-in Windows.Graphics.Capture + Media Foundation and does not need ffmpeg; macOS uses a bundled ScreenCaptureKit sidecar and does not need external ffmpeg; Linux needs `ffmpeg` on `PATH`. On Linux, still captures that include the cursor also prefer ffmpeg `x11grab` (falling back without the cursor). Polishing does not need ffmpeg.
- **Recording a minimized window**: once minimized, the system stops compositing the window and there is nothing to capture. On Windows it is restored first (same as capture); if it cannot be restored the call fails with a clear message instead of leaving an unplayable empty file.
- **macOS permissions**: window capture and recording need Screen Recording permission; restoring a minimized window before capturing it needs Accessibility permission.
- **macOS sidecar**: ScreenCaptureKit recording is provided by [src-tauri/snapshot-recorder/](src-tauri/snapshot-recorder/). Both `npm run tauri dev` and `npm run tauri build` prepare it automatically, and packaged builds place it next to the main executable in `Contents/MacOS/`.
- **Prompt polishing** needs an OpenAI-compatible endpoint, configured under Settings. The API key goes to the OS keychain, never to a config file.

## About the names

The same thing goes by different names in a few places. Noted here so nobody
"unifies" them later:

| Where | Name |
|---|---|
| Repository | `Application-Snapshot-for-system` |
| UI and macOS .app | 应用快照 (Application Snapshot) |
| Executable (`productName`) | `snapshot` |
| Bundle identifier | `com.appsnapshot.prompt-pet-shortcut` |

The `prompt-pet-shortcut` in that last one is an early project name. **It must
not change**: the identifier is the key the OS uses to find the config directory
and keychain entries, so changing it would orphan every installed user's
settings and API key. It stays on purpose, not by oversight.

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
| `xdotool` | Restore a minimized target window before capture; scrolling capture paging |
| StatusNotifierHost | System tray (KDE native; GNOME needs an AppIndicator extension) |

Recording picks its backend from the session, not from `$DISPLAY`: a Wayland session always tries portal ScreenCast first, because XWayland leaves `$DISPLAY` set and `x11grab` cannot see native Wayland windows. Only when the portal is unavailable does it fall back to `x11grab`, and the capability line then says that the fallback records the X server's view only. An X11 session goes straight to `x11grab`.

**Autostart** is opt-in via Settings → 开机静默自启动. It writes `~/.config/autostart/com.appsnapshot.prompt-pet-shortcut.desktop`. An **optional** systemd `--user` unit example lives at `scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example` (advanced; never enabled by the app).

More detail: [scripts/linux/README.md](scripts/linux/README.md).

Every push and pull request runs [`.github/workflows/check.yml`](.github/workflows/check.yml): frontend `npm run build`, plus `cargo check` / `cargo test` on Linux (via `scripts/linux/install-deps.sh`), Windows, and macOS.

## Shortcuts

Nothing is bound by default. Snapshot, fullscreen capture, scrolling capture (Linux/X11 only for now), recording and prompt polishing can each be given a global shortcut on the Shortcuts page; they fire while the window is minimised.

Post-capture behaviour depends on “after capture” and “auto-save local”: copy to clipboard by default, or open the annotate window / save-as dialog. Local history is written only when auto-save is on. Clipboard auto-clear delay is configurable (about 60 seconds by default; skipped if something else was copied meanwhile). Recordings are written as MP4 to the downloads folder by default; their independent destination can be opened or changed from Shortcuts → Clipboard & saving.

## Companion assets

Pick a `.zip` full of GIFs on the Companion page, or just pick a single GIF. **GIF is the only accepted format**; anything else inside the archive is skipped.

Video (MP4 / WebM) is not supported for now: playback goes through each platform's embedded WebView, and the three engines accept different codecs — the same pack animates on one machine and shows a blank square on another.

One file per action; a file name containing `idle` becomes the default pose. Assets are read on demand and never copied, so moving the original breaks the companion. Limits: 100MB per archive, 50MB per GIF.

## Release

Pushing a `v*` tag builds all three platforms — the macOS universal app bundle (zipped), the two Windows installers, and Linux `.deb` / `.AppImage` for x86_64 and aarch64 (native runners, no cross-compilation; Linux artifacts resume once the compile blockers fixed in this tree are on the default branch) — writes `SHA256SUMS.txt`, and creates a draft + pre-release. There is no signing and no notarization.

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
