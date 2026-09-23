use super::*;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlatformCapabilities {
    os: String,
    display_server: String,
    recording: CapabilityStatus,
    ocr: CapabilityStatus,
    autostart: CapabilityStatus,
    /// 滚动长截图只有 Linux/X11 实现，其余平台不暴露这一项
    #[cfg(target_os = "linux")]
    scrolling: CapabilityStatus,
    include_cursor: CapabilityStatus,
    tray_note: String,
    /// 设置页展示的额外说明（门户忽略项、焦点抢占等）
    notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityStatus {
    available: bool,
    detail: String,
}

#[cfg(target_os = "macos")]
const MACOS_RECORDER_HELPER: &str = "snapshot-recorder";

#[cfg(target_os = "macos")]
pub(crate) fn macos_recorder_program() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        let bundled = exe.with_file_name(MACOS_RECORDER_HELPER);
        if bundled.is_file() {
            return bundled;
        }
    }

    let triple = if cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else {
        "x86_64-apple-darwin"
    };
    let prepared =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("{MACOS_RECORDER_HELPER}-{triple}"));
    if prepared.is_file() {
        return prepared;
    }

    PathBuf::from(MACOS_RECORDER_HELPER)
}

#[cfg(target_os = "macos")]
fn macos_recorder_capability() -> Result<(), String> {
    let output = Command::new(macos_recorder_program())
        .arg("--probe")
        .output()
        .map_err(|_| "未找到 ScreenCaptureKit 录制组件，请重新安装应用".to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err("ScreenCaptureKit 录制组件不可用，请重新安装应用".into())
    }
}

