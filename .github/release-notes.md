# 应用快照 {{VERSION}}

按机器选下面其中一个附件。

## 下载哪一份

| 你的机器 | 附件 |
|---|---|
| macOS（Apple Silicon 或 Intel） | `Application-Snapshot-{{VERSION}}-macos-universal.zip` |
| Windows x64（Intel / AMD） | `Application-Snapshot-{{VERSION}}-windows-x64.exe` |
| Windows ARM64（骁龙本等） | `Application-Snapshot-{{VERSION}}-windows-arm64.exe` |
| Linux x86_64，Debian / Ubuntu 系 | `Application-Snapshot-{{VERSION}}-linux-x86_64.deb` |
| Linux x86_64，其它发行版 | `Application-Snapshot-{{VERSION}}-linux-x86_64.AppImage` |
| Linux aarch64，Debian / Ubuntu 系 | `Application-Snapshot-{{VERSION}}-linux-aarch64.deb` |
| Linux aarch64，其它发行版 | `Application-Snapshot-{{VERSION}}-linux-aarch64.AppImage` |

没有 32 位 Windows 包。Mac 两种芯片是**同一份** zip，不要找第二个 Mac 包。Linux 同一架构的 deb 与 AppImage 是同一个程序的两种装法，装一个就够。

校验：`SHA256SUMS.txt`。

## 安装

- **macOS**：解压后把 `snapshot.app` 拖到「应用程序」。当前构建是 ad-hoc 签名、未公证；若系统提示无法验证开发者，按住 Control 点击 → 打开。第一次截图会要屏幕录制权限。
- **Windows**：运行对应架构的安装程序。需要 Microsoft Edge WebView2 运行时，Windows 11 自带，Windows 10 上安装程序会按需下载。未用 Trusted Root 代码签名证书签过名时，Smart App Control / SmartScreen 可能拦截。
- **Linux（deb）**：`sudo apt install ./Application-Snapshot-{{VERSION}}-linux-<架构>.deb`，依赖由 apt 一并装上。
- **Linux（AppImage）**：`chmod +x` 之后直接运行。部分发行版需要先装 `libfuse2`。

## 可选依赖

| 功能 | 需要 |
|---|---|
| 窗口录制 | `ffmpeg`（三端通用；Linux 上 X11 走 x11grab，纯 Wayland 走 portal ScreenCast） |
| 文字识别 | Linux 需要 `tesseract` 加至少一个语言包（`tesseract-ocr-chi-sim` 等）；macOS 用系统 Vision，Windows 用系统 OCR，都不必另装 |
| 截图前还原最小化窗口 | Linux 需要 `xdotool` |
| 系统托盘 | Linux 需要 StatusNotifierHost（KDE 原生支持；GNOME 需装 AppIndicator 扩展） |

桌宠素材不包含在发行包里，放好后再到设置里切换桌面形式。

## 系统要求

- macOS 14+
- Windows 10 或 Windows 11
- Linux：X11 或 Wayland 桌面会话；deb 需要 Debian 12 / Ubuntu 24.04 及以上（webkit2gtk-4.1）
