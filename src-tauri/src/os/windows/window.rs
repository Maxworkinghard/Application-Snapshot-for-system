//! 窗口操作：还原最小化的窗口。

use std::{thread, time::Duration};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{IsIconic, ShowWindow, SW_RESTORE},
};

/// xcap 在 Windows 上的窗口 id 就是 HWND。
pub(crate) fn restore_minimized(id: u32) -> Result<(), String> {
    let handle: HWND = id as usize as *mut _;
    unsafe {
        // ShowWindow 的返回值是「调用前窗口是否可见」，不是成败，据此判断会误报成功。
        let _ = ShowWindow(handle, SW_RESTORE);
    }
    thread::sleep(Duration::from_millis(220));
    // 真正的成败只能事后问：仍是最小化就说明系统或目标应用没有接受这次还原。
    if unsafe { IsIconic(handle) } != 0 {
        return Err("目标窗口仍处于最小化，未能还原".into());
    }
    Ok(())
}
