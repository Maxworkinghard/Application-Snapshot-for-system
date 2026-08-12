//! 托盘图标（StatusNotifierItem 协议，ksni 实现）。
//! GNOME 默认不显示 SNI 托盘（需 AppIndicator 扩展），KDE / 其他桌面原生支持；
//! 注册失败不影响主流程。

use std::sync::mpsc::Sender;

use crate::Msg;

struct Tray {
    tx: Sender<Msg>,
    shortcut: String,
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
        vec![
            StandardItem {
                label: format!("截取当前应用窗口（{}）", self.shortcut),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(Msg::Capture);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "退出应用快照".into(),
                activate: Box::new(|tray: &mut Self| {
                    let _ = tray.tx.send(Msg::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(tx: Sender<Msg>, shortcut: String) {
    use ksni::blocking::TrayMethods;
    match (Tray { tx, shortcut }).spawn() {
        // Handle 只是控制柄，泄漏它让托盘活到进程结束
        Ok(handle) => std::mem::forget(handle),
        Err(e) => eprintln!("windowsnap: 托盘不可用（{e}），不影响快捷键与截图"),
    }
}
