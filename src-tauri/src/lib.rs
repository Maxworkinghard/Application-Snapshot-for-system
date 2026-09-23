mod capture;
mod ocr;
mod pet;
mod platform;
mod polish;
mod recording;
mod settings;
mod snapshots;
mod tracker;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows_recorder;

use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::Local;
use image::{DynamicImage, ImageFormat, RgbaImage};
use parking_lot::Mutex;
// 只有 macOS 的 recorder sidecar 要按行读子进程输出
use serde::Serialize;
use serde_json::{json, Value};
#[cfg(target_os = "macos")]
use std::io::{BufRead, BufReader};
use std::{
    borrow::Cow,
    fs,
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
// macOS 的录制 sidecar 走 stdin/stdout 管道通信，只有它需要 Stdio
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
use std::process::Stdio;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, State, WindowEvent,
};
use xcap::{Monitor, Window};

struct AppState {
    settings_path: PathBuf,
    snapshots_dir: PathBuf,
    settings: Mutex<settings::Settings>,
    tracker: Arc<Mutex<tracker::TrackerState>>,
    recorder: Mutex<recording::Recorder>,
    pet_position_revision: AtomicU64,
    quick_menu_anchor: Mutex<Option<(f64, f64)>>,
    /// 标注窗口待编辑 PNG（RGBA 编码前的原始 PNG 字节）
    annotate_png: Mutex<Option<Vec<u8>>>,
    annotate_title: Mutex<String>,
}

