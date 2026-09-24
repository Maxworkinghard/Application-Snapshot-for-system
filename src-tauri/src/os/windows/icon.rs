//! Windows 可执行文件图标提取，用于窗口列表和上一应用显示。

use image::{Rgba, RgbaImage};
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
    },
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::{
        Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON},
        WindowsAndMessaging::{DestroyIcon, DrawIconEx, PrivateExtractIconsW, DI_NORMAL},
    },
};

/// 进程图标，编码成 PNG（媒体协议 icon/<pid>/<边长> 用）。
pub(crate) fn app_icon_png(pid: u32, size: u32) -> Option<Vec<u8>> {
    crate::capture::encode_png(&icon_for_process(pid, size)?).ok()
}

/// `size` 是想要的像素边长：窗口列表只显示 24px，桌宠最大 68px，没必要按 256px 取。
fn icon_for_process(pid: u32, size: u32) -> Option<RgbaImage> {
    let size = size as i32;
    unsafe {
        let process: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let mut path = vec![0u16; 32768];
        let mut len = path.len() as u32;
        let ok = QueryFullProcessImageNameW(process, 0, path.as_mut_ptr(), &mut len);
        CloseHandle(process);
        if ok == 0 {
            return None;
        }
        path.truncate(len as usize);
        path.push(0);

        let mut extracted_icon = null_mut();
        let mut icon_id = 0u32;
        let extracted = PrivateExtractIconsW(
            path.as_ptr(),
            0,
            size,
            size,
            &mut extracted_icon,
            &mut icon_id,
            1,
            0,
        );
        let icon = if extracted > 0 && extracted != u32::MAX && !extracted_icon.is_null() {
            extracted_icon
        } else {
            let mut info: SHFILEINFOW = zeroed();
            let result = SHGetFileInfoW(
                path.as_ptr(),
                0,
                &mut info,
                size_of::<SHFILEINFOW>() as u32,
                SHGFI_ICON | SHGFI_LARGEICON,
            );
            if result == 0 || info.hIcon.is_null() {
                return None;
            }
            info.hIcon
        };

        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            DestroyIcon(icon);
            return None;
        }
        let mut bitmap_info: BITMAPINFO = zeroed();
        bitmap_info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
        bitmap_info.bmiHeader.biWidth = size;
        bitmap_info.bmiHeader.biHeight = -size;
        bitmap_info.bmiHeader.biPlanes = 1;
        bitmap_info.bmiHeader.biBitCount = 32;
        bitmap_info.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if bitmap.is_null() || bits.is_null() {
            DeleteDC(dc);
            DestroyIcon(icon);
            return None;
        }
        let old = SelectObject(dc, bitmap);
        let _ = DrawIconEx(dc, 0, 0, icon, size, size, 0, null_mut(), DI_NORMAL);
        let raw = std::slice::from_raw_parts(bits as *const u8, (size * size * 4) as usize);
        let mut image = RgbaImage::new(size as u32, size as u32);
        let pixels = raw.as_chunks::<4>().0;
        let has_alpha = pixels.iter().any(|pixel| pixel[3] != 0);
        for (index, pixel) in pixels.iter().enumerate() {
            let alpha = if has_alpha {
                pixel[3]
            } else if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
                0
            } else {
                255
            };
            let x = (index as u32) % size as u32;
            let y = (index as u32) / size as u32;
            image.put_pixel(x, y, Rgba([pixel[2], pixel[1], pixel[0], alpha]));
        }
        SelectObject(dc, old);
        DeleteObject(bitmap);
        DeleteDC(dc);
        DestroyIcon(icon);
        Some(image)
    }
}
