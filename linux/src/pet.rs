//! X11 桌面宠物：56×56 的 override-redirect 圆形悬浮窗，
//! 显示「上一个前台应用」的图标（_NET_WM_ICON），点击弹出功能菜单，
//! 可拖动、位置持久化并钳制在屏幕内。对应 macOS 端的 DesktopPetController。

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shape;
use x11rb::protocol::xproto::{
    AtomEnum, Arc as XArc, ChangeGCAux, ChangeWindowAttributesAux, ConfigureWindowAux,
    ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, ImageFormat, ImageOrder, Rectangle,
    Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::{pet_position, save_pet_position, Msg, Result};

const PET_SIZE: u16 = 56;
const ICON_SIZE: u32 = 44;
const DRAG_THRESHOLD: i32 = 4;
const MARGIN: i32 = 8;

/// 手动 intern 的原子（atom_manager! 宏要求首个条目为无值形式，这里更直接）。
struct PetAtoms {
    net_active_window: u32,
    net_wm_icon: u32,
}

impl PetAtoms {
    fn intern(conn: &impl Connection) -> Result<Self> {
        Ok(Self {
            net_active_window: conn.intern_atom(false, b"_NET_ACTIVE_WINDOW")?.reply()?.atom,
            net_wm_icon: conn.intern_atom(false, b"_NET_WM_ICON")?.reply()?.atom,
        })
    }
}

/// 启动桌面宠物线程；任何失败只打印日志，不影响主功能。
/// `initially_visible` 为假时窗口不显示（GIF 桌宠模式下悬浮球离场），
/// 但事件循环照常运行——「截取上一个应用」依赖这里的窗口跟踪。
pub fn spawn(tx: Sender<Msg>, initially_visible: bool) {
    std::thread::spawn(move || {
        if let Err(e) = run(tx, initially_visible) {
            eprintln!("windowsnap: 桌面宠物不可用（{e}），不影响截图与其他功能");
        }
    });
}

/// 录制期间隐藏宠物（x11grab 抓屏幕区域，宠物入镜会录进去）。
struct PetControl {
    conn: Arc<RustConnection>,
    window: Window,
}

static PET_CONTROL: Mutex<Option<PetControl>> = Mutex::new(None);

/// 「上一个前台应用」窗口 ID（悬浮球当前显示图标的目标），供「截取上一个应用」快捷键使用。
static PREVIOUS_WINDOW: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub fn previous_window() -> u32 {
    PREVIOUS_WINDOW.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn set_pet_visible(visible: bool) {
    if let Ok(guard) = PET_CONTROL.lock() {
        if let Some(control) = guard.as_ref() {
            let _ = if visible {
                control.conn.map_window(control.window)
            } else {
                control.conn.unmap_window(control.window)
            };
            let _ = control.conn.flush();
        }
    }
}

struct Pet {
    conn: Arc<RustConnection>,
    screen_num: usize,
    root: Window,
    root_depth: u8,
    atoms: PetAtoms,
    window: Window,
    pixmap: u32,
    gc: u32,
    current: Window,
    previous: Window,
    /// 按下左键时指针的 root 坐标；None 表示未按下。
    pressed: Option<(i16, i16)>,
    /// 按下时窗口自身的位置。
    press_origin: (i32, i32),
    dragging: bool,
}

fn run(tx: Sender<Msg>, visible: bool) -> Result<()> {
    let (raw_conn, screen_num) = x11rb::connect(None)?;
    let conn: Arc<RustConnection> = Arc::new(raw_conn);
    let atoms = PetAtoms::intern(&conn)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let root_depth = screen.root_depth;

    let window = conn.generate_id()?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        window,
        root,
        -100,
        -100,
        PET_SIZE,
        PET_SIZE,
        0,
        WindowClass::INPUT_OUTPUT,
        x11rb::COPY_FROM_PARENT,
        &CreateWindowAux::new()
            .background_pixel(0)
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::BUTTON_MOTION,
            )
            .override_redirect(1),
    )?
    .check()?;

    let pixmap = conn.generate_id()?;
    conn.create_pixmap(root_depth, pixmap, window, PET_SIZE, PET_SIZE)?
        .check()?;

    let gc = conn.generate_id()?;
    conn.create_gc(gc, pixmap, &CreateGCAux::new())?.check()?;

    apply_circle_shape(&conn, window, root)?;

    let mut pet = Pet {
        conn: Arc::clone(&conn),
        screen_num,
        root,
        root_depth,
        atoms,
        window,
        pixmap,
        gc,
        current: x11rb::NONE,
        previous: x11rb::NONE,
        pressed: None,
        press_origin: (0, 0),
        dragging: false,
    };

    // 跟踪活动窗口变化（事件掩码按客户端独立，不影响截图后端）
    conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?
    .check()?;

    // 初始位置：配置的位置（钳制到屏幕内）或默认右下角
    let screen_w = screen.width_in_pixels as i32;
    let screen_h = screen.height_in_pixels as i32;
    let default_x = screen_w - PET_SIZE as i32 - 16;
    let default_y = screen_h - PET_SIZE as i32 - 16;
    let (x, y) = pet_position().unwrap_or((default_x, default_y));
    let (x, y) = clamp_position(x, y, screen_w, screen_h);
    conn.configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?
        .check()?;

    pet.track_active_window();
    pet.paint_fallback();
    if visible {
        conn.map_window(window)?;
    }
    conn.flush()?;

    *PET_CONTROL.lock().unwrap() = Some(PetControl {
        conn: Arc::clone(&conn),
        window,
    });

    pet.event_loop(tx)
}

