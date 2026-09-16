//! 原生 Wayland layer-shell；无该协议时由调用者回退 XWayland。
use std::fs::{File, OpenOptions};
use std::os::fd::AsFd;
use std::os::unix::fs::{FileExt, OpenOptionsExt};
use std::sync::{Arc, mpsc::{Receiver, Sender}};
use std::time::{Duration, Instant};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, QueueHandle, WEnum};
use wayland_client::protocol::{wl_buffer, wl_callback, wl_compositor, wl_output, wl_pointer, wl_region, wl_registry, wl_seat, wl_shm, wl_shm_pool, wl_surface};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};
use super::{Control, PetPose, decode::Rgba, render::{FrameCache, WIDTH, HEIGHT}, state::PetState};
use crate::{Msg, Result};

pub struct Surface { conn: Connection, queue: EventQueue<WaylandState>, state: WaylandState }
struct Output { object: wl_output::WlOutput, size: (i32,i32), scale: i32, rotated: bool }
#[derive(Default)]
struct WaylandState {
    compositor: Option<wl_compositor::WlCompositor>,
    shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    shm: Option<wl_shm::WlShm>,
    outputs: Vec<Output>,
    pointer: Option<wl_pointer::WlPointer>,
    surface: Option<wl_surface::WlSurface>,
    layer: Option<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1>,
    callback: Option<wl_callback::WlCallback>,
    busy: [bool;2],
    configured: bool,
    closed: bool,
    events: Vec<Input>,
    cursor: (f64,f64),
}
enum Input { Press(f64,f64), Motion(f64,f64), Release, Menu }

impl Surface {
    pub fn connect() -> Result<Self> {
        let conn = Connection::connect_to_env()?;
        let mut queue = conn.new_event_queue();
        let mut state = WaylandState::default();
        conn.display().get_registry(&queue.handle(), ());
        queue.roundtrip(&mut state)?;
        if state.shell.is_none() { return Err("合成器未提供 zwlr_layer_shell_v1".into()); }
        if state.compositor.is_none() || state.shm.is_none() { return Err("缺少 wl_compositor / wl_shm".into()); }
        queue.roundtrip(&mut state)?;
        Ok(Self { conn, queue, state })
    }

