//! 本机能力（偏好设置「本机与模型」里的可用 / 不可用）。每一项能不能用由各平台自己判断，见 `os::capabilities`。

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
    pub(crate) autostart: CapabilityStatus,
    /// 滚动长截图只有 Linux/X11 实现，其余平台不下发这一项
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) scrolling: Option<CapabilityStatus>,
    pub(crate) include_cursor: CapabilityStatus,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CapabilityStatus {
    pub(crate) available: bool,
}

impl CapabilityStatus {
    pub(crate) fn yes() -> Self {
        Self { available: true }
    }

    // Windows 的各项能力都是系统自带、不需要运行时探测
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub(crate) fn no() -> Self {
        Self { available: false }
    }

    /// 探测成功就是可用；失败原因界面上不再展示，只看能不能用
    #[cfg_attr(target_os = "windows", allow(dead_code))]
    pub(crate) fn probe<E>(result: Result<(), E>) -> Self {
        Self {
            available: result.is_ok(),
        }
    }
}

#[tauri::command]
pub(crate) fn platform_capabilities() -> PlatformCapabilities {
    os::capabilities()
}
