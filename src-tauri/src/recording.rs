use super::*;

#[derive(Default)]
pub(crate) struct Recorder {
    child: Option<Child>,
    target: Option<String>,
    started_at: Option<u64>,
    output_path: Option<PathBuf>,
    /// Windows/macOS 子进程的 stderr。后台线程持续读取，避免管道写满，并在异常退出时
    /// 把真实原因回传给 UI。
    diagnostic: Option<Arc<Mutex<String>>>,
    last_message: Option<String>,
    /// Linux：portal 路径的停止句柄（x11grab 时为 None）。
    #[cfg(target_os = "linux")]
    linux_active: Option<linux::ActiveRecording>,
    /// Windows：WGC 采集会话的停止句柄，录制不再经由子进程。
    #[cfg(target_os = "windows")]
    windows_active: Option<windows_recorder::ActiveRecording>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RecordingStatus {
    pub(crate) active: bool,
    pub(crate) target: Option<String>,
    pub(crate) started_at: Option<u64>,
    /// 启动时的 UI 提示（如 Linux portal 需重新选窗）；停止时为 None。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) message: Option<String>,
}

#[tauri::command]
pub(crate) fn get_recording_status(state: State<'_, AppState>) -> RecordingStatus {
    let mut recorder = state.recorder.lock();
    refresh_recording_process(&mut recorder);
    recording_status(&recorder)
}

fn recording_status(recorder: &Recorder) -> RecordingStatus {
    recording_status_with_message(recorder, None)
}

fn recording_status_with_message(recorder: &Recorder, message: Option<String>) -> RecordingStatus {
    let active = recorder.child.is_some() || {
        #[cfg(target_os = "linux")]
        {
            recorder.linux_active.is_some()
        }
        #[cfg(target_os = "windows")]
        {
            recorder.windows_active.is_some()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    };
    RecordingStatus {
        active,
        target: recorder.target.clone(),
        started_at: recorder.started_at,
        message: message.or_else(|| recorder.last_message.clone()),
    }
}

fn recorder_diagnostic(recorder: &Recorder) -> String {
    recorder
        .diagnostic
        .as_ref()
        .map(|value| value.lock().trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn refresh_recording_process(recorder: &mut Recorder) {
    let Some(child) = recorder.child.as_mut() else {
        return;
    };
    let Ok(Some(status)) = child.try_wait() else {
        return;
    };
    let detail = recorder_diagnostic(recorder);
    recorder.last_message = Some(if detail.is_empty() {
        format!("录制进程意外退出（{status}）")
    } else {
        format!("录制进程意外退出：{detail}")
    });
    recorder.child = None;
    recorder.target = None;
    recorder.started_at = None;
    recorder.output_path = None;
    recorder.diagnostic = None;
}

#[cfg(not(target_os = "linux"))]
#[cfg(target_os = "macos")]
fn capture_child_stderr(stderr: Option<std::process::ChildStderr>) -> Arc<Mutex<String>> {
    let diagnostic = Arc::new(Mutex::new(String::new()));
    if let Some(mut stderr) = stderr {
        let destination = diagnostic.clone();
        thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr.read_to_string(&mut text);
            if text.len() > 8_000 {
                text = text.split_off(text.len() - 8_000);
            }
            *destination.lock() = text;
        });
    }
    diagnostic
}

#[cfg(target_os = "macos")]
fn spawn_macos_recorder(
    target: &TrackedWindow,
    output: &PathBuf,
    include_cursor: bool,
) -> Result<(Child, Arc<Mutex<String>>), String> {
    let mut command = Command::new(macos_recorder_program());
    command
        .arg("--window-id")
        .arg(target.id.to_string())
        .arg("--output")
        .arg(output);
    if include_cursor {
        command.arg("--include-cursor");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 ScreenCaptureKit 录制组件：{error}"))?;
    let diagnostic = capture_child_stderr(child.stderr.take());
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "录制组件 stdout 不可用".to_string())?;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut first = String::new();
        let result = reader
            .read_line(&mut first)
            .map(|_| first.trim().to_string())
            .map_err(|error| error.to_string());
        let _ = sender.send(result);

        // 保持 stdout 管道打开并排空 FINISHED，避免 sidecar 收尾时遭遇 SIGPIPE。
        let mut rest = String::new();
        while reader.read_line(&mut rest).unwrap_or(0) > 0 {
            rest.clear();
        }
    });

