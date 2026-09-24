use super::*;

/// 把 RGBA 位图编码成 PNG 字节（标注窗口、应用图标用）。
pub(crate) fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| format!("编码图像失败：{error}"))?;
    Ok(bytes)
}

#[tauri::command]
pub(crate) fn capture_window(
    app: AppHandle,
    state: State<'_, AppState>,
    id: Option<u32>,
) -> Result<String, String> {
    let target_id = id
        .or_else(|| {
            state
                .tracker
                .lock()
                .previous
                .as_ref()
                .map(|window| window.id)
        })
        .ok_or_else(|| "还没有上一个应用可截取".to_string())?;
    let window = Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|window| window.id().ok() == Some(target_id))
        .ok_or_else(|| "目标窗口已经关闭".to_string())?;
    if window.is_minimized().unwrap_or(false) {
        os::restore_minimized(target_id)?;
    }
    let app_name = window.app_name().unwrap_or_else(|_| "应用".into());
    let include_cursor = state.settings.lock().include_cursor;
    let area = match (window.x(), window.y(), window.width(), window.height()) {
        (Ok(x), Ok(y), Ok(width), Ok(height)) => Some(Area {
            x,
            y,
            width,
            height,
        }),
        _ => None,
    };
    let (image, cursor_degraded) = capture_area(include_cursor, area, "窗口", || {
        window
            .capture_image()
            .map_err(|error| format!("截取失败：{error}"))
    })?;
    finalize_capture(&app, &state, image, &app_name, cursor_degraded)
}

/// 一块屏幕区域（窗口或显示器）在屏幕上的位置与逻辑尺寸
pub(crate) struct Area {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: u32,
    /// 只有 Linux 让 ffmpeg 按矩形抓时要用；Windows/macOS 合成光标只需宽度换算缩放
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) height: u32,
}

/// 截一块区域；开了「包含光标」就交给平台把光标带上。
///
/// 各平台带光标的办法不同（Windows/macOS 截完再按热点合成，Linux 让 ffmpeg 直接带光标抓），
/// 但失败了都退回不带光标的截图，并把原因拼进成功提示，不让用户以为开关生效了。
fn capture_area(
    include_cursor: bool,
    area: Option<Area>,
    what: &str,
    capture: impl FnOnce() -> Result<RgbaImage, String>,
) -> Result<(RgbaImage, Option<String>), String> {
    if !include_cursor {
        return Ok((capture()?, None));
    }
    let Some(area) = area else {
        let note = cursor_degrade_note(&format!("无法读取{what}几何，已回退无光标截图"));
        return Ok((capture()?, Some(note)));
    };
    let (image, failure) = os::capture_with_cursor(&area, capture)?;
    Ok((image, failure.map(|error| cursor_degrade_note(&error))))
}

/// 把光标合成失败的原因压成短中文，附在截图成功 toast 后。
fn cursor_degrade_note(error: &str) -> String {
    let head = error.split(" / ").next().unwrap_or(error).trim();
    let short: String = head.chars().take(48).collect();
    let ellipsis = if head.chars().count() > 48 { "…" } else { "" };
    format!("（未能包含鼠标光标：{short}{ellipsis}）")
}

/// 统一收尾：按偏好决定是否落盘、剪贴板清空时限、截后动作与闪烁/音效提示。
/// `cursor_degraded`：Linux 带光标静帧失败后回退时的说明，会拼进成功文案。
pub(crate) fn finalize_capture(
    app: &AppHandle,
    state: &AppState,
    image: RgbaImage,
    app_name: &str,
    cursor_degraded: Option<String>,
) -> Result<String, String> {
    let settings = state.settings.lock().clone();
    let mut archived = false;
    if settings.auto_save_local {
        archived = snapshots::store_snapshot(state, &image, app_name).is_ok();
    }

    // after_capture：annotate 打开标注窗；saveas 走另存对话框；默认剪贴板。
    let annotate = settings.after_capture == "annotate";
    if settings.after_capture == "saveas" {
        save_image_as_dialog(app, &image, &settings.snapshot_format)?;
    }

    if annotate {
        open_annotate_window(app, state, &image, app_name)?;
    } else {
        copy_image_to_clipboard(
            image,
            snapshots::clipboard_clear_delay(&settings.clipboard_auto_clear),
        )?;
    }

    let _ = app.emit(
        "capture-feedback",
        json!({
            "flash": settings.flash_on_capture,
            "shutterSound": settings.shutter_sound,
            "customSoundPath": settings.custom_sound_path,
        }),
    );
    if settings.hide_after_copy {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
    }

    let suffix = cursor_degraded.unwrap_or_default();
    if annotate {
        if archived {
            Ok(format!("已打开标注：{app_name}（已存入历史）{suffix}"))
        } else if settings.auto_save_local {
            Ok(format!("已打开标注：{app_name}（未能存入历史）{suffix}"))
        } else {
            Ok(format!("已打开标注：{app_name}{suffix}"))
        }
    } else {
        let clear_label = snapshots::format_clear_label(&settings.clipboard_auto_clear);
        if archived {
            Ok(format!(
                "已复制 {app_name} 并存入历史，{clear_label}{suffix}"
            ))
        } else if settings.auto_save_local {
            Ok(format!(
                "已复制 {app_name}，{clear_label}（未能存入历史）{suffix}"
            ))
        } else {
            Ok(format!("已复制 {app_name}，{clear_label}{suffix}"))
        }
    }
}

