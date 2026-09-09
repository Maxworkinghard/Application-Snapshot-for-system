# 应用快照

跨平台的窗口截图小工具：截取应用窗口并自动复制到系统剪贴板，随后可在任意支持图片的输入框直接粘贴。截图不落盘、不上传，写入剪贴板 60 秒后自动清空（期间复制过别的内容则跳过）。

| 平台 | 技术栈 | 支持架构 | 状态 |
|---|---|---|---|
| macOS（本目录） | Swift + ScreenCaptureKit | Apple Silicon + Intel（universal 2） | ✅ 日常使用中 |
| [Windows](windows/) | .NET Framework 4.0+ WinForms + Win32 | x64 | ✅ Windows 真机运行验证 |
| [Linux](linux/) | Rust + X11 / Wayland portal | x86_64 + aarch64（任意架构可自行编译） | ⚠️ 编译已验证，待真机运行验证 |

## 三端功能对照

| 功能 | macOS | Windows | Linux |
|---|---|---|---|
| 全局快捷键截图 | ✅ `⌥⇧2` | ✅ `Alt+Shift+2` | ✅ `Alt+Shift+2`（X11 / Wayland portal） |
| 窗口录制 MP4 | ✅ `⌥⇧R` | ✅ `Alt+Shift+R` | ✅ ffmpeg（X11；Wayland 无标准接口） |
| 悬浮球（显示上一个前台应用图标，可拖动） | ✅ | ✅ | ✅ X11 |
| 点击悬浮球弹操作菜单 | ✅ | ✅ | ✅ |
| 从窗口列表选择截图 | ✅ | ✅ | ✅ X11 / Wayland 不可用 |
| 提示词润色（剪贴板草稿 → 确认 → 大模型改写 → 写回，处理中可停止） | ✅ | ✅ | ✅ |
| 托盘 / 菜单栏入口 | ✅ 菜单栏 | ✅ 托盘 | ✅ StatusNotifierItem 托盘 |
| 轻量 Toast / 桌面通知 | ✅ | ✅ | ✅ |
| 剪贴板 60 秒自动清空 | ✅ | ✅ | ✅ |

润色默认参数三端一致：`max_tokens = 16384`、`temperature = 0.3`、超时 180 秒；提示词改写规则与流程（确认 → 写回）保持同一份行为约定。

## macOS 版功能

- `⌥⇧2`：立即截取当前应用的最前窗口并复制（PNG + TIFF 双格式），成功时播放系统截屏同款快门声
- `⌥⇧R`：录制当前应用窗口为 MP4（H.264，菜单栏或浮动控制条停止）
- 菜单栏 `润色 Prompt`：改写剪贴板中的提示词草稿，确认后写回剪贴板，处理中可停止
- 菜单栏 `设置保存目录…`：更改录制文件的保存位置（默认「下载」；目录失效时自动回退）
- 菜单栏 `设置快捷键…`：录制并保存自定义全局快捷键
- 菜单栏 `选择其他窗口…`：用 macOS 原生窗口选择方式点选并复制（右键/双指点击取消）
- 桌面小宠物：悬浮圆形图标，显示上一个前台应用，点击可截取/录制/润色，可拖动、位置持久化
- 截图/录制/润色完成显示轻量 Toast 提示
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
