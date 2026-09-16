//! 桌面通知（org.freedesktop.Notifications），对应 macOS 端的 Toast。

use std::collections::HashMap;

use crate::gifpet::{post_pose, PetPose};

/// 通知的结果类型，决定桌宠的反应动作（与 Windows 端 Toast.cs 的三分支一致）。
pub enum NotifyKind {
    Success,
    Warning,
    Error,
}

/// 发通知，失败静默（无通知服务时不影响截图主流程）。
/// 未分类的通知一律按 Warning 归类，与 Windows 端的 default 分支一致。
pub fn notify(summary: &str, body: &str) {
    notify_kind(summary, body, NotifyKind::Warning);
}

/// 发通知并驱动桌宠做出反应：先派姿势再走 D-Bus，与 Windows 弹 Toast 前先 SetPose 同序。
pub fn notify_kind(summary: &str, body: &str, kind: NotifyKind) {
    post_pose(match kind {
        NotifyKind::Success => PetPose::Jumping,
        NotifyKind::Error => PetPose::Failed,
        NotifyKind::Warning => PetPose::Waiting,
    });
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
