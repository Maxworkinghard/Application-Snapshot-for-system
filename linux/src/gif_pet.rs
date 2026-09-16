//! X11 GIF 桌宠（与 Windows 端 Pet.cs 行为对齐）：override-redirect 的
//! 32 位 ARGB 窗口逐帧渲染 GIF 素材，猫毛边缘保留半透明。
//! 姿势状态机：idle / 跑动循环，挥手 / 起跳 / 失败 / 等待单次播完回 idle；
//! Toast 事件反应：成功跳一下、错误趴下、其余待机。
//! 素材不随应用分发（版权考虑），放在 $XDG_DATA_HOME/windowsnap/pet/<形象>/*.gif。

use std::fs::File;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use x11rb::connection::{Connection, RequestConnection};
use x11rb::protocol::xproto::{
    ColormapAlloc, ConfigureWindowAux, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask,
    ImageFormat, VisualClass, Window, WindowClass,
};
use x11rb::protocol::Event;
use x11rb::rust_connection::RustConnection;

use crate::notify::ToastKind;
use crate::{settings, Msg, Result};

/// 展示尺寸（96 DPI 设计值；GIF 原始 192×208，等比缩放显示）。
const PET_WIDTH: i32 = 155;
const PET_HEIGHT: i32 = 168;
const DRAG_THRESHOLD: i32 = 4;
const MARGIN: i32 = 8;
/// 姿势顺序与 Windows 端 PetPose 枚举一致；(文件名, 是否循环)。
const POSES: [(&str, bool); 7] = [
    ("idle", true),
    ("waving", false),
    ("jumping", false),
    ("failed", false),
    ("waiting", false),
    ("running-left", true),
    ("running-right", true),
];
const POSE_IDLE: usize = 0;
const POSE_JUMPING: usize = 2;
const POSE_FAILED: usize = 3;
const POSE_WAITING: usize = 4;
const POSE_RUN_LEFT: usize = 5;
const POSE_RUN_RIGHT: usize = 6;
/// 帧推进检查周期；GIF 帧延迟下限 20ms 与 Windows 端一致。
const TICK: Duration = Duration::from_millis(8);
const MIN_FRAME_DELAY: Duration = Duration::from_millis(20);

/// 主线程发来的控制命令（可见性 / 换形象 / 事件反应）。
/// SetVisible / Reload 带完成通道：调用方等窗口真正 map/unmap 或素材加载完，
/// 避免 x11grab / get_image 把还没离场的猫拍进去。
pub enum PetCmd {
    SetVisible {
        visible: bool,
        done: Sender<bool>,
    },
    Reload {
        skin: String,
        done: Sender<()>,
    },
    SetPose(usize),
}

static CMD_TX: Mutex<Option<Sender<PetCmd>>> = Mutex::new(None);

fn send_cmd(cmd: PetCmd) -> bool {
    match CMD_TX.lock().unwrap().as_ref() {
        Some(tx) => tx.send(cmd).is_ok(),
        None => false,
    }
}

/// 录制 / 截图期间离场（x11grab 与 get_image 都抓屏幕像素）；模式切换时隐藏。
/// 返回桌宠窗口最终是否可见。线程未启动、素材无法加载、超时均视为不可见。
pub fn set_visible(visible: bool) -> bool {
    let (done, rx) = mpsc::channel();
    if !send_cmd(PetCmd::SetVisible { visible, done }) {
        return false;
    }
    rx.recv_timeout(Duration::from_secs(5)).unwrap_or(false)
}

/// 换形象：桌宠显示中立即换装，位置沿用 desktoppet_x/y。
/// 返回命令是否被桌宠线程处理完（线程未启动或超时则为 false）。
pub fn reload(skin: &str) -> bool {
    let (done, rx) = mpsc::channel();
    if !send_cmd(PetCmd::Reload {
        skin: skin.to_string(),
        done,
    }) {
        return false;
    }
    rx.recv_timeout(Duration::from_secs(5)).is_ok()
}

