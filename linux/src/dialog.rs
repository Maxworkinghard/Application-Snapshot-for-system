//! 桌面对话框（zenity / kdialog 二选一，都没有则自动降级）。
//! Linux 无系统级确认弹窗 API，统一走桌面环境自带的对话框工具，
//! 与托盘通知同为进程外 UI。

use std::process::Command;

/// 可用的对话框工具。
enum DialogTool {
    Zenity,
    Kdialog,
}

fn detect_tool() -> Option<DialogTool> {
    if tool_runs("zenity", &["--version"]) {
        Some(DialogTool::Zenity)
    } else if tool_runs("kdialog", &["--version"]) {
        Some(DialogTool::Kdialog)
    } else {
        None
    }
}

fn tool_runs(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// 润色前确认：返回 false 表示用户取消。
pub fn confirm_polish(preview: &str) -> bool {
    let message = format!("确认后将用润色结果替换剪切板内容：\n\n{preview}");
    match detect_tool() {
        Some(DialogTool::Zenity) => Command::new("zenity")
            .args(["--question", "--title", "应用快照", "--width", "420"])
            .args(["--text", &message])
            .args(["--ok-label", "润色并替换", "--cancel-label", "取消"])
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        Some(DialogTool::Kdialog) => Command::new("kdialog")
            .args(["--title", "应用快照"])
            .args(["--yes-label", "润色并替换", "--no-label", "取消"])
            .arg("--yesno")
            .arg(&message)
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        None => {
            // 没有对话框工具：无法获得用户确认就不动剪贴板
            crate::notify::notify(
                "无法确认润色",
                "未找到 zenity 或 kdialog，请安装其一后再使用润色",
            );
            false
        }
    }
}

/// 弹出操作菜单，返回所选条目的下标；取消/关闭返回 None。
pub fn choose_action(items: &[&str]) -> Option<usize> {
    match detect_tool() {
        Some(DialogTool::Zenity) => {
            let mut command = Command::new("zenity");
            command
                .args(["--list", "--title", "应用快照", "--hide-header"])
                .args(["--text", "选择操作", "--column", "操作"])
                .args(["--width", "300", "--height", "320"]);
            for item in items {
                command.arg(item);
            }
            let output = command.output().ok()?;
            if !output.status.success() {
                return None;
            }
            let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
            items.iter().position(|item| *item == selected)
        }
        Some(DialogTool::Kdialog) => {
            let mut command = Command::new("kdialog");
            command.args(["--title", "应用快照"]).arg("--menu").arg("选择操作");
            for (index, item) in items.iter().enumerate() {
                command.arg(index.to_string()).arg(item);
            }
            let output = command.output().ok()?;
            if !output.status.success() {
                return None;
            }
            let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
            selected.parse::<usize>().ok()
        }
        None => {
            crate::notify::notify(
                "无法打开菜单",
                "未找到 zenity 或 kdialog，请安装其一以使用悬浮球菜单",
            );
            None
        }
    }
}

/// 窗口选择列表：返回所选标题；取消/关闭返回 None。
pub fn choose_window(titles: &[String]) -> Option<String> {
    if titles.is_empty() {
        crate::notify::notify("没有可截取的窗口", "未找到其他可见窗口");
        return None;
    }
    match detect_tool() {
        Some(DialogTool::Zenity) => {
            let mut command = Command::new("zenity");
            command
                .args(["--list", "--title", "应用快照", "--hide-header"])
                .args(["--text", "选择要截取的窗口", "--column", "窗口"])
                .args(["--width", "480", "--height", "400"]);
            for title in titles {
                command.arg(title);
            }
            let output = command.output().ok()?;
            if !output.status.success() {
                return None;
            }
            let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if selected.is_empty() {
                None
            } else {
                Some(selected)
            }
        }
        Some(DialogTool::Kdialog) => {
            let mut command = Command::new("kdialog");
            command
                .args(["--title", "应用快照"])
                .arg("--menu")
                .arg("选择要截取的窗口");
            for (index, title) in titles.iter().enumerate() {
                command.arg((index + 1).to_string()).arg(title);
            }
            let output = command.output().ok()?;
            if !output.status.success() {
                return None;
            }
            let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let index = selected.parse::<usize>().ok()?;
            titles.get(index - 1).cloned()
        }
        None => {
            crate::notify::notify(
                "无法选择窗口",
                "未找到 zenity 或 kdialog，请安装其一以使用窗口选择",
            );
            None
        }
    }
}
