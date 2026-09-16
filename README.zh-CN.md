# Application Snapshot

**简体中文** · [English](README.md)

界面与 macOS 的「应用快照.app」仍显示「应用快照」。

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

Windows 10 或 11，构建需要 .NET 10 SDK（运行自包含 exe 不必再装 .NET）：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\build.ps1
```

Linux 需要 Rust 工具链；`curl` / `ffmpeg` / `zenity` 等按功能另装，见 [linux/README.md](linux/README.md)：

```bash
cd linux
./build-linux.sh
```

桌宠素材不随包分发，目录空则不能启用。路径详见 [windows/](windows/)、[linux/](linux/) README。

推 `v*` tag 会在三系统打包并生成 `SHA256SUMS.txt`，创建 draft + pre-release。当前无签名、无公证。

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
