//! X11 桌面悬浮件，两种形式在设置里二选一：
//! - 悬浮球（默认）：56×56 圆形窗口，显示「上一个前台应用」的图标（_NET_WM_ICON）
//! - 桌宠：按用户自备的 GIF 素材逐帧播放，用 shape 掩码抠出镂空边缘（不依赖合成器）
//!
//! 两种形式都是 override-redirect 窗口：单击弹功能菜单，右键开设置，可拖动、
//! 位置持久化并钳制在屏幕内。对应 macOS 端 DesktopPetController 与 Windows 端 PetForm。

use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::shape;
use x11rb::protocol::xproto::{
    Arc as XArc, AtomEnum, ChangeGCAux, ChangeWindowAttributesAux, ClientMessageEvent,
    ConfigureWindowAux, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, ImageFormat,
    ImageOrder, Rectangle, Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::pet_assets::{self, Clips, PetPose};
use crate::{
    desktop_pet_position, pet_position, save_desktop_pet_position, save_pet_position, Msg, Result,
};

const BUBBLE_SIZE: u16 = 56;
const ICON_SIZE: u32 = 44;
const DRAG_THRESHOLD: i32 = 4;
const MARGIN: i32 = 8;
/// 桌宠模式的轮询步长：动画靠它推进，同时保证拖动跟手。
const PET_TICK: Duration = Duration::from_millis(8);

/// 手动 intern 的原子（atom_manager! 宏要求首个条目为无值形式，这里更直接）。
struct PetAtoms {
    net_active_window: u32,
    net_wm_icon: u32,
    wake: u32,
}

impl PetAtoms {
    fn intern(conn: &impl Connection) -> Result<Self> {
        Ok(Self {
            net_active_window: conn.intern_atom(false, b"_NET_ACTIVE_WINDOW")?.reply()?.atom,
            net_wm_icon: conn.intern_atom(false, b"_NET_WM_ICON")?.reply()?.atom,
            wake: conn.intern_atom(false, b"_WINDOWSNAP_WAKE")?.reply()?.atom,
        })
    }
}

/// 启动桌面悬浮件线程；任何失败只打印日志，不影响主功能。
/// 设置里换了形式或形象时线程重建窗口再来一轮（run 返回 true）。
pub fn spawn(tx: Sender<Msg>) {
    std::thread::spawn(move || loop {
        match run(tx.clone()) {
            Ok(true) => continue,
            Ok(false) => break,
            Err(e) => {
                eprintln!("windowsnap: 桌面悬浮件不可用（{e}），不影响截图与其他功能");
                break;
            }
        }
    });
}

/// 录制期间隐藏悬浮件（x11grab 抓屏幕区域，入镜会录进去）。
struct PetControl {
    conn: Arc<RustConnection>,
    window: Window,
    wake: u32,
}

static PET_CONTROL: Mutex<Option<PetControl>> = Mutex::new(None);

/// 「上一个前台应用」窗口 ID（悬浮球当前显示图标的目标），供「截取上一个应用」快捷键使用。
static PREVIOUS_WINDOW: AtomicU32 = AtomicU32::new(0);

/// 截图/录制期间的隐藏状态；重建窗口时沿用，避免重建后突然入镜。
static VISIBLE: AtomicBool = AtomicBool::new(true);

/// 设置保存后请求重建窗口（换形式 / 换形象）。
static RESTART: AtomicBool = AtomicBool::new(false);

/// 待播动作：0 = 无，其余为 PetPose 序号 + 1。
static PENDING_POSE: AtomicU8 = AtomicU8::new(0);

pub fn previous_window() -> u32 {
    PREVIOUS_WINDOW.load(Ordering::SeqCst)
}

pub fn set_pet_visible(visible: bool) {
    VISIBLE.store(visible, Ordering::SeqCst);
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

/// 设置里改了桌面形式或桌宠形象后调用：请求重建窗口。
/// 悬浮球模式阻塞在 wait_for_event 上，所以顺带发一个唤醒事件把它叫醒。
pub fn request_restart() {
    RESTART.store(true, Ordering::SeqCst);
    if let Ok(guard) = PET_CONTROL.lock() {
        if let Some(control) = guard.as_ref() {
            // 事件掩码为空时，X 把事件送回创建该窗口的客户端，正好是悬浮件线程自己
            let event = ClientMessageEvent::new(32, control.window, control.wake, [0u32; 5]);
            let _ = control
                .conn
                .send_event(false, control.window, EventMask::NO_EVENT, event);
            let _ = control.conn.flush();
        }
    }
}

/// 把一次桌面通知翻译成桌宠动作，与 Windows / macOS 端 Toast → PetPose 同规则：
/// 成功起跳、失败垂头，其余一律待机。
pub fn notify_pose(summary: &str) {
    let pose = if summary.contains("失败") || summary.contains("未保存") || summary.contains("不可用")
    {
        PetPose::Failed
    } else if summary.contains("已复制") || summary.contains("已保存") || summary.contains("完成") {
        PetPose::Jumping
    } else {
        PetPose::Waiting
    };
    PENDING_POSE.store(pose.index() as u8 + 1, Ordering::SeqCst);
}

fn take_pending_pose() -> Option<PetPose> {
    match PENDING_POSE.swap(0, Ordering::SeqCst) {
        0 => None,
        value => PetPose::ALL.get(value as usize - 1).copied(),
    }
}

/// 桌宠形式下的运行时：素材、当前动作与帧，以及每帧重画的 shape 掩码。
struct PetRuntime {
    clips: Clips,
    pose: PetPose,
    frame: usize,
    next_at: Instant,
    mask: u32,
    mask_gc: u32,
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
    width: u16,
    height: u16,
    current: Window,
    previous: Window,
    /// 按下左键时指针的 root 坐标；None 表示未按下。
    pressed: Option<(i16, i16)>,
    /// 按下时窗口自身的位置。
    press_origin: (i32, i32),
    dragging: bool,
    /// None = 悬浮球形式。
    runtime: Option<PetRuntime>,
}

fn run(tx: Sender<Msg>) -> Result<bool> {
    let (raw_conn, screen_num) = x11rb::connect(None)?;
    let conn: Arc<RustConnection> = Arc::new(raw_conn);
    let atoms = PetAtoms::intern(&conn)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let root_depth = screen.root_depth;
    let shaped = conn
        .extension_information(shape::X11_EXTENSION_NAME)?
        .is_some();

    // 桌宠：选了桌宠形式、本地素材可用、且有 shape 扩展（否则抠不出镂空）才启用，
    // 任一不满足都回退悬浮球，与 Windows / macOS 端的回退规则一致。
    let settings = crate::settings::load();
    let clips = if settings.ui_mode == "pet" && shaped {
        pet_assets::resolve_skin(&settings.pet_skin).and_then(|skin| pet_assets::load(&skin))
    } else {
        None
    };
    let (width, height) = match &clips {
        Some(clips) => (clips.width as u16, clips.height as u16),
        None => (BUBBLE_SIZE, BUBBLE_SIZE),
    };

    let window = conn.generate_id()?;
    conn.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        window,
        root,
        -100,
        -100,
        width,
        height,
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
    conn.create_pixmap(root_depth, pixmap, window, width, height)?
        .check()?;

    let gc = conn.generate_id()?;
    conn.create_gc(gc, pixmap, &CreateGCAux::new())?.check()?;

    let runtime = match clips {
        Some(clips) => {
            let mask = conn.generate_id()?;
            conn.create_pixmap(1, mask, root, width, height)?.check()?;
            let mask_gc = conn.generate_id()?;
            conn.create_gc(
                mask_gc,
                mask,
                &CreateGCAux::new().foreground(0).graphics_exposures(0),
            )?
            .check()?;
            Some(PetRuntime {
                clips,
                pose: PetPose::Idle,
                frame: 0,
                next_at: Instant::now(),
                mask,
                mask_gc,
            })
        }
        None => {
            apply_circle_shape(&conn, window, root)?;
            None
        }
    };

    let mut pet = Pet {
        conn: Arc::clone(&conn),
        screen_num,
        root,
        root_depth,
        atoms,
        window,
        pixmap,
        gc,
        width,
        height,
        current: x11rb::NONE,
        previous: x11rb::NONE,
        pressed: None,
        press_origin: (0, 0),
        dragging: false,
        runtime,
    };

    // 跟踪活动窗口变化（事件掩码按客户端独立，不影响截图后端）
    conn.change_window_attributes(
        root,
        &ChangeWindowAttributesAux::new().event_mask(EventMask::PROPERTY_CHANGE),
    )?
    .check()?;

    // 初始位置：配置的位置（钳制到屏幕内）或默认右下角；两种形式各记各的位置
    let screen_w = screen.width_in_pixels as i32;
    let screen_h = screen.height_in_pixels as i32;
    let default_x = screen_w - width as i32 - 16;
    let default_y = screen_h - height as i32 - 16;
    let saved = if pet.runtime.is_some() {
        desktop_pet_position()
    } else {
        pet_position()
    };
    let (x, y) = saved.unwrap_or((default_x, default_y));
    let (x, y) = clamp_position(x, y, width, height, screen_w, screen_h);
    conn.configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?
        .check()?;

    pet.track_active_window();
    if pet.runtime.is_some() {
        pet.play(PetPose::Idle, 0)?;
    } else {
        pet.paint_fallback();
    }
    // 截图/录制期间重建时保持隐藏，别突然入镜
    if VISIBLE.load(Ordering::SeqCst) {
        conn.map_window(window)?;
    }
    conn.flush()?;

    *PET_CONTROL.lock().unwrap() = Some(PetControl {
        conn: Arc::clone(&conn),
        window,
        wake: pet.atoms.wake,
    });

    let restart = pet.event_loop(tx)?;
    // 重建前先拆掉旧窗口，否则屏幕上会留下一个不响应的空壳
    if let Some(runtime) = &pet.runtime {
        let _ = conn.free_gc(runtime.mask_gc);
        let _ = conn.free_pixmap(runtime.mask);
    }
    let _ = conn.free_gc(gc);
    let _ = conn.free_pixmap(pixmap);
    let _ = conn.destroy_window(window);
    let _ = conn.flush();
    *PET_CONTROL.lock().unwrap() = None;
    Ok(restart)
}

/// 圆形窗口（shape 扩展不可用时退化为方形），仅悬浮球形式使用。
fn apply_circle_shape(conn: &Arc<RustConnection>, window: Window, root: Window) -> Result<()> {
    if conn.extension_information(shape::X11_EXTENSION_NAME)?.is_none() {
        return Ok(());
    }
    let mask = conn.generate_id()?;
    conn.create_pixmap(1, mask, root, BUBBLE_SIZE, BUBBLE_SIZE)?
        .check()?;
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
            width: BUBBLE_SIZE,
            height: BUBBLE_SIZE,
        }],
    )?;
    conn.change_gc(mask_gc, &ChangeGCAux::new().foreground(1))?;
    conn.poly_fill_arc(
        mask,
        mask_gc,
        &[XArc {
            x: 1,
            y: 1,
            width: BUBBLE_SIZE - 2,
            height: BUBBLE_SIZE - 2,
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
    /// 返回 true 表示要按新设置重建窗口。
    fn event_loop(&mut self, tx: Sender<Msg>) -> Result<bool> {
        loop {
            if RESTART.swap(false, Ordering::SeqCst) {
                return Ok(true);
            }
            if self.runtime.is_some() {
                // 桌宠要推进动画，只能轮询；步长同时决定拖动的跟手程度
                if let Some(pose) = take_pending_pose() {
                    self.set_pose(pose)?;
                }
                self.advance_frame_if_due()?;
                while let Some(event) = self.conn.poll_for_event()? {
                    self.handle_event(event, &tx)?;
                }
                std::thread::sleep(PET_TICK);
            } else {
                // 悬浮球没有动画，阻塞等事件；request_restart 会发唤醒事件把它叫醒
                let event = self.conn.wait_for_event()?;
                self.handle_event(event, &tx)?;
            }
        }
    }

    fn handle_event(&mut self, event: Event, tx: &Sender<Msg>) -> Result<()> {
        match event {
            Event::PropertyNotify(event) => {
                if event.window == self.root && event.atom == self.atoms.net_active_window {
                    self.track_active_window();
                }
            }
            Event::Expose(_) => {
                self.copy_pixmap();
            }
            Event::ButtonPress(event) if event.detail == 3 => {
                // 右键：打开统一设置（快捷键绑定 + 润色服务 + 桌面形式）
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
                        let (x, y) =
                            clamp_position(x, y, self.width, self.height, screen_w, screen_h);
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
                    if self.runtime.is_some() {
                        save_desktop_pet_position(geometry.x as i32, geometry.y as i32);
                    } else {
                        save_pet_position(geometry.x as i32, geometry.y as i32);
                    }
                } else {
                    let _ = tx.send(Msg::TogglePanel);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn screen_size(&self) -> (i32, i32) {
        let screen = &self.conn.setup().roots[self.screen_num];
        (
            screen.width_in_pixels as i32,
            screen.height_in_pixels as i32,
        )
    }

    // ---------- 桌宠动画 ----------

    /// 切到某个动作；没有独立素材就不动（保持当前动画），已在播的循环动作也不打断，
    /// 与 Windows 端 SetPose 同规则。
    fn set_pose(&mut self, pose: PetPose) -> Result<()> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(());
        };
        if !runtime.clips.has(pose) || (runtime.pose == pose && pose.loops()) {
            return Ok(());
        }
        self.play(pose, 0)
    }

    fn play(&mut self, pose: PetPose, frame: usize) -> Result<()> {
        {
            let Some(runtime) = self.runtime.as_mut() else {
                return Ok(());
            };
            let (index, delay) = match runtime.clips.clip(pose) {
                Some(clip) if !clip.frames.is_empty() => {
                    let index = frame.min(clip.frames.len() - 1);
                    (index, clip.frames[index].delay_ms)
                }
                _ => return Ok(()),
            };
            runtime.pose = pose;
            runtime.frame = index;
            runtime.next_at = Instant::now() + Duration::from_millis(delay as u64);
        }
        self.paint_pet_frame()
    }

    fn advance_frame_if_due(&mut self) -> Result<()> {
        let next = {
            let Some(runtime) = self.runtime.as_ref() else {
                return Ok(());
            };
            if Instant::now() < runtime.next_at {
                return Ok(());
            }
            let Some(clip) = runtime.clips.clip(runtime.pose) else {
                return Ok(());
            };
            let index = runtime.frame + 1;
            if index < clip.frames.len() {
                (runtime.pose, index)
            } else if clip.loops {
                (runtime.pose, 0)
            } else {
                // 单次动画播完回待机
                (PetPose::Idle, 0)
            }
        };
        self.play(next.0, next.1)
    }

    fn paint_pet_frame(&mut self) -> Result<()> {
        let (rgb, rects) = {
            let Some(runtime) = self.runtime.as_ref() else {
                return Ok(());
            };
            let Some(clip) = runtime.clips.clip(runtime.pose) else {
                return Ok(());
            };
            let Some(frame) = clip.frames.get(runtime.frame) else {
                return Ok(());
            };
            (
                opaque_rgb(&frame.rgba),
                mask_rects(&frame.rgba, self.width, self.height),
            )
        };
        self.apply_frame_shape(&rects)?;
        self.put_image(&rgb);
        Ok(())
    }

    /// 逐帧重画 shape 掩码：GIF 只有全透明 / 不透明两种像素，按行取不透明像素的
    /// 连续段填进 1-bit 掩码即可精确还原镂空，也免去 1-bit 位图在不同字节序 /
    /// 位序下的打包细节。
    fn apply_frame_shape(&self, rects: &[Rectangle]) -> Result<()> {
        let Some(runtime) = self.runtime.as_ref() else {
            return Ok(());
        };
        self.conn
            .change_gc(runtime.mask_gc, &ChangeGCAux::new().foreground(0))?;
        self.conn.poly_fill_rectangle(
            runtime.mask,
            runtime.mask_gc,
            &[Rectangle {
                x: 0,
                y: 0,
                width: self.width,
                height: self.height,
            }],
        )?;
        self.conn
            .change_gc(runtime.mask_gc, &ChangeGCAux::new().foreground(1))?;
        // 单个请求有长度上限，矩形分批发
        for chunk in rects.chunks(2048) {
            self.conn
                .poly_fill_rectangle(runtime.mask, runtime.mask_gc, chunk)?;
        }
        shape::mask(
            &self.conn,
            shape::SO::SET,
            shape::SK::BOUNDING,
            self.window,
            0,
            0,
            runtime.mask,
        )?;
        shape::mask(
            &self.conn,
            shape::SO::SET,
            shape::SK::INPUT,
            self.window,
            0,
            0,
            runtime.mask,
        )?;
        Ok(())
    }

    // ---------- 悬浮球：跟踪活动窗口与图标 ----------

    /// _NET_ACTIVE_WINDOW 变化：推进 current/previous 并重绘「上一个应用」图标。
    /// 桌宠形式不画图标，但仍要跟踪窗口——「截取上一个应用」依赖它。
    fn track_active_window(&mut self) {
        let active = self.query_active_window();
        if active == x11rb::NONE || active == self.window {
            return;
        }
        if active != self.current {
            self.previous = self.current;
            self.current = active;
            PREVIOUS_WINDOW.store(self.previous, Ordering::SeqCst);
            if self.previous != x11rb::NONE && self.runtime.is_none() {
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
        let offset = (BUBBLE_SIZE as u32 - ICON_SIZE) / 2;
        for dy in 0..ICON_SIZE {
            for dx in 0..ICON_SIZE {
                let sx = dx * width / ICON_SIZE;
                let sy = dy * height / ICON_SIZE;
                let pixel = pixels[(sy * width + sx) as usize];
                let alpha = ((pixel >> 24) & 0xff) as u32;
                if alpha == 0 {
                    continue;
                }
                let index = (((offset + dy) * BUBBLE_SIZE as u32 + offset + dx) * 3) as usize;
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
        for y in 0..BUBBLE_SIZE as i32 {
            for x in 0..BUBBLE_SIZE as i32 {
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
        let size = BUBBLE_SIZE as i32;
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
        let width = self.width as usize;
        let height = self.height as usize;
        let stride = (width * bpp).div_ceil(pad) * pad / 8;
        let lsb_first = setup.image_byte_order == ImageOrder::LSB_FIRST;
        let mut data = vec![0u8; stride * height];
        for (row, line) in rgb.chunks(width * 3).enumerate().take(height) {
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
            self.width,
            self.height,
            0,
            0,
            0,
            self.root_depth,
            &data,
        );
        self.copy_pixmap();
    }

    fn copy_pixmap(&self) {
        let _ = self.conn.copy_area(
            self.pixmap,
            self.window,
            self.gc,
            0,
            0,
            0,
            0,
            self.width,
            self.height,
        );
        let _ = self.conn.flush();
    }
}

/// RGBA 帧 → 紧凑 RGB；透明像素填黑，反正会被 shape 掩码裁掉。
fn opaque_rgb(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = vec![0u8; rgba.len() / 4 * 3];
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        if pixel[3] == 0 {
            continue;
        }
        rgb[index * 3] = pixel[0];
        rgb[index * 3 + 1] = pixel[1];
        rgb[index * 3 + 2] = pixel[2];
    }
    rgb
}

/// 每行不透明像素的连续段 → shape 掩码矩形。
fn mask_rects(rgba: &[u8], width: u16, height: u16) -> Vec<Rectangle> {
    let mut rects = Vec::new();
    let width = width as usize;
    for y in 0..height as usize {
        let row = y * width;
        let mut x = 0usize;
        while x < width {
            if rgba[(row + x) * 4 + 3] == 0 {
                x += 1;
                continue;
            }
            let start = x;
            while x < width && rgba[(row + x) * 4 + 3] != 0 {
                x += 1;
            }
            rects.push(Rectangle {
                x: start as i16,
                y: y as i16,
                width: (x - start) as u16,
                height: 1,
            });
        }
    }
    rects
}

fn put_pixel(image: &mut [u8], x: i32, y: i32, (r, g, b): (u8, u8, u8)) {
    if x < 0 || y < 0 || x >= BUBBLE_SIZE as i32 || y >= BUBBLE_SIZE as i32 {
        return;
    }
    let index = ((y * BUBBLE_SIZE as i32 + x) * 3) as usize;
    image[index] = r;
    image[index + 1] = g;
    image[index + 2] = b;
}

/// 位置钳制：保持在屏幕可见区域内（距边缘至少 MARGIN），与 macOS 端一致。
fn clamp_position(x: i32, y: i32, width: u16, height: u16, screen_w: i32, screen_h: i32) -> (i32, i32) {
    let x = x.clamp(MARGIN, (screen_w - width as i32 - MARGIN).max(MARGIN));
    let y = y.clamp(MARGIN, (screen_h - height as i32 - MARGIN).max(MARGIN));
    (x, y)
}