fn save_image_as_dialog(
    app: &AppHandle,
    image: &RgbaImage,
    format_setting: &str,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt;
    let (format, ext) = snapshots::snapshot_format_parts(format_setting);
    let suggested = format!("snapshot-{}.{}", Local::now().format("%Y%m%d-%H%M%S"), ext);
    let picked = app
        .dialog()
        .file()
        .set_file_name(&suggested)
        .add_filter("Image", &[ext, "png", "jpg", "webp"])
        .blocking_save_file();
    let Some(file_path) = picked else {
        return Ok(()); // 用户取消另存，不视为错误
    };
    let path = file_path.into_path().map_err(|error| error.to_string())?;
    // 用户在对话框里改了扩展名就按扩展名存；认不出或不支持的扩展名才回退偏好格式
    let format = ImageFormat::from_path(&path)
        .ok()
        .filter(|guessed| {
            matches!(
                guessed,
                ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
            )
        })
        .unwrap_or(format);
    snapshots::write_image(image, &path, format).map_err(|error| format!("另存为失败：{error}"))
}

fn copy_image_to_clipboard(image: RgbaImage, clear_after: Option<Duration>) -> Result<(), String> {
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

    if let Some(delay) = clear_after {
        thread::spawn(move || {
            // 到点前要确认剪贴板里还是这张图（用户可能已经复制了别的）。只留指纹不留原图：
            // 4K 截图一份约 33MB，没必要在内存里攥满整个清空时限。
            let expected = image_fingerprint(width, height, &bytes);
            drop(bytes);
            thread::sleep(delay);
            if let Ok(mut clipboard) = Clipboard::new() {
                if let Ok(current) = clipboard.get_image() {
                    if image_fingerprint(current.width, current.height, &current.bytes) == expected
                    {
                        let _ = clipboard.clear();
                    }
                }
            }
        });
    }
    Ok(())
}

fn image_fingerprint(width: usize, height: usize, bytes: &[u8]) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    (width, height, bytes).hash(&mut hasher);
    hasher.finish()
}

fn open_annotate_window(
    app: &AppHandle,
    state: &AppState,
    image: &RgbaImage,
    title: &str,
) -> Result<(), String> {
    let png = encode_png(image)?;
    *state.annotate_png.lock() = Some(png);
    *state.annotate_title.lock() = title.to_string();
    let window = app
        .get_webview_window("annotate")
        .ok_or_else(|| "标注窗口不存在".to_string())?;
    // 按图幅大致缩放窗口，避免小图撑满或大图溢出
    let w = image.width().clamp(480, 1280) as f64;
    let h = (image.height().clamp(360, 900) + 56) as f64;
    let _ = window.set_size(LogicalSize::new(w, h));
    let _ = window.center();
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())?;
    let _ = app.emit("annotate-ready", json!({ "title": title }));
    Ok(())
}

#[tauri::command]
pub(crate) fn get_annotate_image(state: State<'_, AppState>) -> Result<Option<String>, String> {
    let Some(png) = state.annotate_png.lock().clone() else {
        return Ok(None);
    };
    Ok(Some(format!(
        "data:image/png;base64,{}",
        BASE64.encode(png)
    )))
}

#[tauri::command]
pub(crate) fn annotate_get_title(state: State<'_, AppState>) -> String {
    state.annotate_title.lock().clone()
}

fn decode_png_base64(data: &str) -> Result<RgbaImage, String> {
    let trimmed = data
        .strip_prefix("data:image/png;base64,")
        .or_else(|| data.strip_prefix("data:image/jpeg;base64,"))
        .unwrap_or(data);
    let bytes = BASE64
        .decode(trimmed.trim())
        .map_err(|error| format!("标注图解码失败：{error}"))?;
    image::load_from_memory(&bytes)
        .map(|img| img.to_rgba8())
        .map_err(|error| format!("标注图解析失败：{error}"))
}

#[tauri::command]
pub(crate) fn annotate_copy(
    app: AppHandle,
    state: State<'_, AppState>,
    image_data: String,
) -> Result<String, String> {
    let image = decode_png_base64(&image_data)?;
    let settings = state.settings.lock().clone();
    if settings.auto_save_local {
        let _ = snapshots::store_snapshot(&state, &image, "标注");
    }
    copy_image_to_clipboard(
        image,
        snapshots::clipboard_clear_delay(&settings.clipboard_auto_clear),
    )?;
    hide_annotate_window(&app, &state);
    Ok(format!(
        "已复制标注图，{}",
        snapshots::format_clear_label(&settings.clipboard_auto_clear)
    ))
}

