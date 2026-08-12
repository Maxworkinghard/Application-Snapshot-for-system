//! 桌面通知（org.freedesktop.Notifications），对应 macOS 端的 Toast。

use std::collections::HashMap;

/// 发通知，失败静默（无通知服务时不影响截图主流程）。
pub fn notify(summary: &str, body: &str) {
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
