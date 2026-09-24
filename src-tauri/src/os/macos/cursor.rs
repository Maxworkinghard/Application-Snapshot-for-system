//! 静帧截图合成鼠标光标。
//!
//! xcap 的窗口位图不含光标，录制那条路有 ScreenCaptureKit 的 `showsCursor`，静帧
//! 没有对应开关，只能截完再自己画上去——与 Windows 侧同样的做法。

use crate::capture::blend_cursor;
use image::RgbaImage;
use objc2::AnyThread;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSCursor, NSDeviceRGBColorSpace, NSGraphicsContext,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CFRelease(cf: *const c_void);
}

/// 光标位置：全局坐标、左上原点、点为单位——正好是 xcap 报窗口位置用的坐标系。
///
/// 不用 `NSEvent::mouseLocation`：那个是左下原点，翻 y 要主屏高度，而 `NSScreen`
/// 在 objc2 里需要 `MainThreadMarker`，截图不一定跑在主线程上。
fn cursor_position() -> Option<(f64, f64)> {
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event);
        Some((point.x, point.y))
    }
}

/// 当前系统光标：按 `scale` 渲染成像素位图，附带热点（点单位，左上原点）。
///
/// 走 PNG 中转而不是直接读 `bitmapData`：省掉一段裸指针算术，光标只有几十像素，
/// 这点编解码开销可以忽略。
// currentSystemCursor 已被苹果标记弃用，推荐改用 ScreenCaptureKit 的
// SCStreamConfiguration.showsCursor。这里仍然用它，因为替代品 NSCursor::currentCursor
// 只知道**本应用**的光标：截别人的窗口时它返回我们自己的箭头，而不是对方正在显示的
// I 形/手形光标，语义是错的。录制已使用 ScreenCaptureKit；静帧仍需单独迁移。
#[allow(deprecated)]
fn cursor_bitmap(scale: f64) -> Option<(RgbaImage, f64, f64)> {
    unsafe {
        let cursor = NSCursor::currentSystemCursor()?;
        let hot_spot = cursor.hotSpot();
        let nsimage = cursor.image();
        let size = nsimage.size();
        if size.width < 1.0 || size.height < 1.0 {
            return None;
        }
        let pixel_width = (size.width * scale).round().max(1.0);
        let pixel_height = (size.height * scale).round().max(1.0);
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            pixel_width as isize,
            pixel_height as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )?;
        let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&rep)?;
        NSGraphicsContext::saveGraphicsState_class();
        NSGraphicsContext::setCurrentContext(Some(&context));
        nsimage.drawInRect(NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(pixel_width, pixel_height),
        ));
        NSGraphicsContext::restoreGraphicsState_class();
        let png = rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())?;
        let decoded = image::load_from_memory(&png.to_vec()).ok()?.to_rgba8();
        Some((decoded, hot_spot.x, hot_spot.y))
    }
}

/// 光标左上角落在截图里的像素坐标：先减热点，再把「点」按 DPI 比例换成像素。
fn overlay_origin_px(
    cursor: (f64, f64),
    hot_spot: (f64, f64),
    window_origin: (i32, i32),
    scale: f64,
) -> (i64, i64) {
    let left = (cursor.0 - hot_spot.0 - window_origin.0 as f64) * scale;
    let top = (cursor.1 - hot_spot.1 - window_origin.1 as f64) * scale;
    (left.round() as i64, top.round() as i64)
}

/// 把当前光标合成进刚截下来的位图。
///
/// `window_origin` / `logical_width` 是 xcap 报的窗口左上角与逻辑宽度（点），
/// 用来换算截图像素与点的比例（Retina 上是 2）。光标不在这张图范围内时什么都不做。
pub fn overlay_into(
    image: &mut RgbaImage,
    window_origin: (i32, i32),
    logical_width: u32,
) -> Result<(), String> {
    if logical_width == 0 {
        return Err("窗口逻辑宽度为 0，无法换算缩放".into());
    }
    let scale = image.width() as f64 / logical_width as f64;
    if !(0.5..=4.0).contains(&scale) {
        return Err(format!("截图与窗口的缩放比例异常（{scale:.2}）"));
    }
    let position = cursor_position().ok_or("取不到光标位置")?;
    let (bitmap, hot_x, hot_y) = cursor_bitmap(scale).ok_or("取不到当前系统光标位图")?;
    let (left, top) = overlay_origin_px(position, (hot_x, hot_y), window_origin, scale);
    blend_cursor(image, &bitmap, left, top);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_subtracts_hot_spot_and_window_offset() {
        // 光标在 (120,140)，窗口左上角 (100,100)，热点 (4,4)，1x 屏
        assert_eq!(
            overlay_origin_px((120.0, 140.0), (4.0, 4.0), (100, 100), 1.0),
            (16, 36)
        );
    }

    #[test]
    fn retina_scale_doubles_the_offset() {
        // 同样的点位，2x 屏上像素偏移要翻倍
        assert_eq!(
            overlay_origin_px((120.0, 140.0), (4.0, 4.0), (100, 100), 2.0),
            (32, 72)
        );
    }
}
