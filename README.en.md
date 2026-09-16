# 应用快照

[中文](README.md)

Captures the current application window and copies it to the system clipboard. Nothing is written to disk or uploaded. The clipboard entry is cleared after about 60 seconds, unless something else was copied in the meantime.

- **macOS** (this directory): Swift 6 + ScreenCaptureKit. One universal 2 binary (arm64 + x86_64). Packaging scripts exist; there are no automated tests.
- **Windows** ([windows/](windows/)): .NET 10 WinForms, self-contained exe. Separate native builds for x64 and ARM64; `build.ps1` checks the PE machine type. Runtime on ARM hardware is not recorded in this repo.
- **Linux** ([linux/](linux/)): Rust. The release script emits `x86_64` and `aarch64` tarballs. It has not been run on a real Linux desktop.

Default shortcuts (changeable in settings):

| | macOS | Windows / Linux |
|---|---|---|
| Capture | Option-Shift-2 | Alt+Shift+2 |
| Record | Option-Shift-R | Alt+Shift+R |

Recording on Linux uses ffmpeg x11grab and is X11-only. Wayland limits for capture, shortcuts, and the desktop pet are in [linux/README.md](linux/README.md).

## Build from source

macOS 14 or later, with Xcode and a Swift 6 toolchain:

```bash
./scripts/build-app.sh
```

Output: `dist/应用快照.app`. The script fails if the arm64 or x86_64 slice is missing.

Windows 10 or 11. Building requires the .NET 10 SDK; the published exe is self-contained:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\build.ps1
```

Output: `windows/dist/win-x64/AppSnapshot.exe`, `windows/dist/win-arm64/AppSnapshot.exe`.

Linux needs a Rust toolchain. Optional runtime tools (`curl`, `ffmpeg`, `zenity`, …) are listed in [linux/README.md](linux/README.md):

```bash
cd linux
./build-linux.sh
```

Output: `linux/target/release/windowsnap`.

## Release

The version string is in `VERSION` (currently `0.1.0`). Pushing a `v*` tag runs [`.github/workflows/release.yml`](.github/workflows/release.yml): it builds on macOS, Windows, and Ubuntu, writes `SHA256SUMS.txt`, and opens a **draft + pre-release**. It does not publish automatically. Builds are not Authenticode-signed and not notarized.

Asset names (`{ver}` is the version without the `v` prefix):

- `Application-Snapshot-{ver}-macos-universal.zip`
- `Application-Snapshot-{ver}-windows-x64.exe`
- `Application-Snapshot-{ver}-windows-arm64.exe`
- `Application-Snapshot-{ver}-linux-x86_64.tar.gz`
- `Application-Snapshot-{ver}-linux-aarch64.tar.gz`
- `SHA256SUMS.txt`

The Mac zip still contains `应用快照.app`. There is no 32-bit Windows build.

## Data directories

Pet GIFs are not shipped. An empty directory means pet mode cannot be enabled.

| | Config / data | Pet assets |
|---|---|---|
| macOS | `~/Library/Application Support/WindowSnap/` | `…/WindowSnap/pet/<skin>/` |
| Windows | `%APPDATA%\AppSnapshot\` | `%APPDATA%\AppSnapshot\pet\<skin>\` |
| Linux | `~/.config/windowsnap/` | `$XDG_DATA_HOME/windowsnap/pet/<skin>/` (default `~/.local/share/windowsnap/pet/<skin>/`) |

Filenames the code loads: `idle` `waving` `jumping` `failed` `waiting` `running-left` `running-right` (`.gif`).

## Names

| Where | Name |
|---|---|
| UI / `.app` | 应用快照 |
| macOS executable, Swift package | WindowSnap |
| Windows assembly / exe | AppSnapshot |
| Linux binary and config dir | windowsnap |
| GitHub asset prefix | Application-Snapshot |

## License

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
