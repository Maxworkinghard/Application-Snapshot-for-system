//! XDG 开机自启（opt-in，与偏好设置里的「开机时静默启动」对齐）。
//!
//! 不写 systemd user unit，也不默认 enable——Windows 侧同样是用户勾选后才生效。
//! 只在 `~/.config/autostart/` 放 / 删一份 `.desktop`。

use std::env;
use std::fs;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const DESKTOP_NAME: &str = "com.appsnapshot.snapshot.desktop";

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

    fs::create_dir_all(autostart_dir())
        .map_err(|error| format!("无法创建 autostart 目录：{error}"))?;
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

/// 探测给定目录是否可写（能力面板 / 单测复用）。
pub(crate) fn probe_autostart_dir(dir: &Path) -> Result<(), String> {
    match fs::create_dir_all(dir) {
        Ok(()) => {
            let probe = dir.join(".snapshot-autostart-write-probe");
            match fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o644)
                .open(&probe)
            {
                Ok(mut file) => {
                    let _ = file.write_all(b"ok");
                    let _ = fs::remove_file(&probe);
                    Ok(())
                }
                Err(error) => Err(format!(
                    "无法写入 ~/.config/autostart（{error}）；仍可保存偏好，但系统自启项写不进去"
                )),
            }
        }
        Err(error) => Err(format!(
            "无法创建 ~/.config/autostart（{error}）；仍可保存偏好，但系统自启项写不进去"
        )),
    }
}

/// 探测 XDG autostart 目录是否可写（给能力面板用）。
pub fn autostart_capability() -> Result<(), String> {
    probe_autostart_dir(&autostart_dir())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_subdir(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let dir = env::temp_dir().join(format!("snapshot-autostart-test-{label}-{stamp}"));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn probe_ok_on_writable_dir() {
        let dir = unique_temp_subdir("ok");
        fs::create_dir_all(&dir).expect("create temp autostart dir");
        assert!(probe_autostart_dir(&dir).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn probe_err_on_non_writable_dir() {
        let dir = unique_temp_subdir("ro");
        fs::create_dir_all(&dir).expect("create temp autostart dir");
        let mut perms = fs::metadata(&dir).expect("meta").permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&dir, perms).expect("chmod 555");
        let result = probe_autostart_dir(&dir);
        // restore writable so cleanup works
        let mut perms = fs::metadata(&dir).expect("meta").permissions();
        perms.set_mode(0o755);
        let _ = fs::set_permissions(&dir, perms);
        let _ = fs::remove_dir_all(&dir);
        assert!(
            result.is_err(),
            "expected Err on non-writable dir, got {result:?}"
        );
    }
}
