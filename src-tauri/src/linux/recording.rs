//! Linux 录制：Wayland 会话优先 xdg-desktop-portal ScreenCast（经 xcap）+ PipeWire 帧
//! → ffmpeg rawvideo；X11 会话用 ffmpeg x11grab。Wayland 下门户不可用时才退回 x11grab，
//! 那条退路只抓得到 XWayland 的画面，由能力文案说明。

use std::env;
use std::io::Write;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use xcap::{Monitor, Window};

/// 当前会话是 X11 还是 Wayland（给错误信息与设置页用）。
pub fn display_server_label() -> &'static str {
    // 与 is_wayland_session 对齐：也认 XDG_SESSION_TYPE=wayland
    if is_wayland_session() {
        if env::var_os("DISPLAY").is_none() {
            "Wayland"
        } else {
            "Wayland (XWayland available)"
        }
    } else if env::var_os("DISPLAY").is_some() {
        "X11"
    } else {
        "unknown"
    }
}

/// 当前会话是 Wayland——不管 XWayland 有没有把 `$DISPLAY` 撑起来。
pub fn is_wayland_session() -> bool {
    env::var_os("WAYLAND_DISPLAY").is_some()
        || env::var("XDG_SESSION_TYPE")
            .map(|v| v.eq_ignore_ascii_case("wayland"))
            .unwrap_or(false)
}

/// 录制后端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingBackend {
    /// portal ScreenCast + PipeWire → ffmpeg
    Portal,
    /// ffmpeg x11grab。`xwayland_only` 表示这是 Wayland 会话里 portal 不可用后的退路，
    /// 抓到的只是 X server 的画面，原生 Wayland 窗口会是黑的。
    X11Grab { xwayland_only: bool },
}

/// Wayland 会话一律**先试 portal**，而不是看 `$DISPLAY` 在不在。
///
/// GNOME / KDE 的 Wayland 会话基本都跑着 XWayland，`$DISPLAY` 是有值的。按 DISPLAY
/// 判断会让这些桌面全部落到 x11grab，而 x11grab 抓不到原生 Wayland 窗口——录出来是
/// 黑屏，且没有任何失败信号，比直接报错更糟。portal 真的不可用时才退回 x11grab，
/// 并由能力文案说明这条退路的局限。
pub fn pick_recording_backend() -> Result<RecordingBackend, String> {
    if is_wayland_session() {
        return match portal_screencast_available() {
            Ok(()) => Ok(RecordingBackend::Portal),
            Err(_) if env::var_os("DISPLAY").is_some() => Ok(RecordingBackend::X11Grab {
                xwayland_only: true,
            }),
            Err(detail) => Err(format!(
                "当前是 {} 会话且没有 X11 DISPLAY。门户录制不可用：{detail}",
                display_server_label()
            )),
        };
    }
    if env::var_os("DISPLAY").is_some() {
        return Ok(RecordingBackend::X11Grab {
            xwayland_only: false,
        });
    }
    Err(format!(
        "当前是 {} 会话且没有 X11 DISPLAY，窗口录制需要 X11/XWayland 或可用的 xdg-desktop-portal ScreenCast / recording needs X11 DISPLAY or ScreenCast portal",
        display_server_label()
    ))
}

fn ensure_ffmpeg() -> Result<(), String> {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return Err(
            "未找到 ffmpeg，请安装后将它加入 PATH（如 apt install ffmpeg） / ffmpeg not found on PATH"
                .into(),
        );
    }
    Ok(())
}

/// 探测 session bus 上 ScreenCast portal 是否存在（不弹授权框）。
pub fn portal_screencast_available() -> Result<(), String> {
    let conn = zbus::blocking::Connection::session().map_err(|error| {
        format!(
            "无法连接 session D-Bus（ScreenCast 需要）：{error} / session D-Bus unavailable: {error}"
        )
    })?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.DBus.Introspectable",
    )
    .map_err(|error| format!("xdg-desktop-portal 不可用：{error} / portal missing: {error}"))?;
    let xml: String = proxy
        .call("Introspect", &())
        .map_err(|error| format!("无法探测 portal：{error} / cannot introspect portal: {error}"))?;
    if !xml.contains("org.freedesktop.portal.ScreenCast") {
        return Err(
            "桌面未提供 org.freedesktop.portal.ScreenCast（请安装 xdg-desktop-portal 及对应后端，如 portal-gtk / portal-gnome / portal-kde） / ScreenCast portal interface missing"
                .into(),
        );
    }
    Ok(())
}

