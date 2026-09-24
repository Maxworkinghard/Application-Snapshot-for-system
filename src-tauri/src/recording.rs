//! 窗口录制的共享部分：选目标、定落盘位置、记录状态。
//! 怎么录由各平台自己决定（WGC / ScreenCaptureKit / ffmpeg+portal），见 `os::start_recording`。

use super::*;

/// 录制选项，来自设置里的偏好
pub(crate) struct RecordOptions {
    pub(crate) include_cursor: bool,
    pub(crate) system_audio: bool,
    pub(crate) microphone: bool,
}

struct Active {
    session: os::Recording,
    target: String,
    started_at: u64,
    output: PathBuf,
}

/// 录完的一段，用来写活动记录
pub(crate) struct Finished {
    pub(crate) target: String,
    pub(crate) duration_ms: u64,
    pub(crate) output: PathBuf,
}

#[derive(Default)]
pub(crate) struct Recorder {
    active: Option<Active>,
    /// 上一次录制异常结束的原因，在下一次状态查询里带给界面
    last_message: Option<String>,
}

impl Recorder {
    fn status(&self, message: Option<String>) -> RecordingStatus {
        RecordingStatus {
            active: self.active.is_some(),
            target: self.active.as_ref().map(|active| active.target.clone()),
            started_at: self.active.as_ref().map(|active| active.started_at),
            message: message.or_else(|| self.last_message.clone()),
        }
    }

    /// 录制可能自己结束了（macOS 的录制组件崩溃等）：发现了就收尾并记下原因。
    fn refresh(&mut self) {
        if let Some(reason) = self
            .active
            .as_mut()
            .and_then(|active| active.session.exit_reason())
        {
            self.active = None;
            self.last_message = Some(reason);
        }
    }

    pub(crate) fn stop(&mut self) -> Option<Finished> {
        let active = self.active.take()?;
        let finished = Finished {
            target: active.target,
            duration_ms: now_millis().saturating_sub(active.started_at),
            output: active.output,
        };
        match active.session.stop() {
            Ok(()) => Some(finished),
            Err(message) => {
                self.last_message = Some(message);
                None
            }
        }
    }
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
    recorder.refresh();
    recorder.status(None)
}

/// 按窗口 id 取一份录制目标。窗口在点选之后、开录之前被关掉是常事，
/// 所以这里要报「已关闭」而不是沉默地退回「上一个应用」。
fn list_tracked_window(id: u32) -> Result<tracker::TrackedWindow, String> {
    let window = Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|window| window.id().ok() == Some(id))
        .ok_or_else(|| "目标窗口已关闭".to_string())?;
    Ok(tracker::TrackedWindow {
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
    os::open_folder(&path)?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub(crate) fn toggle_recording(
    app: AppHandle,
    state: State<'_, AppState>,
    target_id: Option<u32>,
) -> Result<RecordingStatus, String> {
    let mut recorder = state.recorder.lock();
    recorder.refresh();
    if recorder.active.is_some() {
        let finished = recorder.stop();
        let status = recorder.status(None);
        drop(recorder);
        match finished {
            Some(finished) => activity::record(
                &app,
                activity::Activity::new("record", finished.target)
                    .meta(format_duration(finished.duration_ms))
                    .detail(finished.output.to_string_lossy().to_string()),
            ),
            None => activity::record_error(
                &app,
                "录制没能保存",
                status.message.as_deref().unwrap_or("录制组件没有正常结束"),
            ),
        }
        let _ = app.emit("recording-changed", &status);
        return Ok(status);
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

    let options = RecordOptions {
        include_cursor: settings.include_cursor,
        system_audio: settings.record_system_audio,
        microphone: settings.record_microphone,
    };
    let (session, message) = os::start_recording(target.id, &options, &output)?;
    recorder.active = Some(Active {
        session,
        target: target.app_name,
        started_at: now_millis(),
        output,
    });
    let status = recorder.status(message);
    let _ = app.emit("recording-changed", &status);
    Ok(status)
}

/// 03:07、1:02:03
pub(crate) fn format_duration(ms: u64) -> String {
    let seconds = ms / 1000;
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_duration;

    #[test]
    fn durations_read_like_a_clock() {
        assert_eq!(format_duration(0), "00:00");
        assert_eq!(format_duration(133_400), "02:13");
        assert_eq!(format_duration(3_723_000), "1:02:03");
    }
}
