mod dbus_service;
mod notify;
mod tray;
mod wayland;
mod x11;

use std::sync::mpsc;
use std::time::{Duration, Instant};

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Result<T> = std::result::Result<T, Error>;

/// 主循环消息：来自热键 / 托盘 / D-Bus 触发。
pub enum Msg {
    Capture,
    Quit,
}

pub struct Config {
    pub shortcut: String,
}

const AUTO_CLEAR: Duration = Duration::from_secs(60);

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

fn load_config() -> Config {
    let mut shortcut = "Alt+Shift+2".to_string();
    let path = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".config")))
        .map(|base| base.join("windowsnap/config.toml"));
    if let Some(path) = path {
        if let Ok(text) = std::fs::read_to_string(path) {
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or("").trim();
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim() == "shortcut" {
                        let value = value.trim().trim_matches('"').trim_matches('\'');
                        if !value.is_empty() {
                            shortcut = value.to_string();
                        }
                    }
                }
            }
        }
    }
    Config { shortcut }
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
           windowsnap            常驻运行：全局快捷键 + 托盘 + 60 秒自动清空剪贴板\n\
           windowsnap capture    截取一次：优先转发给常驻进程，否则独立执行\n\
         \n\
         配置：~/.config/windowsnap/config.toml\n\
           shortcut = \"Alt+Shift+2\"   # X11 下生效；Wayland 由 GlobalShortcuts portal 或桌面环境绑定"
    );
}

fn run_daemon() -> Result<()> {
    let config = load_config();
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

    tray::spawn(tx.clone(), config.shortcut.clone());

    match detect_session()? {
        Session::X11 => {
            let backend = x11::X11Backend::new()?;
            match backend.grab_hotkey(&config.shortcut) {
                Ok(()) => notify::notify(
                    "应用快照已启动",
                    &format!("按 {} 截取当前活动窗口", config.shortcut),
                ),
                Err(e) => notify::notify(
                    "快捷键注册失败",
                    &format!("{e}。仍可用命令 windowsnap capture 触发"),
                ),
            }
            backend.spawn_event_thread(tx.clone());
            event_loop(rx, || backend.capture_and_copy(), || backend.clear_if_owned());
        }
        Session::Wayland => {
            let backend = wayland::WaylandBackend::new();
            wayland::spawn_global_shortcuts(tx.clone(), config.shortcut.clone());
            notify::notify(
                "应用快照已启动",
                "Wayland 下由 GlobalShortcuts portal 或桌面环境快捷键触发",
            );
            event_loop(rx, || backend.capture_and_copy(), || backend.clear_if_owned());
        }
    }
    Ok(())
}

/// 主事件循环：串行处理截取请求，管理 60 秒自动清空。
fn event_loop(
    rx: mpsc::Receiver<Msg>,
    mut capture: impl FnMut() -> Result<String>,
    clear: impl Fn(),
) {
    let mut deadline: Option<Instant> = None;
    loop {
        let msg = match deadline {
            Some(d) => match rx.recv_timeout(d.saturating_duration_since(Instant::now())) {
                Ok(m) => Some(m),
                Err(mpsc::RecvTimeoutError::Timeout) => None,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            },
        };
        match msg {
            Some(Msg::Capture) => {
                // 吸掉排队的重复触发（按住快捷键会自动重复）
                let mut quit_after = false;
                while let Ok(extra) = rx.try_recv() {
                    if matches!(extra, Msg::Quit) {
                        quit_after = true;
                        break;
                    }
                }
                match capture() {
                    Ok(name) => {
                        notify::notify("已复制窗口截图", &format!("{name} — 60 秒后自动清空"));
                        deadline = Some(Instant::now() + AUTO_CLEAR);
                    }
                    Err(e) => {
                        notify::notify("截取失败", &e.to_string());
                    }
                }
                if quit_after {
                    return;
                }
            }
            Some(Msg::Quit) => return,
            None => {
                clear();
                deadline = None;
            }
        }
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
            notify::notify("已复制窗口截图", &format!("{name} — 60 秒后自动清空"));
            // X11 剪贴板由 owner 进程供数，需存活到被替换或超时
            backend.serve_until(Instant::now() + AUTO_CLEAR);
            backend.clear_if_owned();
        }
        Session::Wayland => {
            let png = wayland::take_screenshot()?;
            wayland::copy_background(png)?;
            notify::notify("已复制窗口截图", "独立模式下不自动清空剪贴板");
        }
    }
    Ok(())
}
