# 待真机验证登记（Pending device verification）

本机是 macOS。以下项目**无法在本次开发机器上验证**，登记在此以便在对应平台的真机上补验。
每条都注明「为什么本机验不了」「要在哪一端验」「验什么」。补验完成后请勾掉并注明日期与结论。

约定：Windows 端由审查者在 Windows 真机补验；Linux 端需在真实 Linux 桌面上跑一次。

## 一、窗口录制

- [ ] **被遮挡窗口的录制画面**（三端共享逻辑，重点 Windows / macOS）
  - 验什么：录制时目标窗口被其他窗口遮挡，产物视频仍能正常播放，画面方向正确、时间轴连续（补帧）。
  - 为什么本机验不了：需要人为制造遮挡并逐帧比对；且采集行为依赖各平台合成器。

- [ ] **最小化目标的选择与还原**（Windows 为主）
  - 验什么：最小化的窗口能否出现在候选列表里被选中；开始录制前是否先还原它（Windows 走 `os::restore_minimized`，见 `src-tauri/src/os/windows/`；最小化时 DWM 不再合成，不还原会写出空文件）。
  - 已知现象：文件资源管理器未出现在候选列表，原因与可复现性待确认。
  - 为什么本机验不了：Windows 专有路径（`SW_RESTORE` / `IsIconic`），macOS 编译不到。

- [ ] **Windows 真机录制用例**（`os/windows/recorder.rs`，原 `windows_recorder.rs`，5 个 `#[ignore]` 用例）
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
  - 2026-09-25：Linux 测试机没有 `pactl`，跳过，仍待补验。

## 三、平台能力与打包

- [ ] **Windows 光标叠加截图**（PR #13 遗留的阻塞项）
  - 验什么：在设置里开启「截屏包含鼠标光标」后，检查截图中光标是否显示、位置是否正确。
  - 背景：PR #13 审查结论为阻塞，原因是这项真机验证未完成；审查者当时只显示桌宠、未能打开主设置窗口，系统托盘也没找到应用图标。这是验证缺口，不是已确认的代码缺陷。
  - 为什么本机验不了：Windows 专有的光标合成路径。

- [ ] **Linux Wayland portal ScreenCast**、**托盘点击行为**、**打包安装包**
  - 验什么：Wayland（GNOME / KDE）下 portal ScreenCast 选窗/选屏与录制；托盘菜单与左键点击（取决于 StatusNotifierHost / AppIndicator 扩展）；打包产物能否安装并启动。
  - 为什么本机验不了：需要对应合成器与发行版环境。
  - 2026-09-25（X11 测试机）：deb / rpm / AppImage 都能构建，还没装到系统里验启动；测试机没有 ScreenCast 门户、PipeWire 和 StatusNotifierHost，portal 与托盘都没测。

## 四、2026-09-24 代码优化系列（#21–#25）新增

这一批由 Windows 端完成并合入 main。Windows 上已做过真机端到端验证的，标在「已验」里；下面列的是还缺真人操作或只能在另外两端验的项。

**已在 Windows 真机验过**（隔离身份的测试实例，经真实命令 / 真实 WebView）：
- 快捷键：Rust 侧注册，按下后执行动作；整组保存「要么全生效要么全不生效」；启动时的冲突被记录。
- media:// 协议：快照、桌宠 GIF、图标、自定义音效能加载，越权请求被拒；正式打包模式下 CSP 生效时同样正常。
- 平台目录重构后：本机能力、录制开始 / 停止 / 落盘都通过。
- CSS 删减：7 个页面 × 浅色 / 深色 + 3 个交互状态，逐元素计算样式与删减前一致。

**三端都要真人验：**
- [ ] **全局快捷键**（#22，注册从网页挪到了 Rust）
  - 验什么：绑定后真的按一下能触发；绑一个被别的程序占着的键，整组不保存并点名该键；重启后若有键被占，打开主窗口会提示。
  - 已知疑点：设置页录键读的是按键产生的字符（`event.key`），美式键盘上 Alt+Shift+2 可能被记成 `Alt+Shift+@`。解析器只认 `2` 不认 `@`，会报「不是有效的快捷键」。请用真键盘录一次确认；属实的话改成读 `event.code`。
  - Linux 2026-09-25：把绑定写进设置后重启，按键能触发；设置页里真人录键没测。
- [ ] **图片与音频**（#23，改走 media:// 协议）
  - 验什么：快照历史的缩略图与放大预览；桌宠形象（导入 GIF / zip、动作轮换）；快捷菜单窗口列表的应用图标，以及桌宠在「应用图标」模式下的图标清晰度；自定义快门音效的试听与截图时播放（这项此前一直放不出来）。
- [ ] **这些界面 CSS 比对没覆盖到**（#24，浏览器预览渲染不出来）
  - 验什么：桌宠窗口、快捷菜单、标注窗口；历史页有卡片时；桌宠页导入了素材时。看有没有样式缺失。

**Windows：**
- [ ] 带光标截图（沿用上面第三节的遗留项，这次代码只是搬家）。
- [ ] 「另存为」改了行为：按对话框里实际填的扩展名存（`.png` / `.jpg` / `.webp`），认不出才回退偏好格式。
- [ ] 剪贴板定时清空：只比对图像指纹。截图后期间没复制别的，会被清空；复制了别的，不应被清空。

