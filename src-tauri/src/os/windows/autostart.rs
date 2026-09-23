//! Windows 开机自启：在 HKCU 的 Run 键下写一个值。
//! 不走「启动」文件夹的 .lnk —— 那需要 COM IShellLink，而且用户手动删掉快捷方式后
//! 设置项仍显示开启，状态会和系统对不上。注册表读写都在当前用户下，无需提权。

use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};
use windows_sys::Win32::System::Registry::{
    RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
};

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
/// 注册表值名。改名会在用户机器上留下卸不掉的旧值，所以固定不动。
const VALUE_NAME: &str = "AppSnapshot";
/// ERROR_FILE_NOT_FOUND：值本来就不存在，删除按成功处理。
const ERROR_FILE_NOT_FOUND: u32 = 2;

fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text).encode_wide().chain(once(0)).collect()
}

/// 可执行文件路径要带引号：路径含空格时，不加引号会被 Windows 拆成程序名 + 参数。
fn command_line() -> Result<Vec<u16>, String> {
    let exe = std::env::current_exe().map_err(|error| format!("无法定位可执行文件：{error}"))?;
    Ok(wide(&format!("\"{}\"", exe.display())))
}

pub fn apply(enabled: bool) -> Result<(), String> {
    let key = wide(RUN_KEY);
    let name = wide(VALUE_NAME);
    let status = if enabled {
        let value = command_line()?;
        // REG_SZ 的字节数要含结尾的 NUL，少算会让读取方拿到没有终止符的串。
        let bytes = (value.len() * 2) as u32;
        unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                REG_SZ,
                value.as_ptr().cast(),
                bytes,
            )
        }
    } else {
        let status = unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
        if status == ERROR_FILE_NOT_FOUND {
            0
        } else {
            status
        }
    };
    if status != 0 {
        let action = if enabled { "写入" } else { "移除" };
        return Err(format!("无法{action}开机启动项（注册表错误 {status}）"));
    }
    Ok(())
}
