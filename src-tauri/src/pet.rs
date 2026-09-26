use super::settings::{default_appearance_id, emit_settings, persist_settings, Settings};
use super::*;
use serde::Deserialize;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetAsset {
    pub(crate) id: String,
    pub(crate) name: String,
    /// 实际读取的文件。新导入的会复制进应用数据目录，旧版导入的仍指向原位置
    pub(crate) path: String,
    #[serde(default)]
    pub(crate) entry: String,
    #[serde(default)]
    pub(crate) animations: Vec<String>,
    /// 导入时用户选的原文件，只做展示
    #[serde(default)]
    pub(crate) source: String,
    #[serde(default)]
    pub(crate) size_bytes: u64,
    #[serde(default)]
    pub(crate) imported_at: u64,
    /// 最近一次被选为伴侣的时间，形象架按它排序
    #[serde(default)]
    pub(crate) last_used_at: u64,
    /// 素材文件找不到了（旧版按原路径读取，原文件被移走就会这样）。每次读盘重算
    #[serde(default)]
    pub(crate) missing: bool,
}

/// 名字超过这个长度就截断，免得形象架上一行放不下
const PET_NAME_LIMIT: usize = 40;

pub(crate) fn refresh_missing(assets: &mut [PetAsset]) {
    for asset in assets {
        asset.missing = !Path::new(&asset.path).is_file();
    }
}

fn pets_dir(state: &AppState) -> &Path {
    &state.pets_dir
}

fn thumb_path(state: &AppState, id: &str) -> PathBuf {
    pets_dir(state).join(format!("{id}.thumb2.png"))
}

/// 动作名里有斜杠和中文，拿它的哈希（FNV-1a）当文件名
fn fnv(entry: &str) -> u64 {
    entry.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ byte as u64).wrapping_mul(0x0100_0000_01b3)
    })
}

/// 走路动作处理后的缓存
fn walk_cache_path(state: &AppState, id: &str, entry: &str) -> PathBuf {
    pets_dir(state).join(format!("{id}.walk2-{:016x}.gif", fnv(entry)))
}

/// 抠掉背景以后的那份素材
fn key_cache_path(state: &AppState, id: &str, entry: &str) -> PathBuf {
    pets_dir(state).join(format!("{id}.keyed-{:016x}.gif", fnv(entry)))
}