**macOS / Linux**（#25 平台目录重构，代码搬到 `src-tauri/src/os/<平台>/`，本机只过了 CI 编译）：
- [ ] macOS：录制开始 / 停止，以及停止异常时的提示；带光标截图；最小化窗口还原；开机自启开关。
- Linux（2026-09-25 在 X11 测试机 `DISPLAY=:3` 上测了 39e4cae，报告要点见第六节）：
  - [x] x11grab 录制：启停正常，产物 h264 816×484、4.13 秒，可播。
  - [x] 带光标截图：`includeCursor` 开启后全屏截图走 ffmpeg x11grab，没有降级提示。
  - [x] 开机自启：打开后生成 `~/.config/autostart/com.appsnapshot.snapshot.desktop`。
  - [ ] 滚动长截图：动作能跑通，但测试页不够长，拼出来的高度约等于一屏。要在真正能滚好几屏的页面上确认拼接。
  - [ ] portal 录制：测试机没有 ScreenCast 门户和 PipeWire，没测。

**已知的原有问题**（不是这批引入的，登记备查）：
- 模型设置对话框「拉取模型」时刷新图标不转：组件用了 `spinning` 类，但唯一的样式规则挂在一个没人用的父类 `.model-control` 下，删 CSS 前就不生效。

## 五、本轮已在本机完成的验证（供对照）

macOS 上已通过：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`（0 警告）、`cargo check --all-targets`、`cargo test`（25 通过）、`npm test`（14 通过）、`npm run build`。
CI（`check.yml`）三端 Rust job 与 frontend job 全绿，其中 Windows / Linux job 覆盖了本机编译不到的端。

## 六、2026-09-25 Linux 测试发现的问题（已修）与复测项

X11 测试机、39e4cae 上发现三处问题，都已在 main 上修好，需要在 Linux 上复测：

- [ ] **`WAYLAND_DISPLAY` 设了空值被当成 Wayland**
  - 现象：环境里有 `WAYLAND_DISPLAY=`（空串）时，应用判成 Wayland 会话，滚动长截图报「需要 X11」；`check-env.sh` 却判成 X11，两边说法相反。
  - 修法：会话判断收拢到 `src-tauri/src/os/linux/session.rs`，`DISPLAY` / `WAYLAND_DISPLAY` / `XDG_SESSION_TYPE` 为空时都算没设，和 `check-env.sh` 一致。
  - 复测：`export WAYLAND_DISPLAY=` 后启动，滚动长截图、x11grab 录制都能用，设置页「系统」一栏显示 X11。
- [ ] **没有 `~/.config/user-dirs.dirs` 时录制失败**
  - 现象：录制目录没填时，报「无法确定录制保存目录」。`dirs::download_dir()` 在 Linux 上只认这个文件里登记的下载目录。
  - 修法：拿不到就退回 `~/Downloads`，目录不存在会自动建。
  - 复测：删掉或挪走 `user-dirs.dirs`、设置里录制目录留空，录一段，产物应落在 `~/Downloads`。
- [ ] **`npm test` 在 Node 20 上跑不了**（环境问题，代码没毛病）
  - 原因：jsdom 依赖的 undici 8 要 Node ≥ 22.19，它调用的 `worker_threads.markAsUncloneable` 在 Node 20 里没有；README 早就写的是 Node 22，但 `install-deps.sh` 还提示「Node.js 20+」。
  - 修法：`install-deps.sh` 改成提示 Node 22+；`check-env.sh` 新增 Node 版本检查，低于 22 直接标出来。
  - 复测：换 Node 22 后 `npm test` 全过。

另外，Prompt 页在这次测试之后改过（#31：润色结果改为弹窗，顶部「草稿」换成规则切换），报告里 F2 测的是旧版，需要按新流程再测一遍。

## 七、Linux 支持线：Ubuntu 22.04+ / Debian 12+（2026-09-25 起）

发版的 Linux 包改在 Ubuntu 22.04 上打，同一份 deb / AppImage 覆盖 22.04、24.04 和 Debian 12。
CI（`linux-packages.yml`）每次发版都会在 22.04 和 24.04（x86_64 / aarch64）上装包，并在 Xvfb 里无头启动一遍；下面这些是 CI 覆盖不到、要在真桌面上看的：

- [ ] Ubuntu 22.04 GNOME，X11 与 Wayland 各一次：装 deb（再试一次 AppImage），启动、窗口截图、录制（X11 走 x11grab，Wayland 走 portal）、装了 AppIndicator 扩展后的托盘。
  - Wayland 录制要重点看：22.04 上 PipeWire 的绑定是按 1.0.5 的头文件编的，运行时用的是系统的 0.3.48（原因见 `scripts/linux/README.md`）。CI 只验证了能链接、能启动，没有真的走一遍 portal 录制。
- [ ] Ubuntu 24.04：装同一份 deb，行为和以前在 24.04 上打的包一致（不回归）。
- [ ] 任一 KDE 桌面（X11 与 Wayland）：AppImage 启动、托盘。
- [ ] 没装 `ffmpeg` / `xdotool` 时：「本机能力」和相关功能给出的提示说得清楚缺什么、怎么装。