    let line = match receiver.recv_timeout(Duration::from_secs(20)) {
        Ok(Ok(line)) => line,
        Ok(Err(error)) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("读取录制组件启动状态失败：{error}"));
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err("等待 ScreenCaptureKit 首帧超时".into());
        }
    };

    if line.starts_with("READY ") {
        return Ok((child, diagnostic));
    }

    let _ = child.kill();
    let _ = child.wait();
    if let Some(message) = line.strip_prefix("ERROR ") {
        Err(message.to_string())
    } else if line.is_empty() {
        let detail = diagnostic.lock().trim().to_string();
        Err(if detail.is_empty() {
            "ScreenCaptureKit 录制组件启动后没有返回状态".into()
        } else {
            detail
        })
    } else {
        Err(format!("录制组件返回了未知状态：{line}"))
    }
}

/// 按窗口 id 取一份录制目标。窗口在点选之后、开录之前被关掉是常事，
/// 所以这里要报「已关闭」而不是沉默地退回「上一个应用」。
fn list_tracked_window(id: u32) -> Result<TrackedWindow, String> {
    let window = Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|window| window.id().ok() == Some(id))
        .ok_or_else(|| "目标窗口已关闭".to_string())?;
    Ok(TrackedWindow {
        id,
        pid: window.pid().unwrap_or_default(),
        app_name: window.app_name().unwrap_or_else(|_| "应用".into()),
        title: window.title().unwrap_or_default(),
    })
}

/// 录制产物目录：优先 recording_dir，其次沿用截图的 save_dir，都没填就落到「下载」。
/// 保留 save_dir 这一层回退，是为了不改变升级前只设过 save_dir 的用户的去向。
fn resolve_recording_dir(settings: &settings::Settings) -> Result<PathBuf, String> {
    for candidate in [settings.recording_dir.trim(), settings.save_dir.trim()] {
        if !candidate.is_empty() {
            return Ok(PathBuf::from(snapshots::expand_user_path(candidate)));
        }
    }
    dirs::download_dir()
        .or_else(dirs::document_dir)
        .ok_or_else(|| "无法确定录制保存目录".to_string())
}

/// 在系统文件管理器里打开录制目录。目录可能还没建过，先建出来再打开。
#[tauri::command]
pub(crate) fn open_recordings_dir(state: State<'_, AppState>) -> Result<String, String> {
    let path = resolve_recording_dir(&state.settings.lock())?;
    fs::create_dir_all(&path).map_err(|error| format!("无法创建录制目录：{error}"))?;
    snapshots::open_in_file_manager(&path)?;
    Ok(path.to_string_lossy().to_string())
}

