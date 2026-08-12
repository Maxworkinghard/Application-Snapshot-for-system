//! X11 后端：全局快捷键（XGrabKey）、活动窗口截图（root 裁剪）、
//! 原生 CLIPBOARD selection 所有权（供数 image/png）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, ConnectionExt, CreateWindowAux, EventMask, GrabMode,
    ImageFormat, ImageOrder, ModMask, PropMode, SelectionNotifyEvent, Window, WindowClass,
    SELECTION_NOTIFY_EVENT,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

use crate::{Msg, Result};

x11rb::atom_manager! {
    Atoms:
    AtomsCookie {
        CLIPBOARD,
        TARGETS,
        INCR,
        IMAGE_PNG: b"image/png",
        NET_ACTIVE_WINDOW: b"_NET_ACTIVE_WINDOW",
        NET_FRAME_EXTENTS: b"_NET_FRAME_EXTENTS",
        GTK_FRAME_EXTENTS: b"_GTK_FRAME_EXTENTS",
        WM_CLASS,
    }
}

pub struct X11Backend {
    conn: RustConnection,
    screen_num: usize,
    atoms: Atoms,
    clip_win: Window,
    /// 当前供数的 PNG 内容
    content: Mutex<Option<Arc<Vec<u8>>>>,
    /// 是否仍持有 CLIPBOARD 所有权
    owned: AtomicBool,
}

impl X11Backend {
    pub fn new() -> Result<Arc<Self>> {
        let (conn, screen_num) = x11rb::connect(None)?;
        let atoms = Atoms::new(&conn)?.reply()?;
        let root = conn.setup().roots[screen_num].root;

        let clip_win = conn.generate_id()?;
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            clip_win,
            root,
            -1,
            -1,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &CreateWindowAux::new(),
        )?
        .check()?;
        conn.flush()?;