/// 录制是否可用：X11/`$DISPLAY` 或 portal ScreenCast。
pub fn recording_available() -> Result<(), String> {
    ensure_ffmpeg()?;
    pick_recording_backend().map(|_| ())
}

#[derive(Debug, Clone)]
struct AudioInput {
    source: String,
}

fn pulse_sources() -> Result<Vec<String>, String> {
    let output = Command::new("pactl")
        .args(["list", "short", "sources"])
        .output()
        .map_err(|_| "未找到 pactl；Linux 音频录制需要 PulseAudio 或 PipeWire-Pulse".to_string())?;
    if !output.status.success() {
        return Err("无法读取 PulseAudio/PipeWire 音频输入设备".into());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect())
}

fn pulse_ffmpeg_available() -> Result<(), String> {
    ensure_ffmpeg()?;
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-devices"])
        .output()
        .map_err(|_| "无法检查 ffmpeg 的 PulseAudio 支持".to_string())?;
    let listing = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if listing.contains("pulse") {
        Ok(())
    } else {
        Err("当前 ffmpeg 未编译 PulseAudio 输入支持".into())
    }
}

fn system_audio_source() -> Result<String, String> {
    pulse_ffmpeg_available()?;
    let sink = Command::new("pactl")
        .args(["get-default-sink"])
        .output()
        .map_err(|_| "未找到 pactl；无法定位系统播放设备".to_string())?;
    if !sink.status.success() {
        return Err("没有可用的默认系统播放设备".into());
    }
    let sink = String::from_utf8_lossy(&sink.stdout).trim().to_string();
    if sink.is_empty() {
        return Err("没有可用的默认系统播放设备".into());
    }
    let monitor = format!("{sink}.monitor");
    if pulse_sources()?.iter().any(|source| source == &monitor) {
        Ok(monitor)
    } else {
        Err("默认播放设备没有可录制的 monitor 输入".into())
    }
}

fn microphone_source() -> Result<String, String> {
    pulse_ffmpeg_available()?;
    let source = Command::new("pactl")
        .args(["get-default-source"])
        .output()
        .map_err(|_| "未找到 pactl；无法定位默认麦克风".to_string())?;
    if !source.status.success() {
        return Err("没有可用的默认麦克风输入".into());
    }
    let source = String::from_utf8_lossy(&source.stdout).trim().to_string();
    if source.is_empty() || source.ends_with(".monitor") {
        return Err("没有可用的默认麦克风输入".into());
    }
    if pulse_sources()?.iter().any(|item| item == &source) {
        Ok(source)
    } else {
        Err("默认麦克风没有可录制的 PulseAudio/PipeWire 输入".into())
    }
}

pub fn system_audio_capability() -> crate::CapabilityStatus {
    match system_audio_source() {
        Ok(_) => crate::CapabilityStatus {
            available: true,
            detail: "PulseAudio/PipeWire：录制默认播放设备的系统混音".into(),
        },
        Err(detail) => crate::CapabilityStatus {
            available: false,
            detail,
        },
    }
}

pub fn microphone_capability() -> crate::CapabilityStatus {
    match microphone_source() {
        Ok(_) => crate::CapabilityStatus {
            available: true,
            detail: "PulseAudio/PipeWire：录制默认麦克风输入".into(),
        },
        Err(detail) => crate::CapabilityStatus {
            available: false,
            detail,
        },
    }
}

fn selected_audio_inputs(
    record_system_audio: bool,
    record_microphone: bool,
) -> Result<Vec<AudioInput>, String> {
    let mut inputs = Vec::new();
    if record_system_audio {
        inputs.push(AudioInput {
            source: system_audio_source()?,
        });
    }
    if record_microphone {
        inputs.push(AudioInput {
            source: microphone_source()?,
        });
    }
    Ok(inputs)
}

