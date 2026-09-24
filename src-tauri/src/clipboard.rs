//! 剪贴板里放着我们的图时，界面要知道「是哪张、多久后清空」，并能提前清或取消清。
//!
//! 原先清空计时只活在一个后台线程里，界面看不见它；现在状态放进 AppState，
//! 每次变化都广播 `clipboard-changed`，主窗口侧栏和输入框据此画倒计时。

use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClipboardState {
    /// 剪贴板里是什么，一句话
    pub(crate) label: String,
    /// 来自历史里的哪张快照（没存入历史时为空）
    pub(crate) snapshot_id: Option<String>,
    /// 放进剪贴板的时刻
    pub(crate) armed_at: u64,
    /// 何时自动清空；None 表示不会自动清
    pub(crate) clear_at: Option<u64>,
    #[serde(skip)]
    fingerprint: u64,
}

/// 把图放进剪贴板，并按偏好安排自动清空。
pub(crate) fn copy_image(
    app: &AppHandle,
    state: &AppState,
    image: RgbaImage,
    label: &str,
    snapshot_id: Option<String>,
) -> Result<(), String> {
    let delay = snapshots::clipboard_clear_delay(&state.settings.lock().clipboard_auto_clear);
    let width = image.width() as usize;
    let height = image.height() as usize;
    let bytes = image.into_raw();
    Clipboard::new()
        .and_then(|mut clipboard| {
            clipboard.set_image(ImageData {
                width,
                height,
                bytes: Cow::Borrowed(&bytes),
            })
        })
        .map_err(|error| error.to_string())?;
    // 到点前要确认剪贴板里还是这张图（用户可能已经复制了别的）。只留指纹不留原图：
    // 4K 截图一份约 33MB，没必要在内存里攥满整个清空时限。
    let fingerprint = image_fingerprint(width, height, &bytes);
    drop(bytes);

    let generation = state.clipboard_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let now = now_millis();
    let armed = ClipboardState {
        label: label.to_string(),
        snapshot_id,
        armed_at: now,
        clear_at: delay.map(|delay| now + delay.as_millis() as u64),
        fingerprint,
    };
    *state.clipboard.lock() = Some(armed.clone());
    let _ = app.emit("clipboard-changed", Some(&armed));

    if let Some(delay) = delay {
        let app = app.clone();
        thread::spawn(move || {
            thread::sleep(delay);
            let Some(state) = app.try_state::<AppState>() else {
                return;
            };
            // 期间又复制了别的图、或用户点了「现在清空 / 这次别清」，这一轮就作废
            if state.clipboard_generation.load(Ordering::SeqCst) != generation {
                return;
            }
            clear_if_ours(fingerprint);
            *state.clipboard.lock() = None;
            let _ = app.emit("clipboard-changed", None::<ClipboardState>);
        });
    }
    Ok(())
}

/// 剪贴板里仍是我们放的那张图时才清，别把用户后来复制的东西清掉
fn clear_if_ours(fingerprint: u64) -> bool {
    let Ok(mut clipboard) = Clipboard::new() else {
        return false;
    };
    match clipboard.get_image() {
        Ok(current)
            if image_fingerprint(current.width, current.height, &current.bytes) == fingerprint =>
        {
            clipboard.clear().is_ok()
        }
        _ => false,
    }
}

pub(crate) fn image_fingerprint(width: usize, height: usize, bytes: &[u8]) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    (width, height, bytes).hash(&mut hasher);
    hasher.finish()
}

#[tauri::command]
pub(crate) fn get_clipboard_state(state: State<'_, AppState>) -> Option<ClipboardState> {
    let mut current = state.clipboard.lock();
    // 清空线程到点会置空；这里再兜一层，防止极端情况下留下过期状态
    if current
        .as_ref()
        .and_then(|armed| armed.clear_at)
        .is_some_and(|clear_at| clear_at + 2_000 < now_millis())
    {
        *current = None;
    }
    current.clone()
}

/// 「现在清空」
#[tauri::command]
pub(crate) fn clear_clipboard_now(app: AppHandle, state: State<'_, AppState>) {
    state.clipboard_generation.fetch_add(1, Ordering::SeqCst);
    if let Some(armed) = state.clipboard.lock().take() {
        clear_if_ours(armed.fingerprint);
    }
    let _ = app.emit("clipboard-changed", None::<ClipboardState>);
}

/// 「这次别清」：图留在剪贴板里，只是不再倒计时
#[tauri::command]
pub(crate) fn keep_clipboard(app: AppHandle, state: State<'_, AppState>) {
    state.clipboard_generation.fetch_add(1, Ordering::SeqCst);
    let mut current = state.clipboard.lock();
    if let Some(armed) = current.as_mut() {
        armed.clear_at = None;
    }
    let _ = app.emit("clipboard-changed", current.as_ref());
}

/// 输入框「复制并关闭」：润色结果写进剪贴板。文字不自动清空——它不是截图那种敏感内容。
#[tauri::command]
pub(crate) fn copy_text(text: String) -> Result<(), String> {
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .map_err(|error| format!("写入剪贴板失败：{error}"))
}

/// 输入框「润色剪贴板里的文字」：先把文字读出来放进润色视图，用户看过结果再决定要不要复制
#[tauri::command]
pub(crate) fn read_clipboard_text() -> Result<String, String> {
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .map_err(|_| "剪贴板里没有文字".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_changes_with_pixels_and_size() {
        let a = image_fingerprint(2, 1, &[0, 0, 0, 255, 1, 1, 1, 255]);
        let b = image_fingerprint(2, 1, &[0, 0, 0, 255, 1, 1, 2, 255]);
        let c = image_fingerprint(1, 2, &[0, 0, 0, 255, 1, 1, 1, 255]);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a, image_fingerprint(2, 1, &[0, 0, 0, 255, 1, 1, 1, 255]));
    }
}
