use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OcrCapability {
    available: bool,
    language: Option<String>,
    detail: String,
}

/// 把 RGBA 位图编码成 PNG 字节，喂给 WinRT 的 BitmapDecoder。
/// 走 PNG 而不是直接构造 SoftwareBitmap，是为了绕开 IBufferByteAccess 那套 COM 互操作。
fn encode_png(image: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    DynamicImage::ImageRgba8(image.clone())
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| format!("编码图像失败：{error}"))?;
    Ok(bytes)
}

#[tauri::command]
pub(crate) fn ocr_capability() -> OcrCapability {
    let report = ocr::report();
    OcrCapability {
        available: report.available,
        language: report.language,
        detail: report.detail,
    }
}

/// 识别历史库里的某张快照
#[tauri::command]
pub(crate) async fn ocr_snapshot(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let record = snapshots::read_snapshot_index(&snapshots::snapshot_index_path(&state))
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| "找不到这条快照".to_string())?;
    let bytes = fs::read(snapshots::history_dir(&state).join(&record.file_name))
        .map_err(|_| "快照文件已被移动或删除".to_string())?;
    // WinRT 这套调用是阻塞的，挪到阻塞线程池，别卡住界面
    tauri::async_runtime::spawn_blocking(move || ocr::adapter().recognize_png(&bytes))
        .await
        .map_err(|error| error.to_string())?
}

/// 识别当前剪贴板里的图片
#[tauri::command]
pub(crate) async fn ocr_clipboard() -> Result<String, String> {
    let image = Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_image())
        .map_err(|_| "剪贴板里没有图片，请先截一张".to_string())?;
    let width = image.width as u32;
    let height = image.height as u32;
    let buffer = RgbaImage::from_raw(width, height, image.bytes.into_owned())
        .ok_or_else(|| "剪贴板图像数据不完整".to_string())?;
    let png = encode_png(&buffer)?;
    tauri::async_runtime::spawn_blocking(move || ocr::adapter().recognize_png(&png))
        .await
        .map_err(|error| error.to_string())?
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
        restore_minimized_window(target_id)?;
    }
    let app_name = window.app_name().unwrap_or_else(|_| "应用".into());
    let include_cursor = state.settings.lock().include_cursor;
    let (image, cursor_degraded) = capture_window_image(&window, include_cursor)?;
    finalize_capture(&app, &state, image, &app_name, cursor_degraded)
}

fn capture_window_image(
    window: &Window,
    include_cursor: bool,
) -> Result<(RgbaImage, Option<String>), String> {
    #[cfg(target_os = "linux")]
    if include_cursor {
        match (window.x(), window.y(), window.width(), window.height()) {
            (Ok(x), Ok(y), Ok(width), Ok(height)) => {
                match linux::capture_region_with_cursor(x, y, width, height) {
                    Ok(image) => return Ok((image, None)),
                    Err(error) => {
                        let note = cursor_degrade_note(&error);
                        let image = window
                            .capture_image()
                            .map_err(|error| format!("截取失败：{error}"))?;
                        return Ok((image, Some(note)));
                    }
                }
            }
            _ => {
                let note = cursor_degrade_note("无法读取窗口几何，已回退无光标截图");
                let image = window
                    .capture_image()
                    .map_err(|error| format!("截取失败：{error}"))?;
                return Ok((image, Some(note)));
            }
        }
    }
    let _ = include_cursor;
    #[allow(unused_mut, unused_assignments)]
    let mut image = window
        .capture_image()
        .map_err(|error| format!("截取失败：{error}"))?;
    #[allow(unused_mut)]
    let mut note = None;
    #[cfg(target_os = "macos")]
    if include_cursor {
        // xcap 截不到光标，截完按窗口原点与 DPI 比例把当前系统光标合成上去
        note = match (window.x(), window.y(), window.width()) {
            (Ok(x), Ok(y), Ok(width)) => mac_cursor::overlay_into(&mut image, (x, y), width)
                .err()
                .map(|error| cursor_degrade_note(&error)),
            _ => Some(cursor_degrade_note("无法读取窗口几何，已回退无光标截图")),
        };
    }
    #[cfg(target_os = "windows")]
    if include_cursor {
        // 同 macOS：xcap 只给窗口像素，光标要自己画
        note = match (window.x(), window.y(), window.width()) {
            (Ok(x), Ok(y), Ok(width)) => windows_cursor::overlay_into(&mut image, (x, y), width)
                .err()
                .map(|error| cursor_degrade_note(&error)),
            _ => Some(cursor_degrade_note("无法读取窗口几何，已回退无光标截图")),
        };
    }
    Ok((image, note))
}