fn append_audio_inputs(command: &mut Command, inputs: &[AudioInput]) {
    for input in inputs {
        command.args(["-thread_queue_size", "512", "-f", "pulse", "-i"]);
        command.arg(&input.source);
    }
}

fn append_mux_args(command: &mut Command, audio_inputs: &[AudioInput]) {
    command.args(["-map", "0:v:0"]);
    match audio_inputs.len() {
        0 => {}
        1 => {
            command.args(["-map", "1:a:0", "-c:a", "aac", "-b:a", "160k"]);
        }
        count => {
            let filter = audio_inputs
                .iter()
                .enumerate()
                .map(|(index, _)| format!("[{}:a:0]", index + 1))
                .collect::<String>()
                + &format!("amix=inputs={count}:duration=longest:dropout_transition=0[mixed]");
            command.args([
                "-filter_complex",
                &filter,
                "-map",
                "[mixed]",
                "-c:a",
                "aac",
                "-b:a",
                "160k",
            ]);
        }
    }
}

/// 能力探测文案（设置 / diagnostics）。
pub fn recording_capability_detail() -> String {
    if let Err(detail) = ensure_ffmpeg() {
        return detail;
    }
    match pick_recording_backend() {
        Ok(RecordingBackend::Portal) => format!(
            "portal ScreenCast + PipeWire → ffmpeg（忽略 target_id / include_cursor）· {}",
            display_server_label()
        ),
        Ok(RecordingBackend::X11Grab {
            xwayland_only: true,
        }) => format!(
            "ffmpeg x11grab（门户不可用，只能录到 XWayland 的画面，原生 Wayland 窗口会是黑的）· {}",
            display_server_label()
        ),
        Ok(RecordingBackend::X11Grab {
            xwayland_only: false,
        }) => {
            format!("ffmpeg x11grab · {}", display_server_label())
        }
        Err(detail) => detail,
    }
}

/// 组装 x11grab 输入参数：`-f x11grab … -i :N.N+x,y`
///
/// 旧实现写死 `:0.0`，远程桌面 / 多显示 / `DISPLAY=:3` 会录错屏。
pub fn build_ffmpeg_grab_args(target_id: u32, include_cursor: bool) -> Result<Vec<String>, String> {
    ensure_ffmpeg()?;
    if env::var_os("DISPLAY").is_none() {
        return Err("x11grab 需要 $DISPLAY".into());
    }

    let display = env::var("DISPLAY").unwrap_or_else(|_| ":0".into());
    // ffmpeg 的 x11grab 接受 `:3.0+x,y`；若 DISPLAY 已是 `:3.0` 就原样用。
    let display_spec = if display.contains('.') {
        display
    } else {
        format!("{display}.0")
    };

    let window = Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|window| window.id().ok() == Some(target_id))
        .ok_or_else(|| "目标窗口已关闭".to_string())?;

    let width = window.width().map_err(|e| e.to_string())?.max(2);
    let height = window.height().map_err(|e| e.to_string())?.max(2);
    // 奇数边长会导致 yuv420p 编码失败，向下取偶
    let width = width & !1;
    let height = height & !1;
    let x = window.x().map_err(|e| e.to_string())?;
    let y = window.y().map_err(|e| e.to_string())?;

    let mut args = vec![
        "-f".into(),
        "x11grab".into(),
        "-framerate".into(),
        "30".into(),
        "-video_size".into(),
        format!("{width}x{height}"),
        "-i".into(),
        format!("{display_spec}+{x},{y}"),
    ];
    // x11grab 默认带鼠标；关闭时显式关掉，与设置项对齐
    if !include_cursor {
        args.extend(["-draw_mouse".into(), "0".into()]);
    }
    Ok(args)
}

/// 停止策略：x11grab 写 `q`；portal rawvideo 靠关闭 stdin EOF。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopKind {
    FfmpegQuit,
    StdinEof,
}

