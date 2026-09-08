//! 托盘图标（StatusNotifierItem 协议，ksni 实现）。
//! GNOME 默认不显示 SNI 托盘（需 AppIndicator 扩展），KDE / 其他桌面原生支持；
//! 注册失败不影响主流程。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;

use crate::Msg;

struct Tray {
    tx: Sender<Msg>,
    shortcut: String,
    /// 是否正在录制（菜单据此切换「录制当前窗口 / 停止录制」）。
    recording: Arc<AtomicBool>,
    /// 是否正在润色（菜单据此切换「润色 Prompt / 停止润色」）。
    polishing: Arc<AtomicBool>,
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        "windowsnap".into()
    }

    fn title(&self) -> String {
        "应用快照".into()
    }

    fn icon_name(&self) -> String {
        "camera-photo".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        let recording = self.recording.load(Ordering::SeqCst);
        let polishing = self.polishing.load(Ordering::SeqCst);

        let capture = StandardItem {
            label: format!("截取当前应用窗口（{}）", self.shortcut),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.tx.send(Msg::Capture);
            }),
            ..Default::default()
        }
        .into();

        let list = StandardItem {
            label: "应用快照…（选择窗口）".into(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.tx.send(Msg::CaptureList);
            }),
            ..Default::default()
        }
        .into();

        let record = if recording {
            StandardItem {
                label: "停止录制".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(Msg::StopRecording);
                }),
                ..Default::default()
            }
        } else {
            StandardItem {
                label: "录制当前窗口".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(Msg::StartRecording);
                }),
                ..Default::default()
            }
        }
        .into();

        let polish = if polishing {
            StandardItem {
                label: "停止润色".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(Msg::StopPolish);
                }),
                ..Default::default()
            }
        } else {
            StandardItem {
                label: "润色 Prompt".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(Msg::Polish);
                }),
                ..Default::default()
            }
        }
        .into();

        let undo = StandardItem {
            label: "撤销润色".into(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.tx.send(Msg::UndoPolish);
            }),
            ..Default::default()
        }
        .into();

        let quit = StandardItem {
            label: "退出应用快照".into(),
            activate: Box::new(|tray: &mut Self| {
                let _ = tray.tx.send(Msg::Quit);
            }),
            ..Default::default()
        }
        .into();

        vec![
            capture,
            list,
            record,
            polish,
            undo,
            ksni::MenuItem::Separator,
            quit,
        ]
    }
}

pub fn spawn(
    tx: Sender<Msg>,
    shortcut: String,
    recording: Arc<AtomicBool>,
    polishing: Arc<AtomicBool>,
) {
    use ksni::blocking::TrayMethods;
    match (Tray {
        tx,
        shortcut,
        recording,
        polishing,
    })
    .spawn()
    {
        // Handle 只是控制柄，泄漏它让托盘活到进程结束
        Ok(handle) => std::mem::forget(handle),
        Err(e) => eprintln!("windowsnap: 托盘不可用（{e}），不影响快捷键与截图"),
    }
}