/// 把光标合成失败的原因压成短中文，附在截图成功 toast 后。
#[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
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
    let dynamic = DynamicImage::ImageRgba8(image.clone());
    let result = match format {
        ImageFormat::Jpeg => dynamic.to_rgb8().save_with_format(&path, ImageFormat::Jpeg),
        other => {
            // 若用户改了扩展名，按路径猜测；失败再回退偏好格式
            if let Ok(guessed) = ImageFormat::from_path(&path) {
                if guessed == ImageFormat::Jpeg {
                    dynamic.to_rgb8().save_with_format(&path, ImageFormat::Jpeg)
                } else {
                    dynamic.save_with_format(&path, guessed)
                }
            } else {
                dynamic.save_with_format(&path, other)
            }
        }
    };
    result.map_err(|error| format!("另存为失败：{error}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
pub(crate) fn restore_minimized_window(id: u32) -> Result<(), String> {
    use windows_sys::Win32::{
        Foundation::HWND,
        UI::WindowsAndMessaging::{IsIconic, ShowWindow, SW_RESTORE},
    };
    let handle: HWND = id as usize as *mut _;
    unsafe {
        // ShowWindow 的返回值是「调用前窗口是否可见」，不是成败，据此判断会误报成功。
        let _ = ShowWindow(handle, SW_RESTORE);
    }
    thread::sleep(Duration::from_millis(220));
    // 真正的成败只能事后问：仍是最小化就说明系统或目标应用没有接受这次还原。
    if unsafe { IsIconic(handle) } != 0 {
        return Err("目标窗口仍处于最小化，未能还原".into());
    }
    Ok(())
}

/// macOS：用 Accessibility API 还原最小化窗口。
/// AX 没有 Windows hwnd 那样的窗口句柄，只能按进程 + 标题对齐，
/// 详见 mac_ax::unminimize_window。
#[cfg(target_os = "macos")]
pub(crate) fn restore_minimized_window(id: u32) -> Result<(), String> {
    mac_ax::unminimize_window(id)?;
    // Dock 还原动画比 Windows 的 SW_RESTORE 慢，多等一会再截
    thread::sleep(Duration::from_millis(400));
    Ok(())
}

#[cfg(target_os = "macos")]
mod mac_cursor {
    //! 静帧截图合成鼠标光标。
    //!
    //! xcap 的窗口位图不含光标，录制那条路有 ScreenCaptureKit 的 `showsCursor`，静帧
    //! 没有对应开关，只能截完再自己画上去——与 Windows 侧同样的做法。

    use image::RgbaImage;
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSCursor, NSDeviceRGBColorSpace, NSGraphicsContext,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    extern "C" {
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
        fn CFRelease(cf: *const c_void);
    }

    /// 光标位置：全局坐标、左上原点、点为单位——正好是 xcap 报窗口位置用的坐标系。
    ///
    /// 不用 `NSEvent::mouseLocation`：那个是左下原点，翻 y 要主屏高度，而 `NSScreen`
    /// 在 objc2 里需要 `MainThreadMarker`，截图不一定跑在主线程上。
    fn cursor_position() -> Option<(f64, f64)> {
        unsafe {
            let event = CGEventCreate(std::ptr::null());
            if event.is_null() {
                return None;
            }
            let point = CGEventGetLocation(event);
            CFRelease(event);
            Some((point.x, point.y))
        }
    }

    /// 当前系统光标：按 `scale` 渲染成像素位图，附带热点（点单位，左上原点）。
    ///
    /// 走 PNG 中转而不是直接读 `bitmapData`：省掉一段裸指针算术，光标只有几十像素，
    /// 这点编解码开销可以忽略。
    // currentSystemCursor 已被苹果标记弃用，推荐改用 ScreenCaptureKit 的
    // SCStreamConfiguration.showsCursor。这里仍然用它，因为替代品 NSCursor::currentCursor
    // 只知道**本应用**的光标：截别人的窗口时它返回我们自己的箭头，而不是对方正在显示的
    // I 形/手形光标，语义是错的。录制已使用 ScreenCaptureKit；静帧仍需单独迁移。
    #[allow(deprecated)]
    fn cursor_bitmap(scale: f64) -> Option<(RgbaImage, f64, f64)> {
        unsafe {
            let cursor = NSCursor::currentSystemCursor()?;
            let hot_spot = cursor.hotSpot();
            let nsimage = cursor.image();
            let size = nsimage.size();
            if size.width < 1.0 || size.height < 1.0 {
                return None;
            }
            let pixel_width = (size.width * scale).round().max(1.0);
            let pixel_height = (size.height * scale).round().max(1.0);
            let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
                NSBitmapImageRep::alloc(),
                std::ptr::null_mut(),
                pixel_width as isize,
                pixel_height as isize,
                8,
                4,
                true,
                false,
                NSDeviceRGBColorSpace,
                0,
                0,
            )?;
            let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
            NSGraphicsContext::saveGraphicsState_class();
            NSGraphicsContext::setCurrentContext(Some(&context));
            nsimage.drawInRect(NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(pixel_width, pixel_height),
            ));
            NSGraphicsContext::restoreGraphicsState_class();
            let png = rep.representationUsingType_properties(
                NSBitmapImageFileType::PNG,
                &NSDictionary::new(),
            )?;
            let decoded = image::load_from_memory(&png.to_vec()).ok()?.to_rgba8();
            Some((decoded, hot_spot.x, hot_spot.y))
        }
    }

    /// 光标左上角落在截图里的像素坐标：先减热点，再把「点」按 DPI 比例换成像素。
    fn overlay_origin_px(
        cursor: (f64, f64),
        hot_spot: (f64, f64),
        window_origin: (i32, i32),
        scale: f64,
    ) -> (i64, i64) {
        let left = (cursor.0 - hot_spot.0 - window_origin.0 as f64) * scale;
        let top = (cursor.1 - hot_spot.1 - window_origin.1 as f64) * scale;
        (left.round() as i64, top.round() as i64)
    }

    /// source-over 合成，超出边界的部分裁掉。
    fn blend(canvas: &mut RgbaImage, overlay: &RgbaImage, left: i64, top: i64) {
        for (ox, oy, pixel) in overlay.enumerate_pixels() {
            let x = left + ox as i64;
            let y = top + oy as i64;
            if x < 0 || y < 0 || x >= canvas.width() as i64 || y >= canvas.height() as i64 {
                continue;
            }
            let alpha = pixel.0[3] as f32 / 255.0;
            if alpha <= 0.0 {
                continue;
            }
            let base = canvas.get_pixel_mut(x as u32, y as u32);
            for channel in 0..3 {
                base.0[channel] = (pixel.0[channel] as f32 * alpha
                    + base.0[channel] as f32 * (1.0 - alpha))
                    .round()
                    .clamp(0.0, 255.0) as u8;
            }
            base.0[3] = base.0[3].max(pixel.0[3]);
        }
    }

    /// 把当前光标合成进刚截下来的位图。
    ///
    /// `window_origin` / `logical_width` 是 xcap 报的窗口左上角与逻辑宽度（点），
    /// 用来换算截图像素与点的比例（Retina 上是 2）。光标不在这张图范围内时什么都不做。
    pub fn overlay_into(
        image: &mut RgbaImage,
        window_origin: (i32, i32),
        logical_width: u32,
    ) -> Result<(), String> {
        if logical_width == 0 {
            return Err("窗口逻辑宽度为 0，无法换算缩放".into());
        }
        let scale = image.width() as f64 / logical_width as f64;
        if !(0.5..=4.0).contains(&scale) {
            return Err(format!("截图与窗口的缩放比例异常（{scale:.2}）"));
        }
        let position = cursor_position().ok_or("取不到光标位置")?;
        let (bitmap, hot_x, hot_y) = cursor_bitmap(scale).ok_or("取不到当前系统光标位图")?;
        let (left, top) = overlay_origin_px(position, (hot_x, hot_y), window_origin, scale);
        blend(image, &bitmap, left, top);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use image::Rgba;

        #[test]
        fn origin_subtracts_hot_spot_and_window_offset() {
            // 光标在 (120,140)，窗口左上角 (100,100)，热点 (4,4)，1x 屏
            assert_eq!(
                overlay_origin_px((120.0, 140.0), (4.0, 4.0), (100, 100), 1.0),
                (16, 36)
            );
        }

        #[test]
        fn retina_scale_doubles_the_offset() {
            // 同样的点位，2x 屏上像素偏移要翻倍
            assert_eq!(
                overlay_origin_px((120.0, 140.0), (4.0, 4.0), (100, 100), 2.0),
                (32, 72)
            );
        }

        #[test]
        fn blend_respects_alpha_and_clips_out_of_bounds() {
            let mut canvas = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
            let overlay = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 255]));
            blend(&mut canvas, &overlay, 3, 3); // 只有左上角那个像素落在画布内
            assert_eq!(canvas.get_pixel(3, 3).0, [255, 255, 255, 255]);
            assert_eq!(canvas.get_pixel(0, 0).0, [0, 0, 0, 255]);
        }

        #[test]
        fn fully_transparent_overlay_changes_nothing() {
            let mut canvas = RgbaImage::from_pixel(2, 2, Rgba([10, 20, 30, 255]));
            let overlay = RgbaImage::from_pixel(2, 2, Rgba([255, 255, 255, 0]));
            blend(&mut canvas, &overlay, 0, 0);
            assert_eq!(canvas.get_pixel(0, 0).0, [10, 20, 30, 255]);
        }

        #[test]
        fn half_alpha_blends_halfway() {
            let mut canvas = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 255]));
            let overlay = RgbaImage::from_pixel(1, 1, Rgba([255, 255, 255, 128]));
            blend(&mut canvas, &overlay, 0, 0);
            let value = canvas.get_pixel(0, 0).0[0];
            assert!((127..=129).contains(&value), "got {value}");
        }
    }
}