        Ok(Arc::new(Self {
            conn,
            screen_num,
            atoms,
            clip_win,
            content: Mutex::new(None),
            owned: AtomicBool::new(false),
        }))
    }

    fn root(&self) -> Window {
        self.conn.setup().roots[self.screen_num].root
    }

    // ---------- 快捷键 ----------

    pub fn grab_hotkey(&self, shortcut: &str) -> Result<()> {
        let (mods, keysym) = parse_shortcut(shortcut)?;
        let keycode = self
            .keysym_to_keycode(keysym)?
            .ok_or_else(|| format!("当前键盘布局中找不到按键：{shortcut}"))?;

        // 对 NumLock / CapsLock 的四种组合都注册，避免锁定键导致失效
        for extra in [
            ModMask::default(),
            ModMask::M2,
            ModMask::LOCK,
            ModMask::M2 | ModMask::LOCK,
        ] {
            self.conn
                .grab_key(
                    false,
                    self.root(),
                    mods | extra,
                    keycode,
                    GrabMode::ASYNC,
                    GrabMode::ASYNC,
                )?
                .check()
                .map_err(|_| format!("快捷键 {shortcut} 可能已被其他程序占用"))?;
        }
        self.conn.flush()?;
        Ok(())
    }

    fn keysym_to_keycode(&self, keysym: u32) -> Result<Option<u8>> {
        let setup = self.conn.setup();
        let min = setup.min_keycode;
        let max = setup.max_keycode;
        let mapping = self
            .conn
            .get_keyboard_mapping(min, max - min + 1)?
            .reply()?;
        let per = mapping.keysyms_per_keycode as usize;
        if per == 0 {
            return Ok(None);
        }
        for (index, chunk) in mapping.keysyms.chunks(per).enumerate() {
            if chunk.contains(&keysym) {
                return Ok(Some(min + index as u8));
            }
        }
        Ok(None)
    }

    // ---------- 事件处理 ----------

    /// 常驻模式：后台线程处理热键与剪贴板请求。
    pub fn spawn_event_thread(self: &Arc<Self>, tx: Sender<Msg>) {
        let backend = Arc::clone(self);
        std::thread::spawn(move || loop {
            match backend.conn.wait_for_event() {
                Ok(event) => backend.handle_event(event, Some(&tx)),
                Err(_) => return, // 连接断开（X 会话结束）
            }
        });
    }

    /// 独立模式：轮询处理事件直到超时或失去剪贴板所有权。
    pub fn serve_until(&self, deadline: Instant) {
        while Instant::now() < deadline && self.owned.load(Ordering::SeqCst) {
            match self.conn.poll_for_event() {
                Ok(Some(event)) => self.handle_event(event, None),
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(25)),
                Err(_) => return,
            }
        }
    }

    fn handle_event(&self, event: Event, tx: Option<&Sender<Msg>>) {
        match event {
            Event::KeyPress(_) => {
                if let Some(tx) = tx {
                    let _ = tx.send(Msg::Capture);
                }
            }
            Event::SelectionRequest(req) => {
                let _ = self.answer_selection_request(
                    req.requestor,
                    req.selection,
                    req.target,
                    req.property,
                    req.time,
                );
            }
            Event::SelectionClear(clear) => {
                if clear.selection == self.atoms.CLIPBOARD {
                    self.owned.store(false, Ordering::SeqCst);
                }
            }
            _ => {}
        }
    }

    fn answer_selection_request(
        &self,
        requestor: Window,
        selection: u32,
        target: u32,
        property: u32,
        time: u32,
    ) -> Result<()> {
        let property = if property == x11rb::NONE { target } else { property };
        let served = if target == self.atoms.TARGETS {
            let targets: [u32; 2] = [self.atoms.TARGETS, self.atoms.IMAGE_PNG];
            self.conn
                .change_property32(
                    PropMode::REPLACE,
                    requestor,
                    property,
                    AtomEnum::ATOM,
                    &targets,
                )
                .is_ok()
        } else if target == self.atoms.IMAGE_PNG {
            let png = self.content.lock().unwrap().clone();
            match png {
                Some(png) => self
                    .conn
                    .change_property8(
                        PropMode::REPLACE,
                        requestor,
                        property,
                        self.atoms.IMAGE_PNG,
                        &png,
                    )
                    .is_ok(),
                None => false,
            }
        } else {
            false
        };

        let notify = SelectionNotifyEvent {
            response_type: SELECTION_NOTIFY_EVENT,
            sequence: 0,
            time,
            requestor,
            selection,
            target,
            property: if served { property } else { x11rb::NONE },
        };
        self.conn
            .send_event(false, requestor, EventMask::NO_EVENT, notify)?;
        self.conn.flush()?;
        Ok(())
    }

    // ---------- 截图 ----------

    /// 截取活动窗口 → PNG → 接管 CLIPBOARD。返回应用名用于通知。
    pub fn capture_and_copy(&self) -> Result<String> {
        let active = self.active_window()?;
        let (x, y, width, height) = self.window_rect_on_root(active)?;
        let png = self.grab_root_region(x, y, width, height)?;

        *self.content.lock().unwrap() = Some(Arc::new(png));
        self.conn
            .set_selection_owner(self.clip_win, self.atoms.CLIPBOARD, x11rb::CURRENT_TIME)?
            .check()?;
        let owner = self.conn.get_selection_owner(self.atoms.CLIPBOARD)?.reply()?;
        if owner.owner != self.clip_win {
            return Err("未能接管剪贴板（可能被剪贴板管理器抢占）".into());
        }
        self.owned.store(true, Ordering::SeqCst);
        self.conn.flush()?;

        Ok(self.window_app_name(active))
    }

    pub fn clear_if_owned(&self) {
        if self.owned.swap(false, Ordering::SeqCst) {
            let _ = self
                .conn
                .set_selection_owner(x11rb::NONE, self.atoms.CLIPBOARD, x11rb::CURRENT_TIME);
            let _ = self.conn.flush();
            *self.content.lock().unwrap() = None;
        }
    }

    fn active_window(&self) -> Result<Window> {
        let reply = self
            .conn
            .get_property(
                false,
                self.root(),
                self.atoms.NET_ACTIVE_WINDOW,
                AtomEnum::WINDOW,
                0,
                1,
            )?
            .reply()?;
        let window = reply
            .value32()
            .and_then(|mut values| values.next())
            .unwrap_or(x11rb::NONE);
        if window == x11rb::NONE || window == self.clip_win {
            return Err("没有找到可截取的活动窗口".into());
        }
        Ok(window)
    }

    /// 活动窗口在 root 上的矩形：客户区 + WM 边框（_NET_FRAME_EXTENTS）
    /// − CSD 阴影（_GTK_FRAME_EXTENTS），并裁剪到屏幕内。
    fn window_rect_on_root(&self, window: Window) -> Result<(i32, i32, u32, u32)> {
        let geometry = self.conn.get_geometry(window)?.reply()?;
        let coords = self
            .conn
            .translate_coordinates(window, self.root(), 0, 0)?
            .reply()?;

        let mut x = coords.dst_x as i32;
        let mut y = coords.dst_y as i32;
        let mut w = geometry.width as i32;
        let mut h = geometry.height as i32;

        if let Some([left, right, top, bottom]) =
            self.extents_property(window, self.atoms.NET_FRAME_EXTENTS)
        {
            x -= left;
            y -= top;
            w += left + right;
            h += top + bottom;
        }
        if let Some([left, right, top, bottom]) =
            self.extents_property(window, self.atoms.GTK_FRAME_EXTENTS)
        {
            x += left;
            y += top;
            w -= left + right;
            h -= top + bottom;
        }

        let screen = &self.conn.setup().roots[self.screen_num];
        let root_w = screen.width_in_pixels as i32;
        let root_h = screen.height_in_pixels as i32;
        let x0 = x.clamp(0, root_w);
        let y0 = y.clamp(0, root_h);
        let x1 = (x + w).clamp(0, root_w);
        let y1 = (y + h).clamp(0, root_h);
        if x1 - x0 < 1 || y1 - y0 < 1 {
            return Err("活动窗口不在屏幕内".into());
        }
        Ok((x0, y0, (x1 - x0) as u32, (y1 - y0) as u32))
    }

    fn extents_property(&self, window: Window, atom: u32) -> Option<[i32; 4]> {
        let reply = self
            .conn
            .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 4)
            .ok()?
            .reply()
            .ok()?;
        let values: Vec<u32> = reply.value32()?.collect();
        if values.len() == 4 {
            Some([values[0] as i32, values[1] as i32, values[2] as i32, values[3] as i32])
        } else {
            None
        }
    }

    fn grab_root_region(&self, x: i32, y: i32, width: u32, height: u32) -> Result<Vec<u8>> {
        let image = self
            .conn
            .get_image(
                ImageFormat::Z_PIXMAP,
                self.root(),
                x as i16,
                y as i16,
                width as u16,
                height as u16,
                !0,
            )?
            .reply()?;

        let setup = self.conn.setup();
        let format = setup
            .pixmap_formats
            .iter()
            .find(|f| f.depth == image.depth)
            .ok_or("不支持的屏幕像素格式")?;
        let bpp = format.bits_per_pixel as usize;
        if bpp != 24 && bpp != 32 {
            return Err(format!("不支持的像素位深：{bpp} bpp").into());
        }
        let pad = format.scanline_pad as usize;
        let stride = (width as usize * bpp).div_ceil(pad) * pad / 8;
        let bytes_per_pixel = bpp / 8;
        let lsb_first = setup.image_byte_order == ImageOrder::LSB_FIRST;

        let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
        for row in 0..height as usize {
            let line = &image.data[row * stride..];
            for col in 0..width as usize {
                let p = &line[col * bytes_per_pixel..col * bytes_per_pixel + bytes_per_pixel];
                // ZPixmap：LSBFirst 为 BGR(X)，MSBFirst 为 (X)RGB
                let (r, g, b) = match (lsb_first, bytes_per_pixel) {
                    (true, 4) => (p[2], p[1], p[0]),
                    (true, 3) => (p[2], p[1], p[0]),
                    (false, 4) => (p[1], p[2], p[3]),
                    _ => (p[0], p[1], p[2]),
                };
                rgb.extend_from_slice(&[r, g, b]);
            }
        }

        let mut png_data = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut png_data, width, height);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(&rgb)?;
        }
        Ok(png_data)
    }

    fn window_app_name(&self, window: Window) -> String {
        let reply = match self
            .conn
            .get_property(false, window, self.atoms.WM_CLASS, AtomEnum::STRING, 0, 256)
            .ok()
            .and_then(|c| c.reply().ok())
        {
            Some(reply) => reply,
            None => return "应用".to_string(),
        };
        // WM_CLASS = "instance\0class\0"，取 class 段
        let parts: Vec<&[u8]> = reply.value.split(|&b| b == 0).filter(|s| !s.is_empty()).collect();
        parts
            .last()
            .map(|s| String::from_utf8_lossy(s).to_string())
            .unwrap_or_else(|| "应用".to_string())
    }
}

