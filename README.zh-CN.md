# Application Snapshot

**简体中文** · [English](README.md)

界面与 macOS .app 显示名为「应用快照」。

截取当前应用窗口，写入系统剪贴板。不落盘、不上传。约 60 秒后清空；期间若又复制了别的内容，则跳过这次清空。

实现目录：[macos/](macos/)、[windows/](windows/)、[linux/](linux/)。

默认快捷键（可在设置里改）：

| | macOS | Windows / Linux |
|---|---|---|
| 截屏 | Option-Shift-2 | Alt+Shift+2 |
| 录制 | Option-Shift-R | Alt+Shift+R |

## 从源码运行

macOS：

```bash
./macos/scripts/build-app.sh
```

Windows：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\scripts\build.ps1
```

Linux：

```bash
cd linux
./scripts/build-linux.sh
```

推 `v*` tag 会在三系统打包并生成 `SHA256SUMS.txt`，创建 draft + pre-release。当前无签名、无公证。

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
