# 应用快照（Linux 版）

macOS 版的 Linux 移植。常驻后台，按全局快捷键截取当前活动窗口并复制到剪贴板，60 秒后自动清空（期间复制过别的内容则跳过）。

> **状态**：源码在 Linux x86_64 / aarch64 两个目标上通过编译检查（`cargo check --target`），**未在真实 Linux 桌面上运行验证**。首次跑通时如有问题请反馈报错原文。

## 与 macOS / Windows 版的功能对应

| macOS | Linux |
|---|---|
| Carbon `RegisterEventHotKey` | X11：`XGrabKey`；Wayland：GlobalShortcuts portal（GNOME 48+ / KDE 6） |
| `ScreenCaptureKit` 截窗口 | X11：`_NET_ACTIVE_WINDOW` + root 裁剪；Wayland：Screenshot portal（无法只截单窗口，由桌面环境决定交互） |
| `NSPasteboard` (PNG) | X11：原生 CLIPBOARD selection（`image/png`）；Wayland：data-control 协议 |
| `NSStatusBar` 菜单栏 | StatusNotifierItem 托盘（KDE 原生支持；GNOME 需 AppIndicator 扩展） |
| Toast 提示 | `org.freedesktop.Notifications` 桌面通知 |
| 60 秒自动清空 | 同款逻辑（仍持有剪贴板且未被覆盖才清空） |
| 桌面宠物 / 窗口录制 | 不提供（保持 Linux 版最小化） |

## 构建

需要 Rust 工具链（https://rustup.rs）。依赖全部为纯 Rust 实现（x11rb / zbus / wl-clipboard-rs / png / ksni），**无需安装任何 C 库头文件**。

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

## 运行

```bash
windowsnap            # 常驻：快捷键 + 托盘 + 自动清空
windowsnap capture    # 触发一次截取（优先转发给常驻进程）
```

开机自启（systemd 用户服务）：

```bash
install -Dm644 windowsnap.service ~/.config/systemd/user/windowsnap.service
systemctl --user enable --now windowsnap.service
```

## 配置

`~/.config/windowsnap/config.toml`：

```toml
shortcut = "Alt+Shift+2"   # 默认值，对应 macOS 版 ⌥⇧2
```

修饰键支持 `Ctrl` / `Alt` / `Shift` / `Super`，主键支持字母、数字、F1–F24 及常用符号。改完重启进程生效。

## X11 与 Wayland 的行为差异

- **X11**：截取当前活动窗口（含标题栏，去 GTK 阴影），全局快捷键开箱即用。
  窗口截图取自屏幕合成结果，若截图瞬间有置顶窗口遮挡目标窗口，遮挡部分会一并截入。
- **Wayland**：出于安全模型限制，应用无法静默截取任意窗口：
  - 截图走 xdg-desktop-portal，**首次会弹权限/交互对话框**（各桌面环境行为不同，多为全屏截图）
  - 全局快捷键走 GlobalShortcuts portal（GNOME 48+ / KDE 6 支持，绑定时会弹一次确认框）；
    不支持的环境请在系统快捷键设置里把组合键绑定到命令 `windowsnap capture`
- 使用剪贴板管理器（GPaste / klipper 等）时，剪贴板所有权会被管理器接管，60 秒自动清空随之失效（与 macOS 版行为一致）。

## 项目结构

```
linux/
├── Cargo.toml
├── build-linux.sh
├── windowsnap.service      ← systemd 用户服务
└── src/
    ├── main.rs             ← CLI 入口、配置、主事件循环（60 秒清空计时）
    ├── x11.rs              ← XGrabKey 热键 + 活动窗口截图 + CLIPBOARD 供数
    ├── wayland.rs          ← portal 截图 / GlobalShortcuts + data-control 剪贴板
    ├── dbus_service.rs     ← local.windowsnap D-Bus 端点（CLI 触发 + 单实例锁）
    ├── tray.rs             ← StatusNotifierItem 托盘
    └── notify.rs           ← 桌面通知
```

## 已知未验证点（需要在真实 Linux 桌面上跑一次）

1. X11 各 WM 下 `_NET_FRAME_EXTENTS` / `_GTK_FRAME_EXTENTS` 的边框裁剪效果
2. GNOME / KDE 的 Screenshot portal 返回文件的路径格式与权限弹框行为
3. GlobalShortcuts portal 的 `BindShortcuts` 确认框与 `Activated` 信号
4. 大尺寸截图（4K 窗口，PNG 数 MB）经 X11 property 传输的兼容性
5. wl-clipboard-rs 前台供数模式在 GNOME（ext-data-control-v1）上的表现