/// Toast 事件 → 姿势反应，与 Windows 端 PetController.OnToastNotified 一致：
/// 成功起跳、错误趴下、其余待机。
pub fn react(kind: ToastKind) {
    match kind {
        ToastKind::Success => {
            let _ = send_cmd(PetCmd::SetPose(POSE_JUMPING));
        }
        ToastKind::Error => {
            let _ = send_cmd(PetCmd::SetPose(POSE_FAILED));
        }
        ToastKind::Info => {
            let _ = send_cmd(PetCmd::SetPose(POSE_WAITING));
        }
    }
}

// ---------- 素材发现（目录约定与 Windows 端 README 契约一致） ----------

fn pet_root() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share"))
        })
        .map(|base| base.join("windowsnap/pet"))
}

/// 可用形象 = pet 目录下含至少一个 gif 的子目录，按名称排序。
pub fn available_skins() -> Vec<String> {
    let mut skins = Vec::new();
    let Some(root) = pet_root() else { return skins };
    let Ok(entries) = std::fs::read_dir(root) else { return skins };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let has_gif = path
            .read_dir()
            .map(|dir| {
                dir.flatten()
                    .any(|f| f.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("gif")))
            })
            .unwrap_or(false);
        if has_gif {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                skins.push(name.to_string());
            }
        }
    }
    skins.sort();
    skins
}

/// 已保存形象存在则用之，否则回退第一个可用形象；没有素材返回 None。
fn resolve_skin() -> Option<String> {
    let skins = available_skins();
    let saved = settings::load().pet_skin;
    if skins.iter().any(|s| *s == saved) {
        Some(saved)
    } else {
        skins.into_iter().next()
    }
}

// ---------- GIF 解码 ----------

/// 单帧：已缩放到展示尺寸并预乘的 ARGB 数据（X ZPixmap 32bpp 小端字节序）。
#[derive(Clone)]
struct Frame {
    data: Vec<u8>,
    delay: Duration,
}

#[derive(Clone)]
struct PoseClip {
    frames: Vec<Frame>,
    loop_pose: bool,
}

