use super::settings::{default_appearance_id, emit_settings, persist_settings, Settings};
use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetAsset {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) entry: String,
    #[serde(default)]
    pub(crate) animations: Vec<String>,
}

#[tauri::command]
pub(crate) fn select_pet_appearance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Settings, String> {
    let mut settings = state.settings.lock();
    if id != "app-icon" && !settings.pet_assets.iter().any(|asset| asset.id == id) {
        return Err("找不到这个形象".into());
    }
    settings.selected_appearance_id = id;
    persist_settings(&state.settings_path, &settings)?;
    if let Some(window) = app.get_webview_window("pet") {
        let _ = window.show();
    }
    let result = settings.clone();
    emit_settings(&app, &result);
    Ok(result)
}

#[tauri::command]
pub(crate) fn add_pet_asset(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<Settings, String> {
    let canonical = fs::canonicalize(path.trim()).map_err(|_| "无法读取桌宠文件".to_string())?;
    if !canonical.is_file() {
        return Err("请选择一个 ZIP 压缩包或 GIF 图片".into());
    }
    let extension = canonical
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    let single_gif = extension == "gif";
    if extension != "zip" && !single_gif {
        return Err("桌宠形象只支持 ZIP 压缩包或单个 GIF".into());
    }
    let metadata = fs::metadata(&canonical).map_err(|error| error.to_string())?;
    // 单个 GIF 按单动画的上限算，压缩包按整包算
    let limit = if single_gif { 50 } else { 100 } * 1024 * 1024;
    if metadata.len() > limit {
        return Err(if single_gif {
            "桌宠 GIF 不能超过 50MB".to_string()
        } else {
            "桌宠压缩包不能超过 100MB".to_string()
        });
    }
    let animations = find_pet_animation_entries(&canonical)?;
    let preview_entry = animations
        .first()
        .cloned()
        .ok_or_else(|| "这份素材里没有可用动画".to_string())?;
    let path = canonical.to_string_lossy().to_string();
    let mut settings = state.settings.lock();
    if let Some(existing) = settings
        .pet_assets
        .iter_mut()
        .find(|asset| asset.path == path)
    {
        existing.entry = preview_entry;
        existing.animations = animations;
        settings.selected_appearance_id = existing.id.clone();
    } else {
        let asset = PetAsset {
            id: format!("pet-{}", now_millis()),
            name: canonical
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("桌宠")
                .to_string(),
            path,
            entry: preview_entry,
            animations,
        };
        settings.selected_appearance_id = asset.id.clone();
        settings.pet_assets.push(asset);
    }
    persist_settings(&state.settings_path, &settings)?;
    if let Some(window) = app.get_webview_window("pet") {
        let _ = window.show();
    }
    let result = settings.clone();
    emit_settings(&app, &result);
    Ok(result)
}

#[tauri::command]
pub(crate) fn delete_pet_asset(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Settings, String> {
    if id == "app-icon" {
        return Err("默认应用图标不可删除".into());
    }
    let mut settings = state.settings.lock();
    let original_len = settings.pet_assets.len();
    settings.pet_assets.retain(|asset| asset.id != id);
    if settings.pet_assets.len() == original_len {
        return Err("找不到这个形象".into());
    }
    if settings.selected_appearance_id == id {
        settings.selected_appearance_id = default_appearance_id();
    }
    persist_settings(&state.settings_path, &settings)?;
    let result = settings.clone();
    emit_settings(&app, &result);
    Ok(result)
}

#[tauri::command]
pub(crate) fn get_pet_asset_data_url(
    state: State<'_, AppState>,
    id: String,
    entry: Option<String>,
) -> Result<String, String> {
    let asset = state
        .settings
        .lock()
        .pet_assets
        .iter()
        .find(|asset| asset.id == id)
        .cloned()
        .ok_or_else(|| "找不到这个形象".to_string())?;
    let archive_path = PathBuf::from(&asset.path);
    let requested = entry.filter(|value| !value.is_empty());
    let entry_name = if let Some(requested) = requested {
        if !asset
            .animations
            .iter()
            .any(|candidate| candidate == &requested)
        {
            return Err("压缩包内没有这个动画".into());
        }
        requested
    } else if asset.entry.is_empty() {
        find_pet_animation_entries(&archive_path)?
            .into_iter()
            .next()
            .ok_or_else(|| "压缩包内没有可预览的形象".to_string())?
    } else {
        asset.entry
    };
    if !is_pet_gif(&entry_name) {
        return Err("这个动作不是 GIF，无法预览".into());
    }
    // 直接导入的 GIF 本身就是素材，没有压缩包可拆
    if is_pet_gif(&asset.path) {
        let bytes = fs::read(&archive_path).map_err(|_| "桌宠 GIF 已被移动或删除".to_string())?;
        if bytes.len() > 50 * 1024 * 1024 {
            return Err("桌宠动画不能超过 50MB".into());
        }
        return Ok(format!("data:image/gif;base64,{}", BASE64.encode(bytes)));
    }
    let file = fs::File::open(&archive_path).map_err(|_| "桌宠压缩包已被移动或删除".to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "桌宠压缩包已损坏".to_string())?;
    let mut entry = archive
        .by_name(&entry_name)
        .map_err(|_| "压缩包内的预览动画已丢失".to_string())?;
    if entry.size() > 50 * 1024 * 1024 {
        return Err("桌宠动画不能超过 50MB".into());
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|_| "读取桌宠动画失败".to_string())?;
    Ok(format!("data:image/gif;base64,{}", BASE64.encode(bytes)))
}

/// 收集一份桌宠素材里可用的动作。
///
/// 传进来的可能是压缩包，也可能是直接导入的单个 GIF——后者本身就是唯一的动作。
pub(crate) fn find_pet_animation_entries(path: &PathBuf) -> Result<Vec<String>, String> {
    if is_pet_gif(&path.to_string_lossy()) {
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| "无法读取这个 GIF 的文件名".to_string())?;
        return Ok(vec![name.to_string()]);
    }

    let file = fs::File::open(path).map_err(|_| "无法读取桌宠压缩包".to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| "桌宠压缩包已损坏".to_string())?;
    let mut found = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| "无法读取桌宠压缩包目录".to_string())?;
        if entry.is_dir() || entry.size() > 50 * 1024 * 1024 {
            continue;
        }
        let name = entry.name().to_string();
        let lower = name.to_lowercase();
        if !is_pet_gif(&lower) {
            continue;
        }
        // 包根目录下的 idle.gif 没有前导分隔符，单独按文件名判一次，
        // 否则会被当成普通动作排到后面去
        let file_name = lower.rsplit('/').next().unwrap_or(lower.as_str());
        let is_idle = file_name.starts_with("idle")
            || lower.contains("/idle")
            || lower.contains("_idle")
            || lower.contains("-idle");
        found.push((if is_idle { 0 } else { 1 }, lower, name));
    }
    found.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
    if found.is_empty() {
        return Err("压缩包内没有 GIF 形象资源".into());
    }
    Ok(found.into_iter().map(|item| item.2).collect())
}

