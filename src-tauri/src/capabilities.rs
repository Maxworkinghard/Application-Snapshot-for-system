//! 本机能力说明（设置页「本机能力」一栏）。每一项说什么由各平台自己给，见 `os::capabilities`。

use super::os;
use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlatformCapabilities {
    pub(crate) os: String,
    pub(crate) display_server: String,
    pub(crate) recording: CapabilityStatus,
    pub(crate) recording_system_audio: CapabilityStatus,
    pub(crate) recording_microphone: CapabilityStatus,
    pub(crate) ocr: CapabilityStatus,
    pub(crate) autostart: CapabilityStatus,
    /// 滚动长截图只有 Linux/X11 实现，其余平台不下发这一项
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scrolling: Option<CapabilityStatus>,
    pub(crate) include_cursor: CapabilityStatus,
    pub(crate) tray_note: String,
    /// 设置页展示的额外说明（门户忽略项、焦点抢占等）
    pub(crate) notes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityStatus {
    pub(crate) available: bool,
    pub(crate) detail: String,
}

impl CapabilityStatus {
    pub(crate) fn available(detail: impl Into<String>) -> Self {
        Self {
            available: true,
            detail: detail.into(),
        }
    }

    /// 探测成功就用 `detail` 说明这项能力；失败就把失败原因原样给用户看。
    // Windows 的各项能力都是系统自带、不需要运行时探测
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub(crate) fn probe(result: Result<(), String>, detail: impl Into<String>) -> Self {
        match result {
            Ok(()) => Self::available(detail),
            Err(reason) => Self {
                available: false,
                detail: reason,
            },
        }
    }
}

#[tauri::command]
pub(crate) fn platform_capabilities() -> PlatformCapabilities {
    os::capabilities()
}
