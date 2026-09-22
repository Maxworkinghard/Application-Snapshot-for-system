# Application Snapshot

**简体中文** · [English](README.md)

界面里显示的名字是「应用快照」；打包产物按 Tauri 的 `productName` 命名，macOS 上是 `snapshot.app`。

截取当前应用窗口到剪贴板并留存本地历史，通过 OpenAI 兼容接口润色 Prompt，调用系统 OCR 提取文字，并在桌面上放一只会动的伴侣。

## 仓库结构

**仓库根目录的 Tauri 2 工程就是整个应用**，一套代码覆盖三端：前端 React 在 [src/](src/)，原生侧在 [src-tauri/](src-tauri/)。更早的三套独立实现（Swift / C# / Rust）在主线覆盖之后已经删除，需要时从 git 历史里取。

## 各平台能力

系统能力统一在一层接口之后，每个平台一个 adapter，调用方不随系统变化。

| | Windows | macOS | Linux |
|---|---|---|---|
| 窗口截图 | xcap | xcap | xcap |
| 窗口录制 | ffmpeg `gdigrab` | ffmpeg `avfoundation`（采主屏整屏后按窗口裁剪） | Wayland 走 portal ScreenCast + PipeWire → ffmpeg；X11 走 ffmpeg `x11grab` |
| 文字识别 | `Windows.Media.Ocr` | Vision（经 `snapshot-ocr` 桥） | `tesseract` |
| 快照历史、桌面伴侣、Prompt 润色 | 有 | 有 | 有 |

**真机验证**：目前以 Windows 为主（adapter、托盘、快捷键、截图/录制等已在真机跑过）。macOS / Linux 能力主要来自代码路径、编译检查与适配层实现；macOS 的 `snapshot-ocr` 桥与部分窗口能力（截图、图标、Accessibility 还原最小化）有过真机验证，录制走 ffmpeg `avfoundation`（非 ScreenCaptureKit）已接入并通过编译/单测，但端到端实测仍有限。Linux 在修复编译阻断后已恢复 `cargo check`；X11 路径（xcap / x11grab / tesseract / xdotool / XDG 自启）按适配层实现，Wayland portal ScreenCast、托盘点击、打包安装包等仍可能需本机再验。

## 依赖

- **录制**需要 `ffmpeg` 且在 `PATH` 中。Linux 上「截屏包含鼠标光标」的静帧也会优先走 ffmpeg `x11grab`（失败则回退为无光标截图）；OCR、润色不需要 ffmpeg。
- **macOS 权限**：窗口截图与录制需要「屏幕录制」权限；还原已最小化的窗口再截图需要「辅助功能」权限。
- **Linux 的 OCR** 需要 `tesseract` 及至少一个语言包（`apt install tesseract-ocr tesseract-ocr-chi-sim`）。
- **macOS 的 OCR** 需要 `snapshot-ocr`。Vision 没有系统自带的命令行入口，主线改为调用一个 Swift 小桥（[src-tauri/snapshot-ocr/](src-tauri/snapshot-ocr/)）：stdin 收 PNG，stdout 每行输出一行识别结果；`--probe` 报告识别语言。`npm run tauri build` 会自动构建并以 Tauri sidecar 形式打进包（`bundle.externalBin` 写在 [src-tauri/tauri.macos.conf.json](src-tauri/tauri.macos.conf.json) 里，免得 Windows / Linux 构建去找一个只有 macOS 才有的二进制；产物落在 `Contents/MacOS/` 主程序旁边）；单独构建用 `bash scripts/build-ocr-sidecar.sh`。从源码运行时：`cd src-tauri/snapshot-ocr && swift build -c release`，把产物拷到 `PATH` 内任意目录（如 `~/.local/bin`）——应用优先用与自己同目录的 sidecar，找不到才回退 `PATH`。
- **Prompt 润色**需要一个 OpenAI 兼容端点，在「模型设置」里填写。API Key 存入系统钥匙串，不写进配置文件。

## 从源码运行

```bash
npm install
npm run tauri dev
```

正式构建：

```bash
npm run build          # 仅前端
npm run tauri build    # 安装包
```


## Linux

```bash
# Debian / Ubuntu — 编译与可选运行时依赖
sudo bash scripts/linux/install-deps.sh
bash scripts/linux/check-env.sh

npm install
npm run tauri dev            # 开发
bash scripts/linux/build.sh  # 正式二进制；bundler 成功时还有 deb / AppImage
```

| 可选工具 | 能力 |
|---|---|
| `ffmpeg` | 窗口录制（X11：`x11grab`；Wayland portal：`rawvideo` 编码）；带光标静帧（失败回退无光标） |
| `tesseract` + 语言包 | OCR |
| `xdotool` | 截图前还原已最小化的目标窗口；滚动长截图翻页 |
| StatusNotifierHost | 系统托盘（KDE 原生；GNOME 需 AppIndicator 扩展） |

录制后端按**会话类型**选，不看 `$DISPLAY`：Wayland 会话一律先试 portal ScreenCast——因为 XWayland 会让 `$DISPLAY` 有值，而 `x11grab` 看不到原生 Wayland 窗口。只有门户不可用时才退回 `x11grab`，此时能力文案会明说这条退路只能录到 X server 的画面。X11 会话直接走 `x11grab`。

**开机自启**需在「设置 → 开机静默自启动」中勾选，才会写入 `~/.config/autostart/…desktop`（这是受支持的主路径）。可选的 systemd `--user` 单元示例见 `scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example`（高级；应用不会替你 enable）。

细节见 [scripts/linux/README.md](scripts/linux/README.md)。

每次 push / PR 会跑 [`.github/workflows/check.yml`](.github/workflows/check.yml)：前端 `npm run build`，以及 Linux（经 `scripts/linux/install-deps.sh`）、Windows、macOS 上的 `cargo check` / `cargo test`。

## 快捷键

默认不绑定任何键。应用快照、区域截图、全屏截图、滚动长截图（目前仅 Linux/X11）、录制、润色 Prompt、提取文字都可以在「快捷操作」页各绑一个全局快捷键，窗口最小化时同样触发。

截图完成后的行为由「截图完成后动作」与「自动写入本地文件」决定：默认复制到剪贴板；可选打开标注窗或另存为。仅在开启自动保存时写入本地历史。剪贴板自动清空时限可在设置中配置（默认约 60 秒；期间若又复制了别的内容则跳过这次清空）。录制保存为 MP4 到下载目录（或自定义保存目录）。提取出的文字直接替换剪贴板内容。

## 伴侣素材包

伴侣形象来自「桌面伴侣」页选择的 `.zip`。支持 GIF、WebP、APNG、PNG，以及 MP4、WebM。视频编码白名单按内置 WebView 内核区分：Windows（WebView2）接受 H.264 / HEVC / AV1 / VP9；macOS（WKWebView）与 Linux（WebKitGTK）保守仅认 H.264 / HEVC（WebM 额外接受 VP8/VP9，不含 AV1）——导入时会校验编码，放不出来的素材包会被点名拒绝，而不是装进去之后显示一片空白。

一个动作一个文件，文件名含 `idle` 的作为默认形象。压缩包不会解压、按需读取，所以移走原文件形象会失效。上限：整包 100MB，单个文件 50MB。

## 发布

推 `v*` tag 会打三端的包——macOS universal（zip）、Windows 两个架构的安装程序、以及 Linux x86_64 与 aarch64 的 `.deb` / `.AppImage`（原生 runner，不交叉编译；Linux 编译阻断修复后 CI 可再产出这些产物）——生成 `SHA256SUMS.txt`，创建 draft + pre-release。当前无签名、无公证。

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