/// 桌宠素材只收 GIF。
///
/// 视频要交给各端内置的 WebView 解码，而三端内核（WebView2 / WKWebView /
/// WebKitGTK）认的编码各不相同——同一个包在这台能动、在那台是一片空白，
/// 只能靠解析容器逐个编码做白名单去兜。先砍掉，留一种到处都动得起来的。
fn is_pet_gif(path: &str) -> bool {
    path.rsplit('.')
        .next()
        .map(|extension| extension.eq_ignore_ascii_case("gif"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_zip(name: &str, entries: &[&str]) -> PathBuf {
        let path = std::env::temp_dir().join(format!("snapshot-pet-test-{name}.zip"));
        let file = fs::File::create(&path).expect("建测试包失败");
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for entry in entries {
            writer.start_file(*entry, options).expect("写入条目失败");
            writer.write_all(b"fake-bytes").expect("写入内容失败");
        }
        writer.finish().expect("收尾失败");
        path
    }

    #[test]
    fn only_gif_counts_as_a_pet_asset() {
        assert!(is_pet_gif("a.gif"));
        assert!(is_pet_gif("a.GIF"));
        assert!(!is_pet_gif("a.mp4"));
        assert!(!is_pet_gif("a.webp"));
        assert!(!is_pet_gif("a.png"));
        assert!(!is_pet_gif("noext"));
    }

    #[test]
    fn non_gif_entries_are_skipped_and_idle_comes_first() {
        let path = make_zip(
            "mixed",
            &[
                "cover.png",
                "pose/walk.mp4",
                "pose/walk.gif",
                "pose/idle.gif",
            ],
        );
        let found = find_pet_animation_entries(&path).expect("应当找到 GIF");
        assert_eq!(
            found,
            vec!["pose/idle.gif".to_string(), "pose/walk.gif".to_string()]
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn idle_at_package_root_is_still_the_default_pose() {
        let path = make_zip("root-idle", &["walk.gif", "idle.gif"]);
        let found = find_pet_animation_entries(&path).expect("应当找到 GIF");
        assert_eq!(found, vec!["idle.gif".to_string(), "walk.gif".to_string()]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_package_with_only_video_is_rejected() {
        let path = make_zip("video-only", &["cat/idle.mp4", "cat/walk.webm"]);
        assert!(
            find_pet_animation_entries(&path).is_err(),
            "只剩视频的包应当被拒绝"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn a_single_gif_file_is_its_own_animation() {
        let path = std::env::temp_dir().join("snapshot-pet-test-single.gif");
        fs::write(&path, b"fake-gif").expect("写测试 GIF 失败");
        let found = find_pet_animation_entries(&path).expect("直接导入的 GIF 应当可用");
        assert_eq!(found, vec!["snapshot-pet-test-single.gif".to_string()]);
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn package_without_any_supported_asset_is_rejected() {
        let path = make_zip("empty", &["readme.txt"]);
        assert!(find_pet_animation_entries(&path).is_err());
    }
}
