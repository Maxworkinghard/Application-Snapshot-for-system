//! 系统能力：文字识别
//!
//! 三端的 OCR 引擎完全不同，没有任何共用代码：
//!   - Windows  Windows.Media.Ocr（WinRT，系统自带）
//!   - macOS    Vision.framework（需经 Swift 桥调用）
//!   - Linux    tesseract（外部可执行文件）
//!
//! 所以这里只统一「接口」，不统一「实现」——成熟的系统能力照用，
//! 不为了语言一致把它们重写一遍。上层只依赖 `OcrAdapter`，
//! 换平台时换的是 adapter，调用方一行不动。

#[cfg(not(target_os = "windows"))]
use std::process::{Command, Stdio};

/// 引擎可用性报告，直接序列化给前端
pub struct OcrReport {
    pub available: bool,
    pub language: Option<String>,
    pub detail: String,
}

pub trait OcrAdapter: Send + Sync {
    /// 后端名称，出现在错误信息里，方便判断走的是哪条路径
    fn backend(&self) -> &'static str;

    /// 引擎是否就绪。可用时返回识别语言的可读名称。
    fn language(&self) -> Result<String, String>;

    /// 识别 PNG 字节。统一用 PNG 作为入参，避免每个平台各自定义位图格式。
    fn recognize_png(&self, png: &[u8]) -> Result<String, String>;
}

/// 取当前平台的 adapter
pub fn adapter() -> &'static dyn OcrAdapter {
    &PLATFORM
}

pub fn report() -> OcrReport {
    match PLATFORM.language() {
        Ok(language) => OcrReport {
            available: true,
            detail: format!("{} · 识别语言：{language}", PLATFORM.backend()),
            language: Some(language),
        },
        Err(detail) => OcrReport {
            available: false,
            language: None,
            detail,
        },
    }
}

// ===========================================================================
// Windows —— Windows.Media.Ocr
// ===========================================================================

#[cfg(target_os = "windows")]
pub struct PlatformOcr;

#[cfg(target_os = "windows")]
static PLATFORM: PlatformOcr = PlatformOcr;

#[cfg(target_os = "windows")]
impl OcrAdapter for PlatformOcr {
    fn backend(&self) -> &'static str {
        "Windows.Media.Ocr"
    }

    fn language(&self) -> Result<String, String> {
        use windows::Media::Ocr::OcrEngine;
        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "系统没有可用的 OCR 语言包，请在「设置 → 时间和语言 → 语言」中添加".to_string())?;
        let language = engine.RecognizerLanguage().map_err(|error| error.to_string())?;
        language
            .DisplayName()
            .map(|name| name.to_string_lossy())
            .map_err(|error| error.to_string())
    }

    fn recognize_png(&self, png: &[u8]) -> Result<String, String> {
        use windows::Graphics::Imaging::BitmapDecoder;
        use windows::Media::Ocr::OcrEngine;
        use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};

        let stream = InMemoryRandomAccessStream::new().map_err(|error| error.to_string())?;
        let writer = DataWriter::CreateDataWriter(&stream).map_err(|error| error.to_string())?;
        writer.WriteBytes(png).map_err(|error| error.to_string())?;
        writer.StoreAsync().map_err(|e| e.to_string())?.join().map_err(|e| e.to_string())?;
        writer.FlushAsync().map_err(|e| e.to_string())?.join().map_err(|e| e.to_string())?;
        writer.DetachStream().map_err(|error| error.to_string())?;
        stream.Seek(0).map_err(|error| error.to_string())?;

        let decoder = BitmapDecoder::CreateAsync(&stream)
            .map_err(|e| e.to_string())?
            .join()
            .map_err(|error| format!("解码图像失败：{error}"))?;
        let bitmap = decoder
            .GetSoftwareBitmapAsync()
            .map_err(|e| e.to_string())?
            .join()
            .map_err(|error| format!("读取位图失败：{error}"))?;

        let engine = OcrEngine::TryCreateFromUserProfileLanguages()
            .map_err(|_| "系统没有可用的 OCR 语言包，请在「设置 → 时间和语言 → 语言」中添加".to_string())?;
        let result = engine
            .RecognizeAsync(&bitmap)
            .map_err(|e| e.to_string())?
            .join()
            .map_err(|error| format!("识别失败：{error}"))?;

        // Text() 会把整页拼成一行，按 Lines 自己拼才能保留换行
        let lines = result.Lines().map_err(|error| error.to_string())?;
        let mut out = Vec::new();
        for line in lines {
            if let Ok(text) = line.Text() {
                out.push(text.to_string_lossy());
            }
        }
        finish(out)
    }
}

