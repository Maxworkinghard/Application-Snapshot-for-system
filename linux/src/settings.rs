//! 配置读写：~/.config/windowsnap/config.toml。
//! 简易 `key = "value"` 行格式（保留注释与其他行），文件权限 600。

use std::collections::HashMap;
use std::path::PathBuf;

use crate::polish::{PolishConfig, PolishProtocolKind};

pub struct Settings {
    /// 截取当前应用快捷键；空 = 未绑定。
    pub shortcut: String,
    /// 录制快捷键；空 = 未绑定。
    pub shortcut_record: String,
    /// 截取上一个应用快捷键；空 = 未绑定（默认未绑定，用户在设置中自行决定）。
    pub shortcut_previous_app: String,
    /// 润色快捷键；空 = 未绑定（默认未绑定）。
    pub shortcut_polish: String,
    pub save_dir: String,
    pub pet: Option<(i32, i32)>,
    /// 桌面形式：true = 桌宠，其余（含缺省）= 悬浮球。
    pub ui_mode_pet: bool,
    /// 桌宠形象（素材根目录下的子目录名）；空 = 由 resolve_skin 兜底。
    pub pet_skin: String,
    /// 桌宠位置，与悬浮球的 pet_x/pet_y 刻意分开：两个窗口尺寸不同，共用会瞬移。
    pub desktop_pet: Option<(i32, i32)>,
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
        let value = if key.trim() == "pet.skin" && value.starts_with('"') {
            // 形象名可含引号/换行；只对新字段解转义，不改变既有配置语义。
            serde_json::Deserializer::from_str(value).into_iter::<String>().next()
                .and_then(Result::ok).unwrap_or_default()
        } else if let Some(rest) = value.strip_prefix('"') {
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
    // 显式写成空值 = 解除绑定；未配置则用默认
    let shortcut_record = match map.get("shortcut_record") {
        Some(value) if !value.is_empty() => value.clone(),
        Some(_) => String::new(),
        None => "Alt+Shift+R".to_string(),
    };
    // 截取上一个应用 / 润色：默认不绑定，由用户在设置中自行决定
    let shortcut_previous_app = map.get("shortcut_previous_app").cloned().unwrap_or_default();
    let shortcut_polish = map.get("shortcut_polish").cloned().unwrap_or_default();
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
    let ui_mode_pet = map.get("ui.mode").is_some_and(|value| value.trim() == "pet");
    let pet_skin = map.get("pet.skin").cloned().unwrap_or_default();
    let desktop_pet = match (
        map.get("desktoppet.x").and_then(|value| value.parse().ok()),
        map.get("desktoppet.y").and_then(|value| value.parse().ok()),
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
        shortcut_record,
        shortcut_previous_app,
        shortcut_polish,
        save_dir,
        pet,
        ui_mode_pet,
        pet_skin,
        desktop_pet,
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

pub fn save_desktop_pet_position(x: i32, y: i32) {
    upsert(&[
        ("desktoppet.x", &x.to_string()),
        ("desktoppet.y", &y.to_string()),
    ]);
}

/// 设置对话框保存桌面形式。
pub fn save_ui_mode(pet: bool) {
    upsert(&[("ui.mode", if pet { "pet" } else { "bubble" })]);
}

/// 设置对话框保存桌宠形象。
pub fn save_pet_skin(skin: &str) {
    upsert(&[("pet.skin", skin)]);
}

/// 设置对话框保存快捷键（空值即解除绑定，含默认项）。
pub fn save_shortcuts(capture: &str, record: &str, previous_app: &str, polish: &str) {
    upsert(&[
        ("shortcut", capture),
        ("shortcut_record", record),
        ("shortcut_previous_app", previous_app),
        ("shortcut_polish", polish),
    ]);
}

/// 设置对话框保存润色服务配置。
pub fn save_polish(kind: &str, base_url: &str, model: &str, api_key: &str) {
    upsert(&[
        ("polish.kind", kind),
        ("polish.base_url", base_url),
        ("polish.model", model),
        ("polish.api_key", api_key),
    ]);
}

/// 桌宠素材根目录：$XDG_DATA_HOME/windowsnap/pet，回退 $HOME/.local/share。
/// 素材是数据不是配置，不放 ~/.config；也不自动创建，目录不存在本身就是「没有素材」。
pub fn pet_asset_root() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map(|base| base.join("windowsnap/pet"))
}

/// 可用形象：素材根目录下每个至少含一个 *.gif 的直接子目录（文件名不限），
/// 按不区分大小写排序；任何错误都吞掉，返回已经收集到的部分。
pub fn available_skins() -> Vec<String> {
    let mut skins = Vec::new();
    let Some(root) = pet_asset_root() else { return skins };
    let Ok(entries) = std::fs::read_dir(root) else { return skins };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&path) else { continue };
        let has_gif = files.flatten().any(|file| {
            file.path().is_file() && file.path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
        });
        if has_gif {
            if let Some(name) = entry.file_name().to_str() {
                skins.push(name.to_string());
            }
        }
    }
    skins.sort_by_key(|name| name.to_lowercase());
    skins
}

/// 解析当前形象：配置里的值仍然可用就用它，否则退到第一个，一个都没有返回 None
/// （None 是「桌宠不可用」的唯一信号）。不回写配置，解析值与存盘值可以长期不一致。
pub fn resolve_skin(current: &str) -> Option<String> {
    let skins = available_skins();
    if skins.iter().any(|skin| skin == current) {
        return Some(current.to_string());
    }
    skins.into_iter().next()
}

/// 按 key 原地更新 config.toml 中的行（保留注释与其他配置），文件不存在则创建。
fn upsert(entries: &[(&str, &str)]) {
    let Some(path) = config_path() else { return };
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .map(|text| text.lines().map(str::to_string).collect())
        .unwrap_or_default();
    for (key, value) in entries {
        let line = if *key == "pet.skin" { format!("{key} = {}", serde_json::to_string(value).unwrap()) } else { format!("{key} = \"{value}\"") };
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