/// 形象删掉、或者重新导入时，把由它派生出来的缓存一起清掉：
/// 缩略图、走路动作、抠过背景的素材
fn remove_derived_cache(state: &AppState, id: &str) {
    let prefixes = [
        format!("{id}.thumb"),
        format!("{id}.walk-"),
        format!("{id}.walk2-"),
        format!("{id}.keyed-"),
    ];
    let Ok(entries) = fs::read_dir(pets_dir(state)) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if prefixes
            .iter()
            .any(|prefix| name.to_string_lossy().starts_with(prefix.as_str()))
        {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// 选中后广播、落盘、顺手把桌宠窗口叫出来
fn commit(app: &AppHandle, state: &AppState, settings: &mut Settings) -> Result<Settings, String> {
    refresh_missing(&mut settings.pet_assets);
    persist_settings(&state.settings_path, settings)?;
    let result = settings.clone();
    emit_settings(app, &result);
    Ok(result)
}

#[tauri::command]
pub(crate) fn select_pet_appearance(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<Settings, String> {
    let mut settings = state.settings.lock();
    if id != "app-icon" {
        let asset = settings
            .pet_assets
            .iter_mut()
            .find(|asset| asset.id == id)
            .ok_or_else(|| "找不到这个形象".to_string())?;
        if !Path::new(&asset.path).is_file() {
            return Err("这个形象的素材文件找不到了".into());
        }
        asset.last_used_at = now_millis();
    }
    settings.selected_appearance_id = id;
    let result = commit(&app, &state, &mut settings)?;
    if let Some(window) = app.get_webview_window("pet") {
        let _ = window.show();
    }
    Ok(result)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetImportFailure {
    path: String,
    reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PetImportResult {
    settings: Settings,
    imported: usize,
    failed: Vec<PetImportFailure>,
}

/// 一次导入多个 ZIP / GIF（对话框多选或拖进窗口）。
///
/// 每个文件复制一份进应用数据目录再用：原先按原路径读取，用户清一次「下载」文件夹，
/// 形象就大面积失效。坏的那几个单独列出原因，不连累其它。
#[tauri::command]
pub(crate) fn add_pet_assets(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<PetImportResult, String> {
    let mut imported = Vec::new();
    let mut failed = Vec::new();
    for path in paths {
        match prepare_import(&state, &path) {
            Ok(asset) => imported.push(asset),
            Err(reason) => failed.push(PetImportFailure { path, reason }),
        }
    }
    let count = imported.len();
    let mut settings = state.settings.lock();
    for asset in imported {
        let id = match settings
            .pet_assets
            .iter_mut()
            .find(|existing| !existing.source.is_empty() && existing.source == asset.source)
        {
            // 同一个原文件再导入一次：当作更新，保留名字与 id
            Some(existing) => {
                let old_path = existing.path.clone();
                existing.path = asset.path;
                existing.entry = asset.entry;
                existing.animations = asset.animations;
                existing.size_bytes = asset.size_bytes;
                existing.last_used_at = asset.last_used_at;
                if old_path != existing.path {
                    remove_owned_file(&state, &old_path);
                }
                remove_derived_cache(&state, &existing.id);
                existing.id.clone()
            }
            None => {
                let id = asset.id.clone();
                settings.pet_assets.push(asset);
                id
            }
        };
        settings.selected_appearance_id = id;
    }
    let result = commit(&app, &state, &mut settings)?;
    drop(settings);
    if count > 0 {
        if let Some(window) = app.get_webview_window("pet") {
            let _ = window.show();
        }
    }
    Ok(PetImportResult {
        settings: result,
        imported: count,
        failed,
    })
}

/// 校验一个文件、复制进应用目录、读出动作表；还没登记进设置
fn prepare_import(state: &AppState, raw_path: &str) -> Result<PetAsset, String> {
    let canonical =
        fs::canonicalize(raw_path.trim()).map_err(|_| "无法读取这个文件".to_string())?;
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
        return Err("只支持 ZIP 压缩包或单个 GIF".into());
    }
    let size_bytes = fs::metadata(&canonical)
        .map_err(|error| error.to_string())?
        .len();
    // 单个 GIF 按单动画的上限算，压缩包按整包算
    let limit = if single_gif { 50 } else { 100 } * 1024 * 1024;
    if size_bytes > limit {
        return Err(if single_gif {
            "GIF 不能超过 50MB".to_string()
        } else {
            "压缩包不能超过 100MB".to_string()
        });
    }
    let animations = find_pet_animation_entries(&canonical)?;
    let entry = animations
        .first()
        .cloned()
        .ok_or_else(|| "这份素材里没有可用动画".to_string())?;

    let now = now_millis();
    // 同一毫秒导入多个时靠序号区分
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let id = format!("pet-{now}-{}", SEQUENCE.fetch_add(1, Ordering::Relaxed));
    let dir = pets_dir(state);
    fs::create_dir_all(dir).map_err(|error| format!("无法创建素材目录：{error}"))?;
    let stored = dir.join(format!("{id}.{extension}"));
    fs::copy(&canonical, &stored).map_err(|error| format!("复制素材失败：{error}"))?;

    Ok(PetAsset {
        id,
        name: canonical
            .file_stem()
            .and_then(|value| value.to_str())
            .map(|stem| truncate(stem, PET_NAME_LIMIT))
            .unwrap_or_else(|| "伴侣".into()),
        path: stored.to_string_lossy().to_string(),
        entry,
        animations,
        source: canonical.to_string_lossy().to_string(),
        size_bytes,
        imported_at: now,
        last_used_at: now,
        missing: false,
    })
}

/// 只删我们自己复制进来的文件，用户原来的文件不碰
fn remove_owned_file(state: &AppState, path: &str) {
    let path = Path::new(path);
    if path.starts_with(pets_dir(state)) {
        let _ = fs::remove_file(path);
    }
}

#[tauri::command]
pub(crate) fn rename_pet_asset(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Settings, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("名字不能为空".into());
    }
    let mut settings = state.settings.lock();
    let asset = settings
        .pet_assets
        .iter_mut()
        .find(|asset| asset.id == id)
        .ok_or_else(|| "找不到这个形象".to_string())?;
    asset.name = truncate(name, PET_NAME_LIMIT);
    commit(&app, &state, &mut settings)
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
    let position = settings
        .pet_assets
        .iter()
        .position(|asset| asset.id == id)
        .ok_or_else(|| "找不到这个形象".to_string())?;
    let removed = settings.pet_assets.remove(position);
    remove_owned_file(&state, &removed.path);
    remove_derived_cache(&state, &removed.id);
    if settings.selected_appearance_id == id {
        settings.selected_appearance_id = default_appearance_id();
    }
    commit(&app, &state, &mut settings)
}

/// 形象架上的静态缩略图：默认动作的第一帧。几十个 GIF 同时动起来既吵又费电，
/// 架子上只放静态帧，只有当前那只会动。首次取用时生成并缓存。
pub(crate) fn read_thumbnail(state: &AppState, id: &str) -> Result<Vec<u8>, String> {
    let path = thumb_path(state, id);
    if let Ok(bytes) = fs::read(&path) {
        return Ok(bytes);
    }
    let gif = read_animation(state, id, None)?;
    let frame = image::load_from_memory_with_format(&gif, ImageFormat::Gif)
        .map_err(|_| "读不出这份素材的第一帧".to_string())?
        .to_rgba8();
    let small = image::imageops::thumbnail(
        &frame,
        frame.width().min(200),
        (frame.height() as f64 * frame.width().min(200) as f64 / frame.width().max(1) as f64)
            .round()
            .max(1.0) as u32,
    );
    let png = capture::encode_png(&small)?;
    fs::create_dir_all(pets_dir(state)).map_err(|error| error.to_string())?;
    let _ = fs::write(&path, &png);
    Ok(png)
}

/// 读出某个形象的一段 GIF 动画（媒体协议用）。不指定动作时取该形象的默认动作。
/// 背景是画进 GIF 里的白底这种，这里顺手抠成透明再给出去（见 pet_key）。
pub(crate) fn read_animation(
    state: &AppState,
    id: &str,
    entry: Option<&str>,
) -> Result<Vec<u8>, String> {
    let (entry_name, raw) = read_raw_animation(state, id, entry)?;
    let cache = key_cache_path(state, id, &entry_name);
    if let Ok(bytes) = fs::read(&cache) {
        return Ok(bytes);
    }
    let bytes = match pet_key::key_out(&raw) {
        Ok(Some(keyed)) => keyed,
        Ok(None) => raw,
        Err(error) => {
            eprintln!("snapshot: background left as is ({entry_name}): {error}");
            raw
        }
    };
    // 左右两个方向可能同时来要同一个动作：各写各的临时文件再改名，免得读到或写出半个文件
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let partial = cache.with_extension(format!(
        "partial{}",
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::write(&partial, &bytes).is_ok() && fs::rename(&partial, &cache).is_err() {
        let _ = fs::remove_file(&partial);
    }
    Ok(bytes)
}

/// 按动作名把素材原样读出来（压缩包里的条目，或者直接导入的那个 GIF），不碰背景。
fn read_raw_animation(
    state: &AppState,
    id: &str,
    entry: Option<&str>,
) -> Result<(String, Vec<u8>), String> {
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
        // 只认登记过的动作名，不能借路径读压缩包里的任意条目
        if !asset
            .animations
            .iter()
            .any(|candidate| candidate == requested)
        {
            return Err("压缩包内没有这个动画".into());
        }
        requested.to_string()
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
    let bytes = if is_pet_gif(&asset.path) {
        let bytes = fs::read(&archive_path).map_err(|_| "桌宠 GIF 已被移动或删除".to_string())?;
        if bytes.len() > 50 * 1024 * 1024 {
            return Err("桌宠动画不能超过 50MB".into());
        }
        bytes
    } else {
        let file =
            fs::File::open(&archive_path).map_err(|_| "桌宠压缩包已被移动或删除".to_string())?;
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
        bytes
    };
    Ok((entry_name, bytes))
}

/// 拖动桌宠时播的走路动作（媒体协议用）。横穿画布的改成原地走（见 pet_walk），
/// 结果按动作缓存在素材目录里；本来就原地走、或者处理不了的照用原图，拖动时总有图可播。
pub(crate) fn read_walk(state: &AppState, id: &str, entry: &str) -> Result<Vec<u8>, String> {
    let cache = walk_cache_path(state, id, entry);
    if let Ok(bytes) = fs::read(&cache) {
        return Ok(bytes);
    }
    // 先按普通动作读一遍：顺带校验这是登记过的动作
    let walk = read_animation(state, id, Some(entry))?;
    let idle = read_animation(state, id, None).unwrap_or_default();
    let bytes = match pet_walk::walk_in_place(&walk, &idle) {
        Ok(Some(converted)) => converted,
        Ok(None) => walk,
        Err(error) => {
            eprintln!("snapshot: walk animation left as is ({entry}): {error}");
            walk
        }
    };
    // 左右两个方向可能同时来要同一个动作：各写各的临时文件再改名，免得读到或写出半个文件
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let partial = cache.with_extension(format!(
        "partial{}",
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    if fs::write(&partial, &bytes).is_ok() && fs::rename(&partial, &cache).is_err() {
        let _ = fs::remove_file(&partial);
    }
    Ok(bytes)
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
    use std::io::Write;

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