#[cfg(target_os = "macos")]
mod mac_ax {
    use core_foundation::{
        array::CFArray,
        base::{CFType, TCFType},
        boolean::CFBoolean,
        string::CFString,
    };
    use std::{ffi::c_void, os::raw::c_int, ptr};

    type AXUIElementRef = *const c_void;

    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: *const c_void,
            value: *mut *const c_void,
        ) -> c_int;
        fn AXUIElementSetAttributeValue(
            element: AXUIElementRef,
            attribute: *const c_void,
            value: *const c_void,
        ) -> c_int;
        fn CFRelease(cf: *const c_void);
    }

    pub fn unminimize_window(window_id: u32) -> Result<(), String> {
        let target = xcap::Window::all()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|window| window.id().ok() == Some(window_id))
            .ok_or_else(|| "目标窗口已关闭".to_string())?;
        let pid = target.pid().map_err(|error| error.to_string())?;
        let title = target.title().unwrap_or_default();
        unsafe {
            if !AXIsProcessTrusted() {
                return Err(
                    "还原最小化窗口需要「辅助功能」权限，请在系统设置 → 隐私与安全性中授权后重试"
                        .into(),
                );
            }
            let app = AXUIElementCreateApplication(pid as c_int);
            if app.is_null() {
                return Err("无法访问目标应用".to_string());
            }
            let result = unminimize_app_windows(app, &title);
            CFRelease(app);
            result
        }
    }

    /// Windows 的 SW_RESTORE 只还原被指向的那一个窗口；AX 这边没有窗口句柄，
    /// 用 xcap 侧的标题去对 AXTitle，尽量只还原被截的那个。
    /// 标题为空或对不上时（个别应用不暴露 AXTitle），退回还原该进程全部最小化窗口。
    /// 标题对不上时的去向。
    ///
    /// 原先是「一律还原该进程全部最小化窗口」——只有一个最小化窗口时这和还原那一个
    /// 是同一件事，但有好几个时会把用户收起来的窗口一并掀开，而且多半还猜错。
    /// 所以只在唯一确定时代劳，含糊时交回给用户，向 Windows 的 SW_RESTORE
    /// 「只动被指向的那一个」靠拢。
    #[derive(Debug, PartialEq, Eq)]
    enum Fallback {
        /// 只有一个最小化窗口，不会猜错
        RestoreOnly(isize),
        /// 没有最小化的窗口，无事可做
        NothingToDo,
        /// 多个最小化窗口且标题对不上，不替用户决定
        Ambiguous(usize),
    }

    fn decide_fallback(minimized: &[isize]) -> Fallback {
        match minimized {
            [] => Fallback::NothingToDo,
            [only] => Fallback::RestoreOnly(*only),
            many => Fallback::Ambiguous(many.len()),
        }
    }

    unsafe fn unminimize_app_windows(app: AXUIElementRef, title: &str) -> Result<(), String> {
        let attr_windows = CFString::new("AXWindows");
        let mut raw: *const c_void = ptr::null();
        let err = AXUIElementCopyAttributeValue(
            app,
            attr_windows.as_concrete_TypeRef() as *const c_void,
            &mut raw,
        );
        if err != 0 || raw.is_null() {
            return Err("无法读取目标应用的窗口列表".into());
        }
        let windows: CFArray<CFType> = CFArray::wrap_under_create_rule(raw as _);
        let attr_minimized = CFString::new("AXMinimized");

        // 先扫一遍：标题命中哪些、哪些确实处于最小化
        let mut title_hits: Vec<isize> = Vec::new();
        let mut minimized: Vec<isize> = Vec::new();
        for index in 0..windows.len() {
            let Some(item) = windows.get(index) else {
                continue;
            };
            let element = item.as_concrete_TypeRef() as AXUIElementRef;
            if !title.is_empty() && ax_string(element, "AXTitle").as_deref() == Some(title) {
                title_hits.push(index);
            }
            if is_minimized(element, &attr_minimized) {
                minimized.push(index);
            }
        }

        let targets: Vec<isize> = if title_hits.is_empty() {
            match decide_fallback(&minimized) {
                Fallback::NothingToDo => return Ok(()),
                Fallback::RestoreOnly(index) => vec![index],
                Fallback::Ambiguous(count) => {
                    return Err(format!(
                        "目标窗口已最小化，但该应用有 {count} 个最小化窗口、且系统没有报告可用的窗口标题，\
                         无法确定是哪一个。请先手动还原目标窗口再截图。"
                    ))
                }
            }
        } else {
            // 标题命中的里面只还原确实最小化的那些
            title_hits
                .into_iter()
                .filter(|index| minimized.contains(index))
                .collect()
        };

        for index in targets {
            let Some(item) = windows.get(index) else {
                continue;
            };
            let element = item.as_concrete_TypeRef() as AXUIElementRef;
            let _ = AXUIElementSetAttributeValue(
                element,
                attr_minimized.as_concrete_TypeRef() as *const c_void,
                CFBoolean::false_value().as_concrete_TypeRef() as *const c_void,
            );
        }
        Ok(())
    }

    unsafe fn is_minimized(element: AXUIElementRef, attr_minimized: &CFString) -> bool {
        let mut value: *const c_void = ptr::null();
        if AXUIElementCopyAttributeValue(
            element,
            attr_minimized.as_concrete_TypeRef() as *const c_void,
            &mut value,
        ) != 0
            || value.is_null()
        {
            return false;
        }
        CFBoolean::wrap_under_create_rule(value as _) == CFBoolean::true_value()
    }

    unsafe fn ax_string(element: AXUIElementRef, attribute: &str) -> Option<String> {
        let name = CFString::new(attribute);
        let mut raw: *const c_void = ptr::null();
        if AXUIElementCopyAttributeValue(
            element,
            name.as_concrete_TypeRef() as *const c_void,
            &mut raw,
        ) != 0
            || raw.is_null()
        {
            return None;
        }
        Some(CFString::wrap_under_create_rule(raw as _).to_string())
    }

    #[cfg(test)]
    mod tests {
        use super::{decide_fallback, Fallback};

        #[test]
        fn single_minimized_window_is_unambiguous() {
            assert_eq!(decide_fallback(&[2]), Fallback::RestoreOnly(2));
        }

        #[test]
        fn nothing_minimized_is_a_no_op() {
            assert_eq!(decide_fallback(&[]), Fallback::NothingToDo);
        }

        #[test]
        fn several_minimized_windows_are_left_to_the_user() {
            // 原先这种情况会把三个窗口全掀开
            assert_eq!(decide_fallback(&[0, 1, 4]), Fallback::Ambiguous(3));
        }
    }
}
#[cfg(target_os = "linux")]
pub(crate) fn restore_minimized_window(id: u32) -> Result<(), String> {
    linux::restore_minimized(id)
}

