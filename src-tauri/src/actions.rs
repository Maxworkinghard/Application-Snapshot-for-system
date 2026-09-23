use super::*;

#[tauri::command]
pub(crate) fn show_quick_menu(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    let window = app
        .get_webview_window("quick-menu")
        .ok_or_else(|| "快捷菜单窗口不存在".to_string())?;
    *state.quick_menu_anchor.lock() = Some((x, y));
    window
        .set_size(LogicalSize::new(286.0, 150.0))
        .map_err(|e| e.to_string())?;
    position_quick_menu(&window, x, y, 150.0)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_quick_menu_expanded(
    app: AppHandle,
    state: State<'_, AppState>,
    expanded: bool,
) -> Result<(), String> {
    let window = app
        .get_webview_window("quick-menu")
        .ok_or_else(|| "快捷菜单窗口不存在".to_string())?;
    let height = if expanded { 246.0 } else { 150.0 };
    window
        .set_size(LogicalSize::new(286.0, height))
        .map_err(|e| e.to_string())?;
    if let Some((x, y)) = *state.quick_menu_anchor.lock() {
        position_quick_menu(&window, x, y, height)?;
    }
    Ok(())
}

fn position_quick_menu(
    window: &tauri::WebviewWindow,
    x: f64,
    y: f64,
    logical_height: f64,
) -> Result<(), String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let menu_width = 286.0 * scale;
    let menu_height = logical_height * scale;
    let sx = x * scale;
    let sy = y * scale;

    // 光标（即图标）所在的显示器，用它把桌面划成四个象限
    let monitor = window.available_monitors().ok().and_then(|monitors| {
        monitors.into_iter().find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            sx >= position.x as f64
                && sx <= (position.x + size.width as i32) as f64
                && sy >= position.y as f64
                && sy <= (position.y + size.height as i32) as f64
        })
    });

    // 图标在哪个象限，菜单就往图标朝向屏幕中心的那只角展开：
    // 左上象限→图标右下角，右上→左下角，左下→右上角，右下→左上角。
    // 找不到显示器时按最常见的左上象限处理（向右下展开）
    let (open_right, open_down) = match &monitor {
        Some(monitor) => {
            let position = monitor.position();
            let size = monitor.size();
            let center_x = position.x as f64 + size.width as f64 / 2.0;
            let center_y = position.y as f64 + size.height as f64 / 2.0;
            (sx < center_x, sy < center_y)
        }
        None => (true, true),
    };

    // 18px 搭边：光标刚好搭在菜单角上，选第一项不用挪鼠标
    let mut px = if open_right {
        sx - 18.0
    } else {
        sx - menu_width + 18.0
    };
    let mut py = if open_down {
        sy - 18.0
    } else {
        sy - menu_height + 18.0
    };

    // 钳位兜底：象限逻辑已经朝屏幕中心开了，这层只是防极端多屏/错位
    if let Some(monitor) = monitor {
        let position = monitor.position();
        let size = monitor.size();
        px = px.clamp(
            position.x as f64 + 8.0,
            (position.x + size.width as i32) as f64 - menu_width - 8.0,
        );
        py = py.clamp(
            position.y as f64 + 8.0,
            (position.y + size.height as i32) as f64 - menu_height - 8.0,
        );
    }
    window
        .set_position(PhysicalPosition::new(px.round() as i32, py.round() as i32))
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn hide_quick_menu(app: AppHandle) {
    if let Some(window) = app.get_webview_window("quick-menu") {
        let _ = window.hide();
    }
}

/// 第二个实例启动时把已有窗口唤到前台。
/// 这里刻意不调 center()：窗口多半已在用户摆好的位置上，没必要弹回屏幕中央。
pub(crate) fn focus_existing_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
pub(crate) fn show_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.center();
        let _ = window.set_focus();
    }
}

/// 执行一个全局快捷键动作（由 shortcuts.rs 的按键回调调用）。
///
/// 快捷键版 OCR 会把识别出的文字写回剪贴板：界面上的 ocr_clipboard 只负责返回文字
/// （页面自己展示），走快捷键时用户看不到界面，必须把结果送回剪贴板才有意义。
pub(crate) async fn perform(app: &AppHandle, action: &str) -> Result<String, String> {
    let state = app.state::<AppState>();
    match action {
        "snapshot" => capture::capture_window(app.clone(), state, None),
        "record" => recording::toggle_recording(state, None).map(|status| {
            if status.active {
                status.message.unwrap_or_else(|| "录制已开始".into())
            } else {
                "录制已保存".into()
            }
        }),
        "polish" => polish::polish_clipboard(state).await,
        "ocr" => capture::ocr_clipboard_into_clipboard().await,
        "fullscreen" => {
            let include_cursor = state.settings.lock().include_cursor;
            let (image, name, cursor_degraded) = tauri::async_runtime::spawn_blocking(move || {
                capture::capture_fullscreen_image(include_cursor)
            })
            .await
            .map_err(|error| error.to_string())??;
            capture::finalize_capture(app, &state, image, &name, cursor_degraded)
        }
        // 滚动长截图只有 Linux/X11 有实现（其余平台的 SHORTCUT_ACTIONS 里也没有这一项）
        #[cfg(target_os = "linux")]
        "scrolling" => {
            let target_id = state
                .tracker
                .lock()
                .previous
                .as_ref()
                .map(|window| window.id)
                .ok_or_else(|| "还没有上一个应用可截取".to_string())?;
            let image = tauri::async_runtime::spawn_blocking(move || {
                os::capture_scrolling_window(target_id)
            })
            .await
            .map_err(|error| error.to_string())??;
            capture::finalize_capture(app, &state, image, "滚动长截图", None)
        }
        _ => Err("未知快捷键动作".into()),
    }
}
