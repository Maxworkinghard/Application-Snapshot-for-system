//! 系统能力：文字识别
//!
//! 三端的 OCR 引擎完全不同，没有任何共用代码：
//!   - Windows  Windows.Media.Ocr（WinRT，系统自带）   → os/windows/ocr.rs
//!   - macOS    Vision.framework（经 Swift 桥调用）     → os/macos/ocr.rs
//!   - Linux    tesseract（外部可执行文件）             → os/linux/ocr.rs
//!
//! 所以这里只统一「接口」，不统一「实现」——成熟的系统能力照用，不为了语言一致把它们
//! 重写一遍。各平台只需提供 `OCR_BACKEND`、`ocr_language()`、`ocr_recognize()`，
//! 可用性报告与结果清洗在这里做一次。

use super::capabilities::CapabilityStatus;
use super::os;

/// 引擎可用性报告，直接序列化给前端
pub struct OcrReport {
    pub available: bool,
    pub language: Option<String>,
    pub detail: String,
}

pub fn report() -> OcrReport {
    match os::ocr_language() {
        Ok(language) => OcrReport {
            available: true,
            detail: format!("{} · 识别语言：{language}", os::OCR_BACKEND),
            language: Some(language),
        },
        Err(detail) => OcrReport {
            available: false,
            language: None,
            detail,
        },
    }
}

/// 给「本机能力」一栏用
pub(crate) fn capability() -> CapabilityStatus {
    let report = report();
    CapabilityStatus {
        available: report.available,
        detail: report.detail,
    }
}

/// 识别 PNG 字节。统一用 PNG 作为入参，避免每个平台各自定义位图格式。
pub fn recognize_png(png: &[u8]) -> Result<String, String> {
    finish(os::ocr_recognize(png)?)
}

/// 各后端都输出「逐行文本」，统一在这里清洗并判空
fn finish(lines: Vec<String>) -> Result<String, String> {
    let kept: Vec<String> = lines
        .into_iter()
        .map(|line| line.trim_end().to_string())
        .filter(|line| !line.trim().is_empty())
        .collect();
    if kept.is_empty() {
        return Err("没有识别到文字".into());
    }
    Ok(kept.join("\n"))
}

/// 把 PNG 从 stdin 喂给外部程序，收其 stdout。macOS 的 Vision 桥与 Linux 的
/// tesseract 都是外部可执行文件；Windows 直接调 WinRT，用不上。
#[cfg(unix)]
pub(crate) fn pipe_png(
    program: &str,
    args: &[&str],
    png: &[u8],
) -> std::io::Result<std::process::Output> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    // stdin 必须在 wait 之前 drop，否则子进程等不到 EOF 会一直挂着
    {
        let mut stdin = child.stdin.take().expect("stdin 已配置为管道");
        stdin.write_all(png)?;
    }
    child.wait_with_output()
}

#[cfg(unix)]
pub(crate) fn stderr_or(stderr: &[u8], fallback: &str) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.is_empty() {
        fallback.to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_result_is_reported_as_no_text() {
        assert!(finish(vec![]).is_err());
        assert!(finish(vec!["   ".into(), "\t".into()]).is_err());
    }

    #[test]
    fn lines_are_trimmed_and_joined_with_newlines() {
        let text = finish(vec!["第一行   ".into(), "".into(), "第二行".into()]).unwrap();
        assert_eq!(text, "第一行\n第二行");
    }
}