#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "macos"),
    not(target_os = "linux")
))]
pub(crate) fn restore_minimized_window(_id: u32) -> Result<(), String> {
    Err("目标窗口已最小化，请先还原".into())
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
            thread::sleep(delay);
            if let Ok(mut clipboard) = Clipboard::new() {
                if let Ok(current) = clipboard.get_image() {
                    if current.width == width
                        && current.height == height
                        && current.bytes.as_ref() == bytes.as_slice()
                    {
                        let _ = clipboard.clear();
                    }
                }
            }
        });
    }
    Ok(())
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

#[cfg(target_os = "linux")]
pub(crate) fn capture_scrolling_image(target_id: u32) -> Result<(RgbaImage, String), String> {
    let image = linux::capture_scrolling_window(target_id)?;
    Ok((image, "滚动长截图".into()))
}

pub(crate) async fn ocr_clipboard_into_clipboard() -> Result<String, String> {
    let text = ocr_clipboard().await?;
    let count = text.chars().count();
    Clipboard::new()
        .and_then(|mut clipboard| clipboard.set_text(text))
        .map_err(|error| format!("写入剪贴板失败：{error}"))?;
    Ok(format!("已提取 {count} 个字符到剪贴板"))
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
    #[cfg(target_os = "linux")]
    if include_cursor {
        match linux::capture_primary_with_cursor() {
            Ok(image) => return Ok((image, name, None)),
            Err(error) => {
                let note = cursor_degrade_note(&error);
                let image = monitor.capture_image().map_err(|error| {
                    format!("全屏截取失败：{error} / fullscreen capture failed: {error}")
                })?;
                return Ok((image, name, Some(note)));
            }
        }
    }
    let _ = include_cursor;
    #[allow(unused_mut)]
    let mut image = monitor
        .capture_image()
        .map_err(|error| format!("全屏截取失败：{error} / fullscreen capture failed: {error}"))?;
    #[allow(unused_mut)]
    let mut note = None;
    #[cfg(target_os = "macos")]
    if include_cursor {
        note = match (monitor.x(), monitor.y(), monitor.width()) {
            (Ok(mx), Ok(my), Ok(mw)) => mac_cursor::overlay_into(&mut image, (mx, my), mw)
                .err()
                .map(|error| cursor_degrade_note(&error)),
            _ => Some(cursor_degrade_note("无法读取显示器几何，已回退无光标截图")),
        };
    }
    #[cfg(target_os = "windows")]
    if include_cursor {
        note = match (monitor.x(), monitor.y(), monitor.width()) {
            (Ok(mx), Ok(my), Ok(mw)) => windows_cursor::overlay_into(&mut image, (mx, my), mw)
                .err()
                .map(|error| cursor_degrade_note(&error)),
            _ => Some(cursor_degrade_note("无法读取显示器几何，已回退无光标截图")),
        };
    }
    Ok((image, name, note))
}
