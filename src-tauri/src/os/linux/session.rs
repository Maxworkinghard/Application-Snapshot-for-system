//! 会话类型：X11 还是 Wayland。录制、滚动长截图、带光标静帧、还原最小化都按它选路。
//!
//! 环境变量「设了但是空的」一律按没设处理。`export WAYLAND_DISPLAY=` 这类空值在容器、
//! 远程桌面和一些启动脚本里并不少见，按「存在」算会把 X11 会话误判成 Wayland，
//! 滚动长截图、x11grab 录制都会被挡掉。scripts/linux/check-env.sh 也是这条规则（`-n`）。

use std::env;

fn non_empty(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

/// X11 的 `$DISPLAY`；没设或为空时是 None。
pub fn x11_display() -> Option<String> {
    non_empty("DISPLAY")
}

/// 当前会话是 Wayland——不管 XWayland 有没有把 `$DISPLAY` 撑起来。
pub fn is_wayland_session() -> bool {
    non_empty("WAYLAND_DISPLAY").is_some()
        || non_empty("XDG_SESSION_TYPE").is_some_and(|kind| kind.eq_ignore_ascii_case("wayland"))
}

/// 给错误信息与设置页用的会话名称。
pub fn display_server_label() -> &'static str {
    match (is_wayland_session(), x11_display().is_some()) {
        (true, false) => "Wayland",
        (true, true) => "Wayland (XWayland available)",
        (false, true) => "X11",
        (false, false) => "unknown",
    }
}
