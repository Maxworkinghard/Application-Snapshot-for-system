//! 配置读写：~/.config/windowsnap/config.toml。
//! 简易 `key = "value"` 行格式（保留注释与其他行），文件权限 600。

use std::collections::HashMap;
use std::path::PathBuf;

use crate::polish::{PolishConfig, PolishProtocolKind};

pub struct Settings {
    pub shortcut: String,
    pub save_dir: String,
    pub pet: Option<(i32, i32)>,
    pub polish: PolishConfig,
}

fn config_path() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|base| base.join("windowsnap/config.toml"))
}

fn parse() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(path) = config_path() else { return map };
    let Ok(text) = std::fs::read_to_string(path) else { return map };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        let value = value.trim();
        let value = if let Some(rest) = value.strip_prefix('"') {
            rest.split('"').next().unwrap_or(rest).to_string()
        } else {
            value.split(" #").next().unwrap_or(value).trim().to_string()
        };
        map.insert(key.trim().to_string(), value);
    }
    map
}

pub fn load() -> Settings {
    let map = parse();
    let shortcut = map
        .get("shortcut")
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| "Alt+Shift+2".to_string());
    let home = std::env::var("HOME").unwrap_or_default();
    let save_dir = map
        .get("save_dir")
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| format!("{home}/Videos/应用快照"));
    let pet = match (
        map.get("pet_x").and_then(|value| value.parse().ok()),
        map.get("pet_y").and_then(|value| value.parse().ok()),
    ) {
        (Some(x), Some(y)) => Some((x, y)),
        _ => None,
    };
    let polish = PolishConfig {
        kind: if map
            .get("polish.kind")
            .is_some_and(|value| value.trim() == "anthropic")
        {
            PolishProtocolKind::Anthropic
        } else {
            PolishProtocolKind::OpenAICompatible
        },
        base_url: map.get("polish.base_url").cloned().unwrap_or_default(),
        model: map.get("polish.model").cloned().unwrap_or_default(),
        api_key: map.get("polish.api_key").cloned().unwrap_or_default(),
    };
    Settings {
        shortcut,
        save_dir,
        pet,
        polish,
    }
}

pub fn pet_position() -> Option<(i32, i32)> {
    load().pet
}

pub fn save_directory() -> String {
    load().save_dir
}

pub fn save_pet_position(x: i32, y: i32) {
    upsert(&[("pet_x", &x.to_string()), ("pet_y", &y.to_string())]);
}

/// 按 key 原地更新 config.toml 中的行（保留注释与其他配置），文件不存在则创建。
fn upsert(entries: &[(&str, &str)]) {
    let Some(path) = config_path() else { return };
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default();
    for (key, value) in entries {
        let line = format!("{key} = \"{value}\"");
        match lines.iter_mut().find(|existing| {
            existing
                .split_once('=')
                .map(|(existing_key, _)| existing_key.trim() == *key)
                .unwrap_or(false)
        }) {
            Some(slot) => *slot = line,
            None => lines.push(line),
        }
    }
    if let Some(parent) = path.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if std::fs::write(&path, format!("{}\n", lines.join("\n"))).is_ok() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}
