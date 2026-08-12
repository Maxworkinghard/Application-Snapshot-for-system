//! 常驻进程的 D-Bus 端点：`windowsnap capture` 通过它触发截图。

use std::sync::mpsc::Sender;

use crate::Msg;

pub const NAME: &str = "local.windowsnap";
pub const PATH: &str = "/local/windowsnap";
pub const INTERFACE: &str = "local.windowsnap.Daemon";

struct Daemon {
    tx: Sender<Msg>,
}

#[zbus::interface(name = "local.windowsnap.Daemon")]
impl Daemon {
    fn capture(&self) {
        let _ = self.tx.send(Msg::Capture);
    }
}

pub fn serve(tx: Sender<Msg>) -> zbus::Result<zbus::blocking::Connection> {
    zbus::blocking::connection::Builder::session()?
        .name(NAME)?
        .serve_at(PATH, Daemon { tx })?
        .build()
}

/// 若常驻进程存在则触发它并返回 true。
pub fn trigger_running_daemon() -> bool {
    let call = || -> zbus::Result<()> {
        let conn = zbus::blocking::Connection::session()?;
        let proxy = zbus::blocking::Proxy::new(&conn, NAME, PATH, INTERFACE)?;
        proxy.call::<_, _, ()>("Capture", &())?;
        Ok(())
    };
    call().is_ok()
}
