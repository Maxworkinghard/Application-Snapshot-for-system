# 应用快照 {{VERSION}}

按机器选下面其中一个附件。

## 下载哪一份

| 你的机器 | 附件 |
|---|---|
| macOS（Apple Silicon 或 Intel） | `Application-Snapshot-{{VERSION}}-macos-universal.zip` |
| Windows x64（Intel / AMD） | `Application-Snapshot-{{VERSION}}-windows-x64.exe` |
| Windows ARM64（骁龙本等） | `Application-Snapshot-{{VERSION}}-windows-arm64.exe` |

没有 32 位 Windows 包。Mac 两种芯片是**同一份** zip，不要找第二个 Mac 包。

Linux 这一版不发附件，请从源码构建（见仓库 README）。

校验：`SHA256SUMS.txt`。

## 安装

- **macOS**：解压后把 `snapshot.app` 拖到「应用程序」。当前构建是 ad-hoc 签名、未公证；若系统提示无法验证开发者，按住 Control 点击 → 打开。第一次截图会要屏幕录制权限。
- **Windows**：运行对应架构的安装程序。需要 Microsoft Edge WebView2 运行时，Windows 11 自带，Windows 10 上安装程序会按需下载。未用 Trusted Root 代码签名证书签过名时，Smart App Control / SmartScreen 可能拦截。

录制功能需要 `ffmpeg` 在 `PATH` 上，截图、OCR、润色不需要。桌宠素材不包含在发行包里，放好后再到设置里切换桌面形式。

## 系统要求

- macOS 14+
- Windows 10 或 Windows 11
