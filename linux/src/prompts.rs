//! 润色提示词库：内置提示词常驻可选，用户自定义存
//! ~/.config/windowsnap/prompts.json（权限 600），可切换不替换；
//! 润色调用时实时读取，改名/删除即时生效。

use std::path::PathBuf;

use serde_json::{json, Value};

use crate::polish::SYSTEM_PROMPT;

pub const BUILTIN_NAME: &str = "内置";

#[derive(Clone)]
pub struct CustomPrompt {
    pub name: String,
    pub text: String,
}

fn prompts_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|base| base.join("windowsnap/prompts.json"))
}

fn load_file() -> Value {
    let Some(path) = prompts_path() else { return json!({}) };
    let Ok(text) = std::fs::read_to_string(path) else { return json!({}) };
    serde_json::from_str(&text).unwrap_or_else(|_| json!({}))
}

fn store_file(file: &Value) {
    let Some(path) = prompts_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(file) {
        if std::fs::write(&path, text).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
        }
    }
}

fn parse_custom(file: &Value) -> Vec<CustomPrompt> {
    file.get("custom")
        .and_then(Value::as_array)
        .map(|array| {
            array
                .iter()
                .filter_map(|entry| {
                    let name = entry.get("name")?.as_str()?.trim().to_string();
                    let text = entry
                        .get("text")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    if name.is_empty() {
                        None
                    } else {
                        Some(CustomPrompt { name, text })
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn active_raw(file: &Value) -> String {
    file.get("active")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn write_custom(file: &mut Value, custom: &[CustomPrompt]) {
    file["custom"] = json!(custom
        .iter()
        .map(|p| json!({"name": p.name, "text": p.text}))
        .collect::<Vec<_>>());
}

pub fn custom_list() -> Vec<CustomPrompt> {
    parse_custom(&load_file())
}

/// 当前使用的名称（激活项被删后回退「内置」）。
pub fn active_name() -> String {
    let file = load_file();
    let active = active_raw(&file);
    if parse_custom(&file).iter().any(|p| p.name == active) {
        active
    } else {
        BUILTIN_NAME.to_string()
    }
}

/// 当前生效的系统提示词：激活的自定义项，缺失时回退内置。
pub fn active_prompt() -> String {
    let file = load_file();
    let active = active_raw(&file);
    if active.is_empty() {
        return SYSTEM_PROMPT.to_string();
    }
    parse_custom(&file)
        .into_iter()
        .find(|p| p.name == active)
        .map(|p| p.text)
        .unwrap_or_else(|| SYSTEM_PROMPT.to_string())
}

pub fn set_active(name: &str) {
    let mut file = load_file();
    file["active"] = json!(if name == BUILTIN_NAME { "" } else { name });
    store_file(&file);
}

/// 新增或按原名更新。返回 false 表示名称为空、与内置重名或与其他自定义重名。
pub fn save(name: &str, text: &str, original: Option<&str>) -> bool {
    let name = name.trim();
    if name.is_empty() || name == BUILTIN_NAME {
        return false;
    }

    let mut file = load_file();
    let mut custom = parse_custom(&file);

    if let Some(original) = original {
        if let Some(index) = custom.iter().position(|p| p.name == original) {
            if name != original && custom.iter().any(|p| p.name == name) {
                return false;
            }
            let was_active = active_raw(&file) == original;
            custom[index] = CustomPrompt {
                name: name.to_string(),
                text: text.to_string(),
            };
            write_custom(&mut file, &custom);
            if was_active {
                file["active"] = json!(name);
            }
            store_file(&file);
            return true;
        }
    }

    if custom.iter().any(|p| p.name == name) {
        return false;
    }
    custom.push(CustomPrompt {
        name: name.to_string(),
        text: text.to_string(),
    });
    write_custom(&mut file, &custom);
    store_file(&file);
    true
}

pub fn delete(name: &str) {
    let mut file = load_file();
    let was_active = active_raw(&file) == name;
    let mut custom = parse_custom(&file);
    custom.retain(|p| p.name != name);
    write_custom(&mut file, &custom);
    if was_active {
        file["active"] = json!("");
    }
    store_file(&file);
}