/// portal 路径持有的句柄（停录时 stop + 打断写帧线程）。
pub struct PortalHandle {
    video_recorder: xcap::VideoRecorder,
    stop_flag: Arc<AtomicBool>,
}

/// 一次已启动的 Linux 录制（ffmpeg 子进程 + 可选 portal）。
pub struct ActiveRecording {
    pub child: Child,
    pub stop_kind: StopKind,
    pub portal: Option<PortalHandle>,
}

impl ActiveRecording {
    /// 优雅停止：portal 先停 PipeWire 泵，再 EOF/quit ffmpeg。
    pub fn stop(mut self) {
        if let Some(portal) = self.portal.take() {
            portal.stop_flag.store(true, Ordering::SeqCst);
            let _ = portal.video_recorder.stop();
            // 给写帧线程一点时间关掉 stdin，让 MP4 收尾
            thread::sleep(Duration::from_millis(200));
        }
        match self.stop_kind {
            StopKind::FfmpegQuit => {
                if let Some(stdin) = self.child.stdin.as_mut() {
                    let _ = stdin.write_all(b"q\n");
                }
            }
            StopKind::StdinEof => {
                // stdin 已交给写帧线程；再保险关一次（若仍在）
                drop(self.child.stdin.take());
            }
        }
        let _ = self.child.wait();
    }
}

/// 启动录制：后端由 [`pick_recording_backend`] 决定（Wayland 会话优先 portal）。
///
/// `target_id` / `include_cursor` 在 x11grab 路径生效。portal 路径由桌面选择器挑源；
/// `include_cursor` 目前随门户/合成器默认（xcap ScreenCast 未暴露 cursor_mode）。
///
/// 第二个返回值是启动时给 UI 的提示（portal 需用户重新选窗/屏）；x11grab 为 `None`。
pub fn start_recording(
    target_id: u32,
    include_cursor: bool,
    record_system_audio: bool,
    record_microphone: bool,
    output: &Path,
) -> Result<(ActiveRecording, Option<String>), String> {
    ensure_ffmpeg()?;
    let audio_inputs = selected_audio_inputs(record_system_audio, record_microphone)?;
    match pick_recording_backend()? {
        RecordingBackend::Portal => {
            let active = start_portal_recording(include_cursor, &audio_inputs, output)?;
            // xdg portal 无法沿用应用内选中的 target_id；光标亦由合成器决定。
            let warn =
                "录制已开始：门户将请你重新选择窗口/屏幕；光标由合成器决定（include_cursor 无效）"
                    .to_string();
            Ok((active, Some(warn)))
        }
        RecordingBackend::X11Grab { .. } => {
            let active = start_x11_recording(target_id, include_cursor, &audio_inputs, output)?;
            Ok((active, None))
        }
    }
}

fn start_x11_recording(
    target_id: u32,
    include_cursor: bool,
    audio_inputs: &[AudioInput],
    output: &Path,
) -> Result<ActiveRecording, String> {
    let mut command = Command::new("ffmpeg");
    command.arg("-y");
    for arg in build_ffmpeg_grab_args(target_id, include_cursor)? {
        command.arg(arg);
    }
    append_audio_inputs(&mut command, audio_inputs);
    command.args([
        "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-crf", "23",
    ]);
    append_mux_args(&mut command, audio_inputs);
    command.arg(output);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command
        .spawn()
        .map_err(|_| "未找到 ffmpeg，请安装后将它加入 PATH".to_string())?;
    Ok(ActiveRecording {
        child,
        stop_kind: StopKind::FfmpegQuit,
        portal: None,
    })
}

