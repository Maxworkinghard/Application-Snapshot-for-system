# Application Snapshot

[简体中文](README.zh-CN.md) · **English**

Captures the current application window and copies it to the system clipboard. Nothing is written to disk or uploaded. The clipboard entry is cleared after about 60 seconds, unless something else was copied in the meantime.

- **macOS** ([macos/](macos/)): Swift 6 + ScreenCaptureKit. One universal 2 binary (arm64 + x86_64). Packaging scripts exist; there are no automated tests.
- **Windows** ([windows/](windows/)): .NET 10 WinForms, self-contained exe. Separate native builds for x64 and ARM64; `build.ps1` checks the PE machine type. Runtime on ARM hardware is not recorded in this repo.
- **Linux** ([linux/](linux/)): Rust. The release script emits `x86_64` and `aarch64` tarballs. It has not been run on a real Linux desktop.

Default shortcuts (changeable in settings):

| | macOS | Windows / Linux |
|---|---|---|
| Capture | Option-Shift-2 | Alt+Shift+2 |
| Record | Option-Shift-R | Alt+Shift+R |

Recording on Linux uses ffmpeg x11grab and is X11-only. Wayland limits for capture, shortcuts, and the desktop pet are in [linux/README.md](linux/README.md).

## Repository layout

```
macos/      Swift 6 app (ScreenCaptureKit)
windows/    .NET 10 WinForms app
linux/      Rust app
.github/    release workflow and release-notes template
```

## Build from source

macOS 14 or later, with Xcode and a Swift 6 toolchain:

```bash
./macos/scripts/build-app.sh
```

Windows 10 or 11. Building requires the .NET 10 SDK; the published exe is self-contained:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\scripts\build.ps1
```

Linux needs a Rust toolchain. Optional runtime tools (`curl`, `ffmpeg`, `zenity`, …) are listed in [linux/README.md](linux/README.md):

```bash
cd linux
./scripts/build-linux.sh
```

Pet assets are not shipped. An empty directory means pet mode cannot be enabled. Paths are in the [windows/](windows/) and [linux/](linux/) READMEs.

Pushing a `v*` tag packages on the three systems, writes `SHA256SUMS.txt`, and creates a draft + pre-release. There is no signing and no notarization.

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
