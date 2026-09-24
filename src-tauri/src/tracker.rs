use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreviousApp {
    id: Option<u32>,
    /// 图标由页面按需经 media://icon/<pid>/<边长> 取，这里只给 pid
    pid: Option<u32>,
    name: String,
    title: String,
}

impl Default for PreviousApp {
    fn default() -> Self {
        Self {
            id: None,
            pid: None,
            name: "等待切换应用".into(),
            title: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapturableWindow {
    id: u32,
    pid: u32,
    app_name: String,
    title: String,
}

#[derive(Clone, Debug)]
pub(crate) struct TrackedWindow {
    pub(crate) id: u32,
    pub(crate) pid: u32,
    pub(crate) app_name: String,
    pub(crate) title: String,
}

#[derive(Default)]
pub(crate) struct TrackerState {
    current: Option<TrackedWindow>,
    pub(crate) previous: Option<TrackedWindow>,
    previous_view: PreviousApp,
}

#[tauri::command]
pub(crate) fn get_previous_app(state: State<'_, AppState>) -> PreviousApp {
    state.tracker.lock().previous_view.clone()
}

/// 菜单栏图标、状态项和其它辅助窗口也会出现在系统窗口列表里。
/// 它们通常只有几十像素，选中后录制出来的就只有应用图标。
const MIN_CONTENT_WINDOW_EDGE: u32 = 80;

fn content_window_area_from_size(width: u32, height: u32) -> Option<u64> {
    if width < MIN_CONTENT_WINDOW_EDGE || height < MIN_CONTENT_WINDOW_EDGE {
        return None;
    }
    Some(u64::from(width) * u64::from(height))
}

fn content_window_area(window: &Window) -> Option<u64> {
    let width = window.width().ok()?;
    let height = window.height().ok()?;
    content_window_area_from_size(width, height)
}

#[tauri::command]
pub(crate) fn list_capturable_windows() -> Result<Vec<CapturableWindow>, String> {
    let mut result = Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .filter(|window| !is_own_window(window))
        .filter_map(|window| {
            let area = content_window_area(&window)?;
            let title = window.title().ok()?;
            if title.trim().is_empty() {
                return None;
            }
            Some((
                area,
                CapturableWindow {
                    id: window.id().ok()?,
                    pid: window.pid().ok()?,
                    app_name: window.app_name().unwrap_or_else(|_| "应用".into()),
                    title,
                },
            ))
        })
        .collect::<Vec<_>>();
    result.sort_by(|(left_area, left), (right_area, right)| {
        left.app_name
            .cmp(&right.app_name)
            // 同一应用有多个窗口时，先放最可能是主内容的大窗口。
            .then(right_area.cmp(left_area))
            .then(left.title.cmp(&right.title))
    });
    result.truncate(80);
    Ok(result.into_iter().map(|(_, window)| window).collect())
}

/// 判断某个窗口是不是本应用自己的窗口（桌宠、快捷菜单、主窗口）。
/// 追踪"上一个应用"和录制目标时都必须跳过它们，否则点击桌宠托盘
/// 后桌宠自己会被记成"上一个应用"，图标随后变成终端一类的业务应用。
fn is_own_window(window: &Window) -> bool {
    if let Ok(pid) = window.pid() {
        if pid == std::process::id() {
            return true;
        }
    }
    // WebView2 用独立子进程渲染，pid 不等于主进程，只能靠标题兜底。
    // 三个窗口标题都在 tauri.conf.json 里：snapshot / 桌宠 / 快速操作。
    let title = window.title().unwrap_or_default();
    matches!(title.trim(), "snapshot" | "桌宠" | "快速操作")
}

pub(crate) fn start_tracker(app: AppHandle, tracker: Arc<Mutex<TrackerState>>) {
    thread::spawn(move || loop {
        if let Ok(windows) = Window::all() {
            // 焦点可能落在桌宠/快捷菜单这类自有窗口上，先向前找最近一个
            // 真正的业务窗口作为焦点候选，避免把"上一个应用"记成自己。
            // xcap 的 macOS is_focused 实际按 PID 判断：同一 App 的菜单栏图标
            // 和主窗口都会返回 true。因此先排除小辅助窗，再取面积最大的内容窗。
            let focused = windows
                .into_iter()
                .filter(|window| window.is_focused().unwrap_or(false) && !is_own_window(window))
                .filter_map(|window| content_window_area(&window).map(|area| (area, window)))
                .max_by_key(|(area, _)| *area)
                .map(|(_, window)| window);
            if let Some(focused) = focused {
                if let (Ok(id), Ok(pid)) = (focused.id(), focused.pid()) {
                    let next = TrackedWindow {
                        id,
                        pid,
                        app_name: focused.app_name().unwrap_or_else(|_| "应用".into()),
                        title: focused.title().unwrap_or_default(),
                    };
                    let mut state = tracker.lock();
                    let changed = state
                        .current
                        .as_ref()
                        .map(|current| current.pid != next.pid)
                        .unwrap_or(true);
                    if state.current.is_none() {
                        state.current = Some(next.clone());
                        state.previous = Some(next.clone());
                    } else if changed {
                        state.previous = state.current.replace(next.clone());
                    } else {
                        state.current = Some(next.clone());
                        if state.previous.as_ref().map(|previous| previous.pid) == Some(next.pid) {
                            state.previous = Some(next.clone());
                        }
                    }
                    if changed || state.previous_view.id.is_none() {
                        if let Some(previous) = state.previous.as_ref() {
                            state.previous_view = PreviousApp {
                                id: Some(previous.id),
                                pid: Some(previous.pid),
                                name: previous.app_name.clone(),
                                title: previous.title.clone(),
                            };
                            let _ = app.emit("previous-app-changed", &state.previous_view);
                        }
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(500));
    });
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn rgba_to_png(image: RgbaImage) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .ok()?;
    Some(bytes)
}

/// 进程图标，按页面实际显示的像素边长取（媒体协议用）。
#[cfg(target_os = "windows")]
pub(crate) fn app_icon_png(pid: u32, size: u32) -> Option<Vec<u8>> {
    platform::windows_icon::icon_for_process(pid, size).and_then(rgba_to_png)
}

#[cfg(target_os = "macos")]
pub(crate) fn app_icon_png(pid: u32, size: u32) -> Option<Vec<u8>> {
    platform::mac_icon::png_for_pid(pid, size)
}

#[cfg(target_os = "linux")]
pub(crate) fn app_icon_png(pid: u32, size: u32) -> Option<Vec<u8>> {
    linux::icon_for_process(pid, size).and_then(rgba_to_png)
}

#[cfg(all(
    not(target_os = "windows"),
    not(target_os = "macos"),
    not(target_os = "linux")
))]
pub(crate) fn app_icon_png(_pid: u32, _size: u32) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_items_are_not_treated_as_content_windows() {
        // Shadowrocket 在本机暴露的 Item-0 是 34×24，真正主窗口是 1024×655。
        assert_eq!(content_window_area_from_size(34, 24), None);
        assert_eq!(content_window_area_from_size(79, 600), None);
        assert_eq!(content_window_area_from_size(1024, 655), Some(670_720));
    }
}
