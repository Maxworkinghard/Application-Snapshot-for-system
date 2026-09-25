# Application Snapshot

[![Check](https://github.com/Maxworkinghard/Application-Snapshot-for-system/actions/workflows/check.yml/badge.svg)](https://github.com/Maxworkinghard/Application-Snapshot-for-system/actions/workflows/check.yml)

[简体中文](README.zh-CN.md) | English

A small desktop tool for window screenshots and screen recording, with prompt polishing and a desktop companion on the side. Built with Tauri 2 + React; one codebase for Windows, macOS and Linux.

The interface is Chinese only for now, so menu names below are followed by their Chinese labels.

## Features

- **Window snapshots**: capture a chosen application window straight to the clipboard, or open it for annotation or save it as a file. Fullscreen capture is also available, plus scrolling capture on Linux (X11).
- **Window recording**: record a window to MP4, optionally with system audio and the microphone.
- **Snapshot history**: captures are kept locally (the latest 200) and can be copied again at any time.
- **Prompt polishing**: rewrite prompts through any OpenAI-compatible endpoint, with rewrite rules you can add and remove.
- **Desktop companion**: a small floating window that shows the current app's icon by default, or an animated GIF of your choice. Right-click it to open the input box, from which you can capture, record and polish.
- **Global shortcuts**: every action above can be bound to a global shortcut, which also works while the main window is minimized.

## Platform support

| Feature | Windows | macOS | Linux |
|---|:-:|:-:|:-:|
| Window and fullscreen capture | ✓ | ✓ | ✓ |
| Window recording | ✓ | ✓ | ✓ |
| System audio while recording | ✓ | ✓ | ✓ |
| Microphone while recording | ✓ | macOS 15+ | ✓ |
| Scrolling capture | — | — | X11 only |
| Launch at login | ✓ | ✓ | ✓ |
| Snapshot history, prompt polishing, companion | ✓ | ✓ | ✓ |

Availability is detected at runtime. On Linux, recording and audio rely on external tools; see [Runtime dependencies](#runtime-dependencies).

Windows has had the most testing on real hardware. Some features on macOS and Linux still need to be checked on real devices; see [docs/pending-device-verification.md](docs/pending-device-verification.md) (in Chinese).

## Installation

Download the package for your platform from [Releases](https://github.com/Maxworkinghard/Application-Snapshot-for-system/releases). Windows (x64 / ARM64) and Linux (x86_64 / aarch64) packages are available; macOS follows later. You can also [build from source](#build-from-source).

System requirements:

- **macOS**: 14 or later; one universal package for Apple Silicon and Intel
- **Windows**: 10 or 11, x64 or ARM64
- **Linux**: x86_64 or aarch64 with an X11 or Wayland desktop session; the deb package for Ubuntu 22.04 / Debian 12 or later, the AppImage for other distributions with glibc 2.35 or later. Ubuntu 20.04 and older are not supported

The packages are not signed or notarized. On macOS, Control-click the app and choose Open the first time; on Windows, SmartScreen may block the installer.

### Permissions

On macOS, capture and recording need Screen Recording permission, capturing a minimized window needs Accessibility permission (to restore the window first), and recording the microphone needs Microphone permission.

### Runtime dependencies

Windows and macOS need nothing extra (on Windows 10 the installer downloads the WebView2 runtime if needed). On Linux, install these as needed:

| Tool | Used for |
|---|---|
| `ffmpeg` | Window recording; screenshots that include the mouse pointer (without it, screenshots have no pointer) |
| `pactl` (PulseAudio or PipeWire-Pulse) | Recording system audio and the microphone; ffmpeg must also be built with PulseAudio support |
| `xdotool` | Restoring a minimized window before capture; scrolling capture |
| StatusNotifierHost | System tray (built into KDE; GNOME needs the AppIndicator extension) |

## Usage

Once started, the companion appears on the desktop (showing the current app's icon by default) and the app has an icon in the system tray.

- **Input box**: right-click the companion to open it. From here you can capture a window, capture the full screen, record a window, polish the text on the clipboard, or copy the last snapshot again. Type to filter commands; paste a block of text to start polishing it straight away.
- **Shortcuts**: nothing is bound by default. On the Shortcuts (快捷操作) page you can bind a global shortcut to window snapshot, fullscreen snapshot, window recording, prompt polishing and opening the input box, plus scrolling capture on Linux.
- **Capture**: by default a capture goes to the clipboard, which is cleared after 60 seconds (unless you copied something else in the meantime). In Preferences (偏好设置) you can switch to opening the annotation window or a save dialog instead. The clear delay, image format and snapshot folder are under Shortcuts → Clipboard & saving (快捷操作 → 剪贴板与保存).
- **Recording**: recordings are saved as MP4 in your Downloads folder by default. The folder and whether to include system audio and the microphone are under Shortcuts → Recording (快捷操作 → 录制).
- **Prompt polishing**: first enter the endpoint URL, model and API key under Preferences → Machine & model (偏好设置 → 本机与模型). The API key is stored in the OS keychain, never in a config file. Rewrite rules are managed under Preferences → Polishing rules (偏好设置 → 润色规则).
- **Companion**: on the Companion (桌面伴侣) page, import a `.zip` of GIFs or a single GIF. Use one file per action; a file whose name contains `idle` becomes the default pose. Only GIF is supported and other files in the archive are skipped (each platform's WebView plays different video codecs, so video is not supported for now). Limits: 100 MB per archive, 50 MB per GIF.
- **Appearance**: the Themes (主题库) page switches the layout, color scheme and motion.
- **Launch at login**: turn on 开机时静默启动 in Preferences (偏好设置).

## Build from source

Requirements:

- Node.js 22
- Rust 1.98.1 (picked up automatically from `rust-toolchain.toml`)
- The [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your platform
- On macOS, a Swift toolchain (Xcode or the Command Line Tools) to build the recording sidecar
- On Debian / Ubuntu, `sudo bash scripts/linux/install-deps.sh` installs the build and runtime dependencies

```bash
npm install
npm run tauri dev      # development
npm run tauri build    # installer
```

The macOS recording sidecar ([src-tauri/snapshot-recorder/](src-tauri/snapshot-recorder/)) is built automatically before `tauri dev` and `tauri build`. Linux build and environment-check scripts are described in [scripts/linux/README.md](scripts/linux/README.md).

### Tests

```bash
npm test          # front-end unit tests
npm run build     # type check + front-end build

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Every push and pull request runs [check.yml](.github/workflows/check.yml): the front-end tests and build, plus the Rust checks on Windows, macOS and Linux.

## Project layout

```text
src/                    Front end (React + TypeScript)
├── pages/              Main window pages
└── windows/            Companion, input box, annotation and other separate windows
src-tauri/              Native side (Rust)
├── src/os/<platform>/  Per-platform native code: recording, window control, launch at login, etc.
└── snapshot-recorder/  macOS recording sidecar (Swift + ScreenCaptureKit)
scripts/                Build scripts; linux/ holds Linux dependency and packaging scripts
docs/                   Documentation
```

### Implementation principles

Capabilities the operating system owns (recording, window control and so on) are implemented separately on each platform with its native APIs, rather than settling for a lowest common denominator for the sake of uniform code. Everything that does not depend on the OS (settings, history, polishing, UI) is shared.

Recording is how this came about: all three platforms once shared ffmpeg, and on Windows `gdigrab` recorded hardware-accelerated windows as black frames. Each platform now uses:

| Platform | Recording |
|---|---|
| Windows | Windows.Graphics.Capture + Media Foundation |
| macOS | ScreenCaptureKit window stream + AVAssetWriter |
| Linux | portal ScreenCast + PipeWire → ffmpeg in Wayland sessions; ffmpeg `x11grab` in X11 sessions |

Linux picks the backend from the session type, not from `$DISPLAY`: XWayland sets `$DISPLAY`, but `x11grab` cannot see native Wayland windows, so a Wayland session falls back to `x11grab` only when the portal is unavailable.

Conversely, where platforms differ only in API names and not in results, a mature cross-platform library is used. Window capture, for example, uses xcap everywhere, which is itself three native implementations.

Every extra implementation is another place for bugs that only reproduce on one kind of machine, and behavior drifts between platforms over time. Verification, not implementation, is the real bottleneck: a native path never tried on real hardware is not necessarily more reliable than a portable one that has been.

## Releases

Pushing a `v*` tag runs [release.yml](.github/workflows/release.yml). It builds on native runners for each platform (macOS universal, Windows x64 / ARM64, Linux x86_64 / aarch64 deb and AppImage), writes `SHA256SUMS.txt`, and creates a draft pre-release to be checked by hand before publishing. The Linux packages are built on Ubuntu 22.04 ([linux-packages.yml](.github/workflows/linux-packages.yml)), then installed and launched headless on both 22.04 and 24.04 before they reach the draft. The tag, the `VERSION` file and the version in `src-tauri/tauri.conf.json` must match.

## License

No license has been chosen yet and the repository has no `LICENSE` file, so all rights are reserved by default.