// 每个平台的 cfg 块独立返回；显式 return 避免函数结果依赖当前被编译的分支。
#[allow(clippy::needless_return)]
#[tauri::command]
pub(crate) fn platform_capabilities() -> PlatformCapabilities {
    #[cfg(target_os = "linux")]
    {
        let recording = match linux::recording_available() {
            Ok(()) => CapabilityStatus {
                available: true,
                detail: linux::recording_capability_detail(),
            },
            Err(detail) => CapabilityStatus {
                available: false,
                detail,
            },
        };
        let ocr_report = ocr::report();
        let ocr = CapabilityStatus {
            available: ocr_report.available,
            detail: ocr_report.detail,
        };
        let autostart = match linux::autostart_capability() {
            Ok(()) => CapabilityStatus {
                available: true,
                detail: "写入 XDG autostart（~/.config/autostart，opt-in）".into(),
            },
            Err(detail) => CapabilityStatus {
                available: false,
                detail,
            },
        };
        let scrolling = if linux::is_wayland_session() || std::env::var_os("DISPLAY").is_none() {
            CapabilityStatus {
                available: false,
                detail: "滚动长截图需要 X11/`$DISPLAY`（xdotool）；纯 Wayland 不支持".into(),
            }
        } else {
            CapabilityStatus {
                available: true,
                detail: "X11 下 xcap 连拍 + xdotool 翻页拼接（需 xdotool）".into(),
            }
        };
        let include_cursor = CapabilityStatus {
            available: true,
            detail: "静帧：ffmpeg x11grab 带光标（失败则回退无光标并在成功提示中说明）；录制：x11grab 尊重开关，portal 路径忽略".into(),
        };
        return PlatformCapabilities {
            os: "linux".into(),
            display_server: linux::display_server_label().into(),
            recording,
            ocr,
            autostart,
            scrolling,
            include_cursor,
            tray_note: "托盘菜单（打开设置 / 退出）在有 StatusNotifierHost 时可用（KDE 原生；GNOME 需 AppIndicator 扩展）。缺失时应用仍可运行。左键打开主窗口：Windows/macOS 支持；Linux 本 Tauri/tray-icon 0.24（libayatana-appindicator）无点击回调，通常仅菜单可用。Wayland 下托盘/透明宠物表现依赖合成器，需本机验证。".into(),
            notes: vec![
                "Linux portal 录制忽略 target_id 与 include_cursor（由桌面选择器/合成器决定）；启动时会提示重新选窗/屏".into(),
                "Linux 还原最小化用 xdotool（先 windowmap 再 windowactivate），会抢焦点".into(),
                "Linux 带光标静帧需要 ffmpeg；不可用时回退为无光标截图，并在成功提示中告知".into(),
                "Wayland（GNOME/KDE）下 portal 录制、托盘与宠物透明需在对应合成器上本机验证".into(),
            ],
        };
    }
    #[cfg(target_os = "windows")]
    {
        let ocr_report = ocr::report();
        let recording = CapabilityStatus {
            available: true,
            detail: "Windows.Graphics.Capture + Media Foundation（按窗口句柄采集，不依赖 ffmpeg）"
                .into(),
        };
        return PlatformCapabilities {
            os: "windows".into(),
            display_server: "Win32".into(),
            recording,
            ocr: CapabilityStatus {
                available: ocr_report.available,
                detail: ocr_report.detail,
            },
            autostart: CapabilityStatus {
                available: true,
                detail: "写入 HKCU\\...\\CurrentVersion\\Run（当前用户，opt-in）".into(),
            },
            include_cursor: CapabilityStatus {
                available: true,
                detail: "录制：WGC SetIsCursorCaptureEnabled；静帧：按热点合成系统光标".into(),
            },
            tray_note: "NotifyIcon：左键/双击打开主窗口；右键菜单打开设置/退出。".into(),
            notes: vec!["静帧光标只支持 32 位带 alpha 的现代光标；老式单色光标会跳过合成".into()],
        };
    }
    #[cfg(target_os = "macos")]
    {
        let ocr_report = ocr::report();
        let recording = match macos_recorder_capability() {
            Ok(()) => CapabilityStatus {
                available: true,
                detail: "ScreenCaptureKit 原生窗口流（不受遮挡，支持副屏与窗口移动；首次使用会请求屏幕录制权限）".into(),
            },
            Err(detail) => CapabilityStatus {
                available: false,
                detail,
            },
        };
        return PlatformCapabilities {
            os: "macos".into(),
            display_server: "AppKit".into(),
            recording,
            ocr: CapabilityStatus {
                available: ocr_report.available,
                detail: ocr_report.detail,
            },
            autostart: match mac_autostart::autostart_capability() {
                Ok(()) => CapabilityStatus {
                    available: true,
                    detail: "LaunchAgent：~/Library/LaunchAgents/com.appsnapshot.prompt-pet-shortcut.plist，下次登录生效".into(),
                },
                Err(detail) => CapabilityStatus { available: false, detail },
            },
            include_cursor: CapabilityStatus {
                available: true,
                detail: "录制：ScreenCaptureKit showsCursor；静帧截图：截后按热点与 DPI 比例合成当前系统光标".into(),
            },
            tray_note: "NSStatusItem：左键打开主窗口；菜单打开设置/退出。".into(),
            notes: vec![
                "macOS 还原最小化窗口按 AXTitle 对齐；标题对不上且该应用有多个最小化窗口时，不代劳、提示手动还原".into(),
                "窗口录制使用 ScreenCaptureKit，仅录目标窗口，不包含系统音频或麦克风".into(),
            ],
        };
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
    {
        PlatformCapabilities {
            os: std::env::consts::OS.into(),
            display_server: "unknown".into(),
            recording: CapabilityStatus {
                available: false,
                detail: "未支持".into(),
            },
            ocr: CapabilityStatus {
                available: false,
                detail: "未支持".into(),
            },
            autostart: CapabilityStatus {
                available: false,
                detail: "未支持".into(),
            },
            include_cursor: CapabilityStatus {
                available: false,
                detail: "未支持".into(),
            },
            tray_note: String::new(),
            notes: vec![],
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) mod mac_autostart {
    //! macOS 开机自启（opt-in）：在 `~/Library/LaunchAgents/` 放 / 删一份 LaunchAgent plist。
    //!
    //! 与 Linux 的 XDG autostart 同样是「用户勾选后才生效」，且同样只落文件、不主动
    //! `launchctl bootstrap`——RunAtLoad 的 agent 一旦 bootstrap 会立刻把应用再拉起来一份，
    //! 勾选设置的当下多出一个实例不是用户要的。写进去的条目在下次登录时生效。

    use std::fs;
    use std::path::PathBuf;

    const LABEL: &str = "com.appsnapshot.prompt-pet-shortcut";

    fn agents_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/LaunchAgents")
    }

    fn plist_path() -> PathBuf {
        agents_dir().join(format!("{LABEL}.plist"))
    }

    /// plist 是 XML，可执行文件路径里的 `&`/`<`/`>` 必须转义，否则写出来的是坏文件。
    fn xml_escape(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn plist_body(exe: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
            exe = xml_escape(exe)
        )
    }

    /// 按设置开关写入或删除 LaunchAgent。
    pub fn apply_launch_on_boot(enabled: bool) -> Result<(), String> {
        let path = plist_path();
        if !enabled {
            if path.exists() {
                fs::remove_file(&path).map_err(|error| format!("无法关闭开机自启：{error}"))?;
            }
            return Ok(());
        }

        let exe = std::env::current_exe()
            .map_err(|error| format!("无法定位当前程序：{error}"))?
            // .app 里启动时 current_exe 可能带 symlink，落进 plist 的要是真实路径
            .canonicalize()
            .map_err(|error| format!("无法解析程序路径：{error}"))?;
        fs::create_dir_all(agents_dir())
            .map_err(|error| format!("无法创建 ~/Library/LaunchAgents：{error}"))?;
        fs::write(&path, plist_body(&exe.to_string_lossy()))
            .map_err(|error| format!("无法写入开机自启项：{error}"))?;
        Ok(())
    }

    /// 探测 `~/Library/LaunchAgents` 是否可写（给能力面板用）。
    pub fn autostart_capability() -> Result<(), String> {
        let dir = agents_dir();
        fs::create_dir_all(&dir).map_err(|error| {
            format!(
                "无法创建 ~/Library/LaunchAgents（{error}）；仍可保存偏好，但系统自启项写不进去"
            )
        })?;
        let probe = dir.join(".snapshot-autostart-write-probe");
        fs::write(&probe, b"ok").map_err(|error| {
            format!(
                "无法写入 ~/Library/LaunchAgents（{error}）；仍可保存偏好，但系统自启项写不进去"
            )
        })?;
        let _ = fs::remove_file(&probe);
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn plist_declares_label_and_run_at_load() {
            let body = plist_body("/Applications/snapshot.app/Contents/MacOS/snapshot");
            assert!(body.contains("<string>com.appsnapshot.prompt-pet-shortcut</string>"));
            assert!(body.contains("<key>RunAtLoad</key>"));
            assert!(body
                .contains("<string>/Applications/snapshot.app/Contents/MacOS/snapshot</string>"));
        }

        #[test]
        fn exe_path_is_xml_escaped() {
            // 目录名里带 & 的真实场景：用户把 .app 放在「Tools & Toys」之类的目录下
            let body = plist_body("/Users/me/Tools & Toys/snapshot.app/Contents/MacOS/snapshot");
            assert!(body.contains("Tools &amp; Toys"));
            assert!(!body.contains("Tools & Toys"));
        }

        #[test]
        fn plist_path_sits_in_launch_agents() {
            let path = plist_path();
            assert!(
                path.ends_with("Library/LaunchAgents/com.appsnapshot.prompt-pet-shortcut.plist")
            );
        }
    }
}

/// macOS：NSRunningApplication.icon → 64×64 PNG data URL。
/// 拿到的是 PNG 字节，直接 base64，不必像 Windows 那样走 RgbaImage。
#[cfg(target_os = "macos")]
pub(crate) mod mac_icon {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    use objc2::AnyThread;
    use objc2_app_kit::{
        NSBitmapImageFileType, NSBitmapImageRep, NSDeviceRGBColorSpace, NSGraphicsContext, NSImage,
        NSRunningApplication,
    };
    use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize};

    pub fn png_data_url_for_pid(pid: u32) -> Option<String> {
        unsafe {
            let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)?;
            let icon = app.icon()?;
            let png = downscale_to_png(&icon)?;
            Some(format!("data:image/png;base64,{}", BASE64.encode(png)))
        }
    }

    /// 图标的 TIFF 里带全套尺寸（最大 1024×1024）；setSize 只改逻辑尺寸、
    /// 动不了 TIFFRepresentation 里的像素。要真压到 64×64，得把它画进
    /// 一个新的 64×64 bitmap rep 再导出。
    unsafe fn downscale_to_png(icon: &NSImage) -> Option<Vec<u8>> {
        let rep = NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            64,
            64,
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
        icon.drawInRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(64.0, 64.0)));
        NSGraphicsContext::restoreGraphicsState_class();
        let png = rep
            .representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())?;
        Some(png.to_vec())
    }
}

