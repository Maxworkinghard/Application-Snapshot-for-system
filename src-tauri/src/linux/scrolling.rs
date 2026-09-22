//! 滚动长截图：X11 下对目标窗口连拍 + xdotool 翻页 + 纵向重叠拼接。
//! 纯 Wayland（无 `$DISPLAY`）返回可读双语错误。

use image::RgbaImage;
use std::env;
use std::process::Command;
use std::thread;
use std::time::Duration;
use xcap::Window;

use super::recording::should_use_portal;
use super::window::restore_minimized;

const MAX_FRAMES: usize = 28;
const SCROLL_SETTLE_MS: u64 = 280;
const OVERLAP_SEARCH_MIN: u32 = 24;

/// 对指定窗口做滚动长截图（best-effort）。
pub fn capture_scrolling_window(window_id: u32) -> Result<RgbaImage, String> {
    if should_use_portal() || env::var_os("DISPLAY").is_none() {
        return Err(
            "滚动长截图需要 X11/`$DISPLAY`（xdotool 翻页）。纯 Wayland 尚不支持 / scrolling capture needs X11 DISPLAY; not available on pure Wayland"
                .into(),
        );
    }

    ensure_xdotool()?;

    let window = Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|w| w.id().ok() == Some(window_id))
        .ok_or_else(|| "目标窗口已经关闭 / target window closed".to_string())?;

    if window.is_minimized().unwrap_or(false) {
        restore_minimized(window_id)?;
    }

    activate_window(window_id)?;

    let mut frames: Vec<RgbaImage> = Vec::new();
    frames.push(capture_one(&window)?);

    for _ in 1..MAX_FRAMES {
        scroll_window(window_id)?;
        thread::sleep(Duration::from_millis(SCROLL_SETTLE_MS));
        let window = Window::all()
            .map_err(|error| error.to_string())?
            .into_iter()
            .find(|w| w.id().ok() == Some(window_id))
            .ok_or_else(|| "目标窗口已经关闭 / target window closed".to_string())?;
        let next = capture_one(&window)?;
        let prev = frames.last().expect("frames non-empty");
        if images_nearly_equal(prev, &next) {
            break;
        }
        frames.push(next);
    }

    if frames.len() < 2 {
        return frames
            .into_iter()
            .next()
            .ok_or_else(|| "未能截取滚动帧 / no scrolling frames captured".into());
    }

    stitch_vertical(frames)
}

fn ensure_xdotool() -> Result<(), String> {
    if Command::new("xdotool").arg("-version").output().is_err() {
        return Err(
            "滚动长截图需要 xdotool，请安装后重试（如 apt install xdotool） / install xdotool for scrolling capture"
                .into(),
        );
    }
    Ok(())
}

fn activate_window(id: u32) -> Result<(), String> {
    let status = Command::new("xdotool")
        .args(["windowactivate", "--sync", &id.to_string()])
        .status()
        .map_err(|_| "无法激活目标窗口（xdotool） / xdotool windowactivate failed".to_string())?;
    if !status.success() {
        return Err("无法激活目标窗口 / failed to activate target window".into());
    }
    thread::sleep(Duration::from_millis(120));
    Ok(())
}

fn scroll_window(id: u32) -> Result<(), String> {
    let page = Command::new("xdotool")
        .args(["key", "--window", &id.to_string(), "Page_Down"])
        .status();
    if matches!(page, Ok(code) if code.success()) {
        return Ok(());
    }
    let click = Command::new("xdotool")
        .args([
            "mousemove",
            "--window",
            &id.to_string(),
            "40",
            "40",
            "click",
            "--window",
            &id.to_string(),
            "5",
        ])
        .status();
    match click {
        Ok(code) if code.success() => Ok(()),
        _ => Err("无法向目标窗口发送翻页（xdotool） / xdotool scroll failed".into()),
    }
}

fn capture_one(window: &Window) -> Result<RgbaImage, String> {
    window
        .capture_image()
        .map_err(|error| format!("滚动截取失败：{error} / scrolling frame failed: {error}"))
}

fn images_nearly_equal(a: &RgbaImage, b: &RgbaImage) -> bool {
    if a.width() != b.width() || a.height() != b.height() {
        return false;
    }
    let step = 8u32;
    let mut checked = 0u32;
    let mut close = 0u32;
    let h = a.height();
    let w = a.width();
    let mut y = 0u32;
    while y < h {
        let mut x = 0u32;
        while x < w {
            checked += 1;
            let pa = a.get_pixel(x, y).0;
            let pb = b.get_pixel(x, y).0;
            let diff = pa
                .iter()
                .zip(pb.iter())
                .map(|(l, r)| (*l as i16 - *r as i16).unsigned_abs() as u32)
                .sum::<u32>();
            if diff < 48 {
                close += 1;
            }
            x = x.saturating_add(step);
        }
        y = y.saturating_add(step);
    }
    checked > 0 && (close as f32 / checked as f32) > 0.985
}

