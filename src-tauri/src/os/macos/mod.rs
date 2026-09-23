//! macOS 平台层。
//!
//! 与 Windows / Linux 平台层对外提供同名的一组函数（见 lib.rs 里 `mod os` 的说明），
//! 共享代码只经 `os::` 调用。实现都用系统原生能力：ScreenCaptureKit 录制、Vision OCR
//! （两者经 Swift sidecar）、Accessibility 还原窗口、LaunchAgent 自启。

mod autostart;
mod cursor;
mod icon;
mod ocr;
mod recorder;
mod window;

use crate::capabilities::{CapabilityStatus, PlatformCapabilities};
use crate::capture::Area;
use image::RgbaImage;
use std::{path::Path, process::Command};

pub(crate) use autostart::apply_launch_on_boot as apply_autostart;
pub(crate) use icon::png_for_pid as app_icon_png;
pub(crate) use ocr::{ocr_language, ocr_recognize, OCR_BACKEND};
pub(crate) use recorder::{start_recording, Recording};
pub(crate) use window::restore_minimized;

/// 本平台支持的全局快捷键动作，顺序即设置页的显示顺序
pub(crate) const SHORTCUT_ACTIONS: &[&str] = &["snapshot", "fullscreen", "record", "polish", "ocr"];

/// xcap 截不到光标：截完按窗口原点与 DPI 比例把当前系统光标合成上去。
/// 返回的第二项是光标没合成上的原因（截图本身仍然成功）。
pub(crate) fn capture_with_cursor(
    area: &Area,
    capture: impl FnOnce() -> Result<RgbaImage, String>,
) -> Result<(RgbaImage, Option<String>), String> {
    let mut image = capture()?;
    let failure = cursor::overlay_into(&mut image, (area.x, area.y), area.width).err();
    Ok((image, failure))
}

pub(crate) fn open_folder(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("无法打开访达：{error}"))
}

fn major_version() -> Option<u32> {
    let output = Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()?
        .trim()
        .split('.')
        .next()?
        .parse()
        .ok()
}

pub(crate) fn capabilities() -> PlatformCapabilities {
    let recording = CapabilityStatus::probe(
        recorder::probe(),
        "ScreenCaptureKit 原生窗口流（不受遮挡，支持副屏与窗口移动；首次使用会请求屏幕录制权限）",
    );
    let recording_system_audio = if recording.available {
        CapabilityStatus::available("ScreenCaptureKit：录制当前显示器上的应用系统声音")
    } else {
        recording.clone()
    };
    let microphone_supported = major_version()
        .map(|version| version >= 15)
        .unwrap_or(false);
    let recording_microphone = if !recording.available {
        recording.clone()
    } else if microphone_supported {
        CapabilityStatus::available("ScreenCaptureKit：macOS 15 及以上可录制麦克风")
    } else {
        CapabilityStatus {
            available: false,
            detail: "麦克风采集需要 macOS 15 或更新版本".into(),
        }
    };
    PlatformCapabilities {
        os: "macos".into(),
        display_server: "AppKit".into(),
        recording,
        recording_system_audio,
        recording_microphone,
        ocr: crate::ocr::capability(),
        autostart: CapabilityStatus::probe(
            autostart::autostart_capability(),
            "LaunchAgent：~/Library/LaunchAgents/com.appsnapshot.prompt-pet-shortcut.plist，下次登录生效",
        ),
        scrolling: None,
        include_cursor: CapabilityStatus::available(
            "录制：ScreenCaptureKit showsCursor；静帧截图：截后按热点与 DPI 比例合成当前系统光标",
        ),
        tray_note: "NSStatusItem：左键打开主窗口；菜单打开设置/退出。".into(),
        notes: vec![
            "macOS 还原最小化窗口按 AXTitle 对齐；标题对不上且该应用有多个最小化窗口时，不代劳、提示手动还原".into(),
            "窗口录制使用 ScreenCaptureKit；系统音频可单独录制，麦克风需要 macOS 15 或更新版本".into(),
        ],
    }
}
