//! 最小化窗口还原（X11）。

use std::process::Command;
use std::thread;
use std::time::Duration;

/// 用 xdotool 还原最小化窗口。Wayland 下通常无效，返回可读错误。
///
/// ## 焦点抢占（平台取舍）
/// `xdotool windowactivate` 会把输入焦点抢到目标窗——这是 X11 上可靠取消最小化的
/// 常见代价，能力面板已披露，可接受。调用约定：
/// - **仅**在目标已最小化时调用本函数（见 `capture_window` / 滚动长截图入口）。
/// - 非最小化截图不要 activate，避免无谓抢焦点。
/// - 滚动长截图另有 `scrolling::activate_window`：翻页需要焦点收 `Page_Down`，与还原分开。
///
/// 实现上先 `windowmap --sync`（映射到屏幕），再 `windowactivate --sync` 确保从最小化
/// 真正出来；仅 map 在部分 WM 上仍会停在 IconicState。
pub fn restore_minimized(id: u32) -> Result<(), String> {
    if super::session::x11_display().is_none() {
        return Err("目标窗口已最小化，且当前无 X11 DISPLAY，无法自动还原（Wayland 无通用还原接口；请先手动还原） / minimized restore needs X11 DISPLAY or manual restore on Wayland".into());
    }

    let id_str = id.to_string();
    // 先 map：部分 WM 下足以解除最小化且比 activate 温和；失败不直接报错。
    let _ = Command::new("xdotool")
        .args(["windowmap", "--sync", &id_str])
        .status();

    let status = Command::new("xdotool")
        .args(["windowactivate", "--sync", &id_str])
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
