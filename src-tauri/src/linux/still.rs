//! 静帧截取辅助：X11 下可用 ffmpeg x11grab 带光标抓一帧。

use image::RgbaImage;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use xcap::Monitor;

fn display_spec() -> Result<String, String> {
    std::env::var("DISPLAY").map_err(|_| {
        "无 X11 DISPLAY，无法用 x11grab 抓带光标静帧 / no DISPLAY for x11grab cursor frame"
            .to_string()
    })
}

fn primary_monitor() -> Result<Monitor, String> {
    let monitors = Monitor::all().map_err(|error| error.to_string())?;
    monitors
        .into_iter()
        .find(|m| m.is_primary().unwrap_or(false))
        .or_else(|| {
            Monitor::all()
                .ok()
                .and_then(|items| items.into_iter().next())
        })
        .ok_or_else(|| "未找到显示器".into())
}

fn capture_rect_with_cursor(x: i32, y: i32, width: u32, height: u32) -> Result<RgbaImage, String> {
    if width < 2 || height < 2 {
        return Err("截取区域太小".into());
    }
    let display = display_spec()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = std::env::temp_dir().join(format!("snapshot-cursor-{stamp}.png"));
    let input = format!("{display}+{x},{y}");
    let status = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "x11grab",
            "-draw_mouse",
            "1",
            "-video_size",
            &format!("{width}x{height}"),
            "-i",
            &input,
            "-frames:v",
            "1",
            "-update",
            "1",
        ])
        .arg(&path)
        .status()
        .map_err(|_| {
            "未找到 ffmpeg，无法抓带光标静帧 / ffmpeg not found for cursor still".to_string()
        })?;
    if !status.success() {
        let _ = std::fs::remove_file(&path);
        return Err("ffmpeg x11grab 抓带光标静帧失败 / ffmpeg x11grab cursor frame failed".into());
    }
    let image = image::open(&path)
        .map_err(|error| format!("读取光标静帧失败：{error}"))?
        .to_rgba8();
    let _ = std::fs::remove_file(&path);
    Ok(image)
}

/// 主屏静帧（含鼠标光标）。仅 X11/`$DISPLAY`；失败由调用方回退到 xcap。
pub fn capture_primary_with_cursor() -> Result<RgbaImage, String> {
    let monitor = primary_monitor()?;
    let x = monitor.x().unwrap_or(0);
    let y = monitor.y().unwrap_or(0);
    let width = monitor.width().map_err(|error| error.to_string())?;
    let height = monitor.height().map_err(|error| error.to_string())?;
    capture_rect_with_cursor(x, y, width, height)
}

/// 任意屏幕矩形静帧（含鼠标光标）。窗口截图按窗口矩形调用；失败由调用方回退。
pub fn capture_region_with_cursor(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
) -> Result<RgbaImage, String> {
    capture_rect_with_cursor(x, y, width, height)
}
