# 应用快照

跨平台的窗口截图小工具：截取应用窗口并自动复制到系统剪贴板，随后可在任意支持图片的输入框直接粘贴。截图不落盘、不上传，写入剪贴板 60 秒后自动清空（期间复制过别的内容则跳过）。

| 平台 | 技术栈 | 支持架构 | 状态 |
|---|---|---|---|
| macOS（本目录） | Swift + ScreenCaptureKit | Apple Silicon + Intel（一份 universal 2） | ✅ 日常使用中 |
| [Windows](windows/) | .NET 10 WinForms + Win32（自包含） | x64 原生 + ARM64 原生 | ✅ x64 真机；ARM64 已出原生包，待 ARM 设备验证 |
| [Linux](linux/) | Rust + X11 / Wayland portal | x86_64 + aarch64（任意架构可自行编译） | ⚠️ 编译已验证，待真机运行验证 |

## 三端功能对照

| 功能 | macOS | Windows | Linux |
|---|---|---|---|
| 全局快捷键截图 | ✅ `⌥⇧2` | ✅ `Alt+Shift+2` | ✅ `Alt+Shift+2`（X11 / Wayland portal） |
| 窗口录制 MP4 | ✅ `⌥⇧R` | ✅ `Alt+Shift+R` | ✅ ffmpeg（X11；Wayland 无标准接口） |
| 可选绑定快捷键（截取上一个应用 / 润色，默认不绑定） | ✅ 设置页 | ✅ 设置窗口 | ✅ 设置表单（X11） |
| 统一设置入口（右键悬浮球：快捷键绑定 + 润色 LLM Provider） | ✅ | ✅ | ✅ zenity 表单 |
| 润色提示词库（内置 + 用户自定义，可随时切换不替换） | ✅ 设置页 | ✅ 设置窗口 | ✅ 托盘「管理润色提示词…」 |
| 悬浮球（显示上一个前台应用图标，可拖动） | ✅ | ✅ | ✅ X11 |
| 桌宠模式（设置切换 GIF 形象） | ✅ | ✅ | ✅ X11 / XWayland；原生 layer-shell 覆盖见 [Linux README](linux/) |
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
- 可选绑定：截取上一个前台应用窗口、润色当前剪切板提示词——默认不绑定，在设置页中自行决定
- 菜单栏 `润色 Prompt`：改写剪贴板中的提示词草稿，确认后写回剪贴板，处理中可停止
- 润色提示词库：内置改写规则常驻，可在设置页新建/编辑/删除自定义提示词并随时切换（切换即生效，不替换内置）；选「内置」点「编辑」可基于内置文本另存自定义版本
- 菜单栏 `设置保存目录…`：更改录制文件的保存位置（默认「下载」；目录失效时自动回退）
- 菜单栏 `设置…`（⌘,）或右键悬浮球 `设置…`：统一设置页——绑定/清除全局快捷键（截取当前应用 / 录制 / 截取上一个应用 / 润色提示词，均可在设置中清除），润色提示词管理，以及润色 LLM Provider（协议 / Base URL / 模型 / API Key，Key 存 Keychain）
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

- 产物是 universal 2：同一份 `.app` 含 Apple Silicon（arm64）与 Intel（x86_64）两个切片，M 系列跑原生、Intel 跑原生，不走 Rosetta。缺任一切片时构建脚本会失败
- 有名为 `WindowSnapDev` 的代码签名证书时使用之，否则自动退回 ad-hoc 签名（仅限本机运行；重签后需重新授予屏幕录制权限）
- 开机自启：`./scripts/install-launch-agent.sh`

第一次截取时，macOS 会请求「屏幕与系统音频录制」权限。授权后如果首次截图失败，退出并重新打开应用即可。

## 桌宠模式（可选）

初始桌面形式为**悬浮窗**；在「设置… → 桌面形式」中可切换为**桌宠**并选择形象，选择持久化。三端保存后立即切换，不需要重启。

桌宠素材**不随应用内置或分发**（素材并非本项目制作，避免版权问题），由用户自行放置：

