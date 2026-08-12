# 应用快照

跨平台的窗口截图小工具：按下全局快捷键，截取当前应用的最前窗口并自动复制到系统剪贴板，随后在任意支持图片的输入框直接粘贴。截图不落盘、不上传，写入剪贴板 60 秒后自动清空（期间复制过别的内容则跳过）。

| 平台 | 技术栈 | 支持架构 | 状态 |
|---|---|---|---|
| macOS（本目录） | Swift + ScreenCaptureKit | Apple Silicon + Intel（universal 2） | ✅ 日常使用中 |
| [Windows](windows/) | .NET 8 WPF + Windows.Graphics.Capture | x64 + arm64 | ⚠️ 编译已验证，待真机运行验证 |
| [Linux](linux/) | Rust + X11 / Wayland portal | x86_64 + aarch64（任意架构可自行编译） | ⚠️ 编译已验证，待真机运行验证 |

## macOS 版功能

- `⌥⇧2`：立即截取当前应用的最前窗口并复制（PNG + TIFF 双格式）
- `⌥⇧R`：录制当前应用窗口为 MP4（H.264，菜单栏或浮动控制条停止）
- 菜单栏 `设置保存目录…`：更改录制文件的保存位置（默认「下载」；目录失效时自动回退）
- 菜单栏 `设置快捷键…`：录制并保存自定义全局快捷键
- 菜单栏 `选择其他窗口…`：用 macOS 原生窗口选择方式点选并复制（右键/双指点击取消）
- 桌面小宠物：悬浮圆形图标，显示上一个前台应用，点击可截取/录制该应用窗口，可拖动、位置持久化
- 截图/录制完成显示轻量 Toast 提示
- 剪贴板 60 秒自动清空，期间复制过其他内容则跳过

## macOS 构建与运行

```bash
chmod +x scripts/build-app.sh
./scripts/build-app.sh        # 产出 universal 2（arm64 + x86_64）
open "dist/应用快照.app"
```

- 产物同时支持 Apple Silicon（M 系列）与 Intel 芯片的 Mac
- 有名为 `WindowSnapDev` 的代码签名证书时使用之，否则自动退回 ad-hoc 签名（仅限本机运行；重签后需重新授予屏幕录制权限）
- 开机自启：`./scripts/install-launch-agent.sh`

第一次截取时，macOS 会请求「屏幕与系统音频录制」权限。授权后如果首次截图失败，退出并重新打开应用即可。

## 系统要求

- macOS 14 或更高版本（Apple Silicon 或 Intel）
- Xcode 及 Swift 6 工具链
- Windows / Linux 版要求见各自目录的 README
