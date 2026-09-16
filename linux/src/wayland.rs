//! Wayland 后端：
//! - 截图走 xdg-desktop-portal（org.freedesktop.portal.Screenshot）
//! - 全局快捷键走 GlobalShortcuts portal（GNOME 48+ / KDE 6 支持），
//!   不可用时提示用户在桌面环境里把快捷键绑定到 `windowsnap capture`
//! - 剪贴板走 wl-clipboard-rs（data-control 协议）

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use crate::{notify, Msg, Result};

const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";

pub struct WaylandBackend {
    owned: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
}

impl WaylandBackend {
    pub fn new() -> Self {
        Self {
            owned: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 截图 → 前台线程供数剪贴板（被替换时线程退出）。
    pub fn capture_and_copy(&self) -> Result<String> {
        let png = take_screenshot()?;

        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.owned.store(true, Ordering::SeqCst);
        let owned = Arc::clone(&self.owned);
        let gen_ref = Arc::clone(&self.generation);
        std::thread::spawn(move || {
            let mut options = wl_clipboard_rs::copy::Options::new();
            options.foreground(true); // 阻塞供数，直到剪贴板被别人接管
            let _ = options.copy(
                wl_clipboard_rs::copy::Source::Bytes(png.into_boxed_slice()),
                wl_clipboard_rs::copy::MimeType::Specific("image/png".into()),
            );
            if gen_ref.load(Ordering::SeqCst) == generation {
                owned.store(false, Ordering::SeqCst);
            }
        });

        Ok("屏幕".to_string())
    }

    pub fn clear_if_owned(&self) {
        if self.owned.swap(false, Ordering::SeqCst) {
            let _ = wl_clipboard_rs::copy::clear(
                wl_clipboard_rs::copy::ClipboardType::Regular,
                wl_clipboard_rs::copy::Seat::All,
            );
        }
    }
}

/// 独立模式：后台 fork 供数（进程可退出），不做自动清空。
pub fn copy_background(png: Vec<u8>) -> Result<()> {
    let options = wl_clipboard_rs::copy::Options::new();
    options
        .copy(
            wl_clipboard_rs::copy::Source::Bytes(png.into_boxed_slice()),
            wl_clipboard_rs::copy::MimeType::Specific("image/png".into()),
        )
        .map_err(|e| format!("写入剪贴板失败：{e}"))?;
    Ok(())
}

// ---------- portal 截图 ----------

/// 通过 Screenshot portal 截图并读回 PNG 字节。
/// 说明：portal 无法只截某个窗口；由桌面环境决定交互方式（多为全屏或弹选择框）。
pub fn take_screenshot() -> Result<Vec<u8>> {
    let conn = zbus::blocking::Connection::session()?;

    let mut options: HashMap<&str, Value> = HashMap::new();
    let token = request_token();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("interactive", Value::from(false));

    let (code, results) = portal_request(&conn, "Screenshot", "Screenshot", &token, &("", options))?;
    if code != 0 {
        return Err("截图被取消或失败（portal 返回非 0）".into());
    }
    let uri = results
        .get("uri")
        .and_then(value_as_string)
        .ok_or("portal 未返回截图文件地址")?;

    let path = uri
        .strip_prefix("file://")
        .ok_or_else(|| format!("无法解析截图地址：{uri}"))?;
    let path = percent_decode(path);
    let png = std::fs::read(&path).map_err(|e| format!("读取截图文件失败：{e}"))?;
    // 承诺"不落盘"：读完即删 portal 留下的文件
    let _ = std::fs::remove_file(&path);
    Ok(png)
}

// ---------- GlobalShortcuts portal ----------

pub fn spawn_global_shortcuts(tx: Sender<Msg>, shortcut: String) {
    std::thread::spawn(move || {
        if let Err(e) = run_global_shortcuts(&tx, &shortcut) {
            eprintln!("windowsnap: GlobalShortcuts portal 不可用：{e}");
            notify::notify(
                notify::ToastKind::Error,
                "全局快捷键不可用",
                "请在系统快捷键设置中把组合键绑定到命令：windowsnap capture",
            );
        }
    });
}

fn run_global_shortcuts(tx: &Sender<Msg>, shortcut: &str) -> Result<()> {
    let conn = zbus::blocking::Connection::session()?;

    // 1. CreateSession
    let token = request_token();
    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert("session_handle_token", Value::from("windowsnap"));
    let (code, results) =
        portal_request(&conn, "GlobalShortcuts", "CreateSession", &token, &(options,))?;
    if code != 0 {
        return Err("CreateSession 被拒绝".into());
    }
    let session = results
        .get("session_handle")
        .and_then(value_as_string)
        .ok_or("未返回 session_handle")?;
    let session_path = ObjectPath::try_from(session.as_str())
        .map_err(|e| format!("session_handle 非法：{e}"))?;

    // 2. BindShortcuts（桌面环境可能弹确认框，用户批准一次即可）
    let token = request_token();
    let mut bind_options: HashMap<&str, Value> = HashMap::new();
    bind_options.insert("handle_token", Value::from(token.as_str()));
    let mut shortcut_props: HashMap<&str, Value> = HashMap::new();
    shortcut_props.insert("description", Value::from("截取当前活动窗口并复制"));
    shortcut_props.insert("preferred_trigger", Value::from(shortcut.to_uppercase()));
    let shortcuts = vec![("capture", shortcut_props)];
    let (code, _) = portal_request(
        &conn,
        "GlobalShortcuts",
        "BindShortcuts",
        &token,
        &(&session_path, shortcuts, "", bind_options),
    )?;
    if code != 0 {
        return Err("BindShortcuts 被拒绝".into());
    }

    // 3. 监听 Activated 信号
    let portal = zbus::blocking::Proxy::new(
        &conn,
        PORTAL_DEST,
        PORTAL_PATH,
        "org.freedesktop.portal.GlobalShortcuts",
    )?;
    let signals = portal.receive_signal("Activated")?;
    for message in signals {
        let body = message.body();
        let (_session, shortcut_id, _timestamp, _opts): (
            OwnedObjectPath,
            String,
            u64,
            HashMap<String, OwnedValue>,
        ) = match body.deserialize() {
            Ok(v) => v,
            Err(_) => continue,
        };
        if shortcut_id == "capture" {
            tx.send(Msg::Capture).map_err(|_| "主循环已退出")?;
        }
    }
    Ok(())
}

// ---------- portal request/response 通用流程 ----------

/// portal 的异步应答模式：方法返回 Request 句柄，结果经 Response 信号送达。
/// 用 handle_token 预知句柄路径，先订阅再调用，避免竞态。
fn portal_request<B>(
    conn: &zbus::blocking::Connection,
    interface: &str,
    method: &str,
    token: &str,
    body: &B,
) -> Result<(u32, HashMap<String, OwnedValue>)>
where
    B: zbus::export::serde::ser::Serialize + zbus::zvariant::DynamicType,
{
    let unique = conn
        .unique_name()
        .ok_or("D-Bus 连接缺少 unique name")?
        .trim_start_matches(':')
        .replace('.', "_");
    let request_path = format!("/org/freedesktop/portal/desktop/request/{unique}/{token}");

    let request_proxy = zbus::blocking::Proxy::new(
        conn,
        PORTAL_DEST,
        request_path,
        "org.freedesktop.portal.Request",
    )?;
    let mut signals = request_proxy.receive_signal("Response")?;

    let portal_proxy = zbus::blocking::Proxy::new(
        conn,
        PORTAL_DEST,
        PORTAL_PATH,
        format!("org.freedesktop.portal.{interface}"),
    )?;
    let _handle: OwnedObjectPath = portal_proxy.call(method, body)?;

    let message = signals.next().ok_or("portal 连接中断")?;
    let (code, results): (u32, HashMap<String, OwnedValue>) = message.body().deserialize()?;
    Ok((code, results))
}

fn request_token() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("windowsnap_{}_{n}", std::process::id())
}

fn value_as_string(value: &OwnedValue) -> Option<String> {
    if let Ok(s) = value.downcast_ref::<&str>() {
        return Some(s.to_string());
    }
    if let Ok(p) = value.downcast_ref::<ObjectPath>() {
        return Some(p.to_string());
    }
    None
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&input[i + 1..i + 3], 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}
