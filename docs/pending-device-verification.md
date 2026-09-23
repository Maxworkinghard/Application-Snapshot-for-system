# 待真机验证登记（Pending device verification）

本机是 macOS。以下项目**无法在本次开发机器上验证**，登记在此以便在对应平台的真机上补验。
每条都注明「为什么本机验不了」「要在哪一端验」「验什么」。补验完成后请勾掉并注明日期与结论。

约定：Windows 端由审查者在 Windows 真机补验；Linux 端需在真实 Linux 桌面上跑一次。

## 一、窗口录制

- [ ] **被遮挡窗口的录制画面**（三端共享逻辑，重点 Windows / macOS）
  - 验什么：录制时目标窗口被其他窗口遮挡，产物视频仍能正常播放，画面方向正确、时间轴连续（补帧）。
  - 为什么本机验不了：需要人为制造遮挡并逐帧比对；且采集行为依赖各平台合成器。

- [ ] **最小化目标的选择与还原**（Windows 为主）
  - 验什么：最小化的窗口能否出现在候选列表里被选中；开始录制前是否先还原它（Windows 走 `restore_minimized_window`，最小化时 DWM 不再合成，不还原会写出空文件）。
  - 已知现象：文件资源管理器未出现在候选列表，原因与可复现性待确认。
  - 为什么本机验不了：Windows 专有路径（`SW_RESTORE` / `IsIconic`），macOS 编译不到。

- [ ] **Windows 真机录制用例**（`windows_recorder.rs` 5 个 `#[ignore]` 用例）
  - 运行：`cargo test -- --ignored --nocapture`（Windows 上）
  - 验什么：5 个用例全部实际执行并通过，没有「没有可用窗口，跳过」；录制方向、帧数、最小化 0 帧等断言成立。
  - 为什么本机验不了：`#[cfg(target_os = "windows")]`，macOS 上不参与编译。

## 二、音频录制（本轮新增）

- [ ] **Windows WASAPI 两条音频路径**
  - 验什么：系统音频（loopback，`eRender`）与麦克风（`eCapture`）分别开关后，产物 MP4 里确实带对应音轨；首次使用会弹 Windows 麦克风隐私授权，授权后能录到声音。
  - 为什么本机验不了：Windows 专有；且按用户要求，音视频采集的实际效果留给用户手动体验，不在本机跑采集验证。

- [ ] **音视频时间轴对齐**（Windows）
  - 验什么：音频样本的时间戳与视频帧对齐，播放时无明显超前/滞后或漂移。
  - 实现说明：Windows 侧以「首个音视频样本」为共享基线（`base_time`），音频时间戳减去该基线。
  - 为什么本机验不了：同上。

- [ ] **macOS 系统音频 / 麦克风**（`snapshot-recorder`）
  - 验什么：开启系统音频 / 麦克风后成品 MP4 带对应音轨；麦克风需 macOS 15+，并会先请求麦克风权限（`Info.plist` 已声明 `NSAudioCaptureUsageDescription` / `NSMicrophoneUsageDescription`）。
  - 为什么本机验不了：按用户要求不跑采集验证，留给用户手动体验。

- [ ] **Linux PulseAudio/PipeWire 音频输入**
  - 验什么：`pactl` 能取到默认 sink 的 `.monitor`（系统音频）与默认 source（麦克风）；ffmpeg 带 PulseAudio 编译；portal 与 x11grab 两条路径叠加音频后产物可播。
  - 为什么本机验不了：需要真实 Linux 桌面上的音频栈。

## 三、平台能力与打包

- [ ] **Windows 光标叠加截图**（PR #13 遗留的阻塞项）
  - 验什么：在设置里开启「截屏包含鼠标光标」后，检查截图中光标是否显示、位置是否正确。
  - 背景：PR #13 审查结论为阻塞，原因是这项真机验证未完成；审查者当时只显示桌宠、未能打开主设置窗口，系统托盘也没找到应用图标。这是验证缺口，不是已确认的代码缺陷。
  - 为什么本机验不了：Windows 专有的光标合成路径。

- [ ] **Linux Wayland portal ScreenCast**、**托盘点击行为**、**打包安装包**
  - 验什么：Wayland（GNOME / KDE）下 portal ScreenCast 选窗/选屏与录制；托盘菜单与左键点击（取决于 StatusNotifierHost / AppIndicator 扩展）；打包产物能否安装并启动。
  - 为什么本机验不了：需要对应合成器与发行版环境。

## 四、本轮已在本机完成的验证（供对照）

macOS 上已通过：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`（0 警告）、`cargo check --all-targets`、`cargo test`（25 通过）、`npm test`（14 通过）、`npm run build`。
CI（`check.yml`）三端 Rust job 与 frontend job 全绿，其中 Windows / Linux job 覆盖了本机编译不到的端。
