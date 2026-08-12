# 应用快照 (Windows 版)

macOS 版的 Windows 移植版。常驻任务栏，按下全局快捷键截取当前应用最前窗口并复制到剪贴板；同时支持桌面悬浮小宠物形态。

> **状态**：源码已在 macOS 上用 .NET 8 SDK 交叉编译验证（`win-x64` 与 `win-arm64` 均编译通过，`EnableWindowsTargeting`），**尚未在真实 Windows 上运行验证**。运行时行为的待确认点见文末。

## 系统要求

- Windows 10 版本 2004 (Build 19041) 或更高 / Windows 11
- .NET 8 Desktop Runtime（**x64 机器装 x64 版，ARM 笔记本装 arm64 版**）
- 屏幕捕获权限（首次截图时系统会弹）

## 与 macOS 版的功能对应

| macOS | Windows |
|---|---|
| Carbon `RegisterEventHotKey` | Win32 `user32!RegisterHotKey` + 隐藏 `HwndSource` 收 `WM_HOTKEY` |
| `ScreenCaptureKit` (`SCScreenshotManager`) | `Windows.Graphics.Capture` + 系统自带 `SoftwareBitmap`/`BitmapEncoder`（不依赖 Win2D） |
| `NSPasteboard` (PNG + TIFF) | WPF `DataObject`：`SetImage`（CF_DIB，兼容一切）+ `"PNG"` 流（保透明） |
| `NSPasteboard.changeCount` 自动清空 | `GetClipboardSequenceNumber`（60 秒后未被覆盖才清空） |
| `NSStatusBar.statusItem` | `H.NotifyIcon.Wpf` TaskbarIcon |
| `NSPanel` 桌面宠物 | WPF 透明 borderless `Window` + `Popup`（拖动用 `DragMove`） |
| `NSVisualEffectView` Toast | WPF 透明 borderless `Window` + DispatcherTimer 1.8s |
| `NSWorkspace.didActivateApplication` | `user32!SetWinEventHook(EVENT_SYSTEM_FOREGROUND)` |
| Bundle 单实例 | 命名 `Mutex` 单实例保护 |

## 构建步骤

1. 安装 [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0)
2. （可选）把图标放进 `WindowSnap.Wpf/Resources/`：
   - `camera-viewfinder.ico`（256×256 多尺寸最佳；**缺资源也能构建运行**，托盘/宠物用默认外观）
3. 在 `windows/` 目录下运行：

   ```powershell
   .\build.ps1              # 跟随本机架构（x64 或 arm64）
   .\build.ps1 -Arch arm64  # 指定架构
   .\build.ps1 -Arch all    # 两个架构都出包
   ```

4. 构建产物在 `windows\dist\WindowSnap-<arch>\WindowSnap.exe`

> ARM 版 Windows 也能通过模拟跑 x64 包，但原生 arm64 包的热键响应和截图性能更好。

## 运行

```
windows\dist\WindowSnap-x64\WindowSnap.exe
```

首次启动：
- 任务栏右下角会出现一个相机图标
- 屏幕右下角出现悬浮小宠物（圆形）
- 按下 `Alt+Shift+2` 触发首次截图 → 若系统弹屏幕捕获权限 → 授权后重试

## 默认快捷键

`Alt+Shift+2`（与 macOS 版 `⌥⇧2` 对应），可在托盘菜单 → 设置快捷键… 里录制自定义组合。

## 行为说明

- 截图写入剪贴板为 **CF_DIB 位图 + PNG 流**双格式：微信/QQ/浏览器认 DIB，支持 PNG 格式的应用取 PNG 保透明
- 写入后启动 60 秒倒计时，到点自动清空（期间你复制了别的内容则跳过）
- 多次截图时倒计时重置
- 小宠物显示**上一个前台过的应用**的图标；点击宠物弹气泡，点按钮截取该 app 的窗口
- 双开会弹"已在运行"提示后退出

## 已修复的历史问题（v0.1 初版代码）

初版源码从未编译过，本轮已修复并通过交叉编译验证：