| 平台 | 素材目录 |
|---|---|
| macOS | `~/Library/Application Support/WindowSnap/pet/<形象>/` |
| Windows | `%APPDATA%\AppSnapshot\pet\<形象>\` |
| Linux | `$XDG_DATA_HOME/windowsnap/pet/<形象>/`（默认 `~/.local/share/windowsnap/pet/<形象>/`） |

- 状态文件（192×208 透明背景 GIF）：`idle` / `waving` / `jumping` / `failed` / `waiting` / `running-left` / `running-right`
- 目录为空或素材缺失时，设置中的桌宠选项会提示不可用，应用回退悬浮窗
- 形象列表在每次打开设置窗口时枚举一次，放好素材后重开设置即可选择

桌宠交互：单击打开功能面板（与悬浮窗一致），拖动移动（面板跟随），右键菜单 = 设置 / 退出。透明像素点击穿透。

离场避镜：

- Windows：截图与录制都会让桌宠离场
- Linux：截图与录制都会让桌宠离场（X11 截的是屏幕合成结果，不离场就会入镜）
- macOS：`⌥⇧2` 截图与 MP4 录制走 SCContentFilter 单窗口合成，画面里本就不含桌宠，这两条路径**不会**离场；「选择其他窗口…」走系统 `screencapture`，会离场

Linux 的 Wayland 覆盖不是 100%：原生 layer-shell 只覆盖实现了 `wlr-layer-shell` 的合成器；GNOME Wayland 走 XWayland。不做 xdg-shell 兜底（那会变成普通窗口）。详见 [linux/README.md](linux/README.md)。

## 系统要求

- macOS 14 或更高版本，Apple Silicon 或 Intel（一份应用同时覆盖）
- Xcode 及 Swift 6 工具链
- Windows 10 22H2 或 Windows 11，**x64 或 ARM64 各用对应原生包**；不提供 32 位（win-x86）
- Windows / Linux 版细节见各自目录的 README

## 芯片架构

合格产品按芯片出原生包，不把模拟当正式支持。

| 机器 | 用哪份产物 | 说明 |
|---|---|---|
| Mac Apple Silicon（M 系列） | `dist/应用快照.app` | universal 2 的 arm64 切片 |
| Mac Intel | 同上 | universal 2 的 x86_64 切片 |
| Windows x64（Intel / AMD） | `windows/dist/win-x64/AppSnapshot.exe` | 原生 x64，自包含，无需安装 .NET |
| Windows ARM64（骁龙本等） | `windows/dist/win-arm64/AppSnapshot.exe` | 原生 ARM64，不是 x64 模拟 |
| Windows 32 位 x86 | 不提供 | Windows 11 只有 64 位；32 位对截窗没有收益 |
| Linux x86_64 | `linux` 目录交叉/原生编 | 发行附件 `linux-x86_64` |
| Linux aarch64 | 同上 | 发行附件 `linux-aarch64` |

Windows 两份 exe 都是自包含单文件，体积会比旧的 .NET Framework 4.0 构建大（内嵌运行时）。这是原生 ARM64 的代价：.NET Framework 没有 ARM64 运行时，不能靠改 `PlatformTarget` 出 ARM 包。

录制仍调用系统 PATH 上的 `ffmpeg`，x64 / ARM64 的 ffmpeg 都可以（进程架构与 ffmpeg 不一致时由系统模拟）。

## GitHub 发行包

一个 git tag（`v0.1.0`）对应一条 GitHub Release，下面挂**五种原生附件**加一份校验和。文件名全部 ASCII；Mac zip 里面的应用仍叫「应用快照.app」。

| 附件 | 内容 | 校验 |
|---|---|---|
| `Application-Snapshot-{ver}-macos-universal.zip` | 一份 universal 2 `.app`（arm64 + x86_64） | zip 解开后 `lipo` 必须同时看到两个切片 |
| `Application-Snapshot-{ver}-windows-x64.exe` | .NET 10 自包含单文件 | PE Machine = `x64` |
| `Application-Snapshot-{ver}-windows-arm64.exe` | 同上，ARM64 原生 | PE Machine = `ARM64` |
| `Application-Snapshot-{ver}-linux-x86_64.tar.gz` | `windowsnap` + README + user systemd unit | ELF `EM_X86_64` |
| `Application-Snapshot-{ver}-linux-aarch64.tar.gz` | 同上 | ELF `EM_AARCH64` |
| `SHA256SUMS.txt` | 上述五个文件的 SHA-256 | GNU `sha256sum` 格式 |

不提供 `windows-x86`。不把 Source code zip 当安装包。

本地打包（只在对应操作系统上跑；不会创建 GitHub Release）：

```bash
# macOS
bash scripts/package-macos.sh

# Linux（一次打 x86_64 与 aarch64，需 aarch64-linux-gnu-gcc）
linux/package-release.sh
```

```powershell
# Windows（一次打 x64 与 ARM64）
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\package-release.ps1
```

产物都进仓库根目录 `dist/release/`。

推送 `v*` tag 后，`.github/workflows/release.yml` 会在三种 runner 上各打各的包，收齐后创建 **draft + pre-release**。检查附件无误再在网页上点 Publish。没有签名/公证证书时保持 pre-release，不要当正式版。
