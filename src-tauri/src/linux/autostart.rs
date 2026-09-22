//! XDG 开机自启（opt-in，与设置里「开机静默自启动」对齐）。
//!
//! 不写 systemd user unit，也不默认 enable——Windows 侧同样是用户勾选后才生效。
//! 只在 `~/.config/autostart/` 放 / 删一份 `.desktop`。

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;

const DESKTOP_NAME: &str = "com.appsnapshot.prompt-pet-shortcut.desktop";

fn autostart_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autostart")
}

fn desktop_path() -> PathBuf {
    autostart_dir().join(DESKTOP_NAME)
}

/// 解析当前可执行文件路径，供 Exec= 使用。
fn current_exe() -> Result<PathBuf, String> {
    env::current_exe().map_err(|error| format!("无法定位当前程序：{error}"))
}

fn desktop_entry(exe: &str) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Version=1.0\n\
         Name=snapshot\n\
         Name[zh_CN]=应用快照\n\
         Comment=Application snapshot, prompt polish, companion\n\
         Comment[zh_CN]=应用快照、Prompt 润色、桌宠和快捷键\n\
         Exec=\"{exe}\"\n\
         Icon=snapshot\n\
         Terminal=false\n\
         Categories=Utility;\n\
         StartupNotify=false\n\
         X-GNOME-Autostart-enabled=true\n\
         # Hidden=false is intentional: user opted in via Settings.\n"
    )
}

/// 按设置开关写入或删除 XDG autostart 条目。
pub fn apply_launch_on_boot(enabled: bool) -> Result<(), String> {
    let path = desktop_path();
    if !enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(|error| format!("无法关闭开机自启：{error}"))?;
        }
        return Ok(());
    }

    let exe = current_exe()?;
    let exe_str = exe.to_string_lossy();
    // AppImage / 打包布局下偶发需要 realpath
    let resolved = Command::new("realpath")
        .arg(&*exe_str)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| exe_str.to_string());

    fs::create_dir_all(autostart_dir()).map_err(|error| format!("无法创建 autostart 目录：{error}"))?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o644)
        .open(&path)
        .map_err(|error| format!("无法写入开机自启项：{error}"))?;
    file.write_all(desktop_entry(&resolved).as_bytes())
        .map_err(|error| format!("写入开机自启项失败：{error}"))?;
    Ok(())
}
