//! 窗口录制：ScreenCaptureKit，经 Swift sidecar（snapshot-recorder）调用。
//!
//! sidecar 约定：启动后在 stdout 输出一行 `READY …` 或 `ERROR …`；
//! stdin 收到 `q` 后收尾退出；异常原因写在 stderr。

use crate::recording::RecordOptions;
use parking_lot::Mutex;
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, Command, Stdio},
    sync::{mpsc, Arc},
    thread,
    time::Duration,
};

const MACOS_RECORDER_HELPER: &str = "snapshot-recorder";

pub(crate) fn recorder_program() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let bundled = exe.with_file_name(MACOS_RECORDER_HELPER);
        if bundled.is_file() {
            return bundled;
        }
    }

    let triple = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let prepared =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("{MACOS_RECORDER_HELPER}-{triple}"));
    if prepared.is_file() {
        return prepared;
    }

    PathBuf::from(MACOS_RECORDER_HELPER)
}

/// 录制组件在不在、能不能跑（给「本机能力」一栏用）
pub(crate) fn probe() -> Result<(), String> {
    let output = Command::new(recorder_program())
        .arg("--probe")
        .output()
        .map_err(|_| "未找到 ScreenCaptureKit 录制组件，请重新安装应用".to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err("ScreenCaptureKit 录制组件不可用，请重新安装应用".into())
    }
}

/// 后台线程持续读 sidecar 的 stderr，避免管道写满，并在异常退出时把真实原因回传给 UI。
fn capture_child_stderr(stderr: Option<ChildStderr>) -> Arc<Mutex<String>> {
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

/// 一次进行中的录制
pub(crate) struct Recording {
    child: Child,
    diagnostic: Arc<Mutex<String>>,
}

impl Recording {
    fn diagnostic(&self) -> String {
        self.diagnostic.lock().trim().to_string()
    }

    /// sidecar 自己退出了（崩溃、权限被撤等）就返回原因
    pub(crate) fn exit_reason(&mut self) -> Option<String> {
        let status = self.child.try_wait().ok()??;
        let detail = self.diagnostic();
        Some(if detail.is_empty() {
            format!("录制进程意外退出（{status}）")
        } else {
            format!("录制进程意外退出：{detail}")
        })
    }

    pub(crate) fn stop(mut self) -> Result<(), String> {
        if let Some(stdin) = self.child.stdin.as_mut() {
            let _ = stdin.write_all(b"q\n");
        }
        let Ok(status) = self.child.wait() else {
            return Ok(());
        };
        if status.success() {
            return Ok(());
        }
        // 给读 stderr 的线程一点时间把最后几行收进来
        thread::sleep(Duration::from_millis(20));
        let detail = self.diagnostic();
        Err(if detail.is_empty() {
            format!("录制停止异常（{status}）")
        } else {
            format!("录制停止异常：{detail}")
        })
    }
}

pub(crate) fn start_recording(
    window_id: u32,
    options: &RecordOptions,
    output: &Path,
) -> Result<(Recording, Option<String>), String> {
    let mut command = Command::new(recorder_program());
    command
        .arg("--window-id")
        .arg(window_id.to_string())
        .arg("--output")
        .arg(output);
    if options.include_cursor {
        command.arg("--include-cursor");
    }
    if options.system_audio {
        command.arg("--system-audio");
    }
    if options.microphone {
        command.arg("--microphone");
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
    let (sender, receiver) = mpsc::sync_channel(1);
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
        return Ok((Recording { child, diagnostic }, None));
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