#[tauri::command]
fn show_quick_menu(
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
fn set_quick_menu_expanded(
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
fn hide_quick_menu(app: AppHandle) {
    if let Some(window) = app.get_webview_window("quick-menu") {
        let _ = window.hide();
    }
}

/// 第二个实例启动时把已有窗口唤到前台。
/// 这里刻意不调 center()：窗口多半已在用户摆好的位置上，没必要弹回屏幕中央。
fn focus_existing_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[tauri::command]
fn show_main_window(app: AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.center();
        let _ = window.set_focus();
    }
}

/// 快捷键版 OCR：识别剪贴板里的图片，再把文字写回剪贴板。
/// 界面上的 ocr_clipboard 只负责返回文字（页面自己展示），
/// 走快捷键时用户看不到界面，必须把结果送回剪贴板才有意义。
#[tauri::command]
async fn perform_action(
    action: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    match action.as_str() {
        "snapshot" => capture::capture_window(app, state, None),
        "record" => recording::toggle_recording(state, None).map(|status| {
            if status.active {
                status.message.unwrap_or_else(|| "录制已开始".into())
            } else {
                "录制已保存".into()
            }
        }),
        "recordings" => {
            recording::open_recordings_dir(state).map(|path| format!("已打开录制目录：{path}"))
        }
        "polish" => polish::polish_clipboard(state).await,
        "ocr" => capture::ocr_clipboard_into_clipboard().await,
        "fullscreen" => {
            let app2 = app.clone();
            let include_cursor = state.settings.lock().include_cursor;
            let (image, name, cursor_degraded) = tauri::async_runtime::spawn_blocking(move || {
                capture::capture_fullscreen_image(include_cursor)
            })
            .await
            .map_err(|error| error.to_string())??;
            capture::finalize_capture(&app2, &state, image, &name, cursor_degraded)
        }
        #[cfg(target_os = "linux")]
        "scrolling" => {
            let app2 = app.clone();
            let target_id = state
                .tracker
                .lock()
                .previous
                .as_ref()
                .map(|window| window.id)
                .ok_or_else(|| "还没有上一个应用可截取".to_string())?;
            let (image, name) = tauri::async_runtime::spawn_blocking(move || {
                capture::capture_scrolling_image(target_id)
            })
            .await
            .map_err(|error| error.to_string())??;
            finalize_capture(&app2, &state, image, &name, None)
        }
        _ => Err("未知快捷键动作".into()),
    }
}

pub(crate) fn truncate(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        return value.into();
    }
    value.chars().take(max).collect::<String>() + "…"
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn run() {
    // 配置文件里定义的窗口在 setup() 之前就已创建并开始加载前端，
    // 若把 manage() 留在 setup() 里，前端可能抢先发出命令并撞上
    // "state not managed"。所以状态提到 Builder 阶段准备好。
    //
    // 路径这里自己算：Tauri 的 app_config_dir()/app_data_dir() 实现就是
    // dirs::config_dir()/dirs::data_dir() 再拼 bundle identifier，
    // 标识符从 context 取，和 tauri.conf.json 保持同源，不会写死漂移。
    let context = tauri::generate_context!();
    let identifier = context.config().identifier.clone();
    let settings_path = dirs::config_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(&identifier)
        .join("settings.json");
    let snapshots_dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(&identifier)
        .join("snapshots");
    let settings = settings::read_settings(&settings_path);

    tauri::Builder::default()
        // 必须第一个注册：WebView2 的用户数据目录是独占锁，
        // 第二个实例抢不到就会静默退出（用户看到的是"双击没反应"）。
        // 交给这个插件拦下来，改成把已有窗口唤到前台。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_existing_window(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            settings_path,
            snapshots_dir,
            settings: Mutex::new(settings),
            tracker: Arc::new(Mutex::new(tracker::TrackerState::default())),
            recorder: Mutex::new(recording::Recorder::default()),
            pet_position_revision: AtomicU64::new(0),
            quick_menu_anchor: Mutex::new(None),
            annotate_png: Mutex::new(None),
            annotate_title: Mutex::new(String::new()),
        })
        .setup(|app| {
            let state = app.state::<AppState>();
            let settings = state.settings.lock().clone();
            let tracker = state.tracker.clone();
            if let Some(window) = app.get_webview_window("pet") {
                let window_size = window.outer_size().ok();
                let monitors = window.available_monitors().unwrap_or_default();
                let saved = settings.pet_position.clone().filter(|saved| {
                    monitors.iter().any(|monitor| {
                        let origin = monitor.position();
                        let size = monitor.size();
                        let width = window_size
                            .as_ref()
                            .map(|value| value.width as i32)
                            .unwrap_or(60);
                        let height = window_size
                            .as_ref()
                            .map(|value| value.height as i32)
                            .unwrap_or(60);
                        saved.x >= origin.x
                            && saved.y >= origin.y
                            && saved.x + width <= origin.x + size.width as i32
                            && saved.y + height <= origin.y + size.height as i32
                    })
                });
                if let Some(position) = saved {
                    let _ = window.set_position(PhysicalPosition::new(position.x, position.y));
                } else if let Ok(Some(monitor)) = window.primary_monitor() {
                    let screen = monitor.size();
                    let origin = monitor.position();
                    let width = window_size
                        .as_ref()
                        .map(|value| value.width as i32)
                        .unwrap_or(60);
                    let height = window_size
                        .as_ref()
                        .map(|value| value.height as i32)
                        .unwrap_or(60);
                    let x = origin.x + screen.width as i32 - width - 32;
                    let y = origin.y + (screen.height as i32 - height) / 2;
                    let _ = window.set_position(PhysicalPosition::new(x, y));
                }
            }
            tracker::start_tracker(app.handle().clone(), tracker);

            // 配置里若已勾选自启，启动时把系统启动项与设置对齐（不会默认打开）。
            // 应用被移动过路径时，这一步顺带把启动项里的旧路径刷新掉。
            #[cfg(target_os = "linux")]
            {
                if settings.launch_on_boot {
                    if let Err(error) = linux::apply_launch_on_boot(true) {
                        eprintln!("snapshot: could not sync XDG autostart: {error}");
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                if settings.launch_on_boot {
                    if let Err(error) = platform::windows_autostart::apply(true) {
                        eprintln!("snapshot: could not sync Run key: {error}");
                    }
                }
            }

            #[cfg(target_os = "macos")]
            {
                // 同上：把 LaunchAgent 与设置对齐。应用被移动过位置时，这里会把
                // plist 里的路径刷成当前的。
                if settings.launch_on_boot {
                    if let Err(error) = platform::mac_autostart::apply_launch_on_boot(true) {
                        eprintln!("snapshot: could not sync LaunchAgent: {error}");
                    }
                }
            }

            let open_settings =
                MenuItem::with_id(app, "open-settings", "打开设置", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open_settings, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open-settings" => show_main_window(app.clone()),
                    "quit" => {
                        if let Some(state) = app.try_state::<AppState>() {
                            recording::stop_active_recording(&mut state.recorder.lock());
                        }
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // Left-click (and Windows double-click) opens the main window where the
                    // tray backend emits click events. Linux tray-icon 0.24 via
                    // libayatana-appindicator has no Activate/click callback — menu only.
                    // Right-click / context menu (打开设置 / 退出) is unchanged.
                    match event {
                        TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        }
                        | TrayIconEvent::DoubleClick {
                            button: MouseButton::Left,
                            ..
                        } => {
                            show_main_window(tray.app_handle().clone());
                        }
                        _ => {}
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            // Linux 上若会话没有 StatusNotifierHost（精简环境 / 无扩展的 GNOME），
            // 托盘会建失败；主窗口与快捷键仍应可用，不能把整个 setup 拖死。
            if let Err(error) = tray.build(app) {
                eprintln!("snapshot: system tray unavailable ({error}); continuing without tray");
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            } else if window.label() == "pet" {
                if let WindowEvent::Moved(position) = event {
                    let app = window.app_handle().clone();
                    let revision = {
                        let Some(state) = app.try_state::<AppState>() else {
                            return;
                        };
                        state.settings.lock().pet_position = Some(settings::PetPosition {
                            x: position.x,
                            y: position.y,
                        });
                        state.pet_position_revision.fetch_add(1, Ordering::Relaxed) + 1
                    };
                    thread::spawn(move || {
                        thread::sleep(Duration::from_millis(350));
                        if let Some(state) = app.try_state::<AppState>() {
                            if state.pet_position_revision.load(Ordering::Relaxed) == revision {
                                let settings = state.settings.lock();
                                let _ = settings::persist_settings(&state.settings_path, &settings);
                            }
                        }
                    });
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            settings::load_settings,
            settings::save_prompt_settings,
            settings::fetch_models,
            pet::select_pet_appearance,
            pet::add_pet_asset,
            pet::delete_pet_asset,
            pet::get_pet_asset_data_url,
            settings::save_shortcuts,
            settings::save_preferences,
            tracker::get_previous_app,
            tracker::list_capturable_windows,
            capture::capture_window,
            recording::get_recording_status,
            recording::toggle_recording,
            polish::polish_clipboard,
            polish::polish_text,
            snapshots::list_snapshots,
            snapshots::open_snapshots_dir,
            recording::open_recordings_dir,
            snapshots::get_snapshot_data_url,
            snapshots::delete_snapshot,
            snapshots::clear_snapshots,
            capture::ocr_capability,
            capture::ocr_snapshot,
            capture::ocr_clipboard,
            platform::platform_capabilities,
            show_quick_menu,
            set_quick_menu_expanded,
            hide_quick_menu,
            show_main_window,
            perform_action,
            capture::get_annotate_image,
            capture::annotate_get_title,
            capture::annotate_copy,
            capture::annotate_save,
            capture::annotate_close,
        ])
        .run(context)
        .expect("运行 snapshot 失败");
}

#[cfg(all(test, target_os = "macos"))]
mod mac_icon_tests {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

    #[test]
    fn finder_icon_encodes_as_64px_png() {
        let Ok(output) = std::process::Command::new("pgrep")
            .args(["-x", "Finder"])
            .output()
        else {
            return;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        let Some(pid) = text
            .lines()
            .next()
            .and_then(|line| line.trim().parse::<u32>().ok())
        else {
            return;
        };
        let url =
            crate::platform::mac_icon::png_data_url_for_pid(pid).expect("Finder 应当能取到图标");
        assert!(
            url.starts_with("data:image/png;base64,"),
            "应返回 PNG data URL"
        );
        let png = BASE64
            .decode(url.trim_start_matches("data:image/png;base64,"))
            .expect("data URL 应为合法 base64");
        // 图标 TIFF 里有 1024×1024 原图；不真压尺寸的话这里会得到六百 KB 的大图
        let decoded = image::load_from_memory(&png).expect("导出的应为合法 PNG");
        assert_eq!(
            (decoded.width(), decoded.height()),
            (64, 64),
            "图标应压到 64×64"
        );
        assert!(
            png.len() < 20 * 1024,
            "64×64 图标 PNG 应远小于 20KB，实际 {} 字节",
            png.len()
        );
        // drawInRect 必须真的把图标画进去，而不是导出一张空白图
        let opaque = decoded
            .to_rgba8()
            .pixels()
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(
            opaque > 64,
            "64×64 图标应有可见内容，实际只有 {opaque} 个非透明像素"
        );
    }
}

#[cfg(test)]
mod pet_asset_tests {
    use super::*;

    /// 老配置只有三条绑定，读取后应当补出 ocr，且已有的绑定不能被动到
    #[test]
    fn old_settings_gain_newly_added_shortcut_actions() {
        let path = std::env::temp_dir().join("snapshot-settings-migration.json");
        let legacy = r#"{
            "baseUrl": "https://api.example.com/v1",
            "model": "demo",
            "templates": [{"id":"builtin-default","name":"内置","content":"x","builtin":true}],
            "activeTemplateId": "builtin-default",
            "selectedAppearanceId": "app-icon",
            "petAssets": [],
            "saveDir": "~/LegacyCaptures",
            "shortcuts": [
                {"action":"snapshot","accelerator":"Alt+Shift+2"},
                {"action":"record","accelerator":null},
                {"action":"polish","accelerator":null}
            ]
        }"#;
        fs::write(&path, legacy).expect("写测试配置失败");

        let settings = settings::read_settings(&path);
        let actions: Vec<&str> = settings
            .shortcuts
            .iter()
            .map(|item| item.action.as_str())
            .collect();
        // 平台不支持的动作（如 Win/mac 的 scrolling）不应被迁移补回
        #[cfg(target_os = "linux")]
        assert_eq!(
            actions,
            vec![
                "snapshot",
                "record",
                "polish",
                "fullscreen",
                "scrolling",
                "recordings",
                "ocr"
            ]
        );
        #[cfg(not(target_os = "linux"))]
        assert_eq!(
            actions,
            vec![
                "snapshot",
                "record",
                "polish",
                "fullscreen",
                "recordings",
                "ocr"
            ]
        );

        // 已绑定的键不能在迁移中丢失
        let snapshot = settings
            .shortcuts
            .iter()
            .find(|item| item.action == "snapshot")
            .unwrap();
        assert_eq!(snapshot.accelerator.as_deref(), Some("Alt+Shift+2"));
        // 新补的那条应当是未绑定状态
        let ocr = settings
            .shortcuts
            .iter()
            .find(|item| item.action == "ocr")
            .unwrap();
        assert!(ocr.accelerator.is_none());
        // 老版录制与快照共用 saveDir，新版应将它迁移到独立录制目录。
        assert_eq!(settings.recording_dir, "~/LegacyCaptures");

        let _ = fs::remove_file(&path);
    }
}
