use super::*;

/// 输入框窗口的逻辑宽度：面板 640，两侧各留 40 透明边
pub(crate) const PALETTE_WIDTH: f64 = 720.0;
/// 刚打开时的高度；页面量好自己的实际高度后会马上调 resize_quick_menu
const PALETTE_INITIAL_HEIGHT: f64 = 460.0;
const PALETTE_MIN_HEIGHT: f64 = 200.0;
const PALETTE_MAX_HEIGHT: f64 = 760.0;

/// 右键桌宠：在桌宠旁边展开，往屏幕中心的方向开
#[tauri::command]
pub(crate) fn show_quick_menu(
    app: AppHandle,
    state: State<'_, AppState>,
    x: f64,
    y: f64,
) -> Result<(), String> {
    *state.quick_menu_anchor.lock() = Some((x, y));
    open_palette(&app, &state)
}

/// 全局快捷键呼出：放在光标所在屏幕的上方居中，像系统的搜索框
pub(crate) fn show_palette_centered(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    *state.quick_menu_anchor.lock() = None;
    open_palette(app, &state)
}

fn open_palette(app: &AppHandle, state: &AppState) -> Result<(), String> {
    let window = app
        .get_webview_window("quick-menu")
        .ok_or_else(|| "输入框窗口不存在".to_string())?;
    let height = current_logical_height(&window).unwrap_or(PALETTE_INITIAL_HEIGHT);
    window
        .set_size(LogicalSize::new(PALETTE_WIDTH, height))
        .map_err(|e| e.to_string())?;
    position_palette(app, &window, *state.quick_menu_anchor.lock(), height)?;
    window.show().map_err(|e| e.to_string())?;
    window.set_focus().map_err(|e| e.to_string())?;
    let _ = app.emit_to("quick-menu", "palette-opened", ());
    Ok(())
}

fn current_logical_height(window: &tauri::WebviewWindow) -> Option<f64> {
    let scale = window.scale_factor().ok()?;
    let size = window.inner_size().ok()?;
    Some(size.height as f64 / scale)
}

/// 页面内容变高变矮（进入润色、展开窗口列表）时由页面告诉窗口该多高
#[tauri::command]
pub(crate) fn resize_quick_menu(
    app: AppHandle,
    state: State<'_, AppState>,
    height: f64,
) -> Result<(), String> {
    let window = app
        .get_webview_window("quick-menu")
        .ok_or_else(|| "输入框窗口不存在".to_string())?;
    let height = height.clamp(PALETTE_MIN_HEIGHT, PALETTE_MAX_HEIGHT);
    window
        .set_size(LogicalSize::new(PALETTE_WIDTH, height))
        .map_err(|e| e.to_string())?;
    position_palette(&app, &window, *state.quick_menu_anchor.lock(), height)
}

fn monitor_at(window: &tauri::WebviewWindow, sx: f64, sy: f64) -> Option<tauri::Monitor> {
    window.available_monitors().ok().and_then(|monitors| {
        monitors.into_iter().find(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            sx >= position.x as f64
                && sx <= (position.x + size.width as i32) as f64
                && sy >= position.y as f64
                && sy <= (position.y + size.height as i32) as f64
        })
    })
}

fn position_palette(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    anchor: Option<(f64, f64)>,
    logical_height: f64,
) -> Result<(), String> {
    let scale = window.scale_factor().unwrap_or(1.0);
    let width = PALETTE_WIDTH * scale;
    let height = logical_height * scale;

    let (mut px, mut py, monitor) = match anchor {
        Some((x, y)) => {
            let (sx, sy) = (x * scale, y * scale);
            let monitor = monitor_at(window, sx, sy);
            // 桌宠在哪个象限，就往朝屏幕中心的那只角展开
            let (open_right, open_down) = match &monitor {
                Some(monitor) => {
                    let position = monitor.position();
                    let size = monitor.size();
                    (
                        sx < position.x as f64 + size.width as f64 / 2.0,
                        sy < position.y as f64 + size.height as f64 / 2.0,
                    )
                }
                None => (true, true),
            };
            let px = if open_right {
                sx - 40.0 * scale
            } else {
                sx - width + 40.0 * scale
            };
            let py = if open_down {
                sy - 24.0 * scale
            } else {
                sy - height + 24.0 * scale
            };
            (px, py, monitor)
        }
        None => {
            let cursor = app.cursor_position().ok();
            let monitor = cursor
                .and_then(|point| monitor_at(window, point.x, point.y))
                .or_else(|| window.primary_monitor().ok().flatten());
            match &monitor {
                Some(monitor) => {
                    let position = monitor.position();
                    let size = monitor.size();
                    (
                        position.x as f64 + (size.width as f64 - width) / 2.0,
                        position.y as f64 + size.height as f64 * 0.16,
                        Some(monitor.clone()),
                    )
                }
                None => (200.0, 160.0, None),
            }
        }
    };

    // 钳位兜底：防多屏错位时窗口跑出屏幕
    if let Some(monitor) = monitor {
        let position = monitor.position();
        let size = monitor.size();
        let max_x = (position.x + size.width as i32) as f64 - width;
        let max_y = (position.y + size.height as i32) as f64 - height;
        px = px.clamp(position.x as f64, max_x.max(position.x as f64));
        py = py.clamp(position.y as f64, max_y.max(position.y as f64));
    }
    window
        .set_position(PhysicalPosition::new(px.round() as i32, py.round() as i32))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn hide_quick_menu(app: AppHandle) {
    if let Some(window) = app.get_webview_window("quick-menu") {
        let _ = window.hide();
    }
}

/// 输入框里执行一个会截屏的动作：先把自己藏起来再动手，否则会截进画面里。
/// `target_id` 只对「截一个窗口」有用。失败会记进活动，主窗口的猫会播报。
#[tauri::command]
pub(crate) async fn run_action(
    app: AppHandle,
    action: String,
    target_id: Option<u32>,
) -> Result<String, String> {
    hide_quick_menu(app.clone());
    // 等窗口真的从屏幕上消失（合成器要一两帧）
    let _ =
        tauri::async_runtime::spawn_blocking(|| thread::sleep(Duration::from_millis(160))).await;
    let result = if action == "capture" {
        let state = app.state::<AppState>();
        capture::capture_window(app.clone(), state, target_id)
    } else {
        perform(&app, &action).await
    };
    if let Err(error) = &result {
        activity::record_error(&app, action_failure_title(&action), error);
    }
    result
}

pub(crate) fn action_failure_title(action: &str) -> &'static str {
    match action {
        "snapshot" | "capture" => "截图失败",
        "fullscreen" => "全屏截图失败",
        "scrolling" => "滚动长截图失败",
        "record" => "录制失败",
        "polish" => "润色失败",
        _ => "操作失败",
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
pub(crate) async fn perform(app: &AppHandle, action: &str) -> Result<String, String> {
    let state = app.state::<AppState>();
    match action {
        "snapshot" => capture::capture_window(app.clone(), state, None),
        "palette" => show_palette_centered(app).map(|_| String::new()),
        "record" => recording::toggle_recording(app.clone(), state, None).map(|status| {
            if status.active {
                status.message.unwrap_or_else(|| "录制已开始".into())
            } else {
                "录制已保存".into()
            }
        }),
        "polish" => polish::polish_clipboard(app.clone(), state).await,
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
