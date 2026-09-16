# 应用快照（Linux 版）

macOS 版的 Linux 移植。常驻后台，功能与 macOS / Windows 端对齐：全局快捷键截取当前活动窗口、悬浮球、窗口录制、提示词润色。

> **状态**：源码通过完整编译（`cargo build`），**未在真实 Linux 桌面上运行验证**。首次跑通时如有问题请反馈报错原文。

## 功能

- **快捷键截图**：默认 `Alt+Shift+2`，截取当前活动窗口并复制到剪贴板，成功时播放快门声，60 秒后自动清空（期间复制过别的内容则跳过）
- **快捷键录制**：默认 `Alt+Shift+R`，开始 / 停止录制当前活动窗口
- **可选绑定**：截取上一个前台应用窗口、润色当前剪切板提示词——默认不绑定，在设置中自行决定
- **悬浮球**：圆形悬浮图标，显示上一个前台应用的图标；点击弹出操作菜单，可拖动，位置记忆；右键打开「设置…」
- **桌宠模式（可选）**：设置里把桌面形式切到「桌宠」后，用用户自备的 GIF 形象替代悬浮球；保存后立即切换。素材目录与覆盖范围见下方「桌宠模式」
- **设置**：右键悬浮球 / 桌宠或托盘菜单 `设置…` 打开 zenity 表单（快捷键绑定 + 润色服务 + 桌面形式），保存后即时生效（润色配置更新内存，快捷键重新注册；仅 X11）
- **管理润色提示词…**（托盘）：内置改写规则常驻；可新建/编辑/删除自定义提示词，列表选中点「切换」即时生效（不替换内置）；「编辑」里有「基于内置新建…」入口，自定义提示词存 `~/.config/windowsnap/prompts.json`（权限 600）
- **应用快照**：从窗口列表选择任意窗口截图
- **窗口录制**：`ffmpeg` 录制当前活动窗口为 MP4，保存到配置目录（仅 X11）
- **润色 Prompt**：读取剪贴板中的文字草稿 → 确认 → 大模型改写 → 结果写回剪贴板，处理中可停止
- **托盘菜单**：StatusNotifierItem 托盘，入口与悬浮球菜单一致
- **桌面通知**：`org.freedesktop.Notifications`

### 运行依赖

纯 Rust 库之外，以下外部命令按功能需要：

| 命令 | 用途 | 必需性 |
|---|---|---|
| `curl` | 润色时调用大模型 API | 仅润色需要 |
| `ffmpeg` | 窗口录制（x11grab） | 仅录制需要 |
| `zenity` 或 `kdialog` | 润色确认 / 窗口选择 / 悬浮球菜单 | 悬浮球与快照列表需要 |
| `zenity` | 设置表单（多字段，kdialog 不支持） | 设置界面需要；缺失时可直接编辑 config.toml |
| `canberra-gtk-play`（或 `paplay` / `pw-play`） | 截图快门声（freedesktop camera-shutter 事件） | 可选，缺失时静默 |

未安装时对应功能会报错提示，不影响截图主流程。

## 与 macOS / Windows 版的功能对应

| macOS | Windows | Linux |
|---|---|---|
| Carbon `RegisterEventHotKey` | `RegisterHotKey` | X11：`XGrabKey`；Wayland：GlobalShortcuts portal（GNOME 48+ / KDE 6） |
| `ScreenCaptureKit` 截窗口 | `PrintWindow` / 屏幕像素回退 | X11：`_NET_ACTIVE_WINDOW` + root 裁剪；Wayland：Screenshot portal（无法只截单窗口，由桌面环境决定交互） |
| `NSPasteboard` (PNG) | Win32 剪贴板 | X11：原生 CLIPBOARD selection（`image/png`）；Wayland：data-control 协议 |
| `NSStatusBar` 菜单栏 | `NotifyIcon` 托盘 | StatusNotifierItem 托盘（KDE 原生支持；GNOME 需 AppIndicator 扩展） |
| 桌面宠物 | 悬浮球 / 桌宠 | 悬浮球：X11 圆形窗；桌宠：X11/XWayland override-redirect，或 wlr-layer-shell（见下方覆盖说明） |
| 窗口录制 MP4 | 窗口录制 MP4 | `ffmpeg` x11grab（仅 X11；Wayland 下无标准窗口级录制接口） |
| 润色 Prompt | 润色 Prompt | 剪贴板草稿 → 确认 → API 改写 → 写回，处理中可停止 |
| 统一设置窗口 | 统一设置窗口 | zenity `--forms` 表单（快捷键 + 润色服务，右键悬浮球或托盘打开） |
| Toast 提示 | Toast 提示 | `org.freedesktop.Notifications` 桌面通知 |
| 60 秒自动清空 | 同 | 同款逻辑（仍持有剪贴板且未被覆盖才清空） |

## 构建

