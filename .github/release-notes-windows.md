# 应用快照 {{VERSION}}

这一版先发 Windows，macOS 和 Linux 版稍后发布。

## 下载哪一份

| 你的机器 | 附件 |
|---|---|
| Windows x64（Intel / AMD） | `snapshot-{{VERSION}}-windows-x64.exe` |
| Windows ARM64（骁龙本等） | `snapshot-{{VERSION}}-windows-arm64.exe` |

没有 32 位 Windows 包。

校验：`SHA256SUMS.txt`。

## 安装

运行对应架构的安装程序，装在当前用户下，不需要管理员权限。需要 Microsoft Edge WebView2 运行时，Windows 11 自带，Windows 10 上安装程序会按需下载。

安装包没有用受信任的代码签名证书签过名，Smart App Control / SmartScreen 可能会拦截；SmartScreen 提示时点「更多信息」→「仍要运行」。

装好后不需要另装别的东西。伴侣的 GIF 形象不随安装包提供，装好后在主窗口「桌面伴侣」页导入。

## 系统要求

- Windows 10 或 Windows 11，x64 或 ARM64