    pub fn run(mut self, pet: &mut PetState, tx: &Sender<Msg>, poses: &Receiver<PetPose>, controls: &Receiver<Control>) -> Result<()> {
        let qh = self.queue.handle();
        let compositor = self.state.compositor.as_ref().unwrap().clone();
        let surface = compositor.create_surface(&qh, ());
        // layer-shell 的坐标只在一个 output 内有效，明确绑定到枚举到的第一个 output。
        let output = self.state.outputs.first().ok_or("没有可用 Wayland output")?;
        let logical_size = if output.rotated { (output.size.1/output.scale,output.size.0/output.scale) } else { (output.size.0/output.scale,output.size.1/output.scale) };
        let clamp = |x:i32,y:i32| (x.clamp(8,(logical_size.0-WIDTH as i32-8).max(8)),y.clamp(8,(logical_size.1-HEIGHT as i32-8).max(8)));
        let saved = crate::settings::load().desktop_pet.filter(|&(x,y)| x >= -200 && y >= -200 && x < logical_size.0+200 && y < logical_size.1+200);
        let (x,y) = saved.unwrap_or((logical_size.0-WIDTH as i32-16,logical_size.1-HEIGHT as i32-16));
        let mut position = clamp(x,y);
        let layer = self.state.shell.as_ref().unwrap().get_layer_surface(&surface,Some(&output.object),zwlr_layer_shell_v1::Layer::Overlay,"windowsnap-pet".into(),&qh,());
        self.state.surface = Some(surface.clone());
        self.state.layer = Some(layer.clone());
        configure(&layer,position);
        surface.commit();
        self.conn.flush()?;
        let file = shm_file()?;
        let slot_size = WIDTH as usize * HEIGHT as usize * 4;
        file.set_len((slot_size*2) as u64)?;
        let pool = self.state.shm.as_ref().unwrap().create_pool(file.as_fd(),(slot_size*2) as i32,&qh,());
        let buffers: Vec<_> = (0..2).map(|i| pool.create_buffer((i*slot_size) as i32,WIDTH as i32,HEIGHT as i32,WIDTH as i32*4,wl_shm::Format::Argb8888,&qh,i)).collect();
        pool.destroy();
        let mut cache = FrameCache::default();
        let mut frame: Option<Arc<Rgba>> = pet.start(Instant::now());
        let mut dirty = true;
        let mut visible = crate::desktop::visible();
        let mut pressed: Option<(f64,f64)> = None;
        let mut press_origin = position;
        let mut dragging = false;
        while !self.state.closed {
            self.queue.dispatch_pending(&mut self.state)?;
            if let Some(guard) = self.queue.prepare_read() {
                match guard.read() {
                    Ok(_) => {},
                    Err(wayland_client::backend::WaylandError::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {},
                    Err(e) => return Err(e.into()),
                }
            }
            self.queue.dispatch_pending(&mut self.state)?;
            while let Ok(control) = controls.try_recv() {
                match control {
                    Control::Visible(show,ack) => {
                        if show != visible {
                            visible = show;
                            if let Some(callback) = self.state.callback.take() {
                                // wl_callback 没有 destructor；旧回调到达时按对象身份忽略。
                                drop(callback);
                            }
                            if !show {
                                surface.attach(None,0,0);
                                surface.commit();
                                self.state.configured = false;
                                pressed = None;
                            } else {
                                configure(&layer,position);
                                surface.commit();
                                dirty = true;
                            }
                            self.conn.flush()?;
                            self.queue.roundtrip(&mut self.state)?;
                        }
                        let _ = ack.send(());
                    }
                    Control::Quit => self.state.closed = true,
                }
            }
            while let Ok(pose) = poses.try_recv() {
                if let Some(next) = pet.set_pose(pose,Instant::now()) { frame = Some(next); dirty = true; }
            }
            for event in std::mem::take(&mut self.state.events) {
                match event {
                    Input::Press(x,y) => { pressed = Some((position.0 as f64+x,position.1 as f64+y)); press_origin = position; dragging = false; }
                    Input::Motion(x,y) => if let Some((px,py)) = pressed {
                        let (dx,dy) = (position.0 as f64+x-px,position.1 as f64+y-py);
                        dragging |= dx.abs() >= 4.0 || dy.abs() >= 4.0;
                        if dragging {
                            position = clamp(press_origin.0+dx.round() as i32,press_origin.1+dy.round() as i32);
                            layer.set_margin(position.1,0,0,position.0);
                            surface.commit();
                            if let Some(next) = pet.set_pose(if dx >= 0.0 { PetPose::RunningRight } else { PetPose::RunningLeft },Instant::now()) { frame = Some(next); dirty = true; }
                        }
                    },
                    Input::Release => {
                        pressed = None;
                        if dragging {
                            crate::save_desktop_pet_position(position.0,position.1);
                            if let Some(next) = pet.set_pose(PetPose::Idle,Instant::now()) { frame = Some(next); dirty = true; }
                        } else { let _ = tx.send(Msg::TogglePanel); }
                        dragging = false;
                    }
                    Input::Menu => super::context_menu(tx),
                }
            }
            if let Some(next) = pet.tick(Instant::now()) { frame = Some(next); dirty = true; }
            if visible && self.state.configured && dirty && self.state.callback.is_none() {
                if let Some(slot) = self.state.busy.iter().position(|busy| !busy) {
                    let region = compositor.create_region(&qh,());
                    if let Some(source) = frame.as_ref() {
                        let scaled = cache.get(source);
                        // wl_shm ARGB8888 使用本机字节序；解码缓存固定为小端 BGRA。
                        #[cfg(target_endian = "little")]
                        file.write_all_at(&scaled.pixels,(slot*slot_size) as u64)?;
                        #[cfg(target_endian = "big")]
                        { let mut bytes = scaled.pixels.clone(); for p in bytes.chunks_exact_mut(4) { p.reverse(); } file.write_all_at(&bytes,(slot*slot_size) as u64)?; }
                        for &(x,y,w) in &scaled.spans { region.add(x,y,w,1); }
                    } else { file.write_all_at(&vec![0;slot_size],(slot*slot_size) as u64)?; }
                    surface.set_input_region(Some(&region));
                    region.destroy();
                    surface.attach(Some(&buffers[slot]),0,0);
                    surface.damage(0,0,WIDTH as i32,HEIGHT as i32);
                    self.state.callback = Some(surface.frame(&qh,()));
                    surface.commit();
                    self.state.busy[slot] = true;
                    dirty = false;
                }
            }
            self.conn.flush()?;
            std::thread::sleep(Duration::from_millis(8));
        }
        layer.destroy();
        surface.destroy();
        for buffer in buffers { buffer.destroy(); }
        self.conn.flush()?;
        Ok(())
    }
}

fn configure(layer:&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,position:(i32,i32)) {
    layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top | zwlr_layer_surface_v1::Anchor::Left);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::None);
    layer.set_size(WIDTH as u32,HEIGHT as u32);
    layer.set_margin(position.1,0,0,position.0);
}