/// Windows 开机自启：在 HKCU 的 Run 键下写一个值。
/// 不走「启动」文件夹的 .lnk —— 那需要 COM IShellLink，而且用户手动删掉快捷方式后
/// 设置项仍显示开启，状态会和系统对不上。注册表读写都在当前用户下，无需提权。
#[cfg(target_os = "windows")]
pub(crate) mod windows_autostart {
    use std::{ffi::OsStr, iter::once, os::windows::ffi::OsStrExt};
    use windows_sys::Win32::System::Registry::{
        RegDeleteKeyValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ,
    };

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    /// 注册表值名。改名会在用户机器上留下卸不掉的旧值，所以固定不动。
    const VALUE_NAME: &str = "AppSnapshot";
    /// ERROR_FILE_NOT_FOUND：值本来就不存在，删除按成功处理。
    const ERROR_FILE_NOT_FOUND: u32 = 2;

    fn wide(text: &str) -> Vec<u16> {
        OsStr::new(text).encode_wide().chain(once(0)).collect()
    }

    /// 可执行文件路径要带引号：路径含空格时，不加引号会被 Windows 拆成程序名 + 参数。
    fn command_line() -> Result<Vec<u16>, String> {
        let exe =
            std::env::current_exe().map_err(|error| format!("无法定位可执行文件：{error}"))?;
        Ok(wide(&format!("\"{}\"", exe.display())))
    }

