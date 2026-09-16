//! 桌面呈现的公共控制；截图和录制各持一个隐藏标志。
use std::sync::{Mutex, atomic::{AtomicU32, Ordering}};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt, EventMask};
use x11rb::protocol::Event;

#[derive(Default)]
struct Visibility { capture: bool, recording: bool }
static VISIBILITY: Mutex<Visibility> = Mutex::new(Visibility { capture: false, recording: false });
static PREVIOUS_WINDOW: AtomicU32 = AtomicU32::new(0);

pub fn visible() -> bool {
    let flags = VISIBILITY.lock().unwrap();
    !flags.capture && !flags.recording
}

pub fn hide_for_capture(hidden: bool) {
    VISIBILITY.lock().unwrap().capture = hidden;
    apply_visibility();
}

pub fn hide_for_recording(hidden: bool) {
    VISIBILITY.lock().unwrap().recording = hidden;
    apply_visibility();
}

fn apply_visibility() {
    let visible = visible();
    crate::pet::set_pet_visible(visible);
    crate::gifpet::set_visible(visible);
    if !visible {
        // X11 同步取消映射之后仍留一帧合成时间；Wayland 由控制通道确认提交。
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
}

pub fn previous_window() -> u32 { PREVIOUS_WINDOW.load(Ordering::SeqCst) }

/// 按当前配置切换悬浮球 / 桌宠，保存设置后立即生效（与 macOS / Windows 对齐）。
pub fn apply_presentation(tx: std::sync::mpsc::Sender<crate::Msg>) {
    let settings = crate::settings::load();
    if settings.ui_mode_pet {
        if let Some(skin) = crate::settings::resolve_skin(&settings.pet_skin) {
            if crate::gifpet::is_running_skin(&skin) {
                return;
            }
            crate::pet::shutdown();
            crate::gifpet::shutdown();
            crate::gifpet::spawn(tx, skin);
            return;
        }
    }
    crate::gifpet::shutdown();
    if !crate::pet::is_running() {
        crate::pet::spawn(tx);
    }
}

/// 追踪不依赖悬浮球窗体；切到 GIF 桌宠后「上一个应用」仍可用。
pub fn spawn_tracker() {
    std::thread::spawn(|| {
        if let Err(e) = track() { eprintln!("windowsnap: 活动窗口追踪不可用（{e}）"); }
    });
}

fn track() -> crate::Result<()> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let atom = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW")?.reply()?.atom;
    conn.change_window_attributes(root, &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE))?.check()?;
    let mut current = 0;
    loop {
        let active = conn.get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)?.reply()?
            .value32().and_then(|mut values| values.next()).unwrap_or(0);
        if active != 0 && active != current {
            PREVIOUS_WINDOW.store(current, Ordering::SeqCst);
            current = active;
        }
        loop {
            if matches!(conn.wait_for_event()?, Event::PropertyNotify(e) if e.window == root && e.atom == atom) { break; }
        }
    }
}
