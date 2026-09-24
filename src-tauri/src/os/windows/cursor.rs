//! 静帧截图合成鼠标光标：xcap 只给窗口像素，光标要按热点自己画上去。

use crate::capture::blend_cursor;
use image::{Rgba, RgbaImage};
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetObjectW, SelectObject,
        BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    },
    UI::WindowsAndMessaging::{
        DrawIconEx, GetCursorInfo, GetIconInfo, CURSORINFO, CURSOR_SHOWING, DI_NORMAL, ICONINFO,
    },
};

/// 取当前光标的像素、屏幕位置与热点。热点是光标图内对应「尖端」的那个点，
/// 贴图时要减掉它，否则光标会整体偏右下。
fn cursor_bitmap() -> Option<(RgbaImage, i32, i32, i32, i32)> {
    unsafe {
        let mut info: CURSORINFO = zeroed();
        info.cbSize = size_of::<CURSORINFO>() as u32;
        if GetCursorInfo(&mut info) == 0 || info.flags != CURSOR_SHOWING || info.hCursor.is_null() {
            return None;
        }
        let mut icon: ICONINFO = zeroed();
        if GetIconInfo(info.hCursor, &mut icon) == 0 {
            return None;
        }
        // 单色光标的掩码位图是上下两段（AND + XOR），高度要折半才是真实尺寸。
        let mut bitmap: BITMAP = zeroed();
        let (source, halve) = if icon.hbmColor.is_null() {
            (icon.hbmMask, true)
        } else {
            (icon.hbmColor, false)
        };
        let measured = GetObjectW(
            source,
            size_of::<BITMAP>() as i32,
            (&mut bitmap as *mut BITMAP).cast(),
        );
        if !icon.hbmMask.is_null() {
            DeleteObject(icon.hbmMask);
        }
        if !icon.hbmColor.is_null() {
            DeleteObject(icon.hbmColor);
        }
        if measured == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
            return None;
        }
        let width = bitmap.bmWidth;
        let height = if halve {
            bitmap.bmHeight / 2
        } else {
            bitmap.bmHeight
        };
        if height <= 0 {
            return None;
        }

        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return None;
        }
        let mut bitmap_info: BITMAPINFO = zeroed();
        bitmap_info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bitmap_info.bmiHeader.biWidth = width;
        bitmap_info.bmiHeader.biHeight = -height;
        bitmap_info.bmiHeader.biPlanes = 1;
        bitmap_info.bmiHeader.biBitCount = 32;
        bitmap_info.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut c_void = null_mut();
        let dib = CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if dib.is_null() || bits.is_null() {
            DeleteDC(dc);
            return None;
        }
        let old = SelectObject(dc, dib);
        let drawn = DrawIconEx(
            dc,
            0,
            0,
            info.hCursor,
            width,
            height,
            0,
            null_mut(),
            DI_NORMAL,
        );
        let raw = std::slice::from_raw_parts(bits as *const u8, (width * height * 4) as usize);
        // 现代 Windows 光标是 32 位带 alpha 的。老式单色光标画出来 alpha 全 0，
        // 这时宁可不贴，也好过糊一个黑块在截图上。
        let pixels = raw.as_chunks::<4>().0;
        let usable = drawn != 0 && pixels.iter().any(|pixel| pixel[3] != 0);
        let mut image = RgbaImage::new(width as u32, height as u32);
        if usable {
            for (index, pixel) in pixels.iter().enumerate() {
                let x = (index as i32) % width;
                let y = (index as i32) / width;
                image.put_pixel(
                    x as u32,
                    y as u32,
                    Rgba([pixel[2], pixel[1], pixel[0], pixel[3]]),
                );
            }
        }
        SelectObject(dc, old);
        DeleteObject(dib);
        DeleteDC(dc);
        if !usable {
            return None;
        }
        Some((
            image,
            info.ptScreenPos.x,
            info.ptScreenPos.y,
            icon.xHotspot as i32,
            icon.yHotspot as i32,
        ))
    }
}

/// 截图像素与逻辑点的比例，以及光标贴图的左上角像素坐标。
/// 与 macOS 的 cursor.rs 同一套算法：先按缩放把「光标相对窗口的位移」换算成像素，
/// 再减掉热点——热点是光标图内对应尖端的那个点，不减会整体偏右下。
fn overlay_origin_px(
    cursor_screen: (i32, i32),
    hot_spot: (i32, i32),
    window_origin: (i32, i32),
    scale: f64,
) -> (i32, i32) {
    let left = (f64::from(cursor_screen.0 - window_origin.0) * scale).round() as i32 - hot_spot.0;
    let top = (f64::from(cursor_screen.1 - window_origin.1) * scale).round() as i32 - hot_spot.1;
    (left, top)
}

/// 把当前光标合成进刚截下来的位图。
///
/// `window_origin` / `logical_width` 是 xcap 报的窗口左上角与逻辑宽度，
/// 用来换算截图像素与点的比例。失败一律返回 Err，由调用方拼成降级说明，
/// 避免用户开了「包含光标」却拿到一张没光标的图而毫不知情。
pub fn overlay_into(
    image: &mut RgbaImage,
    window_origin: (i32, i32),
    logical_width: u32,
) -> Result<(), String> {
    if logical_width == 0 {
        return Err("窗口逻辑宽度为 0，无法换算缩放".into());
    }
    let scale = f64::from(image.width()) / f64::from(logical_width);
    if !(0.5..=4.0).contains(&scale) {
        return Err(format!("截图与窗口的缩放比例异常（{scale:.2}）"));
    }
    let (cursor, screen_x, screen_y, hot_x, hot_y) =
        cursor_bitmap().ok_or("取不到当前系统光标位图")?;
    let (left, top) = overlay_origin_px((screen_x, screen_y), (hot_x, hot_y), window_origin, scale);
    blend_cursor(image, &cursor, i64::from(left), i64::from(top));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_subtracts_hot_spot_and_window_offset() {
        // 光标在 (120,140)，窗口左上角 (100,100)，热点 (4,4)，1x 屏
        assert_eq!(
            overlay_origin_px((120, 140), (4, 4), (100, 100), 1.0),
            (16, 36)
        );
    }

    #[test]
    fn high_dpi_scale_doubles_the_offset() {
        // 同样的点位，200% 缩放下像素偏移要翻倍；热点是图内坐标，不参与缩放
        assert_eq!(
            overlay_origin_px((120, 140), (4, 4), (100, 100), 2.0),
            (36, 76)
        );
    }
}
