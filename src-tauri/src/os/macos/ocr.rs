//! 文字识别：Vision.framework，经 Swift 桥调用。
//!
//! Vision 没有系统自带的命令行入口，Rust 侧直接 FFI 到 Vision 需要一整套
//! objc 互操作。这里改用 sidecar：src-tauri/snapshot-ocr/ 是一个只做 OCR 的
//! Swift 小工程，编出一个小可执行文件，Rust 侧按约定调用：
//!
//! ```text
//! snapshot-ocr            # stdin 收 PNG，stdout 出文本，逐行
//! snapshot-ocr --probe    # 探测可用性，stdout 出识别语言
//! ```
//!
//! 这样 Vision 的升级留在 Swift 侧，Rust 侧只认这两个约定。

use crate::ocr::{pipe_png, stderr_or};
use std::process::Command;

pub(crate) const OCR_BACKEND: &str = "Vision.framework";

const MACOS_HELPER: &str = "snapshot-ocr";

/// 打包后 snapshot-ocr 作为 sidecar 与主程序同目录（Contents/MacOS/）；
/// 开发期没有 sidecar 布局，退回 PATH（如 ~/.local/bin/snapshot-ocr）。
fn helper_program() -> String {
    if let Ok(exe) = std::env::current_exe() {
        let sidecar = exe.with_file_name(MACOS_HELPER);
        if sidecar.is_file() {
            return sidecar.to_string_lossy().into_owned();
        }
    }
    MACOS_HELPER.to_string()
}

/// 引擎是否就绪。可用时返回识别语言的可读名称。
pub(crate) fn ocr_language() -> Result<String, String> {
    let output = Command::new(helper_program())
        .arg("--probe")
        .output()
        .map_err(|_| format!("未找到 {MACOS_HELPER}，请先构建 macOS OCR 桥接程序"))?;
    if !output.status.success() {
        return Err(stderr_or(&output.stderr, "Vision 引擎不可用"));
    }
    let language = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if language.is_empty() {
        return Err("Vision 未报告可用的识别语言".into());
    }
    Ok(language)
}

pub(crate) fn ocr_recognize(png: &[u8]) -> Result<Vec<String>, String> {
    let output = pipe_png(&helper_program(), &[], png)
        .map_err(|error| format!("调用 {MACOS_HELPER} 失败：{error}"))?;
    if !output.status.success() {
        return Err(stderr_or(&output.stderr, "识别失败"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect())
}
