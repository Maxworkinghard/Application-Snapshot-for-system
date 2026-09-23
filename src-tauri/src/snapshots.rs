use super::*;
use serde::Deserialize;

const SNAPSHOT_LIMIT: usize = 200;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SnapshotRecord {
    pub(crate) id: String,
    pub(crate) file_name: String,
    pub(crate) app_name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) size_bytes: u64,
    pub(crate) created_at: u64,
}

/// 历史落盘目录：优先用户配置的 save_dir，否则用应用数据目录。
pub(crate) fn history_dir(state: &AppState) -> PathBuf {
    let configured = state.settings.lock().save_dir.trim().to_string();
    if !configured.is_empty() {
        PathBuf::from(expand_user_path(&configured))
    } else {
        state.snapshots_dir.clone()
    }
}

pub(crate) fn snapshot_index_path(state: &AppState) -> PathBuf {
    history_dir(state).join("index.json")
}

pub(crate) fn clipboard_clear_delay(setting: &str) -> Option<Duration> {
    match setting {
        "30s" => Some(Duration::from_secs(30)),
        "5m" => Some(Duration::from_secs(300)),
        "never" => None,
        // "60s" 与未知值一律按 60 秒
        _ => Some(Duration::from_secs(60)),
    }
}

pub(crate) fn snapshot_format_parts(setting: &str) -> (ImageFormat, &'static str) {
    match setting {
        "jpeg" | "jpg" => (ImageFormat::Jpeg, "jpg"),
        "webp" => (ImageFormat::WebP, "webp"),
        _ => (ImageFormat::Png, "png"),
    }
}

pub(crate) fn mime_for_snapshot_file(file_name: &str) -> &'static str {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else {
        "image/png"
    }
}

pub(crate) fn format_clear_label(setting: &str) -> String {
    match setting {
        "30s" => "30 秒后自动清空剪贴板".into(),
        "5m" => "5 分钟后自动清空剪贴板".into(),
        "never" => "不会自动清空剪贴板".into(),
        _ => "60 秒后自动清空剪贴板".into(),
    }
}

pub(crate) fn read_snapshot_index(path: &PathBuf) -> Vec<SnapshotRecord> {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub(crate) fn write_snapshot_index(
    path: &PathBuf,
    records: &[SnapshotRecord],
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let payload = serde_json::to_vec_pretty(records).map_err(|error| error.to_string())?;
    fs::write(path, payload).map_err(|error| error.to_string())
}

/// 把截图落盘并登记进历史索引。
/// 这里失败不该影响"已复制到剪贴板"这件事，所以调用方只记录不中断。
pub(crate) fn store_snapshot(
    state: &AppState,
    image: &RgbaImage,
    app_name: &str,
) -> Result<SnapshotRecord, String> {
    let dir = history_dir(state);
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let format_setting = state.settings.lock().snapshot_format.clone();
    let (format, ext) = snapshot_format_parts(&format_setting);
    let created_at = now_millis();
    let id = format!("snap-{created_at}");
    let file_name = format!("{id}.{ext}");
    let path = dir.join(&file_name);
    let dynamic = DynamicImage::ImageRgba8(image.clone());
    // JPEG 不支持 alpha，先落到 RGB；PNG/WebP 可直接写 RGBA
    let save_result = match format {
        ImageFormat::Jpeg => dynamic.to_rgb8().save_with_format(&path, ImageFormat::Jpeg),
        _ => dynamic.save_with_format(&path, format),
    };
    save_result.map_err(|error| format!("保存快照失败：{error}"))?;
    let size_bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let record = SnapshotRecord {
        id,
        file_name,
        app_name: app_name.to_string(),
        width: image.width(),
        height: image.height(),
        size_bytes,
        created_at,
    };
    let index_path = snapshot_index_path(state);
    let mut records = read_snapshot_index(&index_path);
    records.insert(0, record.clone());
    for stale in records.split_off(records.len().min(SNAPSHOT_LIMIT)) {
        let _ = fs::remove_file(dir.join(&stale.file_name));
    }
    write_snapshot_index(&index_path, &records)?;
    Ok(record)
}

/// 展开 `~/…`；其它路径原样返回。
pub(crate) fn expand_user_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(rest) = trimmed.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    if trimmed == "~" {
        if let Some(home) = dirs::home_dir() {
            return home.to_string_lossy().into_owned();
        }
    }
    trimmed.to_string()
}

