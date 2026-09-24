//! 窗口操作：用 Accessibility API 还原最小化的窗口。

use core_foundation::{
    array::CFArray,
    base::{CFType, TCFType},
    boolean::CFBoolean,
    string::CFString,
};
use std::{ffi::c_void, os::raw::c_int, ptr, thread, time::Duration};

/// AX 没有 Windows hwnd 那样的窗口句柄，只能按进程 + 标题对齐，详见 [`unminimize_window`]。
pub(crate) fn restore_minimized(id: u32) -> Result<(), String> {
    unminimize_window(id)?;
    // Dock 还原动画比 Windows 的 SW_RESTORE 慢，多等一会再截
    thread::sleep(Duration::from_millis(400));
    Ok(())
}

type AXUIElementRef = *const c_void;

extern "C" {
    fn AXIsProcessTrusted() -> bool;
    fn AXUIElementCreateApplication(pid: c_int) -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *mut *const c_void,
    ) -> c_int;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *const c_void,
    ) -> c_int;
    fn CFRelease(cf: *const c_void);
}

pub fn unminimize_window(window_id: u32) -> Result<(), String> {
    let target = xcap::Window::all()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|window| window.id().ok() == Some(window_id))
        .ok_or_else(|| "目标窗口已关闭".to_string())?;
    let pid = target.pid().map_err(|error| error.to_string())?;
    let title = target.title().unwrap_or_default();
    unsafe {
        if !AXIsProcessTrusted() {
            return Err(
                "还原最小化窗口需要「辅助功能」权限，请在系统设置 → 隐私与安全性中授权后重试"
                    .into(),
            );
        }
        let app = AXUIElementCreateApplication(pid as c_int);
        if app.is_null() {
            return Err("无法访问目标应用".to_string());
        }
        let result = unminimize_app_windows(app, &title);
        CFRelease(app);
        result
    }
}

/// Windows 的 SW_RESTORE 只还原被指向的那一个窗口；AX 这边没有窗口句柄，
/// 用 xcap 侧的标题去对 AXTitle，尽量只还原被截的那个。
/// 标题为空或对不上时（个别应用不暴露 AXTitle），退回还原该进程全部最小化窗口。
/// 标题对不上时的去向。
///
/// 原先是「一律还原该进程全部最小化窗口」——只有一个最小化窗口时这和还原那一个
/// 是同一件事，但有好几个时会把用户收起来的窗口一并掀开，而且多半还猜错。
/// 所以只在唯一确定时代劳，含糊时交回给用户，向 Windows 的 SW_RESTORE
/// 「只动被指向的那一个」靠拢。
#[derive(Debug, PartialEq, Eq)]
enum Fallback {
    /// 只有一个最小化窗口，不会猜错
    RestoreOnly(isize),
    /// 没有最小化的窗口，无事可做
    NothingToDo,
    /// 多个最小化窗口且标题对不上，不替用户决定
    Ambiguous(usize),
}

fn decide_fallback(minimized: &[isize]) -> Fallback {
    match minimized {
        [] => Fallback::NothingToDo,
        [only] => Fallback::RestoreOnly(*only),
        many => Fallback::Ambiguous(many.len()),
    }
}

unsafe fn unminimize_app_windows(app: AXUIElementRef, title: &str) -> Result<(), String> {
    let attr_windows = CFString::new("AXWindows");
    let mut raw: *const c_void = ptr::null();
    let err = AXUIElementCopyAttributeValue(
        app,
        attr_windows.as_concrete_TypeRef() as *const c_void,
        &mut raw,
    );
    if err != 0 || raw.is_null() {
        return Err("无法读取目标应用的窗口列表".into());
    }
    let windows: CFArray<CFType> = CFArray::wrap_under_create_rule(raw as _);
    let attr_minimized = CFString::new("AXMinimized");

    // 先扫一遍：标题命中哪些、哪些确实处于最小化
    let mut title_hits: Vec<isize> = Vec::new();
    let mut minimized: Vec<isize> = Vec::new();
    for index in 0..windows.len() {
        let Some(item) = windows.get(index) else {
            continue;
        };
        let element = item.as_concrete_TypeRef() as AXUIElementRef;
        if !title.is_empty() && ax_string(element, "AXTitle").as_deref() == Some(title) {
            title_hits.push(index);
        }
        if is_minimized(element, &attr_minimized) {
            minimized.push(index);
        }
    }

    let targets: Vec<isize> = if title_hits.is_empty() {
        match decide_fallback(&minimized) {
            Fallback::NothingToDo => return Ok(()),
            Fallback::RestoreOnly(index) => vec![index],
            Fallback::Ambiguous(count) => {
                return Err(format!(
                "目标窗口已最小化，但该应用有 {count} 个最小化窗口、且系统没有报告可用的窗口标题，\
                     无法确定是哪一个。请先手动还原目标窗口再截图。"
            ))
            }
        }
    } else {
        // 标题命中的里面只还原确实最小化的那些
        title_hits
            .into_iter()
            .filter(|index| minimized.contains(index))
            .collect()
    };

    for index in targets {
        let Some(item) = windows.get(index) else {
            continue;
        };
        let element = item.as_concrete_TypeRef() as AXUIElementRef;
        let _ = AXUIElementSetAttributeValue(
            element,
            attr_minimized.as_concrete_TypeRef() as *const c_void,
            CFBoolean::false_value().as_concrete_TypeRef() as *const c_void,
        );
    }
    Ok(())
}

unsafe fn is_minimized(element: AXUIElementRef, attr_minimized: &CFString) -> bool {
    let mut value: *const c_void = ptr::null();
    if AXUIElementCopyAttributeValue(
        element,
        attr_minimized.as_concrete_TypeRef() as *const c_void,
        &mut value,
    ) != 0
        || value.is_null()
    {
        return false;
    }
    CFBoolean::wrap_under_create_rule(value as _) == CFBoolean::true_value()
}

unsafe fn ax_string(element: AXUIElementRef, attribute: &str) -> Option<String> {
    let name = CFString::new(attribute);
    let mut raw: *const c_void = ptr::null();
    if AXUIElementCopyAttributeValue(
        element,
        name.as_concrete_TypeRef() as *const c_void,
        &mut raw,
    ) != 0
        || raw.is_null()
    {
        return None;
    }
    Some(CFString::wrap_under_create_rule(raw as _).to_string())
}

#[cfg(test)]
mod tests {
    use super::{decide_fallback, Fallback};

    #[test]
    fn single_minimized_window_is_unambiguous() {
        assert_eq!(decide_fallback(&[2]), Fallback::RestoreOnly(2));
    }

    #[test]
    fn nothing_minimized_is_a_no_op() {
        assert_eq!(decide_fallback(&[]), Fallback::NothingToDo);
    }

    #[test]
    fn several_minimized_windows_are_left_to_the_user() {
        // 原先这种情况会把三个窗口全掀开
        assert_eq!(decide_fallback(&[0, 1, 4]), Fallback::Ambiguous(3));
    }
}
