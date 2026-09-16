# Application Snapshot

[简体中文](README.zh-CN.md) · **English**

Captures the current application window and copies it to the system clipboard. Nothing is written to disk or uploaded. The clipboard entry is cleared after about 60 seconds, unless something else was copied in the meantime.

Implementations: [macos/](macos/), [windows/](windows/), [linux/](linux/).

Default shortcuts (changeable in settings):

| | macOS | Windows / Linux |
|---|---|---|
| Capture | Option-Shift-2 | Alt+Shift+2 |
| Record | Option-Shift-R | Alt+Shift+R |

## Build from source

macOS:

```bash
./macos/scripts/build-app.sh
```

Windows:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\scripts\build.ps1
```

Linux:

```bash
cd linux
./scripts/build-linux.sh
```

Pushing a `v*` tag packages on the three systems, writes `SHA256SUMS.txt`, and creates a draft + pre-release. There is no signing and no notarization.

There is no `LICENSE` file. No license has been added; copyright is reserved by default.
