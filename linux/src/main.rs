mod dbus_service;
mod desktop;
mod dialog;
mod gifpet;
mod notify;
mod pet;
mod polish;
mod prompts;
mod record;
mod settings;
mod tray;
mod wayland;
mod x11;

use std::collections::VecDeque;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use settings::{pet_position, save_desktop_pet_position, save_directory, save_pet_position};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/// 主循环消息：来自热键 / 托盘 / 悬浮球菜单 / D-Bus 触发。
pub enum Msg {
    Capture,
    CaptureList,
    CapturePreviousApp,
    Polish,
    StopPolish,
    StartRecording,
    StopRecording,
    ToggleRecording,
    TogglePanel,
    OpenSettings,
    ManagePrompts,
    Quit,
}

const AUTO_CLEAR: Duration = Duration::from_secs(60);

/// 截图成功后播放快门声：优先 libcanberra（freedesktop 音效主题的
/// camera-shutter 事件），回退 paplay / pw-play 播放主题自带文件。失败静默。
fn play_shutter_sound() {
    if Command::new("canberra-gtk-play")
        .args(["-i", "camera-shutter"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .is_ok()
    {
        return;
    }
    let file = "/usr/share/sounds/freedesktop/stereo/camera-shutter.oga";
    for player in ["paplay", "pw-play"] {
        if Command::new(player)
            .arg(file)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .is_ok()
        {
            return;
        }
    }
}

enum Session {
    X11,
    Wayland,
}

fn detect_session() -> Result<Session> {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        Ok(Session::Wayland)
    } else if std::env::var_os("DISPLAY").is_some() {
        Ok(Session::X11)
    } else {
        Err("未检测到图形会话（WAYLAND_DISPLAY / DISPLAY 均未设置）".into())
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        None | Some("daemon") => run_daemon(),
        Some("capture") => run_capture_once(),
        Some("--help") | Some("-h") | Some("help") => {
            print_help();
            Ok(())
        }
        Some(other) => Err(format!("未知命令：{other}（试试 windowsnap --help）").into()),
    };
    if let Err(e) = result {
        eprintln!("windowsnap: {e}");
        std::process::exit(1);
    }
}

fn print_help() {
    println!(
        "应用快照 Linux 版\n\
         \n\
         用法：\n\
           windowsnap            常驻运行：快捷键截图 + 悬浮球 + 托盘菜单 + 剪贴板 60 秒自动清空\n\
           windowsnap capture    截取一次：优先转发给常驻进程，否则独立执行\n\
         \n\
         功能（与 macOS / Windows 端对齐）：\n\
           - 悬浮球：显示上一个前台应用的图标，点击弹菜单，可拖动、位置记忆\n\
           - 右键悬浮球 / 托盘「设置…」：绑定快捷键（截图 / 录制 / 截取上一个应用 / 润色）与润色服务\n\
           - 应用快照…：从窗口列表选择目标窗口截图\n\
           - 窗口录制：ffmpeg 录制当前活动窗口为 MP4（仅 X11）\n\
           - 润色 Prompt：剪贴板草稿 → 确认 → 大模型改写 → 写回，处理中可停止\n\
         \n\
         配置：~/.config/windowsnap/config.toml\n\
           shortcut = \"Alt+Shift+2\"        # X11 下生效；Wayland 由 GlobalShortcuts portal 或桌面环境绑定\n\
           shortcut_record = \"Alt+Shift+R\"  # 留空 = 不绑定\n\
           shortcut_previous_app = \"\"      # 默认不绑定，在设置中自行决定\n\
           shortcut_polish = \"\"            # 默认不绑定\n\
           save_dir = \"~/Videos/应用快照\"   # 录制文件保存目录\n\
           ui.mode = \"bubble\"              # bubble（悬浮球，默认）| pet（桌宠）\n\
           pet.skin = \"\"                   # 桌宠形象目录名（~/.local/share/windowsnap/pet/<形象>/）\n\
           polish.kind = \"openai\"          # openai | anthropic\n\
           polish.base_url = \"https://api.deepseek.com\"\n\
           polish.model = \"deepseek-chat\"\n\
           polish.api_key = \"YOUR_API_KEY\""
    );
}

struct Daemon {
    backend: Option<Arc<x11::X11Backend>>,
    wayland: Option<wayland::WaylandBackend>,
    recorder: record::Recorder,
    polish_state: polish::PolishState,
    polish_config: polish::PolishConfig,
    /// 剪贴板自动清空的截止时间。
    deadline: Option<Instant>,
    tx: mpsc::Sender<Msg>,
}

impl Daemon {
    fn clipboard_session(&self) -> polish::ClipboardSession {
        match &self.backend {
            Some(backend) => polish::ClipboardSession::X11(Arc::clone(backend)),
            None => polish::ClipboardSession::Wayland,
        }
    }

    /// 返回 false 表示退出主循环。
    fn handle(&mut self, msg: Msg) -> bool {
        match msg {
            Msg::Capture => self.capture(),
            Msg::CaptureList => self.capture_list(),
            Msg::CapturePreviousApp => self.capture_previous_app(),
            Msg::Polish => self.start_polish(),
            Msg::StopPolish => self.stop_polish(),
            Msg::StartRecording => self.start_recording(),
            Msg::StopRecording => self.stop_recording(),
            Msg::ToggleRecording => self.toggle_recording(),
            Msg::TogglePanel => self.open_panel(),
            Msg::OpenSettings => self.open_settings(),
            Msg::ManagePrompts => dialog::manage_prompts(),
            Msg::Quit => return false,
        }
        true
    }

    fn run(&mut self, rx: mpsc::Receiver<Msg>) {
        let mut pending: VecDeque<Msg> = VecDeque::new();
        loop {
            let msg = if let Some(msg) = pending.pop_front() {
                Some(msg)
            } else if let Some(deadline) = self.deadline {
                match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(msg) => Some(msg),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match rx.recv() {
                    Ok(msg) => Some(msg),
                    Err(_) => return,
                }
            };

            let Some(msg) = msg else {
                self.clear_clipboard();
                continue;
            };

            // 吸掉排队的重复截图触发（按住快捷键会自动重复），其他消息暂存稍后处理
            if matches!(msg, Msg::Capture) {
                let mut quit_after = false;
                while let Ok(extra) = rx.try_recv() {
                    if matches!(extra, Msg::Quit) {
                        quit_after = true;
                        break;
                    }
                    if !matches!(extra, Msg::Capture) {
                        pending.push_back(extra);
                    }
                }
                if quit_after {
                    return;
                }
            }

            if !self.handle(msg) {
                return;
            }
        }
    }

    fn clear_clipboard(&mut self) {
        self.deadline = None;
        if let Some(backend) = &self.backend {
            backend.clear_if_owned();
        } else if let Some(backend) = &self.wayland {
            backend.clear_if_owned();
        }
    }

    fn capture(&mut self) {
        desktop::hide_for_capture(true);
        let result = match (&self.backend, &self.wayland) {
            (Some(backend), _) => backend.capture_and_copy(),
            (None, Some(backend)) => backend.capture_and_copy(),
            (None, None) => Err("截图后端未初始化".into()),
        };
        desktop::hide_for_capture(false);
        match result {
            Ok(name) => {
                play_shutter_sound();
                notify::notify_kind(
                    "已复制窗口截图",
                    &format!("{name} — 60 秒后自动清空"),
                    notify::NotifyKind::Success,
                );
                self.deadline = Some(Instant::now() + AUTO_CLEAR);
            }
            Err(e) => notify::notify_kind("截取失败", &e.to_string(), notify::NotifyKind::Error),
        }
    }

    fn capture_list(&mut self) {
        let picked = match &self.backend {
            Some(backend) => match backend.list_windows() {
                Ok(windows) => {
                    let titles: Vec<String> = windows.iter().map(|(_, title)| title.clone()).collect();
                    dialog::choose_window(&titles).and_then(|title| {
                        windows
                            .iter()
                            .find(|(_, existing)| *existing == title)
                            .map(|(window, _)| *window)
                    })
                }
                Err(e) => {
                    notify::notify("获取窗口列表失败", &e.to_string());
                    None
                }
            },
            None => {
                notify::notify(
                    "窗口列表不可用",
                    "Wayland 会话下暂不支持窗口列表，请使用快捷键截取当前窗口",
                );
                None
            }
        };
        let Some(window) = picked else { return };

        desktop::hide_for_capture(true);
        let result = match &self.backend {
            Some(backend) => backend.capture_window(window),
            None => Err("截图后端未初始化".into()),
        };
        desktop::hide_for_capture(false);
        match result {
            Ok(name) => {
                play_shutter_sound();
                notify::notify_kind(
                    "已复制窗口截图",
                    &format!("{name} — 60 秒后自动清空"),
                    notify::NotifyKind::Success,
                );
                self.deadline = Some(Instant::now() + AUTO_CLEAR);
            }
            Err(e) => notify::notify_kind("截取失败", &e.to_string(), notify::NotifyKind::Error),
        }
    }

    fn start_polish(&mut self) {
        if self.polish_state.busy.load(Ordering::SeqCst) {
            notify::notify("润色进行中", "请等待完成，或选择「停止润色」");
            return;
        }
        if !self.polish_config.is_complete() {
            notify::notify(
                "润色未配置",
                "请在 ~/.config/windowsnap/config.toml 填写 polish.base_url / polish.model / polish.api_key",
            );
            return;
        }
        self.polish_state.busy.store(true, Ordering::SeqCst);
        self.polish_state.cancel.store(false, Ordering::SeqCst);
        // 润色结果要留在剪贴板里，取消截图的自动清空倒计时
        self.deadline = None;
        let config = self.polish_config.clone();
        let state = self.polish_state.clone();
        let session = self.clipboard_session();
        std::thread::spawn(move || polish::run_polish(&config, &session, &state));
    }

    fn stop_polish(&mut self) {
        if !self.polish_state.busy.load(Ordering::SeqCst) {
            notify::notify("没有进行中的润色", "");
            return;
        }
        self.polish_state.stop();
        notify::notify("停止润色", "正在中止当前请求");
    }

    fn start_recording(&mut self) {
        if self.recorder.active.load(Ordering::SeqCst) {
            notify::notify("已在录制中", "请先停止当前录制");
            return;
        }
        let Some(backend) = &self.backend else {
            notify::notify_kind(
                "录制失败",
                "窗口录制仅支持 X11 会话（依赖 ffmpeg x11grab）",
                notify::NotifyKind::Error,
            );
            return;
        };
        let (x, y, width, height) = match backend.active_window_rect() {
            Ok(rect) => rect,
            Err(e) => {
                notify::notify_kind("录制失败", &e.to_string(), notify::NotifyKind::Error);
                return;
            }
        };
        // 悬浮球先藏起来，免得被录进画面
        desktop::hide_for_recording(true);
        match self.recorder.start(x, y, width, height) {
            Ok(_) => notify::notify("录制中", "录制当前活动窗口；通过托盘或悬浮球菜单停止"),
            Err(e) => {
                desktop::hide_for_recording(false);
                notify::notify_kind("录制失败", &e.to_string(), notify::NotifyKind::Error);
            }
        }
    }

    fn stop_recording(&mut self) {
        match self.recorder.stop() {
            Ok(path) => notify::notify_kind(
                "录制完成",
                &format!("已保存到 {path}"),
                notify::NotifyKind::Success,
            ),
            Err(e) => notify::notify("录制结束", &e.to_string()),
        }
        desktop::hide_for_recording(false);
    }

    /// 录制快捷键是「开始/停止」二合一：按当前状态切换。
    fn toggle_recording(&mut self) {
        if self.recorder.active.load(Ordering::SeqCst) {
            self.stop_recording();
        } else {
            self.start_recording();
        }
    }

    /// 截取「上一个前台应用」窗口（悬浮球当前显示图标的目标）。
    fn capture_previous_app(&mut self) {
        let window = desktop::previous_window();
        if window == x11rb::NONE {
            notify::notify("尚未记录上一个应用", "切换一次前台应用后再试");
            return;
        }
        let Some(backend) = &self.backend else {
            notify::notify("截取失败", "Wayland 会话下暂不支持截取上一个应用");
            return;
        };
        desktop::hide_for_capture(true);
        let result = backend.capture_window(window);
        desktop::hide_for_capture(false);
        match result {
            Ok(name) => {
                play_shutter_sound();
                notify::notify_kind(
                    "已复制窗口截图",
                    &format!("{name} — 60 秒后自动清空"),
                    notify::NotifyKind::Success,
                );
                self.deadline = Some(Instant::now() + AUTO_CLEAR);
            }
            Err(e) => notify::notify_kind("截取失败", &e.to_string(), notify::NotifyKind::Error),
        }
    }

    /// 右键悬浮球 / 托盘「设置…」：zenity 表单（快捷键绑定 + 润色服务），
    /// 保存后即时生效：润色配置更新内存，快捷键重新注册（仅 X11）。
    fn open_settings(&mut self) {
        let current = settings::load();
        let values = dialog::SettingsFormValues {
            shortcut: current.shortcut.clone(),
            shortcut_record: current.shortcut_record.clone(),
            shortcut_previous_app: current.shortcut_previous_app.clone(),
            shortcut_polish: current.shortcut_polish.clone(),
            polish_kind: if matches!(current.polish.kind, polish::PolishProtocolKind::Anthropic) {
                "anthropic".to_string()
            } else {
                "openai".to_string()
            },
            polish_base_url: current.polish.base_url.clone(),
            polish_model: current.polish.model.clone(),
            polish_api_key: current.polish.api_key.clone(),
            ui_mode_pet: current.ui_mode_pet,
            pet_skin: current.pet_skin.clone(),
        };
        let input = loop {
            let Some(input) = dialog::settings_form(&values) else { return };
            if input.ui_mode_pet && settings::available_skins().is_empty() {
                notify::notify_kind("未启用桌宠", &format!("未找到桌宠素材({}/<形象>/*.gif)",
                    settings::pet_asset_root().map(|p|p.display().to_string()).unwrap_or_default()), notify::NotifyKind::Error);
                continue;
            }
            break input;
        };

        // 快捷键：留空保持不变，填 none 解除绑定
        let resolve_shortcut = |input: &str, current: &str| -> String {
            let input = input.trim();
            if input.is_empty() {
                current.to_string()
            } else if input.eq_ignore_ascii_case("none") {
                String::new()
            } else {
                input.to_string()
            }
        };
        // 润色服务：留空保持不变
        let resolve_text = |input: &str, current: &str| -> String {
            let input = input.trim();
            if input.is_empty() {
                current.to_string()
            } else {
                input.to_string()
            }
        };

        let shortcut = resolve_shortcut(&input.shortcut, &values.shortcut);
        let shortcut_record = resolve_shortcut(&input.shortcut_record, &values.shortcut_record);
        let shortcut_previous_app =
            resolve_shortcut(&input.shortcut_previous_app, &values.shortcut_previous_app);
        let shortcut_polish = resolve_shortcut(&input.shortcut_polish, &values.shortcut_polish);
        let polish_kind = resolve_text(&input.polish_kind, &values.polish_kind).to_ascii_lowercase();
        let polish_base_url = resolve_text(&input.polish_base_url, &values.polish_base_url);
        let polish_model = resolve_text(&input.polish_model, &values.polish_model);
        let polish_api_key = resolve_text(&input.polish_api_key, &values.polish_api_key);

        if !polish_base_url.is_empty()
            && !polish_base_url.starts_with("http://")
            && !polish_base_url.starts_with("https://")
        {
            notify::notify("设置未保存", "润色 Base URL 需以 http:// 或 https:// 开头");
            return;
        }

        // 先留一份旧配置：新快捷键注册失败时回滚，保持原有绑定可用
        let old_settings = settings::load();

        settings::save_shortcuts(&shortcut, &shortcut_record, &shortcut_previous_app, &shortcut_polish);
        settings::save_polish(&polish_kind, &polish_base_url, &polish_model, &polish_api_key);
        settings::save_ui_mode(input.ui_mode_pet);
        if !input.pet_skin.is_empty() { settings::save_pet_skin(&input.pet_skin); }
        desktop::apply_presentation(self.tx.clone());
        self.polish_config = polish::PolishConfig {
            kind: if polish_kind == "anthropic" {
                polish::PolishProtocolKind::Anthropic
            } else {
                polish::PolishProtocolKind::OpenAICompatible
            },
            base_url: polish_base_url,
            model: polish_model,
            api_key: polish_api_key,
        };

        // 快捷键即时生效；Wayland 会话由桌面环境 / portal 管理绑定，重启后生效
        match &self.backend {
            Some(backend) => {
                backend.ungrab_all();
                let failures = register_hotkeys(backend, &settings::load());
                if failures.is_empty() {
                    notify::notify_kind(
                        "设置已保存",
                        "快捷键、润色配置与桌面形式已生效",
                        notify::NotifyKind::Success,
                    );
                } else {
                    // 回滚到旧绑定，避免半绑定状态（新配置已存盘，下次启动仍会尝试）
                    backend.ungrab_all();
                    let _ = register_hotkeys(backend, &old_settings);
                    notify::notify(
                        "部分快捷键注册失败",
                        &format!("{}；已恢复之前的快捷键绑定", failures.join("；")),
                    );
                }
            }
            None => notify::notify_kind(
                "设置已保存",
                "润色配置与桌面形式已生效；Wayland 快捷键由桌面环境管理",
                notify::NotifyKind::Success,
            ),
        }
    }

    /// 悬浮球点击菜单（zenity / kdialog 进程外 UI）。
    fn open_panel(&mut self) {
        let recording = self.recorder.active.load(Ordering::SeqCst);
        let polishing = self.polish_state.busy.load(Ordering::SeqCst);

        let mut items: Vec<&str> = vec!["截取当前应用窗口"];
        if self.backend.is_some() {
            items.push("应用快照…（选择窗口）");
        }
        items.push(if recording { "停止窗口录制" } else { "开始窗口录制" });
        items.push(if polishing { "停止润色" } else { "润色 Prompt" });
        items.push("退出");

        let Some(index) = dialog::choose_action(&items) else { return };
        let Some(choice) = items.get(index).map(|item| item.to_string()) else { return };
        match choice.as_str() {
            "截取当前应用窗口" => {
                self.handle(Msg::Capture);
            }
            "应用快照…（选择窗口）" => {
                self.handle(Msg::CaptureList);
            }
            "开始窗口录制" => {
                self.handle(Msg::StartRecording);
            }
            "停止窗口录制" => {
                self.handle(Msg::StopRecording);
            }
            "润色 Prompt" => {
                self.handle(Msg::Polish);
            }
            "停止润色" => {
                self.handle(Msg::StopPolish);
            }
            "退出" => {
                self.handle(Msg::Quit);
            }
            _ => {}
        }
    }
}

fn run_daemon() -> Result<()> {
    let settings = settings::load();
    let (tx, rx) = mpsc::channel::<Msg>();

    // D-Bus 名同时充当单实例锁
    let _dbus = match dbus_service::serve(tx.clone()) {
        Ok(conn) => Some(conn),
        Err(zbus::Error::NameTaken) => {
            return Err("应用快照已在运行（D-Bus 名 local.windowsnap 已被占用）".into());
        }
        Err(e) => {
            eprintln!("windowsnap: D-Bus 服务不可用（{e}），CLI 触发将走独立模式");
            None
        }
    };

    let recorder = record::Recorder::new();
    let polish_state = polish::PolishState::new();
    tray::spawn(
        tx.clone(),
        settings.shortcut.clone(),
        Arc::clone(&recorder.active),
        Arc::clone(&polish_state.busy),
    );

    let mut daemon = Daemon {
        backend: None,
        wayland: None,
        recorder,
        polish_state,
        polish_config: settings.polish.clone(),
        deadline: None,
        tx: tx.clone(),
    };

    match detect_session()? {
        Session::X11 => {
            let backend = x11::X11Backend::new()?;
            let failures = register_hotkeys(&backend, &settings);
            if failures.is_empty() {
                notify::notify(
                    "应用快照已启动",
                    &format!("{}；右键悬浮球可打开设置", shortcut_summary(&settings)),
                );
            } else {
                notify::notify(
                    "部分快捷键注册失败",
                    &format!("{}。仍可用托盘菜单或悬浮球触发", failures.join("；")),
                );
            }
            backend.spawn_event_thread(tx.clone());
            daemon.backend = Some(backend);
        }
        Session::Wayland => {
            daemon.wayland = Some(wayland::WaylandBackend::new());
            wayland::spawn_global_shortcuts(tx.clone(), settings.shortcut.clone());
            notify::notify(
                "应用快照已启动",
                "Wayland 下由 GlobalShortcuts portal 或桌面环境快捷键触发",
            );
        }
    }

    if std::env::var_os("DISPLAY").is_some() { desktop::spawn_tracker(); }
    desktop::apply_presentation(tx.clone());
    daemon.run(rx);
    Ok(())
}

/// 注册所有已绑定的快捷键，返回失败的描述。
fn register_hotkeys(backend: &x11::X11Backend, settings: &settings::Settings) -> Vec<String> {
    let specs = [
        (&settings.shortcut, x11::HotkeyAction::Capture),
        (&settings.shortcut_record, x11::HotkeyAction::Record),
        (&settings.shortcut_previous_app, x11::HotkeyAction::PreviousApp),
        (&settings.shortcut_polish, x11::HotkeyAction::Polish),
    ];
    let mut failures = Vec::new();
    for (shortcut, action) in specs {
        if shortcut.is_empty() {
            continue;
        }
        if let Err(e) = backend.grab_hotkey(shortcut, action) {
            failures.push(format!("{shortcut}：{e}"));
        }
    }
    failures
}

/// 已绑定快捷键的启动摘要。
fn shortcut_summary(settings: &settings::Settings) -> String {
    let mut parts = Vec::new();
    if !settings.shortcut.is_empty() {
        parts.push(format!("截图 {}", settings.shortcut));
    }
    if !settings.shortcut_record.is_empty() {
        parts.push(format!("录制 {}", settings.shortcut_record));
    }
    if !settings.shortcut_previous_app.is_empty() {
        parts.push(format!("截取上一应用 {}", settings.shortcut_previous_app));
    }
    if !settings.shortcut_polish.is_empty() {
        parts.push(format!("润色 {}", settings.shortcut_polish));
    }
    if parts.is_empty() {
        "未绑定快捷键（右键悬浮球可设置）".to_string()
    } else {
        parts.join("；")
    }
}

fn run_capture_once() -> Result<()> {
    // 常驻进程在跑就转发给它（由它持有剪贴板和倒计时）
    if dbus_service::trigger_running_daemon() {
        return Ok(());
    }

    match detect_session()? {
        Session::X11 => {
            let backend = x11::X11Backend::new()?;
            let name = backend.capture_and_copy()?;
            play_shutter_sound();
            notify::notify("已复制窗口截图", &format!("{name} — 60 秒后自动清空"));
            // X11 剪贴板由 owner 进程供数，需存活到被替换或超时
            backend.serve_until(Instant::now() + AUTO_CLEAR);
            backend.clear_if_owned();
        }
        Session::Wayland => {
            let png = wayland::take_screenshot()?;
            wayland::copy_background(png)?;
            play_shutter_sound();
            notify::notify("已复制窗口截图", "独立模式下不自动清空剪贴板");
        }
    }
    Ok(())
}
