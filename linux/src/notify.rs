//! 桌面通知（org.freedesktop.Notifications），对应 macOS / Windows 端的 Toast。
//! 携带事件种类：桌宠据此做成功 / 失败 / 待机反应动画
//! （对应 Windows 端 ToastController.Notified 事件）。

use std::collections::HashMap;

/// Toast 事件种类，对应 Windows 端 ToastKind。
#[derive(Clone, Copy)]
pub enum ToastKind {
    Success,
    Error,
    Info,
}

/// 发通知并联动桌宠反应；通知本身失败静默（无通知服务时不影响截图主流程）。
pub fn notify(kind: ToastKind, summary: &str, body: &str) {
    crate::gif_pet::react(kind);
    let _ = try_notify(summary, body);
}

fn try_notify(summary: &str, body: &str) -> zbus::Result<()> {
    let conn = zbus::blocking::Connection::session()?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.Notifications",
        "/org/freedesktop/Notifications",
        "org.freedesktop.Notifications",
    )?;
    let _id: u32 = proxy.call(
        "Notify",
        &(
            "应用快照",
            0u32,
            "camera-photo",
            summary,
            body,
            Vec::<String>::new(),
            HashMap::<String, zbus::zvariant::Value>::new(),
            2500i32,
        ),
    )?;
    Ok(())
}