fn find_overlap(prev: &RgbaImage, next: &RgbaImage) -> u32 {
    let width = prev.width().min(next.width());
    let max_overlap = prev.height().min(next.height()).saturating_sub(8);
    if max_overlap < OVERLAP_SEARCH_MIN || width < 8 {
        return OVERLAP_SEARCH_MIN.min(prev.height().min(next.height()) / 3);
    }

    let mut best_overlap = OVERLAP_SEARCH_MIN;
    let mut best_score = u64::MAX;
    let sample_step = 6u32;

    let mut overlap = OVERLAP_SEARCH_MIN;
    while overlap <= max_overlap {
        let mut score = 0u64;
        let mut samples = 0u64;
        let mut row = 0u32;
        while row < overlap {
            let py = prev.height() - overlap + row;
            let mut x = 0u32;
            while x < width {
                let pa = prev.get_pixel(x, py).0;
                let pb = next.get_pixel(x, row).0;
                for i in 0..3 {
                    let d = pa[i] as i32 - pb[i] as i32;
                    score += (d * d) as u64;
                }
                samples += 1;
                x = x.saturating_add(sample_step);
            }
            row = row.saturating_add(sample_step);
        }
        let normalized = if samples == 0 { u64::MAX } else { score / samples };
        if normalized < best_score {
            best_score = normalized;
            best_overlap = overlap;
        }
        overlap += 4;
    }
    best_overlap
}

fn blit_rows(dest: &mut RgbaImage, dest_y: u32, src: &RgbaImage, src_y: u32, rows: u32, width: u32) {
    for row in 0..rows {
        for x in 0..width {
            let pixel = *src.get_pixel(x, src_y + row);
            dest.put_pixel(x, dest_y + row, pixel);
        }
    }
}

fn stitch_vertical(frames: Vec<RgbaImage>) -> Result<RgbaImage, String> {
    let width = frames
        .iter()
        .map(|f| f.width())
        .min()
        .ok_or_else(|| "无帧可拼接".to_string())?;
    if width < 2 {
        return Err("滚动截图像素宽度无效".into());
    }

    let mut total_height = frames[0].height();
    let mut overlaps = Vec::with_capacity(frames.len().saturating_sub(1));
    for i in 1..frames.len() {
        let overlap = find_overlap(&frames[i - 1], &frames[i]);
        overlaps.push(overlap);
        total_height = total_height.saturating_add(frames[i].height().saturating_sub(overlap));
    }

    if total_height > 32_000 {
        return Err(
            "拼接结果过高（可能重叠检测失败），已中止 / stitch height too large; overlap detection may have failed"
                .into(),
        );
    }

    let mut canvas = RgbaImage::new(width, total_height);
    blit_rows(&mut canvas, 0, &frames[0], 0, frames[0].height(), width);

    let mut cursor_y = frames[0].height();
    for (i, frame) in frames.iter().enumerate().skip(1) {
        let overlap = overlaps[i - 1];
        let append_h = frame.height().saturating_sub(overlap);
        if append_h == 0 {
            continue;
        }
        blit_rows(&mut canvas, cursor_y, frame, overlap, append_h, width);
        cursor_y = cursor_y.saturating_add(append_h);
    }

    if cursor_y < total_height {
        Ok(image::imageops::crop_imm(&canvas, 0, 0, width, cursor_y).to_image())
    } else {
        Ok(canvas)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    fn solid(w: u32, h: u32, shade: u8) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([shade, shade, shade, 255]))
    }

    #[test]
    fn nearly_equal_detects_identical() {
        let a = solid(40, 40, 10);
        let b = solid(40, 40, 10);
        assert!(images_nearly_equal(&a, &b));
    }

    #[test]
    fn stitch_appends_with_overlap() {
        let mut top = solid(20, 40, 20);
        let mut bottom = solid(20, 40, 200);
        for y in 24..40 {
            for x in 0..20 {
                top.put_pixel(x, y, Rgba([90, 90, 90, 255]));
                bottom.put_pixel(x, y - 24, Rgba([90, 90, 90, 255]));
            }
        }
        let stitched = stitch_vertical(vec![top, bottom]).expect("stitch");
        assert_eq!(stitched.width(), 20);
        assert!(stitched.height() > 40);
        assert!(stitched.height() <= 64);
    }
}