fn shm_file() -> Result<File> {
    let root = std::env::var_os("XDG_RUNTIME_DIR").ok_or("XDG_RUNTIME_DIR 未设置")?;
    let name = format!("windowsnap-pet-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos());
    let path = std::path::PathBuf::from(root).join(name);
    let file = OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(&path)?;
    std::fs::remove_file(path)?;
    Ok(file)
}

impl Dispatch<wl_registry::WlRegistry,()> for WaylandState {
    fn event(s:&mut Self,r:&wl_registry::WlRegistry,e:wl_registry::Event,_:&(),_:&Connection,q:&QueueHandle<Self>) {
        match e {
            wl_registry::Event::Global { name,interface,version } => match interface.as_str() {
                "wl_compositor" => s.compositor = Some(r.bind(name,version.min(4),q,())),
                "wl_shm" => s.shm = Some(r.bind(name,1,q,())),
                "zwlr_layer_shell_v1" => s.shell = Some(r.bind(name,version.min(4),q,())),
                "wl_seat" => { let _:wl_seat::WlSeat = r.bind(name,version.min(5),q,()); }
                "wl_output" => { let object = r.bind(name,version.min(3),q,()); s.outputs.push(Output { object,size:(0,0),scale:1,rotated:false }); }
                _ => {}
            },
            _ => {}
        }
    }
}
impl Dispatch<wl_output::WlOutput,()> for WaylandState {
    fn event(s:&mut Self,o:&wl_output::WlOutput,e:wl_output::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        let Some(output) = s.outputs.iter_mut().find(|v| v.object == *o) else { return };
        match e {
            wl_output::Event::Mode { flags:WEnum::Value(flags),width,height,.. } if flags.contains(wl_output::Mode::Current) => output.size = (width,height),
            wl_output::Event::Scale { factor } => output.scale = factor.max(1),
            wl_output::Event::Geometry { transform:WEnum::Value(t),.. } => output.rotated = matches!(t,wl_output::Transform::_90 | wl_output::Transform::_270 | wl_output::Transform::Flipped90 | wl_output::Transform::Flipped270),
            _ => {}
        }
    }
}
impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,()> for WaylandState {
    fn event(s:&mut Self,l:&zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,e:zwlr_layer_surface_v1::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        match e {
            zwlr_layer_surface_v1::Event::Configure { serial,.. } => { l.ack_configure(serial); s.configured = true; }
            zwlr_layer_surface_v1::Event::Closed => s.closed = true,
            _ => {}
        }
    }
}
impl Dispatch<wl_buffer::WlBuffer,usize> for WaylandState {
    fn event(s:&mut Self,_:&wl_buffer::WlBuffer,e:wl_buffer::Event,slot:&usize,_:&Connection,_:&QueueHandle<Self>) { if let wl_buffer::Event::Release = e { s.busy[*slot] = false; } }
}
impl Dispatch<wl_callback::WlCallback,()> for WaylandState {
    fn event(s:&mut Self,c:&wl_callback::WlCallback,_:wl_callback::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) { if s.callback.as_ref() == Some(c) { s.callback = None; } }
}
impl Dispatch<wl_seat::WlSeat,()> for WaylandState {
    fn event(s:&mut Self,seat:&wl_seat::WlSeat,e:wl_seat::Event,_:&(),_:&Connection,q:&QueueHandle<Self>) {
        if let wl_seat::Event::Capabilities { capabilities:WEnum::Value(c) } = e {
            if c.contains(wl_seat::Capability::Pointer) && s.pointer.is_none() { s.pointer = Some(seat.get_pointer(q,())); }
            else if !c.contains(wl_seat::Capability::Pointer) { if let Some(p) = s.pointer.take() { p.release(); } }
        }
    }
}
impl Dispatch<wl_pointer::WlPointer,()> for WaylandState {
    fn event(s:&mut Self,_:&wl_pointer::WlPointer,e:wl_pointer::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {
        match e {
            wl_pointer::Event::Enter { surface_x,surface_y,.. } => s.cursor = (surface_x,surface_y),
            wl_pointer::Event::Motion { surface_x,surface_y,.. } => { s.cursor = (surface_x,surface_y); s.events.push(Input::Motion(surface_x,surface_y)); }
            wl_pointer::Event::Button { button,state:WEnum::Value(state),.. } => match (button,state) {
                (0x110,wl_pointer::ButtonState::Pressed) => s.events.push(Input::Press(s.cursor.0,s.cursor.1)),
                (0x110,wl_pointer::ButtonState::Released) => s.events.push(Input::Release),
                (0x111,wl_pointer::ButtonState::Pressed) => s.events.push(Input::Menu),
                _ => {}
            },
            _ => {}
        }
    }
}
delegate_noop!(WaylandState: ignore wl_compositor::WlCompositor);
delegate_noop!(WaylandState: ignore wl_surface::WlSurface);
delegate_noop!(WaylandState: ignore wl_shm::WlShm);
delegate_noop!(WaylandState: ignore wl_region::WlRegion);
delegate_noop!(WaylandState: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);
impl Dispatch<wl_shm_pool::WlShmPool,()> for WaylandState {
    fn event(_:&mut Self,_:&wl_shm_pool::WlShmPool,_:wl_shm_pool::Event,_:&(),_:&Connection,_:&QueueHandle<Self>) {}
}
