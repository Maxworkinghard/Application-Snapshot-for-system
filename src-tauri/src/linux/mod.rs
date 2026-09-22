//! Linux 平台适配：录制显示、最小化还原、XDG 自启、进程图标。
//!
//! 与 Windows adapter 对称——成熟的系统能力直接用（xdotool / XDG / Freedesktop / portal），
//! 不为了语言统一重写。不可用时返回可读错误，不静默假装成功。

mod autostart;
mod icon;
mod recording;
mod scrolling;
mod still;
mod window;

pub use autostart::{apply_launch_on_boot, autostart_capability};
pub use icon::icon_for_process;
pub use recording::{
    display_server_label, is_wayland_session, recording_available, recording_capability_detail,
    start_recording, ActiveRecording,
};
pub use scrolling::capture_scrolling_window;
pub use still::{capture_primary_with_cursor, capture_region_with_cursor};
pub use window::restore_minimized;
