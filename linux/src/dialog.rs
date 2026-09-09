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
                 润色提示词（切换 / 自定义）：托盘菜单「管理润色提示词…」\n\
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

// ---------- 润色提示词管理（内置 + 自定义，可切换不替换） ----------

/// 提示词管理主循环：列表选中点「切换」即时生效；新建 / 编辑 / 删除走额外按钮。
/// 内置只读——「编辑」里提供「基于内置新建…」入口。需要 zenity（多行编辑只有它支持）。
pub fn manage_prompts() {
    if !tool_runs("zenity", &["--version"]) {
        crate::notify::notify(
            "无法管理润色提示词",
            "需要 zenity（多行文本编辑仅 zenity 支持），请安装后再试",
        );
        return;
    }
    loop {
        let active = crate::prompts::active_name();
        let custom = crate::prompts::custom_list();
        let mark = |name: &str| if name == active { "当前使用" } else { "" };

        let mut command = Command::new("zenity");
        command
            .args(["--list", "--title", "润色提示词", "--width", "440", "--height", "340"])
            .args(["--text", "选中一行点「切换」即生效；管理用额外按钮"])
            .args(["--column", "提示词", "--column", "状态"])
            .args(["--ok-label", "切换", "--cancel-label", "关闭"])
            .args(["--extra-button", "新建"])
            .args(["--extra-button", "编辑"])
            .args(["--extra-button", "删除"]);
        command
            .arg(crate::prompts::BUILTIN_NAME)
            .arg(mark(crate::prompts::BUILTIN_NAME));
        for prompt in &custom {
            command.arg(&prompt.name).arg(mark(&prompt.name));
        }
        let Ok(output) = command.output() else { return };
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        match output.status.code() {
            Some(0) => {
                if !text.is_empty() {
                    crate::prompts::set_active(&text);
                    crate::notify::notify("已切换润色提示词", &format!("当前使用：{text}"));
                }
            }
            // zenity 约定：额外按钮退出码 5，stdout 为按钮文字
            Some(5) => match text.as_str() {
                "新建" => create_prompt(""),
                "编辑" => edit_prompt(),
                "删除" => delete_prompt(),
                _ => return,
            },
            _ => return,
        }
    }
}

/// 新建并保存成功后自动切换为当前使用（与 macOS / Windows 一致）。
fn create_prompt(prefill: &str) {
    let Some(name) = entry_prompt_name("") else { return };
    let Some(text) = edit_prompt_text(&name, prefill) else { return };
    if crate::prompts::save(&name, &text, None) {
        crate::prompts::set_active(name.trim());
        crate::notify::notify("已保存并切换", &format!("当前使用：{}", name.trim()));
    } else {
        crate::notify::notify(
            "保存失败",
            "名称为空、与内置重名或已存在同名提示词",
        );
    }
}

/// 编辑自定义项（可改名）；没有自定义项时直接引导「基于内置新建…」。
fn edit_prompt() {
    let custom = crate::prompts::custom_list();
    if custom.is_empty() {
        create_prompt(crate::polish::SYSTEM_PROMPT);
        return;
    }
    let mut command = Command::new("zenity");
    command
        .args(["--list", "--title", "编辑润色提示词", "--width", "380", "--height", "300"])
        .args(["--text", "选择要编辑的提示词"])
        .args(["--column", "名称"]);
    for prompt in &custom {
        command.arg(&prompt.name);
    }
    command.arg("（基于内置新建…）");
    let Ok(output) = command.output() else { return };
    if !output.status.success() {
        return;
    }
    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selected.is_empty() {
        return;
    }
    if selected == "（基于内置新建…）" {
        create_prompt(crate::polish::SYSTEM_PROMPT);
        return;
    }
    let Some(prompt) = custom.iter().find(|p| p.name == selected) else {
        return;
    };
    let Some(new_name) = entry_prompt_name(&prompt.name) else { return };
    let Some(new_text) = edit_prompt_text(&prompt.name, &prompt.text) else { return };
    if crate::prompts::save(&new_name, &new_text, Some(&prompt.name)) {
        crate::notify::notify("已保存", &format!("提示词「{}」已更新", new_name.trim()));
    } else {
        crate::notify::notify(
            "保存失败",
            "名称为空、与内置重名或已存在同名提示词",
        );
    }
}

fn delete_prompt() {
    let custom = crate::prompts::custom_list();
    if custom.is_empty() {
        crate::notify::notify("没有自定义提示词", "内置提示词不可删除");
        return;
    }
    let mut command = Command::new("zenity");
    command
        .args(["--list", "--title", "删除润色提示词", "--width", "380", "--height", "300"])
        .args(["--text", "选择要删除的提示词"])
        .args(["--column", "名称"]);
    for prompt in &custom {
        command.arg(&prompt.name);
    }
    let Ok(output) = command.output() else { return };
    if !output.status.success() {
        return;
    }
    let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let Some(prompt) = custom.iter().find(|p| p.name == selected) else {
        return;
    };
    let confirmed = Command::new("zenity")
        .args(["--question", "--title", "应用快照", "--width", "420"])
        .args([
            "--text",
            &format!(
                "删除提示词「{}」？\n删除后不可恢复；若它是当前使用的提示词，将切回内置。",
                prompt.name
            ),
        ])
        .args(["--ok-label", "删除", "--cancel-label", "取消"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if confirmed {
        crate::prompts::delete(&prompt.name);
    }
}

/// 名称输入（可带默认值）；取消返回 None。
fn entry_prompt_name(default: &str) -> Option<String> {
    let output = Command::new("zenity")
        .args(["--entry", "--title", "润色提示词", "--width", "380"])
        .args(["--text", "名称（不能与「内置」重复）"])
        .args(["--entry-text", default])
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

/// 多行正文编辑：临时文件 + zenity --text-info --editable，stdout 即编辑后内容。
fn edit_prompt_text(title: &str, initial: &str) -> Option<String> {
    let mut path = std::env::temp_dir();
    path.push(format!("windowsnap-prompt-{}.txt", std::process::id()));
    std::fs::write(&path, initial).ok()?;
    let output = Command::new("zenity")
        .args(["--text-info", "--title", title, "--width", "640", "--height", "480"])
        .args(["--editable", "--filename"])
        .arg(&path)
        .output();
    let _ = std::fs::remove_file(&path);
    let output = output.ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim_end().to_string())
    } else {
        None
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