fn start_portal_recording(
    include_cursor: bool,
    audio_inputs: &[AudioInput],
    output: &Path,
) -> Result<ActiveRecording, String> {
    // xcap 的 ScreenCast 未暴露 cursor_mode；保留参数避免调用方分叉，并在能力文案里说明。
    let _ = include_cursor;

    portal_screencast_available()?;

    let monitor = Monitor::all()
        .map_err(|error| {
            format!("无法列出显示器（ScreenCast）：{error} / cannot list monitors: {error}")
        })?
        .into_iter()
        .next()
        .ok_or_else(|| "没有可用显示器 / no monitor available for ScreenCast".to_string())?;

    // 此处会触发门户选择器；无图形会话 / 用户取消会失败。
    let (video_recorder, frame_rx) = monitor.video_recorder().map_err(|error| {
        format!(
            "启动 xdg-desktop-portal ScreenCast 失败：{error}。请在图形 Wayland 会话中允许屏幕共享，并确认已安装 xdg-desktop-portal 与桌面后端。 / ScreenCast failed: {error}"
        )
    })?;

    video_recorder.start().map_err(|error| {
        format!("ScreenCast start 失败：{error} / ScreenCast start failed: {error}")
    })?;

    // 等首帧以确定真实分辨率（门户裁剪/缩放可能与 Monitor 元数据不一致）
    let first = frame_rx
        .recv_timeout(Duration::from_secs(90))
        .map_err(|_| {
            "等待 ScreenCast 画面超时（请在弹窗中选择屏幕/窗口并允许共享） / timed out waiting for ScreenCast frames — approve the portal dialog".to_string()
        })?;

    let width = first.width.max(2) & !1;
    let height = first.height.max(2) & !1;
    if width < 2 || height < 2 {
        let _ = video_recorder.stop();
        return Err("ScreenCast 画面尺寸无效 / invalid ScreenCast frame size".into());
    }

    let mut command = Command::new("ffmpeg");
    command.args([
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-video_size",
        &format!("{width}x{height}"),
        "-framerate",
        "30",
        "-i",
        "pipe:0",
    ]);
    append_audio_inputs(&mut command, audio_inputs);
    command.args([
        "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p", "-crf", "23",
    ]);
    append_mux_args(&mut command, audio_inputs);
    // 门户的视频通过 stdin 输入。停止时关闭该管道，但 PulseAudio 输入仍是实时流；
    // shortest 让 FFmpeg 在视频 EOF 后结束并封装 MP4，而不是继续等待音频设备。
    if !audio_inputs.is_empty() {
        command.arg("-shortest");
    }
    command.arg(output);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command
        .spawn()
        .map_err(|error| format!("启动 ffmpeg 失败：{error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "ffmpeg stdin 不可用".to_string())?;

    let stop_flag = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop_flag);

    // 先写入首帧，再泵后续帧；尺寸不一致时跳过（避免搞坏 rawvideo）
    if let Err(error) = write_even_rgba_frame(
        &mut stdin,
        &first.raw,
        first.width,
        first.height,
        width,
        height,
    ) {
        flag.store(true, Ordering::SeqCst);
        let _ = video_recorder.stop();
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("写入首帧失败：{error}"));
    }

    thread::spawn(move || {
        while !flag.load(Ordering::SeqCst) {
            match frame_rx.recv_timeout(Duration::from_millis(200)) {
                Ok(frame) => {
                    if write_even_rgba_frame(
                        &mut stdin,
                        &frame.raw,
                        frame.width,
                        frame.height,
                        width,
                        height,
                    )
                    .is_err()
                    {
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        // EOF → ffmpeg 收尾 MP4
        drop(stdin);
    });

    Ok(ActiveRecording {
        child,
        stop_kind: StopKind::StdinEof,
        portal: Some(PortalHandle {
            video_recorder,
            stop_flag,
        }),
    })
}

/// 将 RGBA 帧裁成偶数宽高后写入；尺寸不符预期则跳过（Ok）。
fn write_even_rgba_frame(
    stdin: &mut impl Write,
    raw: &[u8],
    src_w: u32,
    src_h: u32,
    dst_w: u32,
    dst_h: u32,
) -> std::io::Result<()> {
    if src_w == dst_w && src_h == dst_h && raw.len() >= (dst_w * dst_h * 4) as usize {
        return stdin.write_all(&raw[..(dst_w * dst_h * 4) as usize]);
    }
    // 同源分辨率但奇偶裁切
    if (src_w & !1) == dst_w && (src_h & !1) == dst_h {
        let mut buf = Vec::with_capacity((dst_w * dst_h * 4) as usize);
        for y in 0..dst_h {
            let start = ((y * src_w) * 4) as usize;
            let end = start + (dst_w * 4) as usize;
            if end > raw.len() {
                return Ok(());
            }
            buf.extend_from_slice(&raw[start..end]);
        }
        return stdin.write_all(&buf);
    }
    // 分辨率中途变化：跳过该帧，避免 rawvideo 错位
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // env mutations must be serialized across tests
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn restore_env(
        old_w: Option<std::ffi::OsString>,
        old_d: Option<std::ffi::OsString>,
        old_s: Option<std::ffi::OsString>,
    ) {
        unsafe {
            match old_w {
                Some(v) => std::env::set_var("WAYLAND_DISPLAY", v),
                None => std::env::remove_var("WAYLAND_DISPLAY"),
            }
            match old_d {
                Some(v) => std::env::set_var("DISPLAY", v),
                None => std::env::remove_var("DISPLAY"),
            }
            match old_s {
                Some(v) => std::env::set_var("XDG_SESSION_TYPE", v),
                None => std::env::remove_var("XDG_SESSION_TYPE"),
            }
        }
    }

    #[test]
    fn display_label_prefers_wayland_without_x11() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_w = std::env::var_os("WAYLAND_DISPLAY");
        let old_d = std::env::var_os("DISPLAY");
        let old_s = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
            std::env::remove_var("DISPLAY");
        }
        assert_eq!(display_server_label(), "Wayland");
        assert!(is_wayland_session());
        restore_env(old_w, old_d, old_s);
    }

    #[test]
    fn x11_display_prefers_x11grab_not_portal() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_w = std::env::var_os("WAYLAND_DISPLAY");
        let old_d = std::env::var_os("DISPLAY");
        let old_s = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::remove_var("WAYLAND_DISPLAY");
            std::env::set_var("DISPLAY", ":3");
            std::env::remove_var("XDG_SESSION_TYPE");
        }
        assert!(!is_wayland_session());
        assert_eq!(
            pick_recording_backend(),
            Ok(RecordingBackend::X11Grab {
                xwayland_only: false
            })
        );
        assert_eq!(display_server_label(), "X11");
        restore_env(old_w, old_d, old_s);
    }

    #[test]
    fn xwayland_session_is_not_treated_as_plain_x11() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_w = std::env::var_os("WAYLAND_DISPLAY");
        let old_d = std::env::var_os("DISPLAY");
        let old_s = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
            std::env::set_var("DISPLAY", ":0");
            std::env::set_var("XDG_SESSION_TYPE", "wayland");
        }
        assert!(is_wayland_session());
        assert_eq!(display_server_label(), "Wayland (XWayland available)");
        // 有门户就走门户，没门户退回 x11grab 但必须标记成 xwayland_only——
        // 绝不能等同于普通 X11，否则会静默录出黑屏
        let backend = pick_recording_backend();
        assert_ne!(
            backend,
            Ok(RecordingBackend::X11Grab {
                xwayland_only: false
            }),
            "Wayland 会话被当成了普通 X11：{backend:?}"
        );
        restore_env(old_w, old_d, old_s);
    }

    #[test]
    fn recording_rejects_missing_display_without_portal_env_gracefully() {
        let _guard = ENV_LOCK.lock().unwrap();
        let old_w = std::env::var_os("WAYLAND_DISPLAY");
        let old_d = std::env::var_os("DISPLAY");
        let old_s = std::env::var_os("XDG_SESSION_TYPE");
        unsafe {
            std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
            std::env::remove_var("DISPLAY");
            std::env::set_var("XDG_SESSION_TYPE", "wayland");
        }
        // 无真实门户会话时：应得到明确错误（portal 探测或 ffmpeg），而不是 panic
        let result = recording_available();
        if let Err(err) = result {
            assert!(
                err.contains("DISPLAY")
                    || err.contains("Wayland")
                    || err.contains("portal")
                    || err.contains("ScreenCast")
                    || err.contains("D-Bus")
                    || err.contains("ffmpeg"),
                "unexpected: {err}"
            );
        }
        restore_env(old_w, old_d, old_s);
    }
}