    pub fn apply(enabled: bool) -> Result<(), String> {
        let key = wide(RUN_KEY);
        let name = wide(VALUE_NAME);
        let status = if enabled {
            let value = command_line()?;
            // REG_SZ 的字节数要含结尾的 NUL，少算会让读取方拿到没有终止符的串。
            let bytes = (value.len() * 2) as u32;
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    name.as_ptr(),
                    REG_SZ,
                    value.as_ptr().cast(),
                    bytes,
                )
            }
        } else {
            let status =
                unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) };
            if status == ERROR_FILE_NOT_FOUND {
                0
            } else {
                status
            }
        };
        if status != 0 {
            let action = if enabled { "写入" } else { "移除" };
            return Err(format!("无法{action}开机启动项（注册表错误 {status}）"));
        }
        Ok(())
    }
}

/// Windows 可执行文件图标提取，用于窗口列表和上一应用显示。
#[cfg(target_os = "windows")]
pub(crate) mod windows_icon {
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

    pub fn icon_for_process(pid: u32) -> Option<RgbaImage> {
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

            const SIZE: i32 = 256;
            let mut extracted_icon = null_mut();
            let mut icon_id = 0u32;
            let extracted = PrivateExtractIconsW(
                path.as_ptr(),
                0,
                SIZE,
                SIZE,
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
            bitmap_info.bmiHeader.biWidth = SIZE;
            bitmap_info.bmiHeader.biHeight = -SIZE;
            bitmap_info.bmiHeader.biPlanes = 1;
            bitmap_info.bmiHeader.biBitCount = 32;
            bitmap_info.bmiHeader.biCompression = BI_RGB;
            let mut bits: *mut c_void = null_mut();
            let bitmap =
                CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                DeleteDC(dc);
                DestroyIcon(icon);
                return None;
            }
            let old = SelectObject(dc, bitmap);
            let _ = DrawIconEx(dc, 0, 0, icon, SIZE, SIZE, 0, null_mut(), DI_NORMAL);
            let raw = std::slice::from_raw_parts(bits as *const u8, (SIZE * SIZE * 4) as usize);
            let mut image = RgbaImage::new(SIZE as u32, SIZE as u32);
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
                let x = (index as u32) % SIZE as u32;
                let y = (index as u32) / SIZE as u32;
                image.put_pixel(x, y, Rgba([pixel[2], pixel[1], pixel[0], alpha]));
            }
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
            DestroyIcon(icon);
            Some(image)
        }
    }
}