- 12 处编译错误（不存在的 API、`IntPtr?` 传参、多字符 char 字面量、缺 using、可见性不一致等）
- `Direct3D11CaptureFrame` 在 `FrameArrived` 回调里被提前 `Dispose`，后续读 `Surface` 必崩
- `Direct3D11CaptureFramePool.Create` 在无 WinRT DispatcherQueue 的 WPF 线程上收不到帧 → 改 `CreateFreeThreaded`
- `Clipboard.SetData("PNG", byte[])` 会被 .NET 序列化污染（且 .NET 8 禁用 BinaryFormatter 直接抛异常）→ 改 `MemoryStream` + `SetImage`
- 60 秒自动清空的序号逻辑自相矛盾（未挂监听时会误删用户剪贴板，挂了监听又被自己的写入干扰永不清空）→ 改 `GetClipboardSequenceNumber`
- 宠物拖动把物理像素赋给 DIP 属性（100% 缩放拖不动，高 DPI 下窗口乱飞）→ 改 `DragMove`
- Toast/宠物用 WinForms `Screen`（物理像素）定位 WPF 窗口（DIP）→ 高 DPI 屏错位 → 改 `SystemParameters`
- 图标资源缺失时启动崩溃 → 容错
- 移除 Win2D 依赖（其 1.x 版本面向 WinAppSDK，在纯 WPF 工程存在运行时风险）→ 全部用系统自带 WinRT API

## 仍需在真实 Windows 上验证的点

1. `GraphicsCaptureItem.As<IGraphicsCaptureItemInterop>()` 互操作路径（CsWinRT 文档标准写法，编译已过，运行待验）
2. 首帧是否能在 2 秒超时内到达（`CreateFreeThreaded` + `FrameArrived`）
3. 粘贴兼容性：微信 / QQ / 浏览器 / Office 能否粘出图
4. H.NotifyIcon.Wpf 2.1.0 在 win-arm64 上的托盘行为
5. 60 秒自动清空、宠物拖动/位置持久化、高 DPI（150%/200%）下 Toast 与宠物位置

## 项目结构

```
windows/
├── README.md                   ← 本文件
├── build.ps1                   ← 构建脚本（-Arch x64|arm64|all）
├── WindowSnap.Wpf.sln
└── WindowSnap.Wpf/
    ├── WindowSnap.Wpf.csproj   ← net8.0-windows10.0.22621.0，RID: win-x64 + win-arm64
    ├── app.manifest            ← PerMonitorV2 DPI / asInvoker
    ├── App.xaml / .cs          ← WPF App 入口 + 单实例 Mutex
    ├── AppDelegate.cs          ← 协调核心（对应 macOS AppDelegate.swift）
    ├── Properties/Settings.cs  ← 极简 XML 设置存储（%APPDATA%\WindowSnap\settings.xml）
    ├── Services/
    │   ├── ShortcutStore.cs
    │   ├── TargetAppTracker.cs  ← SetWinEventHook 跟踪前台 app
    │   └── ClipboardAutoClearService.cs ← GetClipboardSequenceNumber + 60 秒清空
    ├── PInvoke/User32.cs       ← Win32 P/Invoke
    ├── Tray/TrayIconManager.cs ← H.NotifyIcon.Wpf 托盘
    ├── Hotkey/GlobalHotkeyManager.cs ← RegisterHotKey
    ├── Capture/
    │   └── WindowCaptureService.cs ← Windows.Graphics.Capture + SoftwareBitmap/BitmapEncoder + D3D11 互操作
    ├── Pet/DesktopPetWindow.xaml(.cs) ← 悬浮宠物
    ├── Toast/ToastWindow.xaml(.cs)    ← 右上角 HUD
    └── Settings/ShortcutSettingsWindow.xaml(.cs) ← 快捷键录制
```

## 首次跑通后请反馈

1. 截图是否成功？粘贴出来的图是黑屏还是正常？
2. 60 秒后剪贴板是否被清空？期间复制别的内容后是否正确跳过？
3. 宠物图标、拖动、重启后位置是否正常？
4. 若在 ARM 机器上：arm64 包是否正常工作？

有报错请附原文，按报错修。
