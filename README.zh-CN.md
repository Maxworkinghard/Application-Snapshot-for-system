# Application Snapshot

**简体中文** · [English](README.md)

界面与 macOS .app 显示名为「应用快照」。

截取当前应用窗口到剪贴板并留存本地历史，通过 OpenAI 兼容接口润色 Prompt，调用系统 OCR 提取文字，并在桌面上放一只会动的伴侣。

## 仓库结构

**仓库根目录的 Tauri 2 工程是主线**，一套代码覆盖三端：前端 React 在 [src/](src/)，原生侧在 [src-tauri/](src-tauri/)。

[macos/](macos/)（Swift）、[windows/](windows/)（C#）、[linux/](linux/)（Rust）是更早的三套独立实现，**仅作参考保留**，不再承载新功能。凡是它们已经封装好的成熟系统能力，主线通过平台 adapter 直接调用，不为了语言统一重写一遍。

## 各平台能力

系统能力统一在一层接口之后，每个平台一个 adapter，调用方不随系统变化。

| | Windows | macOS | Linux |
|---|---|---|---|
| 窗口截图 | xcap | xcap | xcap |
| 窗口录制 | ffmpeg `gdigrab` | ffmpeg `avfoundation`（采主屏整屏后按窗口裁剪） | ffmpeg `x11grab` |
| 文字识别 | `Windows.Media.Ocr` | Vision（经 `snapshot-ocr` 桥） | `tesseract` |
| 快照历史、桌面伴侣、Prompt 润色 | 有 | 有 | 有 |

目前 Windows 侧的 adapter、macOS 的 `snapshot-ocr` 桥接程序、以及 macOS 的窗口能力（截图、`NSRunningApplication` 取上一个应用图标、Accessibility API 还原最小化窗口）已经在真机上跑过；macOS 录制已接入 ffmpeg `avfoundation` 并通过编译与单测，但尚未端到端实测。Linux 的 adapter 尚未编译、也未运行。

## 依赖

- **录制**需要 `ffmpeg` 且在 `PATH` 中。截图、OCR、润色都不需要。
- **macOS 权限**：窗口截图与录制需要「屏幕录制」权限；还原已最小化的窗口再截图需要「辅助功能」权限。
- **Linux 的 OCR** 需要 `tesseract` 及至少一个语言包（`apt install tesseract-ocr tesseract-ocr-chi-sim`）。
- **macOS 的 OCR** 需要 `PATH` 中有 `snapshot-ocr`。Vision 没有系统自带的命令行入口，主线改为调用一个 Swift 小桥（[src-tauri/snapshot-ocr/](src-tauri/snapshot-ocr/)）：stdin 收 PNG，stdout 每行输出一行识别结果；`--probe` 报告识别语言。构建方式：`cd src-tauri/snapshot-ocr && swift build -c release`，产物拷到 `PATH` 内任意目录（如 `~/.local/bin`）。
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

三套参考实现各自独立构建：

```bash
./macos/scripts/build-app.sh
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\windows\scripts\build.ps1
cd linux && ./scripts/build-linux.sh
```

## 快捷键

默认不绑定任何键。应用快照、录制、润色 Prompt、提取文字都可以在「快捷操作」页各绑一个全局快捷键，窗口最小化时同样触发。

快照进剪贴板并存入本地历史，约 60 秒后清空剪贴板；期间若又复制了别的内容，则跳过这次清空。录制保存为 MP4 到下载目录。提取出的文字直接替换剪贴板内容。

## 伴侣素材包

伴侣形象来自「桌面伴侣」页选择的 `.zip`。支持 GIF、WebP、APNG、PNG，以及 MP4、WebM。视频须为 H.264、HEVC、AV1 或 VP9（macOS 上只认 H.264/HEVC——WKWebView 对 AV1/VP9 的支持随系统版本变化）——导入时会校验编码，放不出来的素材包会被点名拒绝，而不是装进去之后显示一片空白。

一个动作一个文件，文件名含 `idle` 的作为默认形象。压缩包不会解压、按需读取，所以移走原文件形象会失效。上限：整包 100MB，单个文件 50MB。

## 发布

推 `v*` tag 会在三系统打包并生成 `SHA256SUMS.txt`，创建 draft + pre-release。当前无签名、无公证。

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