// ---------- 快捷键解析 ----------

fn parse_shortcut(shortcut: &str) -> Result<(ModMask, u32)> {
    let mut mods = ModMask::default();
    let mut key: Option<u32> = None;
    for part in shortcut.split('+') {
        let part = part.trim();
        match part.to_ascii_lowercase().as_str() {
            "shift" => mods = mods | ModMask::SHIFT,
            "ctrl" | "control" => mods = mods | ModMask::CONTROL,
            "alt" | "option" => mods = mods | ModMask::M1,
            "super" | "win" | "meta" | "cmd" => mods = mods | ModMask::M4,
            name => {
                if key.is_some() {
                    return Err(format!("快捷键 {shortcut} 含多个主键").into());
                }
                key = Some(
                    keysym_from_name(name)
                        .ok_or_else(|| format!("无法识别按键名：{name}"))?,
                );
            }
        }
    }
    let key = key.ok_or_else(|| format!("快捷键 {shortcut} 缺少主键"))?;
    if u32::from(u16::from(mods)) == 0 {
        return Err("快捷键至少需要一个修饰键（如 Alt、Ctrl）".into());
    }
    Ok((mods, key))
}

fn keysym_from_name(name: &str) -> Option<u32> {
    if name.len() == 1 {
        let c = name.chars().next().unwrap().to_ascii_lowercase();
        if c.is_ascii_graphic() {
            return Some(c as u32);
        }
    }
    match name {
        "space" => Some(0x20),
        "return" | "enter" => Some(0xff0d),
        "tab" => Some(0xff09),
        "escape" | "esc" => Some(0xff1b),
        "backspace" => Some(0xff08),
        "delete" => Some(0xffff),
        "insert" => Some(0xff63),
        "home" => Some(0xff50),
        "end" => Some(0xff57),
        "pageup" => Some(0xff55),
        "pagedown" => Some(0xff56),
        "print" | "printscreen" => Some(0xff61),
        f if f.starts_with('f') => f[1..]
            .parse::<u32>()
            .ok()
            .filter(|n| (1..=24).contains(n))
            .map(|n| 0xffbd + n),
        _ => None,
    }
}
