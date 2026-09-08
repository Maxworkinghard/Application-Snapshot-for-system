//! 窗口录制：ffmpeg x11grab 按活动窗口矩形录制，SIGINT 优雅停止以保证 MP4 封装完整。
//! 对应 macOS 端的 WindowRecordingService（ScreenCaptureKit 窗口级捕获）。

use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::Result;

pub struct Recorder {
    /// 是否正在录制（托盘菜单据此切换「停止录制」）。
    pub active: Arc<AtomicBool>,
    child: Arc<Mutex<Option<Child>>>,
    output_path: Arc<Mutex<Option<String>>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
            child: Arc::new(Mutex::new(None)),
            output_path: Arc::new(Mutex::new(None)),
        }
    }

    /// 开始录制给定矩形（活动窗口在 root 上的区域）。
    pub fn start(&self, x: i32, y: i32, width: u32, height: u32) -> Result<String> {
        if self.active.load(Ordering::SeqCst) {
            return Err("已在录制中".into());
        }
        let ffmpeg = find_ffmpeg().ok_or("未找到 ffmpeg，请安装后将其加入 PATH")?;
        let display = std::env::var("DISPLAY")
            .map_err(|_| "未设置 DISPLAY，无法使用 x11grab".to_string())?;

        // yuv420p 要求宽高为偶数
        let width = width & !1;
        let height = height & !1;
        if width < 2 || height < 2 {
            return Err("窗口尺寸无效，无法录制".into());
        }

        let path = make_output_path()?;

        let mut child = Command::new(ffmpeg)
            .args(["-y", "-f", "x11grab", "-framerate", "30"])
            .arg("-video_size")
            .arg(format!("{width}x{}", height))
            .arg("-i")
            .arg(format!("{display}+{},{}", x.max(0), y.max(0)))
            .args(["-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p"])
            .args(["-crf", "23"])
            .arg(&path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("启动 ffmpeg 失败：{e}"))?;

        // 确认 ffmpeg 没有立即失败（x11grab 连不上 DISPLAY 等）
        std::thread::sleep(Duration::from_millis(1200));
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(_) => return Err("录制启动失败（请检查 DISPLAY 与窗口尺寸）".into()),
            None => {}
        }

        *self.child.lock().unwrap() = Some(child);
        *self.output_path.lock().unwrap() = Some(path.clone());
        self.active.store(true, Ordering::SeqCst);
        Ok(path)
    }

    /// 停止录制：SIGINT 让 ffmpeg 写完 MP4 尾部再退出。返回输出路径。
    pub fn stop(&self) -> Result<String> {
        if !self.active.swap(false, Ordering::SeqCst) {
            return Err("当前没有进行中的录制".into());
        }

        let mut child_slot = self.child.lock().unwrap();
        let mut child = child_slot.take();
        let path = self.output_path.lock().unwrap().take();

        if let Some(child) = child.as_mut() {
            // std 的 Child::kill 是 SIGKILL，会损坏 MP4；必须用 SIGINT 优雅停止
            let _ = Command::new("kill")
                .arg("-INT")
                .arg(child.id().to_string())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();

            for _ in 0..100 {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                    Err(_) => break,
                }
            }
            if child.try_wait().map(|status| status.is_none()).unwrap_or(false) {
                let _ = child.kill();
                let _ = child.wait();
            }
        }

        let path = path.ok_or("录制状态异常")?;
        let valid = std::fs::metadata(&path).map(|meta| meta.len() > 0).unwrap_or(false);
        if valid {
            Ok(path)
        } else {
            Err("录制保存失败，文件未生成".into())
        }
    }
}

fn find_ffmpeg() -> Option<String> {
    let candidates = [
        "ffmpeg",
        "/usr/bin/ffmpeg",
        "/usr/local/bin/ffmpeg",
        "/snap/bin/ffmpeg",
    ];
    for candidate in candidates {
        let ok = Command::new(candidate)
            .arg("-version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false);
        if ok {
            return Some(candidate.to_string());
        }
    }
    None
}

fn make_output_path() -> Result<String> {
    use chrono::Local;

    let directory = crate::save_directory();
    std::fs::create_dir_all(&directory)
        .map_err(|e| format!("无法创建保存目录 {directory}：{e}"))?;
    let stamp = Local::now().format("%Y-%m-%d_%H-%M-%S");
    Ok(format!("{directory}/应用快照-{stamp}.mp4"))
}
