# 应用快照 {{VERSION}}

按芯片下载**一份**即可。不要下载 GitHub 自动附带的 Source code。

## 下载哪一份

| 你的机器 | 附件 |
|---|---|
| macOS（Apple Silicon 或 Intel） | `Application-Snapshot-{{VERSION}}-macos-universal.zip` |
| Windows x64（Intel / AMD） | `Application-Snapshot-{{VERSION}}-windows-x64.exe` |
| Windows ARM64（骁龙本等） | `Application-Snapshot-{{VERSION}}-windows-arm64.exe` |
| Linux x86_64 | `Application-Snapshot-{{VERSION}}-linux-x86_64.tar.gz` |
| Linux aarch64 | `Application-Snapshot-{{VERSION}}-linux-aarch64.tar.gz` |

没有 32 位 Windows 包。Mac 两种芯片是**同一份** zip，不要找第二个 Mac 包。

校验：`SHA256SUMS.txt`。

## 安装

- **macOS**：解压后把「应用快照.app」拖到「应用程序」。当前构建是 ad-hoc 签名、未公证；若系统提示无法验证开发者，按住 Control 点击 → 打开。第一次截图会要屏幕录制权限。
- **Windows**：运行对应架构的 exe，不必安装 .NET。未用 Trusted Root 代码签名证书签过名时，Smart App Control / SmartScreen 可能拦截。
- **Linux**：解压后把 `windowsnap` 放到 `~/.local/bin/`。可选：把 `windowsnap.service` 装到 `~/.config/systemd/user/`。Linux 包目前只保证能编过，尚未在真实桌面验证。

桌宠素材不包含在发行包里。放好后再到设置里切换桌面形式。

## 系统要求

- macOS 14+
- Windows 10 22H2 或 Windows 11
- Linux：X11 或 Wayland（功能覆盖见仓库 README）