#[tauri::command]
pub(crate) fn open_snapshots_dir(state: State<'_, AppState>) -> Result<String, String> {
    let path = history_dir(&state);
    fs::create_dir_all(&path).map_err(|error| format!("无法创建快照目录：{error}"))?;

    open_in_file_manager(&path)?;
    Ok(path.to_string_lossy().to_string())
}

/// 在系统文件管理器里打开一个目录。快照目录与录制目录共用。
pub(crate) fn open_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // explorer.exe 即使成功也常返回非 0，所以只看能不能启动，不看退出码
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|error| format!("无法打开文件管理器：{error}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("无法打开访达：{error}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|error| format!("无法打开文件管理器：{error}"))?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn list_snapshots(state: State<'_, AppState>) -> Vec<SnapshotRecord> {
    let index_path = snapshot_index_path(&state);
    let records = read_snapshot_index(&index_path);
    // 文件被手动删掉的条目顺手从索引里剔除，避免历史库里全是打不开的记录
    let (alive, dropped): (Vec<_>, Vec<_>) = records
        .into_iter()
        .partition(|item| history_dir(&state).join(&item.file_name).is_file());
    if !dropped.is_empty() {
        let _ = write_snapshot_index(&index_path, &alive);
    }
    alive
}

#[tauri::command]
pub(crate) fn get_snapshot_data_url(
    state: State<'_, AppState>,
    id: String,
) -> Result<String, String> {
    let record = read_snapshot_index(&snapshot_index_path(&state))
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "找不到这条快照".to_string())?;
    let bytes = fs::read(history_dir(&state).join(&record.file_name))
        .map_err(|_| "快照文件已被移动或删除".to_string())?;
    let mime = mime_for_snapshot_file(&record.file_name);
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

#[tauri::command]
pub(crate) fn delete_snapshot(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<SnapshotRecord>, String> {
    let index_path = snapshot_index_path(&state);
    let mut records = read_snapshot_index(&index_path);
    let position = records
        .iter()
        .position(|item| item.id == id)
        .ok_or_else(|| "找不到这条快照".to_string())?;
    let removed = records.remove(position);
    let _ = fs::remove_file(history_dir(&state).join(&removed.file_name));
    write_snapshot_index(&index_path, &records)?;
    Ok(records)
}

#[tauri::command]
pub(crate) fn clear_snapshots(state: State<'_, AppState>) -> Result<Vec<SnapshotRecord>, String> {
    let index_path = snapshot_index_path(&state);
    for record in read_snapshot_index(&index_path) {
        let _ = fs::remove_file(history_dir(&state).join(&record.file_name));
    }
    write_snapshot_index(&index_path, &[])?;
    Ok(Vec::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_user_path_keeps_absolute_and_expands_tilde() {
        assert_eq!(expand_user_path("/tmp/out"), "/tmp/out");
        assert_eq!(expand_user_path("  /tmp/out  "), "/tmp/out");
        if let Some(home) = dirs::home_dir() {
            let expected = home.join("Videos").to_string_lossy().into_owned();
            assert_eq!(expand_user_path("~/Videos"), expected);
        }
    }

    #[test]
    fn clipboard_clear_delay_honors_preference_tokens() {
        assert_eq!(clipboard_clear_delay("30s"), Some(Duration::from_secs(30)));
        assert_eq!(clipboard_clear_delay("60s"), Some(Duration::from_secs(60)));
        assert_eq!(clipboard_clear_delay("5m"), Some(Duration::from_secs(300)));
        assert_eq!(clipboard_clear_delay("never"), None);
        assert_eq!(
            clipboard_clear_delay("weird"),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn snapshot_format_parts_map_ui_tokens() {
        assert_eq!(snapshot_format_parts("png"), (ImageFormat::Png, "png"));
        assert_eq!(snapshot_format_parts("jpeg"), (ImageFormat::Jpeg, "jpg"));
        assert_eq!(snapshot_format_parts("webp"), (ImageFormat::WebP, "webp"));
        assert_eq!(mime_for_snapshot_file("a.JPG"), "image/jpeg");
        assert_eq!(mime_for_snapshot_file("a.webp"), "image/webp");
        assert_eq!(mime_for_snapshot_file("a.png"), "image/png");
    }
}
