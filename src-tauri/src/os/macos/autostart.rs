//! macOS 开机自启（opt-in）：在 `~/Library/LaunchAgents/` 放 / 删一份 LaunchAgent plist。
//!
//! 与 Linux 的 XDG autostart 同样是「用户勾选后才生效」，且同样只落文件、不主动
//! `launchctl bootstrap`——RunAtLoad 的 agent 一旦 bootstrap 会立刻把应用再拉起来一份，
//! 勾选设置的当下多出一个实例不是用户要的。写进去的条目在下次登录时生效。

use std::fs;
use std::path::PathBuf;

const LABEL: &str = "com.appsnapshot.prompt-pet-shortcut";

fn agents_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Library/LaunchAgents")
}

fn plist_path() -> PathBuf {
    agents_dir().join(format!("{LABEL}.plist"))
}

/// plist 是 XML，可执行文件路径里的 `&`/`<`/`>` 必须转义，否则写出来的是坏文件。
fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn plist_body(exe: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
<key>Label</key>
<string>{LABEL}</string>
<key>ProgramArguments</key>
<array>
    <string>{exe}</string>
</array>
<key>RunAtLoad</key>
<true/>
<key>ProcessType</key>
<string>Interactive</string>
</dict>
</plist>
"#,
        exe = xml_escape(exe)
    )
}

/// 按设置开关写入或删除 LaunchAgent。
pub fn apply_launch_on_boot(enabled: bool) -> Result<(), String> {
    let path = plist_path();
    if !enabled {
        if path.exists() {
            fs::remove_file(&path).map_err(|error| format!("无法关闭开机自启：{error}"))?;
        }
        return Ok(());
    }

    let exe = std::env::current_exe()
        .map_err(|error| format!("无法定位当前程序：{error}"))?
        // .app 里启动时 current_exe 可能带 symlink，落进 plist 的要是真实路径
        .canonicalize()
        .map_err(|error| format!("无法解析程序路径：{error}"))?;
    fs::create_dir_all(agents_dir())
        .map_err(|error| format!("无法创建 ~/Library/LaunchAgents：{error}"))?;
    fs::write(&path, plist_body(&exe.to_string_lossy()))
        .map_err(|error| format!("无法写入开机自启项：{error}"))?;
    Ok(())
}

/// 探测 `~/Library/LaunchAgents` 是否可写（给能力面板用）。
pub fn autostart_capability() -> Result<(), String> {
    let dir = agents_dir();
    fs::create_dir_all(&dir).map_err(|error| {
        format!("无法创建 ~/Library/LaunchAgents（{error}）；仍可保存偏好，但系统自启项写不进去")
    })?;
    let probe = dir.join(".snapshot-autostart-write-probe");
    fs::write(&probe, b"ok").map_err(|error| {
        format!("无法写入 ~/Library/LaunchAgents（{error}）；仍可保存偏好，但系统自启项写不进去")
    })?;
    let _ = fs::remove_file(&probe);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_declares_label_and_run_at_load() {
        let body = plist_body("/Applications/snapshot.app/Contents/MacOS/snapshot");
        assert!(body.contains("<string>com.appsnapshot.prompt-pet-shortcut</string>"));
        assert!(body.contains("<key>RunAtLoad</key>"));
        assert!(
            body.contains("<string>/Applications/snapshot.app/Contents/MacOS/snapshot</string>")
        );
    }

    #[test]
    fn exe_path_is_xml_escaped() {
        // 目录名里带 & 的真实场景：用户把 .app 放在「Tools & Toys」之类的目录下
        let body = plist_body("/Users/me/Tools & Toys/snapshot.app/Contents/MacOS/snapshot");
        assert!(body.contains("Tools &amp; Toys"));
        assert!(!body.contains("Tools & Toys"));
    }

    #[test]
    fn plist_path_sits_in_launch_agents() {
        let path = plist_path();
        assert!(path.ends_with("Library/LaunchAgents/com.appsnapshot.prompt-pet-shortcut.plist"));
    }
}
