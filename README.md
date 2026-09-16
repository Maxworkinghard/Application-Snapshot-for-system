# 应用快照

**简体中文** · [English](README.en.md)

截取当前应用窗口，写入系统剪贴板。不落盘、不上传。约 60 秒后清空；期间若又复制了别的内容，则跳过这次清空。

- **macOS**（本目录）：Swift 6 + ScreenCaptureKit。一份 universal 2（arm64 + x86_64）。有打包脚本，无自动化测试。
- **Windows**（[windows/](windows/)）：.NET 10 WinForms，自包含 exe。x64 与 ARM64 各打一份；`build.ps1` 核对 PE。ARM 设备上的运行未记录。
- **Linux**（[linux/](linux/)）：Rust。发行脚本打 `x86_64` 与 `aarch64`。未在真实 Linux 桌面运行过。

默认快捷键（可在设置里改）：

| | macOS | Windows / Linux |
|---|---|---|
| 截屏 | Option-Shift-2 | Alt+Shift+2 |
| 录制 | Option-Shift-R | Alt+Shift+R |

Linux 上录制走 ffmpeg x11grab，只在 X11 可用。Wayland 的截屏、快捷键和桌宠限制见 [linux/README.md](linux/README.md)。

## 从源码运行

macOS 14 或更高，需要 Xcode 与 Swift 6 工具链：

```bash
./scripts/build-app.sh
```

产物：`dist/应用快照.app`。缺 arm64 或 x86_64 切片会失败。

Windows 10 或 11，构建需要 .NET 10 SDK（运行自包含 exe 不必再装 .NET）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\build.ps1
```

产物：`windows/dist/win-x64/AppSnapshot.exe`、`windows/dist/win-arm64/AppSnapshot.exe`。

Linux 需要 Rust 工具链；`curl` / `ffmpeg` / `zenity` 等按功能另装，见 [linux/README.md](linux/README.md)：

```bash
cd linux
./build-linux.sh
```

产物：`linux/target/release/windowsnap`。

## 发行

版本号在 `VERSION`（现为 `0.1.0`）。推送 `v*` tag 会跑 [`.github/workflows/release.yml`](.github/workflows/release.yml)，在 macOS、Windows、Ubuntu 上打包，写出 `SHA256SUMS.txt`，并创建 **draft + pre-release**（不会自动公开）。当前构建没有 Windows Authenticode，也没有 macOS 公证。

附件名（`{ver}` 为去掉 `v` 的版本号）：

- `Application-Snapshot-{ver}-macos-universal.zip`
- `Application-Snapshot-{ver}-windows-x64.exe`
- `Application-Snapshot-{ver}-windows-arm64.exe`
- `Application-Snapshot-{ver}-linux-x86_64.tar.gz`
- `Application-Snapshot-{ver}-linux-aarch64.tar.gz`
- `SHA256SUMS.txt`

Mac zip 里的应用仍叫「应用快照.app」。不提供 32 位 Windows 包。

## 数据目录

桌宠 GIF 不随应用分发。目录为空则不能启用桌宠。

| | 配置 / 数据 | 桌宠素材 |
|---|---|---|
| macOS | `~/Library/Application Support/WindowSnap/` | `…/WindowSnap/pet/<形象>/` |
| Windows | `%APPDATA%\AppSnapshot\` | `%APPDATA%\AppSnapshot\pet\<形象>\` |
| Linux | `~/.config/windowsnap/` | `$XDG_DATA_HOME/windowsnap/pet/<形象>/`（默认 `~/.local/share/windowsnap/pet/<形象>/`） |

源码加载的文件名：`idle` `waving` `jumping` `failed` `waiting` `running-left` `running-right`（`.gif`）。

## 名称

| 出现位置 | 名称 |
|---|---|
| 界面 / `.app` | 应用快照 |
| macOS 可执行文件、Swift 包 | WindowSnap |
| Windows 程序集 / exe | AppSnapshot |
| Linux 二进制、配置目录 | windowsnap |
| GitHub 附件前缀 | Application-Snapshot |

## 许可

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
