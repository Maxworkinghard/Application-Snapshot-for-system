//! 最小化窗口还原（X11）。

use std::process::Command;
use std::thread;
use std::time::Duration;

/// 用 xdotool 还原最小化窗口。Wayland 下通常无效，返回可读错误。
pub fn restore_minimized(id: u32) -> Result<(), String> {
    if std::env::var_os("DISPLAY").is_none() {
        return Err("目标窗口已最小化，且当前无 X11 DISPLAY，无法自动还原（Wayland 无通用还原接口；请先手动还原） / minimized restore needs X11 DISPLAY or manual restore on Wayland".into());
    }

    let status = Command::new("xdotool")
        .args(["windowactivate", "--sync", &id.to_string()])
        .status();

    match status {
        Ok(code) if code.success() => {
            thread::sleep(Duration::from_millis(220));
            Ok(())
        }
        Ok(_) => Err("无法还原最小化窗口（xdotool 失败），请先手动还原 / xdotool failed to restore minimized window".into()),
        Err(_) => Err("目标窗口已最小化。请安装 xdotool 后重试，或先手动还原窗口 / install xdotool or restore the window manually".into()),
    }
}