#[tauri::command]
pub(crate) fn annotate_save(
    app: AppHandle,
    state: State<'_, AppState>,
    image_data: String,
) -> Result<String, String> {
    let image = decode_png_base64(&image_data)?;
    let format = state.settings.lock().snapshot_format.clone();
    save_image_as_dialog(&app, &image, &format)?;
    hide_annotate_window(&app, &state);
    Ok("已保存标注图".into())
}

#[tauri::command]
pub(crate) fn annotate_close(app: AppHandle, state: State<'_, AppState>) {
    hide_annotate_window(&app, &state);
}

fn hide_annotate_window(app: &AppHandle, state: &AppState) {
    *state.annotate_png.lock() = None;
    if let Some(window) = app.get_webview_window("annotate") {
        let _ = window.hide();
    }
}

fn primary_monitor() -> Result<Monitor, String> {
    let monitors = Monitor::all().map_err(|error| format!("无法枚举显示器：{error}"))?;
    monitors
        .into_iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false))
        .or_else(|| {
            Monitor::all()
                .ok()
                .and_then(|items| items.into_iter().next())
        })
        .ok_or_else(|| "未找到可用显示器".into())
}

pub(crate) fn capture_fullscreen_image(
    include_cursor: bool,
) -> Result<(RgbaImage, String, Option<String>), String> {
    let monitor = primary_monitor()?;
    let name = monitor.name().unwrap_or_else(|_| "全屏".into());
    let area = match (monitor.x(), monitor.y(), monitor.width(), monitor.height()) {
        (Ok(x), Ok(y), Ok(width), Ok(height)) => Some(Area {
            x,
            y,
            width,
            height,
        }),
        _ => None,
    };
    let (image, note) = capture_area(include_cursor, area, "显示器", || {
        monitor
            .capture_image()
            .map_err(|error| format!("全屏截取失败：{error} / fullscreen capture failed: {error}"))
    })?;
    Ok((image, name, note))
}

/// 按 alpha 把光标叠到截图上，落在画布外的像素直接丢弃。
///
/// Windows 与 macOS 都是「截完再合成光标」，合成这一步与平台无关，放在这里共用；
/// Linux 由 ffmpeg 直接带光标抓，用不上。
#[cfg(any(target_os = "windows", target_os = "macos"))]
pub(crate) fn blend_cursor(canvas: &mut RgbaImage, cursor: &RgbaImage, left: i64, top: i64) {
    for (x, y, pixel) in cursor.enumerate_pixels() {
        let alpha = u32::from(pixel[3]);
        if alpha == 0 {
            continue;
        }
        let target_x = left + i64::from(x);
        let target_y = top + i64::from(y);
        if target_x < 0
            || target_y < 0
            || target_x >= i64::from(canvas.width())
            || target_y >= i64::from(canvas.height())
        {
            continue;
        }
        let base = *canvas.get_pixel(target_x as u32, target_y as u32);
        let mix = |over: u8, under: u8| -> u8 {
            ((u32::from(over) * alpha + u32::from(under) * (255 - alpha)) / 255) as u8
        };
        canvas.put_pixel(
            target_x as u32,
            target_y as u32,
            image::Rgba([
                mix(pixel[0], base[0]),
                mix(pixel[1], base[1]),
                mix(pixel[2], base[2]),
                base[3].max(pixel[3]),
            ]),
        );
    }
}

#[cfg(all(test, any(target_os = "windows", target_os = "macos")))]
mod blend_tests {
    use super::blend_cursor;
    use image::{Rgba, RgbaImage};

    #[test]
    fn respects_alpha_and_clips_out_of_bounds() {
        let mut canvas = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        let overlay = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
        blend_cursor(&mut canvas, &overlay, 3, 3); // 只有左上角那个像素落在画布内
        assert_eq!(canvas.get_pixel(3, 3).0, [255, 255, 255, 255]);
        assert_eq!(canvas.get_pixel(0, 0).0, [0, 0, 0, 255]);
    }

    #[test]
    fn fully_transparent_cursor_changes_nothing() {
        let mut canvas = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
        let overlay = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 0]));
        blend_cursor(&mut canvas, &overlay, 0, 0);
        assert_eq!(canvas.get_pixel(0, 0).0, [10, 20, 30, 255]);
    }

    #[test]
    fn half_alpha_blends_halfway() {
        let mut canvas = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
        let overlay = RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 128]));
        blend_cursor(&mut canvas, &overlay, 0, 0);
        let value = canvas.get_pixel(0, 0).0[0];
        assert!((127..=129).contains(&value), "got {value}");
    }
}
