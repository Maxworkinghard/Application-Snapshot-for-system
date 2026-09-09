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

/// 统一设置表单（快捷键绑定 + 润色服务）。
/// zenity forms 无法预填默认值，约定语义：
/// - 字段留空 = 保持当前配置不变
/// - 快捷键字段填 none = 解除该项绑定
/// - 取消/关闭返回 None（不做任何更改）
pub struct SettingsFormValues {
    pub shortcut: String,
    pub shortcut_record: String,
    pub shortcut_previous_app: String,
    pub shortcut_polish: String,
    pub polish_kind: String,
    pub polish_base_url: String,
    pub polish_model: String,
    pub polish_api_key: String,
}

pub fn settings_form(values: &SettingsFormValues) -> Option<SettingsFormValues> {
    let zenity_only = || {
        crate::notify::notify(
            "设置界面不可用",
            "设置表单需要 zenity（kdialog 不支持多字段表单）；也可直接编辑 ~/.config/windowsnap/config.toml",
        );
    };
    match detect_tool() {
        Some(DialogTool::Zenity) => {
            let current_shortcut = |value: &str| {
                if value.is_empty() {
                    "（未绑定）".to_string()
                } else {
                    value.to_string()
                }
            };
            let hint = format!(
                "当前：截取当前应用={}，录制={}，截取上一个应用={}，润色={}；协议={}，模型={}，Base URL={}\n\
                 每项留空表示保持不变；快捷键填 none 表示解除绑定；快捷键格式如 Alt+Shift+2",
                current_shortcut(&values.shortcut),
                current_shortcut(&values.shortcut_record),
                current_shortcut(&values.shortcut_previous_app),
                current_shortcut(&values.shortcut_polish),
                if values.polish_kind == "anthropic" { "anthropic" } else { "openai" },
                values.polish_model,
                values.polish_base_url
            );
            let mut command = Command::new("zenity");
            command
                .args(["--forms", "--title", "应用快照设置", "--width", "500"])
                .args(["--text", &hint])
                .args(["--add-entry", "截取当前应用窗口快捷键"])
                .args(["--add-entry", "录制当前应用窗口快捷键"])
                .args(["--add-entry", "截取上一个应用快捷键"])
                .args(["--add-entry", "润色提示词快捷键"])
                .args(["--add-entry", "润色协议（openai / anthropic）"])
                .args(["--add-entry", "润色 Base URL"])
                .args(["--add-entry", "润色模型"])
                .args(["--add-password", "润色 API Key"]);
            let output = command.output().ok()?;
            if !output.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&output.stdout).trim_end().to_string();
            let fields: Vec<String> = text.split('|').map(str::trim).map(str::to_string).collect();
            if fields.len() != 8 {
                return None;
            }
            Some(SettingsFormValues {
                shortcut: fields[0].clone(),
                shortcut_record: fields[1].clone(),
                shortcut_previous_app: fields[2].clone(),
                shortcut_polish: fields[3].clone(),
                polish_kind: fields[4].clone(),
                polish_base_url: fields[5].clone(),
                polish_model: fields[6].clone(),
                polish_api_key: fields[7].clone(),
            })
        }
        Some(DialogTool::Kdialog) | None => {
            zenity_only();
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
