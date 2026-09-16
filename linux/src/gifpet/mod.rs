//! 桌宠模式（GIF 动画）的后端无关部分：素材解码与姿势状态机。
//! 与悬浮球（pet.rs）互斥，同一时刻只存在一个桌面形象。

pub mod decode;
pub mod state;

use std::sync::mpsc::Sender;
use std::sync::Mutex;

pub use state::PetPose;

/// 通知事件投递姿势用的通道；桌宠线程启动时登记，没有桌宠在跑时发送被静默丢弃。
static POSE_TX: Mutex<Option<Sender<PetPose>>> = Mutex::new(None);

/// 桌宠线程启动时登记自己的姿势接收端。
pub fn set_pose_sender(tx: Sender<PetPose>) {
    if let Ok(mut guard) = POSE_TX.lock() {
        *guard = Some(tx);
    }
}

/// 投递一个姿势；没有桌宠在跑就什么都不做。
pub fn post_pose(pose: PetPose) {
    if let Ok(guard) = POSE_TX.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(pose);
        }
    }
}

mod render;
mod x11_surface;
mod layer_surface;

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

pub(super) enum Control {
    Visible(bool, Sender<()>),
    Quit,
}
static CONTROL: Mutex<Option<Sender<Control>>> = Mutex::new(None);
static CURRENT_SKIN: Mutex<Option<String>> = Mutex::new(None);

pub fn set_visible(visible: bool) {
    let sender = CONTROL.lock().ok().and_then(|guard| guard.clone());
    if let Some(sender) = sender {
        let (tx, rx) = mpsc::channel();
        if sender.send(Control::Visible(visible, tx)).is_ok() {
            let _ = rx.recv_timeout(Duration::from_secs(2));
        }
    }
}

pub fn is_running_skin(skin: &str) -> bool {
    CURRENT_SKIN.lock().ok().and_then(|guard| guard.clone()).as_deref() == Some(skin)
}

/// 停掉桌宠线程并等到窗口拆除；未在跑则立即返回。
pub fn shutdown() {
    let sender = CONTROL.lock().ok().and_then(|guard| guard.clone());
    if let Some(sender) = sender {
        let _ = sender.send(Control::Quit);
    }
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        if CONTROL.lock().map(|guard| guard.is_none()).unwrap_or(true) {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn spawn(tx: Sender<crate::Msg>, skin: String) {
    std::thread::spawn(move || {
        let (pose_tx, poses) = mpsc::channel();
        let (control_tx, controls) = mpsc::channel();
        set_pose_sender(pose_tx);
        *CURRENT_SKIN.lock().unwrap() = Some(skin.clone());
        *CONTROL.lock().unwrap() = Some(control_tx);
        let result = run(tx.clone(), &skin, &poses, &controls);
        *CONTROL.lock().unwrap() = None;
        *POSE_TX.lock().unwrap() = None;
        if CURRENT_SKIN.lock().unwrap().as_deref() == Some(skin.as_str()) {
            *CURRENT_SKIN.lock().unwrap() = None;
        }
        if let Err(e) = result {
            eprintln!("windowsnap: GIF 桌宠不可用（{e}），回退悬浮球");
            crate::pet::spawn(tx);
        }
    });
}

fn run(tx: Sender<crate::Msg>, skin: &str, poses: &Receiver<PetPose>, controls: &Receiver<Control>) -> crate::Result<()> {
    let root = crate::settings::pet_asset_root().ok_or("找不到桌宠素材目录")?;
    let mut state = state::PetState::load(&root.join(skin));
    let result = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        match layer_surface::Surface::connect() {
            Ok(surface) => surface.run(&mut state, &tx, poses, controls),
            Err(e) if std::env::var_os("DISPLAY").is_some() => {
                eprintln!("windowsnap: layer-shell 不可用（{e}），使用 X11/XWayland 桌宠");
                x11_surface::run(&mut state, &tx, poses, controls)
            }
            Err(e) => Err(e),
        }
    } else {
        x11_surface::run(&mut state, &tx, poses, controls)
    };
    state.dispose();
    result
}

/// Linux 的右键菜单沿用进程外对话框，不占住动画线程。
pub(super) fn context_menu(tx: &Sender<crate::Msg>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        match crate::dialog::choose_action(&["设置…", "退出应用快照"]) {
            Some(0) => { let _ = tx.send(crate::Msg::OpenSettings); }
            Some(1) => { let _ = tx.send(crate::Msg::Quit); }
            _ => {}
        }
    });
}