// ===========================================================================
// macOS —— Vision.framework，经 Swift 桥调用
// ===========================================================================
//
// Vision 没有系统自带的命令行入口，Rust 侧直接 FFI 到 Vision 需要一整套
// objc 互操作。这里改用 sidecar：macos/ 目录下已有成熟的 Swift 工程，
// 由它编出一个只做 OCR 的小可执行文件，Rust 侧按约定调用：
//
//     snapshot-ocr            # stdin 收 PNG，stdout 出文本，逐行
//     snapshot-ocr --probe    # 探测可用性，stdout 出识别语言
//
// 这样 Vision 的升级留在 Swift 侧，Rust 侧只认这两个约定。

#[cfg(target_os = "macos")]
pub struct PlatformOcr;

#[cfg(target_os = "macos")]
static PLATFORM: PlatformOcr = PlatformOcr;

#[cfg(target_os = "macos")]
const MACOS_HELPER: &str = "snapshot-ocr";

#[cfg(target_os = "macos")]
impl OcrAdapter for PlatformOcr {
    fn backend(&self) -> &'static str {
        "Vision.framework"
    }

    fn language(&self) -> Result<String, String> {
        let output = Command::new(MACOS_HELPER)
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

    fn recognize_png(&self, png: &[u8]) -> Result<String, String> {
        let output = pipe_png(MACOS_HELPER, &[], png)
            .map_err(|error| format!("调用 {MACOS_HELPER} 失败：{error}"))?;
        if !output.status.success() {
            return Err(stderr_or(&output.stderr, "识别失败"));
        }
        finish(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.to_string())
                .collect(),
        )
    }
}

// ===========================================================================
// Linux —— tesseract
// ===========================================================================
//
// 走外部可执行文件而不是链接 libtesseract：后者会给构建引入一串系统依赖，
// 而本项目录屏本来就在 shell ffmpeg，风格一致。

#[cfg(all(unix, not(target_os = "macos")))]
pub struct PlatformOcr;

#[cfg(all(unix, not(target_os = "macos")))]
static PLATFORM: PlatformOcr = PlatformOcr;

#[cfg(all(unix, not(target_os = "macos")))]
const TESSERACT: &str = "tesseract";

/// 优先中英混排，装不全时退回英文
#[cfg(all(unix, not(target_os = "macos")))]
fn tesseract_languages(installed: &[String]) -> String {
    let mut picked: Vec<&str> = Vec::new();
    for candidate in ["chi_sim", "eng"] {
        if installed.iter().any(|item| item == candidate) {
            picked.push(candidate);
        }
    }
    if picked.is_empty() { "eng".to_string() } else { picked.join("+") }
}

#[cfg(all(unix, not(target_os = "macos")))]
impl OcrAdapter for PlatformOcr {
    fn backend(&self) -> &'static str {
        "tesseract"
    }

    fn language(&self) -> Result<String, String> {
        let output = Command::new(TESSERACT)
            .arg("--list-langs")
            .output()
            .map_err(|_| "未安装 tesseract，请先通过包管理器安装（如 apt install tesseract-ocr）".to_string())?;
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

    fn recognize_png(&self, png: &[u8]) -> Result<String, String> {
        let languages = self.language()?;
        // `-` 表示从 stdin 读，stdout 输出，省掉临时文件
        let output = pipe_png(TESSERACT, &["-", "stdout", "-l", &languages], png)
            .map_err(|error| format!("调用 tesseract 失败：{error}"))?;
        if !output.status.success() {
            return Err(stderr_or(&output.stderr, "识别失败"));
        }
        finish(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.to_string())
                .collect(),
        )
    }
}

// ===========================================================================
// 各 adapter 共用的小工具
// ===========================================================================

/// 把 PNG 从 stdin 喂给外部程序，收其 stdout
#[cfg(not(target_os = "windows"))]
fn pipe_png(program: &str, args: &[&str], png: &[u8]) -> std::io::Result<std::process::Output> {
    use std::io::Write;

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

#[cfg(not(target_os = "windows"))]
fn stderr_or(stderr: &[u8], fallback: &str) -> String {
    let text = String::from_utf8_lossy(stderr).trim().to_string();
    if text.is_empty() { fallback.to_string() } else { text }
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

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn language_pick_prefers_chinese_plus_english() {
        assert_eq!(tesseract_languages(&["eng".into(), "chi_sim".into()]), "chi_sim+eng");
        assert_eq!(tesseract_languages(&["eng".into()]), "eng");
        assert_eq!(tesseract_languages(&["deu".into()]), "eng");
    }
}