需要 Rust 工具链（https://rustup.rs）。依赖全部为纯 Rust 实现（x11rb / zbus / wl-clipboard-rs / png / ksni / serde_json / chrono / gif / gif-dispose / wayland-client / wayland-protocols-wlr），**无需安装任何 C 库头文件**。`wayland-client` 与 `wayland-protocols-wlr` 本就在 `wl-clipboard-rs` 的依赖树里，桌宠 layer-shell 后端把它们提升为直接依赖，没有引入 smithay / memmap2 / 第二个 png。

```bash
cd linux
./build-linux.sh
install -Dm755 target/release/windowsnap ~/.local/bin/windowsnap
```

### 多架构

源码与架构无关，任意架构上原生构建即可（x86_64、aarch64、riscv64 等）。交叉编译示例：

```bash
rustup target add aarch64-unknown-linux-gnu
# 需要对应的交叉链接器，或直接用 cross（https://github.com/cross-rs/cross，需 Docker/Podman）：
cargo install cross
cross build --release --target aarch64-unknown-linux-gnu
```

仅验证编译（无需链接器）：

```bash
cargo check --target x86_64-unknown-linux-gnu
cargo check --target aarch64-unknown-linux-gnu
```

GitHub 发行包一次打两个 GNU 目标（需 `gcc-aarch64-linux-gnu` 做 ARM 链接）：

```bash
./package-release.sh
```

产物：`../dist/release/Application-Snapshot-{版本}-linux-x86_64.tar.gz` 与 `linux-aarch64.tar.gz`。脚本会读 ELF `e_machine`，架构不对就失败。

## 运行

```bash
windowsnap            # 常驻：快捷键截图 + 悬浮球 + 托盘菜单 + 剪贴板 60 秒自动清空
windowsnap capture    # 触发一次截取（优先转发给常驻进程）
```

开机自启（systemd 用户服务）：

```bash
install -Dm644 windowsnap.service ~/.config/systemd/user/windowsnap.service
systemctl --user enable --now windowsnap.service
```

## 配置

`~/.config/windowsnap/config.toml`（文件权限 600，API Key 只存本机）：

```toml
shortcut = "Alt+Shift+2"          # X11 下生效；Wayland 由 GlobalShortcuts portal 或桌面环境绑定
shortcut_record = "Alt+Shift+R"   # 录制快捷键；留空 = 不绑定
shortcut_previous_app = ""        # 截取上一个应用；默认不绑定
shortcut_polish = ""              # 润色提示词；默认不绑定
save_dir = "~/Videos/应用快照"     # 录制文件保存目录（~ 不会自动展开，建议写绝对路径）
pet_x = "1896"                    # 悬浮球位置（拖动后自动写回）
pet_y = "78"
ui.mode = "bubble"                # 桌面形式：bubble（悬浮球，默认）| pet（桌宠）
pet.skin = ""                     # 桌宠形象目录名（~/.local/share/windowsnap/pet/<形象>/）
desktoppet.x = "1750"             # 桌宠位置（拖动后自动写回，与悬浮球分开记）
desktoppet.y = "620"
polish.kind = "openai"            # openai（OpenAI 兼容接口）| anthropic
polish.base_url = "https://api.deepseek.com"
polish.model = "deepseek-chat"
polish.api_key = "sk-..."
```

润色默认参数与 macOS / Windows 端一致：`max_tokens = 16384`、`temperature = 0.3`、超时 180 秒。

修饰键支持 `Ctrl` / `Alt` / `Shift` / `Super`，主键支持字母、数字、F1–F24 及常用符号。直接改文件需重启进程生效；走「设置…」表单保存则即时生效（X11 会重新注册快捷键，润色配置更新内存，桌面形式立即切换）。

## 桌宠模式（可选）

初始桌面形式为**悬浮球**；在「设置… → 桌面形式」中可切换为**桌宠**并选择形象，保存后立即切换。

素材**不随应用内置或分发**，由用户自行放置：

- 目录：`$XDG_DATA_HOME/windowsnap/pet/<形象>/`（未设 XDG 时为 `~/.local/share/windowsnap/pet/<形象>/`）
- 状态文件（192×208 透明背景 GIF）：`idle` / `waving` / `jumping` / `failed` / `waiting` / `running-left` / `running-right`
- 目录为空或素材缺失时，设置会拒绝启用桌宠并回退悬浮球；形象列表每次打开设置时枚举一次

交互与另外两端对齐：单击打开功能菜单，拖动移动（朝向切到 running-left / running-right），右键 = 设置 / 退出；透明像素点击穿透。截图与录制会等窗口真正隐藏后再抓屏，避免桌宠入镜。

### 窗口后端（不是每种 Wayland 都能当桌宠）

