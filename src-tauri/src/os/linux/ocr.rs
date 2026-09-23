//! 文字识别：tesseract。
//!
//! 走外部可执行文件而不是链接 libtesseract：后者会给构建引入一串系统依赖，
//! 而本项目录屏本来就在 shell ffmpeg，风格一致。

use crate::ocr::{pipe_png, stderr_or};
use std::process::Command;

pub(crate) const OCR_BACKEND: &str = "tesseract";

const TESSERACT: &str = "tesseract";

/// 优先中英混排，装不全时退回英文
fn tesseract_languages(installed: &[String]) -> String {
    let mut picked: Vec<&str> = Vec::new();
    for candidate in ["chi_sim", "eng"] {
        if installed.iter().any(|item| item == candidate) {
            picked.push(candidate);
        }
    }
    if picked.is_empty() {
        "eng".to_string()
    } else {
        picked.join("+")
    }
}

/// 引擎是否就绪。可用时返回识别语言。
pub(crate) fn ocr_language() -> Result<String, String> {
    let output = Command::new(TESSERACT)
        .arg("--list-langs")
        .output()
        .map_err(|_| {
            "未安装 tesseract，请先通过包管理器安装（如 apt install tesseract-ocr）".to_string()
        })?;
    if !output.status.success() {
        return Err(stderr_or(&output.stderr, "tesseract 不可用"));
    }
    // 首行是说明文字，其余每行一个语言包
    let installed: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .skip(1)
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect();
    if installed.is_empty() {
        return Err("tesseract 没有安装任何语言包".into());
    }
    Ok(tesseract_languages(&installed))
}

pub(crate) fn ocr_recognize(png: &[u8]) -> Result<Vec<String>, String> {
    let languages = ocr_language()?;
    // `-` 表示从 stdin 读，stdout 输出，省掉临时文件
    let output = pipe_png(TESSERACT, &["-", "stdout", "-l", &languages], png)
        .map_err(|error| format!("调用 tesseract 失败：{error}"))?;
    if !output.status.success() {
        return Err(stderr_or(&output.stderr, "识别失败"));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_pick_prefers_chinese_plus_english() {
        assert_eq!(
            tesseract_languages(&["eng".into(), "chi_sim".into()]),
            "chi_sim+eng"
        );
        assert_eq!(tesseract_languages(&["eng".into()]), "eng");
        assert_eq!(tesseract_languages(&["deu".into()]), "eng");
    }
}