/// 圆形窗口（shape 扩展不可用时退化为方形）。
fn apply_circle_shape(conn: &Arc<RustConnection>, window: Window, root: Window) -> Result<()> {
    if conn.extension_information(shape::X11_EXTENSION_NAME)?.is_none() {
        return Ok(());
    }
    let mask = conn.generate_id()?;
    conn.create_pixmap(1, mask, root, PET_SIZE, PET_SIZE)?.check()?;
    let mask_gc = conn.generate_id()?;
    conn.create_gc(
        mask_gc,
        mask,
        &CreateGCAux::new().foreground(0).graphics_exposures(0),
    )?
    .check()?;
    conn.poly_fill_rectangle(
        mask,
        mask_gc,
        &[Rectangle {
            x: 0,
            y: 0,
            width: PET_SIZE,
            height: PET_SIZE,
        }],
    )?;
    conn.change_gc(mask_gc, &ChangeGCAux::new().foreground(1))?;
    conn.poly_fill_arc(
        mask,
        mask_gc,
        &[XArc {
            x: 1,
            y: 1,
            width: PET_SIZE - 2,
            height: PET_SIZE - 2,
            angle1: 0,
            angle2: 23040, // 360° × 64
        }],
    )?;
    // 显示与命中都裁成圆形，圆外点击直接穿透
    shape::mask(
        conn,
        shape::SO::SET,
        shape::SK::BOUNDING,
        window,
        0,
        0,
        mask,
    )?
    .check()?;
    shape::mask(conn, shape::SO::SET, shape::SK::INPUT, window, 0, 0, mask)?.check()?;
    conn.free_gc(mask_gc)?;
    conn.free_pixmap(mask)?;
    conn.flush()?;
    Ok(())
}

impl Pet {
    fn event_loop(&mut self, tx: Sender<Msg>) -> Result<()> {
        loop {
            match self.conn.wait_for_event()? {
                Event::PropertyNotify(event) => {
                    if event.window == self.root && event.atom == self.atoms.net_active_window {
                        self.track_active_window();
                    }
                }
                Event::Expose(_) => {
                    self.copy_pixmap();
                }
                Event::ButtonPress(event) if event.detail == 3 => {
                    // 右键：打开统一设置（快捷键绑定 + 润色服务）
                    let _ = tx.send(Msg::OpenSettings);
                }
                Event::ButtonPress(event) if event.detail == 1 => {
                    // X 在按钮按下时自动独占指针，松开前事件都发给我们
                    let geometry = self.conn.get_geometry(self.window)?.reply()?;
                    self.press_origin = (geometry.x as i32, geometry.y as i32);
                    self.pressed = Some((event.root_x, event.root_y));
                    self.dragging = false;
                }
                Event::MotionNotify(event) => {
                    if let Some((press_x, press_y)) = self.pressed {
                        if (event.root_x as i32 - press_x as i32).abs() > DRAG_THRESHOLD
                            || (event.root_y as i32 - press_y as i32).abs() > DRAG_THRESHOLD
                        {
                            self.dragging = true;
                        }
                        if self.dragging {
                            let x = self.press_origin.0 + event.root_x as i32 - press_x as i32;
                            let y = self.press_origin.1 + event.root_y as i32 - press_y as i32;
                            let (screen_w, screen_h) = self.screen_size();
                            let (x, y) = clamp_position(x, y, screen_w, screen_h);
                            let _ = self
                                .conn
                                .configure_window(self.window, &ConfigureWindowAux::new().x(x).y(y))?;
                            let _ = self.conn.flush();
                        }
                    }
                }
                Event::ButtonRelease(event) if event.detail == 1 => {
                    self.pressed = None;
                    if self.dragging {
                        self.dragging = false;
                        let geometry = self.conn.get_geometry(self.window)?.reply()?;
                        save_pet_position(geometry.x as i32, geometry.y as i32);
                    } else {
                        let _ = tx.send(Msg::TogglePanel);
                    }
                }
                _ => {}
            }
        }
    }