/// 解析一个姿势 GIF。逐帧合成到画布后整体缩放（GIF 帧可能是局部更新），
/// 解析中途失败返回 None（该姿势不可用，不阻塞其他姿势）。
fn load_pose(skin: &str, pose: (&str, bool)) -> Option<PoseClip> {
    let (name, loop_pose) = pose;
    let path = pet_root()?.join(skin).join(format!("{name}.gif"));
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(File::open(path).ok()?).ok()?;
    let canvas_w = decoder.width() as usize;
    let canvas_h = decoder.height() as usize;
    if canvas_w == 0 || canvas_h == 0 {
        return None;
    }

    let mut canvas = vec![0u8; canvas_w * canvas_h * 4];
    let mut frames = Vec::new();
    while let Some(frame) = decoder.read_next_frame().ok()? {
        let frame_w = frame.width as usize;
        let frame_h = frame.height as usize;
        let offset_x = frame.left as usize;
        let offset_y = frame.top as usize;
        for row in 0..frame_h {
            let dst_y = offset_y + row;
            if dst_y >= canvas_h {
                break;
            }
            for col in 0..frame_w {
                let dst_x = offset_x + col;
                if dst_x >= canvas_w {
                    break;
                }
                let src = (row * frame_w + col) * 4;
                let dst = (dst_y * canvas_w + dst_x) * 4;
                canvas[dst..dst + 4].copy_from_slice(&frame.buffer[src..src + 4]);
            }
        }
        frames.push(Frame {
            data: resize_premultiply(&canvas, canvas_w, canvas_h),
            delay: Duration::from_millis((frame.delay as u64 * 10).max(20)),
        });
        // Background 丢弃：下一帧合成前把这块区域清回透明
        if frame.dispose == gif::DisposalMethod::Background {
            for row in 0..frame_h {
                let dst_y = offset_y + row;
                if dst_y >= canvas_h {
                    break;
                }
                for col in 0..frame_w {
                    let dst_x = offset_x + col;
                    if dst_x >= canvas_w {
                        break;
                    }
                    let dst = (dst_y * canvas_w + dst_x) * 4;
                    canvas[dst..dst + 4].copy_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
    }

    if frames.is_empty() {
        None
    } else {
        Some(PoseClip { frames, loop_pose })
    }
}

/// 双线性缩放到展示尺寸并做 alpha 预乘 + RGBA → BGRA 字节序
/// （X ZPixmap 32bpp 按 u32 (a<<24 | r<<16 | g<<8 | b) 小端存储）。
fn resize_premultiply(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = PET_WIDTH as usize;
    let dst_h = PET_HEIGHT as usize;
    let sample = |x: usize, y: usize, channel: usize| -> f64 {
        src[(y * src_w + x) * 4 + channel] as f64
    };

    let mut out = vec![0u8; dst_w * dst_h * 4];
    for y in 0..dst_h {
        let source_y = (y as f64 + 0.5) * src_h as f64 / dst_h as f64 - 0.5;
        let y0 = source_y.floor().max(0.0) as usize;
        let y1 = (y0 + 1).min(src_h - 1);
        let weight_y = (source_y - y0 as f64).clamp(0.0, 1.0);
        for x in 0..dst_w {
            let source_x = (x as f64 + 0.5) * src_w as f64 / dst_w as f64 - 0.5;
            let x0 = source_x.floor().max(0.0) as usize;
            let x1 = (x0 + 1).min(src_w - 1);
            let weight_x = (source_x - x0 as f64).clamp(0.0, 1.0);
            for channel in 0..4 {
                let top = sample(x0, y0, channel) * (1.0 - weight_x)
                    + sample(x1, y0, channel) * weight_x;
                let bottom = sample(x0, y1, channel) * (1.0 - weight_x)
                    + sample(x1, y1, channel) * weight_x;
                let value = top * (1.0 - weight_y) + bottom * weight_y;
                out[(y * dst_w + x) * 4 + channel] = value.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    for pixel in out.chunks_exact_mut(4) {
        let (red, green, blue, alpha) = (pixel[0], pixel[1], pixel[2], pixel[3]);
        let alpha = alpha as u32;
        pixel[0] = (blue as u32 * alpha / 255) as u8;
        pixel[1] = (green as u32 * alpha / 255) as u8;
        pixel[2] = (red as u32 * alpha / 255) as u8;
        pixel[3] = alpha as u8;
    }
    out
}

// ---------- 窗口与事件循环 ----------

/// 启动桌宠线程。失败只打日志，不影响截图与其他功能（与悬浮球线程同规则）。
/// 返回命令通道的发送端（同时已存入静态，供 set_visible / react / reload 使用）。
pub fn spawn(tx: Sender<Msg>, initially_visible: bool) -> Sender<PetCmd> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<PetCmd>();
    *CMD_TX.lock().unwrap() = Some(cmd_tx.clone());
    std::thread::spawn(move || {
        if let Err(e) = run(tx, cmd_rx, initially_visible) {
            eprintln!("windowsnap: 桌宠不可用（{e}），不影响截图与其他功能");
            *CMD_TX.lock().unwrap() = None;
        }
    });
    cmd_tx
}

struct Pet {
    conn: Arc<RustConnection>,
    screen_num: usize,
    window: Window,
    pixmap: u32,
    gc: u32,
    /// X scanline 对齐（位）；32 深度下通常为 32。
    scanline_pad: usize,
    clips: Vec<Option<PoseClip>>,
    current: usize,
    frame: usize,
    next_frame: Instant,
    visible: bool,
    /// 按下左键时指针的 root 坐标；None 表示未按下。
    pressed: Option<(i16, i16)>,
    /// 按下时窗口自身的位置。
    press_origin: (i32, i32),
    dragging: bool,
}

fn run(tx: Sender<Msg>, cmd_rx: Receiver<PetCmd>, initially_visible: bool) -> Result<()> {
    // 无素材时仍建立窗口与事件循环：用户稍后放入素材并在设置里切换即可生效，
    // 不必重启进程（与 Windows SwitchUiMode / macOS applyDesktopForm 一致）。
    let skin = resolve_skin();
    let (raw_conn, screen_num) = x11rb::connect(None)?;
    let conn: Arc<RustConnection> = Arc::new(raw_conn);
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;

    // 32 位 ARGB visual：真彩色 + 深度 32，窗口逐像素半透明
    let mut argb = None;
    for depth in &screen.allowed_depths {
        if depth.depth != 32 {
            continue;
        }
        for visual in &depth.visuals {
            if visual.class == VisualClass::TRUE_COLOR {
                argb = Some(visual.visual_id);
                break;
            }
        }
        if argb.is_some() {
            break;
        }
    }
    let Some(visual) = argb else {
        return Err("X 服务器没有 32 位 TrueColor visual，桌宠无法逐像素半透明".into());
    };
    let Some(format) = conn.setup().pixmap_formats.iter().find(|f| f.depth == 32).copied() else {
        return Err("X 服务器没有 32 位像素格式".into());
    };

    let colormap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, colormap, root, visual)?.check()?;

    let window = conn.generate_id()?;
    conn.create_window(
        32,
        window,
        root,
        -100,
        -100,
        PET_WIDTH as u16,
        PET_HEIGHT as u16,
        0,
        WindowClass::INPUT_OUTPUT,
        visual,
        &CreateWindowAux::new()
            .background_pixel(0)
            .border_pixel(0)
            .colormap(colormap)
            .override_redirect(1)
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::BUTTON_MOTION,
            ),
    )?
    .check()?;

    let pixmap = conn.generate_id()?;
    conn.create_pixmap(32, pixmap, window, PET_WIDTH as u16, PET_HEIGHT as u16)?
        .check()?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, pixmap, &CreateGCAux::new().graphics_exposures(0))?.check()?;

    // 初始位置：配置的 desktoppet_x/y（钳制到屏幕内）或默认右下角
    let screen_w = screen.width_in_pixels as i32;
    let screen_h = screen.height_in_pixels as i32;
    let (default_x, default_y) = (
        screen_w - PET_WIDTH - MARGIN,
        screen_h - PET_HEIGHT - MARGIN,
    );
    let (x, y) = settings::load()
        .desktoppet
        .map(|(x, y)| clamp_position(x, y, screen_w, screen_h))
        .unwrap_or((default_x, default_y));
    conn.configure_window(window, &ConfigureWindowAux::new().x(x).y(y))?
        .check()?;

    let mut pet = Pet {
        conn: Arc::clone(&conn),
        screen_num,
        window,
        pixmap,
        gc,
        scanline_pad: format.scanline_pad as usize,
        clips: match &skin {
            Some(name) => POSES.iter().map(|pose| load_pose(name, *pose)).collect(),
            None => POSES.iter().map(|_| None).collect(),
        },
        current: POSE_IDLE,
        frame: 0,
        next_frame: Instant::now(),
        visible: false,
        pressed: None,
        press_origin: (0, 0),
        dragging: false,
    };
    ensure_idle_fallback(&mut pet.clips);

    if initially_visible && pet.clips[POSE_IDLE].is_some() {
        conn.map_window(window)?;
        conn.flush()?;
        pet.visible = true;
    }

    pet.event_loop(tx, cmd_rx)
}

impl Pet {
    fn event_loop(&mut self, tx: Sender<Msg>, cmd_rx: Receiver<PetCmd>) -> Result<()> {
        loop {
            // 1. X 事件（非阻塞；动画计时由循环底部的 sleep 驱动）
            while let Some(event) = self.conn.poll_for_event()? {
                match event {
                    Event::Expose(_) => self.blit(),
                    Event::ButtonPress(e) if e.detail == 3 => {
                        // 右键菜单交给主线程（zenity 不阻塞动画，也不堵住 SetVisible 握手）
                        let _ = tx.send(Msg::PetContextMenu);
                    }
                    Event::ButtonPress(e) if e.detail == 1 => {
                        // X 在按钮按下时自动独占指针，松开前事件都发给我们
                        let geometry = self.conn.get_geometry(self.window)?.reply()?;
                        self.press_origin = (geometry.x as i32, geometry.y as i32);
                        self.pressed = Some((e.root_x, e.root_y));
                        self.dragging = false;
                    }
                    Event::MotionNotify(e) => {
                        if let Some((press_x, press_y)) = self.pressed {
                            let delta_x = e.root_x as i32 - press_x as i32;
                            let delta_y = e.root_y as i32 - press_y as i32;
                            if !self.dragging
                                && (delta_x.abs() > DRAG_THRESHOLD || delta_y.abs() > DRAG_THRESHOLD)
                            {
                                self.dragging = true;
                            }
                            if self.dragging {
                                let (screen_w, screen_h) = self.screen_size();
                                let (x, y) = clamp_position(
                                    self.press_origin.0 + delta_x,
                                    self.press_origin.1 + delta_y,
                                    screen_w,
                                    screen_h,
                                );
                                self.conn.configure_window(
                                    self.window,
                                    &ConfigureWindowAux::new().x(x).y(y),
                                )?;
                                self.conn.flush()?;
                                self.set_pose(if delta_x >= 0 {
                                    POSE_RUN_RIGHT
                                } else {
                                    POSE_RUN_LEFT
                                });
                            }
                        }
                    }
                    Event::ButtonRelease(e) if e.detail == 1 => {
                        self.pressed = None;
                        if self.dragging {
                            self.dragging = false;
                            let geometry = self.conn.get_geometry(self.window)?.reply()?;
                            settings::save_desktoppet_position(geometry.x as i32, geometry.y as i32);
                            self.set_pose(POSE_IDLE);
                        } else {
                            // 单击：开关功能菜单（与悬浮球单击行为一致）
                            let _ = tx.send(Msg::TogglePanel);
                        }
                    }
                    _ => {}
                }
            }

            // 2. 控制命令
            while let Ok(cmd) = cmd_rx.try_recv() {
                match cmd {
                    PetCmd::SetVisible { visible, done } => {
                        self.set_visible(visible);
                        let _ = done.send(self.visible);
                    }
                    PetCmd::Reload { skin, done } => {
                        self.reload_skin(&skin);
                        let _ = done.send(());
                    }
                    PetCmd::SetPose(pose) => {
                        if self.visible {
                            self.set_pose(pose);
                        }
                    }
                }
            }

            // 3. 动画推进（隐藏时暂停，重新显示后从当前帧继续）
            let now = Instant::now();
            if self.visible && now >= self.next_frame {
                self.advance(now);
            }

            std::thread::sleep(TICK);
        }
    }

    fn screen_size(&self) -> (i32, i32) {
        let screen = &self.conn.setup().roots[self.screen_num];
        (
            screen.width_in_pixels as i32,
            screen.height_in_pixels as i32,
        )
    }

    fn set_visible(&mut self, visible: bool) {
        if visible && self.clips[POSE_IDLE].is_none() {
            if let Some(skin) = resolve_skin() {
                self.reload_skin(&skin);
            }
            if self.clips[POSE_IDLE].is_none() {
                eprintln!(
                    "windowsnap: 未找到桌宠素材（$XDG_DATA_HOME/windowsnap/pet/<形象>/*.gif），保持隐藏"
                );
                return;
            }
        }
        if self.visible == visible {
            return;
        }
        self.visible = visible;
        let _ = if visible {
            self.conn.map_window(self.window)
        } else {
            self.conn.unmap_window(self.window)
        };
        let _ = self.conn.flush();
        if visible {
            self.next_frame = Instant::now();
            self.blit();
        }
    }

    /// 换形象：重载全部姿势；新形象完全不可用时保持原样并打日志。
    fn reload_skin(&mut self, skin: &str) {
        let clips: Vec<Option<PoseClip>> = POSES.iter().map(|pose| load_pose(skin, *pose)).collect();
        if clips.iter().all(|clip| clip.is_none()) {
            eprintln!("windowsnap: 形象「{skin}」没有可用素材，保持原形象");
            return;
        }
        self.clips = clips;
        ensure_idle_fallback(&mut self.clips);
        self.set_pose(POSE_IDLE);
    }

    /// 切换姿势。循环态重复调用不重置帧（拖动中持续跑动）；一次性动画重播。
    fn set_pose(&mut self, pose: usize) {
        if pose >= self.clips.len() || self.clips[pose].is_none() {
            return;
        }
        if self.current == pose && self.clips[pose].as_ref().is_some_and(|c| c.loop_pose) {
            return;
        }
        self.current = pose;
        self.frame = 0;
        self.next_frame = Instant::now();
        self.blit();
    }

    fn advance(&mut self, now: Instant) {
        let Some(clip) = self.clips[self.current].as_ref() else {
            return;
        };
        self.frame += 1;
        if self.frame >= clip.frames.len() {
            if clip.loop_pose {
                self.frame = 0;
            } else {
                // 单次动画播完回待机
                self.set_pose(POSE_IDLE);
                return;
            }
        }
        let delay = self.clips[self.current]
            .as_ref()
            .and_then(|clip| clip.frames.get(self.frame).map(|frame| frame.delay))
            .unwrap_or(Duration::from_millis(100))
            .max(MIN_FRAME_DELAY);
        self.next_frame = now + delay;
        self.blit();
    }

    /// 当前帧写入后备 pixmap 并刷到窗口。
    fn blit(&self) {
        let Some(clip) = self.clips[self.current].as_ref() else {
            return;
        };
        let Some(frame) = clip.frames.get(self.frame) else {
            return;
        };
        let width = PET_WIDTH as usize;
        let height = PET_HEIGHT as usize;
        let bytes_per_pixel = 4usize;
        let pad = self.scanline_pad.max(8);
        let stride = (width * bytes_per_pixel * 8).div_ceil(pad) * pad / 8;
        let mut data = vec![0u8; stride * height];
        for (row, line) in frame.data.chunks(width * bytes_per_pixel).enumerate() {
            let row_start = row * stride;
            data[row_start..row_start + width * bytes_per_pixel]
                .copy_from_slice(&line[..width * bytes_per_pixel]);
        }
        let _ = self.conn.put_image(
            ImageFormat::Z_PIXMAP,
            self.pixmap,
            self.gc,
            PET_WIDTH as u16,
            PET_HEIGHT as u16,
            0,
            0,
            0,
            32,
            &data,
        );
        let _ = self.conn.copy_area(
            self.pixmap,
            self.window,
            self.gc,
            0,
            0,
            PET_WIDTH as u16,
            PET_HEIGHT as u16,
            0,
            0,
        );
        let _ = self.conn.flush();
    }
}

/// idle 缺失时克隆任意可用姿势，避免空窗口；不 take，以免抽空原来的姿势槽。
fn ensure_idle_fallback(clips: &mut [Option<PoseClip>]) {
    if matches!(clips.get(POSE_IDLE), Some(None)) {
        if let Some(index) = clips.iter().position(|clip| clip.is_some()) {
            clips[POSE_IDLE] = clips[index].clone();
        }
    }
}

/// 位置钳制：保持在屏幕可见区域内（距边缘至少 MARGIN），与悬浮球同规则。
fn clamp_position(x: i32, y: i32, screen_w: i32, screen_h: i32) -> (i32, i32) {
    let x = x.clamp(MARGIN, (screen_w - PET_WIDTH - MARGIN).max(MARGIN));
    let y = y.clamp(MARGIN, (screen_h - PET_HEIGHT - MARGIN).max(MARGIN));
    (x, y)
}
