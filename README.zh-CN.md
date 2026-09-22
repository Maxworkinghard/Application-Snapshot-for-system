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
| 窗口录制 | ffmpeg `gdigrab` | 待接入 ScreenCaptureKit | ffmpeg `x11grab`（X11）；纯 Wayland 走 portal ScreenCast + PipeWire → ffmpeg |
| 文字识别 | `Windows.Media.Ocr` | 待接入 Vision（经 `snapshot-ocr` 桥） | `tesseract` |
| 快照历史、桌面伴侣、Prompt 润色 | 有 | 有 | 有 |

目前 Windows 侧 adapter 在真机上验证最充分。Linux adapter 已可在本仓库编译，覆盖 X11 录制、纯 Wayland 的 portal ScreenCast 录制（端到端需真实图形会话）、tesseract OCR、XDG 开机自启（需用户勾选；systemd 用户单元仅作可选文档）以及尽力而为的应用图标。macOS 的录制 / OCR 桥尚未完成。

## 依赖

- **录制**需要 `ffmpeg` 且在 `PATH` 中。截图、OCR、润色都不需要。
- **Linux 的 OCR** 需要 `tesseract` 及至少一个语言包（`apt install tesseract-ocr tesseract-ocr-chi-sim`）。
- **macOS 的 OCR** 需要 `PATH` 中有 `snapshot-ocr`。Vision 没有系统自带的命令行入口，主线改为调用一个 Swift 小桥：stdin 收 PNG，stdout 每行输出一行识别结果；`--probe` 报告识别语言。该程序尚未编写。
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


## Linux（Tauri 主线）

Linux 上的产品路径是仓库根目录这套 Tauri 应用，不是 `linux/windowsnap`（该目录是更早的参考实现，当前 Release 仍在打它的 tar.gz）。

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
| `ffmpeg` | 窗口录制（有 `$DISPLAY` 时用 `x11grab`；portal 路径用 ffmpeg `rawvideo` 编码） |
| `tesseract` + 语言包 | OCR |
| `xdotool` | 截图前还原已最小化的目标窗口 |
| StatusNotifierHost | 系统托盘（KDE 原生；GNOME 需 AppIndicator 扩展） |

**开机自启**需在「设置 → 开机静默自启动」中勾选，才会写入 `~/.config/autostart/…desktop`（这是受支持的主路径）。可选的 systemd `--user` 单元示例见 `scripts/linux/com.appsnapshot.prompt-pet-shortcut.service.example`（高级；应用不会替你 enable）。

细节见 [scripts/linux/README.md](scripts/linux/README.md)。

## 快捷键

默认不绑定任何键。应用快照、录制、润色 Prompt、提取文字都可以在「快捷操作」页各绑一个全局快捷键，窗口最小化时同样触发。

快照进剪贴板并存入本地历史，约 60 秒后清空剪贴板；期间若又复制了别的内容，则跳过这次清空。录制保存为 MP4 到下载目录。提取出的文字直接替换剪贴板内容。

## 伴侣素材包

伴侣形象来自「桌面伴侣」页选择的 `.zip`。支持 GIF、WebP、APNG、PNG，以及 MP4、WebM。视频须为 H.264、HEVC、AV1 或 VP9——导入时会校验编码，放不出来的素材包会被点名拒绝，而不是装进去之后显示一片空白。

一个动作一个文件，文件名含 `idle` 的作为默认形象。压缩包不会解压、按需读取，所以移走原文件形象会失效。上限：整包 100MB，单个文件 50MB。

## 发布

推 `v*` tag 会在三系统打包并生成 `SHA256SUMS.txt`，创建 draft + pre-release。当前无签名、无公证。

仓库没有 `LICENSE` 文件。尚未添加许可证，默认保留版权。