    fn screen_size(&self) -> (i32, i32) {
        let screen = &self.conn.setup().roots[self.screen_num];
        (
            screen.width_in_pixels as i32,
            screen.height_in_pixels as i32,
        )
    }

    /// _NET_ACTIVE_WINDOW 变化：推进 current/previous 并重绘「上一个应用」图标。
    fn track_active_window(&mut self) {
        let active = self.query_active_window();
        if active == x11rb::NONE || active == self.window {
            return;
        }
        if active != self.current {
            self.previous = self.current;
            self.current = active;
            PREVIOUS_WINDOW.store(self.previous, std::sync::atomic::Ordering::SeqCst);
            if self.previous != x11rb::NONE {
                match self.window_icon(self.previous) {
                    Some((width, height, pixels)) => self.paint_icon(width, height, &pixels),
                    None => self.paint_fallback(),
                }
            }
        }
    }

    fn query_active_window(&self) -> Window {
        self.conn
            .get_property(
                false,
                self.root,
                self.atoms.net_active_window,
                AtomEnum::WINDOW,
                0,
                1,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok())
            .and_then(|reply| reply.value32().and_then(|mut values| values.next()))
            .unwrap_or(x11rb::NONE)
    }

    /// 解析 _NET_WM_ICON，挑一张最接近 ICON_SIZE 的图标（同分取更大图）。
    fn window_icon(&self, window: Window) -> Option<(u32, u32, Vec<u32>)> {
        let probe = self
            .conn
            .get_property(
                false,
                window,
                self.atoms.net_wm_icon,
                AtomEnum::CARDINAL,
                0,
                0,
            )
            .ok()?
            .reply()
            .ok()?;
        let units = (probe.bytes_after as usize).div_ceil(4);
        if units == 0 {
            return None;
        }
        let reply = self
            .conn
            .get_property(
                false,
                window,
                self.atoms.net_wm_icon,
                AtomEnum::CARDINAL,
                0,
                units as u32,
            )
            .ok()?
            .reply()
            .ok()?;
        let data: Vec<u32> = reply.value32()?.collect();
        if data.len() < 3 {
            return None;
        }

        let mut best: Option<(i64, u32, u32, &[u32])> = None;
        let mut offset = 0usize;
        while offset + 2 <= data.len() {
            let width = data[offset];
            let height = data[offset + 1];
            let count = width as usize * height as usize;
            if width == 0 || height == 0 || offset + 2 + count > data.len() {
                break;
            }
            let pixels = &data[offset + 2..offset + 2 + count];
            let score = (width.max(height) as i64 - ICON_SIZE as i64).abs() * 100 - count as i64;
            if best
                .as_ref()
                .is_none_or(|(best_score, ..)| score < *best_score)
            {
                best = Some((score, width, height, pixels));
            }
            offset += 2 + count;
        }
        best.map(|(_, width, height, pixels)| (width, height, pixels.to_vec()))
    }

    // ---------- 绘制 ----------

    /// 把 ARGB 图标按最近邻缩放到 ICON_SIZE 并 alpha 混合到白色圆底。
    fn paint_icon(&mut self, width: u32, height: u32, pixels: &[u32]) {
        let mut image = self.base_image();
        let offset = (PET_SIZE as u32 - ICON_SIZE) / 2;
        for dy in 0..ICON_SIZE {
            for dx in 0..ICON_SIZE {
                let sx = dx * width / ICON_SIZE;
                let sy = dy * height / ICON_SIZE;
                let pixel = pixels[(sy * width + sx) as usize];
                let alpha = ((pixel >> 24) & 0xff) as u32;
                if alpha == 0 {
                    continue;
                }
                let index = (((offset + dy) * PET_SIZE as u32 + offset + dx) * 3) as usize;
                let blend = |fg: u32, bg: u8| (fg * alpha + bg as u32 * (255 - alpha)) / 255;
                image[index] = blend(((pixel >> 16) & 0xff) as u32, image[index]) as u8;
                image[index + 1] = blend(((pixel >> 8) & 0xff) as u32, image[index + 1]) as u8;
                image[index + 2] = blend((pixel & 0xff) as u32, image[index + 2]) as u8;
            }
        }
        self.put_image(&image);
    }