| 会话 | 后端 | 说明 |
|---|---|---|
| X11 | override-redirect + ARGB32 + ShapeInput | 全功能，含跨屏拖动（按指针所在屏的工作区钳制） |
| Wayland（实现了 `zwlr_layer_shell_v1`） | 原生 layer-shell | wlroots 系（sway / Hyprland / river / Wayfire / labwc）+ KWin + COSMIC / niri。位置相对**绑定的那一块 output**，不能像 X11 那样拖到另一块屏 |
| GNOME Wayland（Mutter） | XWayland 上的 X11 后端 | Mutter 至今不实现 wlr-layer-shell（[mutter#973](https://gitlab.gnome.org/GNOME/mutter/-/issues/973) 仍开着）。有 XWayland 时走这条路：override-redirect 窗口按客户端自报坐标放置，且是最顶层（`META_LAYER_OVERRIDE_REDIRECT`） |
| 禁用了 XWayland 的 GNOME 等会话 | **不支持** | 没有 layer-shell，也不做 xdg-shell 兜底 |

**不做 xdg-shell 兜底。** 它能画出一只「窗口里的宠物」：合成器掌握位置、客户端拖不动，还会进 alt-tab 和总览。那不是桌宠，做了只会让支持矩阵变得不诚实。

## X11 与 Wayland 的行为差异

- **X11**：全功能。截取当前活动窗口（含标题栏，去 GTK 阴影）、窗口列表选择、悬浮球、录制、润色。
  窗口截图取自屏幕合成结果，若截图瞬间有置顶窗口遮挡目标窗口，遮挡部分会一并截入。
- **Wayland**：出于安全模型限制，应用无法静默截取任意窗口：
  - 截图走 xdg-desktop-portal，**首次会弹权限/交互对话框**（各桌面环境行为不同，多为全屏截图）
  - 全局快捷键走 GlobalShortcuts portal（GNOME 48+ / KDE 6 支持，绑定时会弹一次确认框）；
    不支持的环境请在系统快捷键设置里把组合键绑定到命令 `windowsnap capture`
  - 窗口列表选择与窗口录制不可用（无标准接口）；润色可用（走 data-control 剪贴板协议）
  - 桌宠：优先 layer-shell；合成器没有该协议且存在 XWayland 时回退 X11 后端；两者都没有则桌宠不可用，截图等主功能不受影响
- 使用剪贴板管理器（GPaste / klipper 等）时，剪贴板所有权会被管理器接管，60 秒自动清空随之失效（与 macOS 版行为一致）。

## 项目结构

```
linux/
├── Cargo.toml
├── build-linux.sh
├── windowsnap.service      ← systemd 用户服务
└── src/
    ├── main.rs             ← CLI 入口、主事件循环（消息分发 + 60 秒清空计时）
    ├── settings.rs         ← config.toml 读写（快捷键 / 保存目录 / 悬浮球与桌宠位置 / 润色配置 / 桌面形式）
    ├── x11.rs              ← XGrabKey 热键 + 活动窗口截图 + 剪贴板（图片与文本）+ 窗口列表
    ├── wayland.rs          ← portal 截图 / GlobalShortcuts + data-control 剪贴板
    ├── desktop.rs          ← 桌面呈现公共控制（截图/录制离场、活动窗口追踪、形式切换）
    ├── pet.rs              ← 悬浮球（圆形窗口 / 活动窗口图标跟踪 / 拖动 / 位置持久化）
    ├── gifpet/             ← GIF 桌宠（解码 / 状态机 / X11 与 layer-shell 窗口）
    ├── record.rs           ← 窗口录制（ffmpeg x11grab）
    ├── polish.rs           ← 提示词润色（确认 → curl 调 API → 写回，处理中可停止）
    ├── tray.rs             ← StatusNotifierItem 托盘
    ├── dialog.rs           ← zenity / kdialog 对话框（润色确认、窗口选择、悬浮球菜单）
    ├── dbus_service.rs     ← local.windowsnap D-Bus 端点（CLI 触发 + 单实例锁）
    └── notify.rs           ← 桌面通知
```

## 已知未验证点（需要在真实 Linux 桌面上跑一次）

1. X11 各 WM 下 `_NET_FRAME_EXTENTS` / `_GTK_FRAME_EXTENTS` 的边框裁剪效果
2. GNOME / KDE 的 Screenshot portal 返回文件的路径格式与权限弹框行为
3. GlobalShortcuts portal 的 `BindShortcuts` 确认框与 `Activated` 信号
4. 大尺寸截图（4K 窗口，PNG 数 MB）经 X11 property 传输的兼容性
5. wl-clipboard-rs 前台供数模式在 GNOME（ext-data-control-v1）上的表现
6. 悬浮球在各 WM 下的 shape 圆形裁剪、`_NET_WM_ICON` 图标读取与拖动手感
7. 桌宠：X11 合成器开/关时的 ShapeBounding、layer-shell 在 sway/Hyprland/KWin 上的 Overlay 层级与点击穿透、GNOME Wayland 的 XWayland 回退、无 XWayland 会话下的失败提示
8. 设置表单切换桌面形式后，旧窗口拆除与新窗口拉起是否在各发行版上瞬时完成
9. ffmpeg 录制参数（crf / preset）在不同机器上的实际效果
10. 润色流程中 zenity / kdialog 的弹窗焦点与取消路径
11. 设置表单（zenity `--forms`）保存后 X11 快捷键重新注册与生效路径
