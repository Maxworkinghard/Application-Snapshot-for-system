# Application Snapshot

[![Check](https://github.com/Maxworkinghard/Application-Snapshot-for-system/actions/workflows/check.yml/badge.svg)](https://github.com/Maxworkinghard/Application-Snapshot-for-system/actions/workflows/check.yml)

**简体中文** · [English](README.md)

界面里显示的名字是「应用快照」；打包产物按 Tauri 的 `productName` 命名，macOS 上是 `snapshot.app`。

截取当前应用窗口到剪贴板并留存本地历史，通过 OpenAI 兼容接口润色 Prompt，调用系统 OCR 提取文字，并在桌面上放一只会动的伴侣。

## 仓库结构

**仓库根目录的 Tauri 2 工程就是整个应用**，一套代码覆盖三端：前端 React 在 [src/](src/)，原生侧在 [src-tauri/](src-tauri/)。更早的三套独立实现（Swift / C# / Rust）在主线覆盖之后已经删除，需要时从 git 历史里取。

## 实现方式的取舍原则

**系统自带的做法明显更好时，该平台单独实现，不迁就统一路径。** 判断依据是实际效果，不是代码整齐。录制就是这么改过来的：原先三端共用 ffmpeg，`gdigrab` 在硬件加速窗口上录出黑屏——为了统一选了个哪端都不够好的公约数。现在 Windows 走 WGC、macOS 走 ScreenCaptureKit、Linux 走 portal/x11grab，每端都比之前强。

反过来，只是 API 名字不同、效果无实质差别的，用成熟的跨平台库或共享实现。窗口截图三端都用 xcap——它内部本来就是三套原生实现，只是维护的人不是我们。自己再写三遍，收益接近零。

新增能力前先问一句：**这个能力归操作系统管吗？** 归——各写各的；不归（计算、文件、网络、界面）——共享。

成本不只在写代码。三套实现意味着三处可能藏 bug，且各自只能在对应机器上复现；行为也会漂移（`restore_minimized_window` 现在三端语义就不同）。**真正的瓶颈是验证而不是实现**——没在真机验过的原生实现，未必比验过的通用实现更可靠。

## 各平台能力

每个平台一套原生实现，共用同一组命令入口。**能力与行为按平台不同**——同一个按钮在三端可能走完全不同的系统 API，边界条件也不一样。差异见下表，以及设置页的「本机能力」（它由各端 adapter 实时报告，而不是写死的文案）。

共享的只是不依赖系统能力的部分：设置、快照历史、Prompt 润色、界面。凡是系统自己提供且各端做法有实质差异的（录制、OCR、窗口控制），一律各写各的——曾经为了三端统一而选公约数方案，结果是哪端都不够好。

| | Windows | macOS | Linux |
|---|---|---|---|
| 窗口截图 | xcap | xcap | xcap |
| 窗口录制 | Windows.Graphics.Capture + Media Foundation | ScreenCaptureKit 原生窗口流 + AVAssetWriter | Wayland 走 portal ScreenCast + PipeWire → ffmpeg；X11 走 ffmpeg `x11grab` |
| 文字识别 | `Windows.Media.Ocr` | Vision（经 `snapshot-ocr` 桥） | `tesseract` |
| 快照历史、桌面伴侣、Prompt 润色 | 有 | 有 | 有 |

**真机验证**：目前以 Windows 为主（adapter、托盘、快捷键、截图/录制等已在真机跑过）。macOS / Linux 能力主要来自代码路径、编译检查与适配层实现；macOS 的 `snapshot-ocr` 桥与部分窗口能力（截图、图标、Accessibility 还原最小化）有过真机验证，录制已切到 ScreenCaptureKit 原生窗口流并由 AVAssetWriter 输出 H.264 MP4。Linux 在修复编译阻断后已恢复 `cargo check`；X11 路径（xcap / x11grab / tesseract / xdotool / XDG 自启）按适配层实现，Wayland portal ScreenCast、托盘点击、打包安装包等仍可能需本机再验。

## 依赖

- **录制**：Windows 走系统自带的 Windows.Graphics.Capture 与 Media Foundation，不需要 ffmpeg；macOS 使用随应用打包的 ScreenCaptureKit sidecar，不依赖外部 ffmpeg；Linux 需要 `ffmpeg` 在 `PATH` 中。Linux 上「截屏包含鼠标光标」的静帧也会优先走 ffmpeg `x11grab`（失败则回退为无光标截图）；OCR、润色不需要 ffmpeg。
- **录制最小化的窗口**：最小化后系统不再为窗口合成画面，录不到任何内容。Windows 上会先把它还原再开录（与截图一致）；还原不了则明确报错，不会留下一个打不开的空文件。
- **macOS 权限**：窗口截图与录制需要「屏幕录制」权限；还原已最小化的窗口再截图需要「辅助功能」权限。
- **Linux 的 OCR** 需要 `tesseract` 及至少一个语言包（`apt install tesseract-ocr tesseract-ocr-chi-sim`）。
- **macOS sidecar**：Vision OCR 由 [src-tauri/snapshot-ocr/](src-tauri/snapshot-ocr/) 提供；ScreenCaptureKit 录制由 [src-tauri/snapshot-recorder/](src-tauri/snapshot-recorder/) 提供。`npm run tauri dev` 和 `npm run tauri build` 都会自动准备二者，正式包内位于 `Contents/MacOS/` 主程序旁边。
- **Prompt 润色**需要一个 OpenAI 兼容端点，在「模型设置」里填写。API Key 存入系统钥匙串，不写进配置文件。

## 名字的来历

同一个东西在几处叫法不同，记在这里免得下次有人去「统一」：

| 出现的地方 | 名字 |
|---|---|
| 仓库 | `Application-Snapshot-for-system` |
| 界面与 macOS .app | 应用快照 |
| 可执行文件（`productName`） | `snapshot` |
| Bundle identifier | `com.appsnapshot.prompt-pet-shortcut` |

最后那个里的 `prompt-pet-shortcut` 是项目早期的名字。**它不能改**——
identifier 是系统用来认配置目录与钥匙串条目的键，改了等于让已安装用户的
设置和 API Key 全部失联。留着它是有意为之，不是漏改。

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

默认不绑定任何键。应用快照、全屏截图、滚动长截图（目前仅 Linux/X11）、录制、润色 Prompt、提取文字都可以在「快捷操作」页各绑一个全局快捷键，窗口最小化时同样触发。

截图完成后的行为由「截图完成后动作」与「自动写入本地文件」决定：默认复制到剪贴板；可选打开标注窗或另存为。仅在开启自动保存时写入本地历史。剪贴板自动清空时限可在设置中配置（默认约 60 秒；期间若又复制了别的内容则跳过这次清空）。录制默认保存为 MP4 到系统「下载」目录；可在「快捷操作 → 剪贴板与保存」中直接打开或更改独立的录制目录。提取出的文字直接替换剪贴板内容。

## 伴侣素材

在「桌面伴侣」页选一个装着多个 GIF 的 `.zip`，或者直接选一个 GIF。**目前只认 GIF**，压缩包内其它格式会被跳过。

视频（MP4 / WebM）暂不支持：播放要交给各端内置的 WebView，而三端内核认的编码各不相同，同一个包在这台能动、在那台是一片空白。

一个动作一个文件，文件名含 `idle` 的作为默认形象。素材按需读取、不会复制，所以移走原文件形象会失效。上限：整包 100MB，单个 GIF 50MB。

## 发布

推 `v*` tag 会打三端的包——macOS universal（zip）、Windows 两个架构的安装程序、以及 Linux x86_64 与 aarch64 的 `.deb` / `.AppImage`（原生 runner，不交叉编译；Linux 编译阻断修复后 CI 可再产出这些产物）——生成 `SHA256SUMS.txt`，创建 draft + pre-release。当前无签名、无公证。

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