// 录制启动的 cfg 分支各自返回；保留显式 return，避免平台分支间形成隐式尾表达式。
#[allow(clippy::needless_return)]
#[tauri::command]
pub(crate) fn toggle_recording(
    state: State<'_, AppState>,
    target_id: Option<u32>,
) -> Result<RecordingStatus, String> {
    let mut recorder = state.recorder.lock();
    refresh_recording_process(&mut recorder);
    if recorder.child.is_some() || {
        #[cfg(target_os = "linux")]
        {
            recorder.linux_active.is_some()
        }
        #[cfg(target_os = "windows")]
        {
            recorder.windows_active.is_some()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            false
        }
    } {
        stop_active_recording(&mut recorder);
        return Ok(recording_status(&recorder));
    }

    // 指定了窗口就录它；没指定才回到「上一个应用」，与窗口快照的口径一致
    let target = match target_id {
        Some(id) => list_tracked_window(id)?,
        None => state
            .tracker
            .lock()
            .previous
            .clone()
            .ok_or_else(|| "还没有上一个应用可录制".to_string())?,
    };
    let settings = state.settings.lock().clone();
    let output_dir = resolve_recording_dir(&settings)?;
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let output = output_dir.join(format!(
        "应用快照-{}.mp4",
        Local::now().format("%Y-%m-%d_%H-%M-%S")
    ));
    recorder.last_message = None;

    #[cfg(target_os = "linux")]
    {
        let (active, start_message) =
            linux::start_recording(target.id, settings.include_cursor, &output)?;
        // child 也放一份，供 status.active 判断；停止走 linux_active
        // ActiveRecording 拥有 child，这里不双持——只用 linux_active
        recorder.linux_active = Some(active);
        // 同步一个占位，让 recording_status 的 active 仍看 child；改为看 linux_active
        recorder.target = Some(target.app_name);
        recorder.started_at = Some(now_millis());
        recorder.output_path = Some(output);
        return Ok(recording_status_with_message(&recorder, start_message));
    }

    #[cfg(target_os = "windows")]
    {
        // xcap 在 Windows 上的窗口 id 就是 HWND，直接交给 WGC 按句柄采集，
        // 不必像 ffmpeg 那样按标题找窗口
        // 最小化的窗口 DWM 不再合成，WGC 一帧也拿不到。与截图一致：先还原再录。
        // 不能无条件 SW_RESTORE——那会把最大化的窗口一并还原掉。
        if windows_recorder::is_minimized(target.id as isize) {
            restore_minimized_window(target.id)
                .map_err(|error| format!("目标窗口已最小化，且无法还原：{error}"))?;
        }
        let active = windows_recorder::start(target.id as isize, settings.include_cursor, &output)?;
        recorder.windows_active = Some(active);
        recorder.target = Some(target.app_name);
        recorder.started_at = Some(now_millis());
        recorder.output_path = Some(output);
        return Ok(recording_status(&recorder));
    }

    #[cfg(target_os = "macos")]
    {
        let (child, diagnostic) = spawn_macos_recorder(&target, &output, settings.include_cursor)?;
        recorder.child = Some(child);
        recorder.diagnostic = Some(diagnostic);
        recorder.target = Some(target.app_name);
        recorder.started_at = Some(now_millis());
        recorder.output_path = Some(output);
        return Ok(recording_status(&recorder));
    }
}

pub(crate) fn stop_active_recording(recorder: &mut Recorder) {
    #[cfg(target_os = "windows")]
    {
        if let Some(active) = recorder.windows_active.take() {
            // stop 内部会 Finalize，MP4 的 moov 在这一步才写进去
            let (path, frames) = active.stop();
            // 一帧都没有时产物是个播放器打不开的空壳，留着只会让人以为录成功了
            if frames == 0 {
                let _ = fs::remove_file(&path);
                eprintln!(
                    "snapshot: recording produced no frames, removed {}",
                    path.display()
                );
            }
            recorder.target = None;
            recorder.started_at = None;
            recorder.output_path = None;
            return;
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(active) = recorder.linux_active.take() {
            active.stop();
            recorder.child = None;
            recorder.target = None;
            recorder.started_at = None;
            recorder.output_path = None;
            recorder.diagnostic = None;
            return;
        }
    }
    if let Some(mut child) = recorder.child.take() {
        if let Some(stdin) = child.stdin.as_mut() {
            let _ = stdin.write_all(b"q\n");
        }
        if let Ok(status) = child.wait() {
            if !status.success() {
                thread::sleep(Duration::from_millis(20));
                let detail = recorder_diagnostic(recorder);
                recorder.last_message = Some(if detail.is_empty() {
                    format!("录制停止异常（{status}）")
                } else {
                    format!("录制停止异常：{detail}")
                });
            }
        }
    }
    recorder.target = None;
    recorder.started_at = None;
    recorder.output_path = None;
    recorder.diagnostic = None;
}