    /// 拿不到应用图标时的相机占位图形。
    fn paint_fallback(&mut self) {
        let mut image = self.base_image();
        let body = (98u8, 100u8, 105u8);
        let lens = (245u8, 245u8, 248u8);
        for y in 21..39 {
            for x in 13..43 {
                put_pixel(&mut image, x, y, body); // 机身
            }
        }
        for y in 17..21 {
            for x in 23..33 {
                put_pixel(&mut image, x, y, body); // 取景器
            }
        }
        for y in 0..PET_SIZE as i32 {
            for x in 0..PET_SIZE as i32 {
                let dist = (x - 28).pow(2) + (y - 30).pow(2);
                if dist <= 36 {
                    put_pixel(&mut image, x, y, lens); // 镜头
                }
            }
        }
        put_pixel(&mut image, 38, 24, lens); // 闪光灯
        put_pixel(&mut image, 39, 24, lens);
        self.put_image(&image);
    }

    /// 56×56 RGB 底图：白圆 + 灰描边，圆外像素置黑（被 shape 裁掉不显示）。
    fn base_image(&self) -> Vec<u8> {
        let size = PET_SIZE as i32;
        let center = (size - 1) as f64 / 2.0;
        let mut image = vec![0u8; size as usize * size as usize * 3];
        for y in 0..size {
            for x in 0..size {
                let dist = ((x as f64 - center).powi(2) + (y as f64 - center).powi(2)).sqrt();
                let (r, g, b) = if dist <= 26.0 {
                    (250u8, 250u8, 252u8)
                } else if dist <= 27.5 {
                    (185u8, 185u8, 190u8)
                } else {
                    continue;
                };
                let index = ((y * size + x) * 3) as usize;
                image[index] = r;
                image[index + 1] = g;
                image[index + 2] = b;
            }
        }
        image
    }

    /// RGB 数据写入后备 pixmap 并刷到窗口。
    fn put_image(&mut self, rgb: &[u8]) {
        let setup = self.conn.setup();
        let Some(format) = setup
            .pixmap_formats
            .iter()
            .find(|format| format.depth == self.root_depth)
            .copied()
        else {
            return;
        };
        let bpp = format.bits_per_pixel as usize;
        if bpp != 24 && bpp != 32 {
            return;
        }
        let pad = format.scanline_pad as usize;
        let width = PET_SIZE as usize;
        let stride = (width * bpp).div_ceil(pad) * pad / 8;
        let lsb_first = setup.image_byte_order == ImageOrder::LSB_FIRST;
        let mut data = vec![0u8; stride * width];
        for (row, line) in rgb.chunks(width * 3).enumerate() {
            for col in 0..width {
                let pixel: u32 = (line[col * 3] as u32) << 16
                    | (line[col * 3 + 1] as u32) << 8
                    | line[col * 3 + 2] as u32;
                let bytes = if lsb_first {
                    pixel.to_le_bytes()
                } else {
                    pixel.to_be_bytes()
                };
                let start = row * stride + col * (bpp / 8);
                data[start..start + bpp / 8].copy_from_slice(&bytes[..bpp / 8]);
            }
        }
        let _ = self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.pixmap,
            self.gc,
            PET_SIZE,
            PET_SIZE,
            0,
            0,
            0,
            self.root_depth,
            &data,
        );
        let _ = self.conn.flush();
        self.copy_pixmap();
    }

    fn copy_pixmap(&self) {
        let size = PET_SIZE as i16;
        let _ = self.conn.copy_area(self.pixmap, self.window, self.gc, 0, 0, size, size, 0, 0);
        let _ = self.conn.flush();
    }
}

fn put_pixel(image: &mut [u8], x: i32, y: i32, (r, g, b): (u8, u8, u8)) {
    if x < 0 || y < 0 || x >= PET_SIZE as i32 || y >= PET_SIZE as i32 {
        return;
    }
    let index = ((y * PET_SIZE as i32 + x) * 3) as usize;
    image[index] = r;
    image[index + 1] = g;
    image[index + 2] = b;
}

/// 位置钳制：保持在屏幕可见区域内（距边缘至少 MARGIN），与 macOS 端一致。
fn clamp_position(x: i32, y: i32, screen_w: i32, screen_h: i32) -> (i32, i32) {
    let size = PET_SIZE as i32;
    let x = x.clamp(MARGIN, (screen_w - size - MARGIN).max(MARGIN));
    let y = y.clamp(MARGIN, (screen_h - size - MARGIN).max(MARGIN));
    (x, y)
}
