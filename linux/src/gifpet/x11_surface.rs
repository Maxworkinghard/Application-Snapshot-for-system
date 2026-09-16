//! ARGB32 桌宠窗口，原生 X11 和 XWayland 共用；不使用主动指针抓取。
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::image::{BitsPerPixel, Image, ImageOrder, ScanlinePad};
use x11rb::protocol::{Event, randr, shape};
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use super::{Control, PetPose, render::{FrameCache, WIDTH, HEIGHT}, state::PetState};
use crate::{Msg, Result};

#[derive(Clone, Copy)]
struct Area { x: i32, y: i32, w: i32, h: i32 }
impl Area {
    fn contains(self, x: i32, y: i32, margin: i32) -> bool {
        x >= self.x - margin && y >= self.y - margin && x < self.x + self.w + margin && y < self.y + self.h + margin
    }
    fn clamp(self, x: i32, y: i32) -> (i32, i32) {
        (x.clamp(self.x + 8, (self.x + self.w - WIDTH as i32 - 8).max(self.x + 8)),
         y.clamp(self.y + 8, (self.y + self.h - HEIGHT as i32 - 8).max(self.y + 8)))
    }
}

fn monitors(conn: &RustConnection, root: Window, screen: usize) -> Vec<Area> {
    if let Ok(cookie) = randr::get_monitors(conn, root, true) {
        if let Ok(reply) = cookie.reply() {
            let mut items = reply.monitors;
            items.sort_by_key(|m| !m.primary);
            let areas: Vec<_> = items.iter().map(|m| Area { x: m.x as i32, y: m.y as i32, w: m.width as i32, h: m.height as i32 }).collect();
            if !areas.is_empty() { return areas; }
        }
    }
    let s = &conn.setup().roots[screen];
    vec![Area { x: 0, y: 0, w: s.width_in_pixels as i32, h: s.height_in_pixels as i32 }]
}

fn working_area(conn: &RustConnection, root: Window, area: Area) -> Area {
    let query = |name: &[u8], count| -> Option<Vec<u32>> {
        let atom = conn.intern_atom(false, name).ok()?.reply().ok()?.atom;
        Some(conn.get_property(false, root, atom, AtomEnum::CARDINAL, 0, count).ok()?.reply().ok()?.value32()?.collect())
    };
    let desktop = query(b"_NET_CURRENT_DESKTOP", 1).and_then(|v| v.first().copied()).unwrap_or(0) as usize;
    let Some(values) = query(b"_NET_WORKAREA", (desktop as u32 + 1) * 4) else { return area };
    let Some(v) = values.get(desktop * 4..desktop * 4 + 4) else { return area };
    let x = area.x.max(v[0] as i32);
    let y = area.y.max(v[1] as i32);
    let w = (area.x + area.w).min(v[0] as i32 + v[2] as i32) - x;
    let h = (area.y + area.h).min(v[1] as i32 + v[3] as i32) - y;
    if w > WIDTH as i32 + 16 && h > HEIGHT as i32 + 16 { Area { x, y, w, h } } else { area }
}

pub fn run(state: &mut PetState, tx: &Sender<Msg>, poses: &Receiver<PetPose>, controls: &Receiver<Control>) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let screen = &conn.setup().roots[screen_num];
    let root = screen.root;
    let visual = screen.allowed_depths.iter().filter(|d| d.depth == 32)
        .flat_map(|d| &d.visuals).find(|v| v.class == VisualClass::TRUE_COLOR && v.red_mask == 0xff0000 && v.green_mask == 0xff00 && v.blue_mask == 0xff)
        .ok_or("X server 未提供 ARGB32 visual")?.visual_id;
    let cmap = conn.generate_id()?;
    conn.create_colormap(ColormapAlloc::NONE, cmap, root, visual)?.check()?;
    let window = conn.generate_id()?;
    conn.create_window(32, window, root, 0, 0, WIDTH, HEIGHT, 0, WindowClass::INPUT_OUTPUT, visual,
        &CreateWindowAux::new().background_pixel(0).border_pixel(0).colormap(cmap).override_redirect(1)
        .event_mask(EventMask::EXPOSURE | EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::BUTTON_MOTION))?.check()?;
    let pixmap = conn.generate_id()?;
    conn.create_pixmap(32, pixmap, window, WIDTH, HEIGHT)?.check()?;
    let gc = conn.generate_id()?;
    conn.create_gc(gc, pixmap, &CreateGCAux::new().graphics_exposures(0))?.check()?;
    let compositor_atom = conn.intern_atom(false, format!("_NET_WM_CM_S{screen_num}").as_bytes())?.reply()?.atom;
    let mut composited = conn.get_selection_owner(compositor_atom)?.reply()?.owner != 0;
    let mut next_compositor_check = Instant::now();
    let mut areas = monitors(&conn, root, screen_num);
    let saved = crate::settings::load().desktop_pet;
    let area = saved.and_then(|(x,y)| areas.iter().copied().find(|a| a.contains(x,y,200))).unwrap_or(areas[0]);
    let area = working_area(&conn, root, area);
    let mut position = area.clamp(saved.filter(|&(x,y)| areas.iter().any(|a| a.contains(x,y,200))).map(|p|p.0).unwrap_or(area.x+area.w-WIDTH as i32-16),
                                  saved.filter(|&(x,y)| areas.iter().any(|a| a.contains(x,y,200))).map(|p|p.1).unwrap_or(area.y+area.h-HEIGHT as i32-16));
    conn.configure_window(window, &ConfigureWindowAux::new().x(position.0).y(position.1))?.check()?;
    let visible = crate::desktop::visible();
    let mut frame = state.start(Instant::now());
    let mut dirty = true;
    let mut cache = FrameCache::default();
    let mut images: HashMap<usize, Image<'static>> = HashMap::new();
    let mut pressed = None;
    let mut press_origin = position;
    let mut dragging = false;
    if visible { conn.map_window(window)?; }
    loop {
        while let Ok(control) = controls.try_recv() {
            match control {
                Control::Visible(show, ack) => {
                    if show { conn.map_window(window)?; } else { conn.unmap_window(window)?; }
                    conn.flush()?;
                    conn.get_input_focus()?.reply()?;
                    let _ = ack.send(());
                }
                Control::Quit => {
                    let _ = conn.destroy_window(window);
                    let _ = conn.free_gc(gc);
                    let _ = conn.free_pixmap(pixmap);
                    let _ = conn.flush();
                    return Ok(());
                }
            }
        }
        while let Ok(pose) = poses.try_recv() {
            if let Some(next) = state.set_pose(pose, Instant::now()) { frame = Some(next); dirty = true; }
        }
        while let Some(event) = conn.poll_for_event()? {
            match event {
                Event::Expose(_) => { frame = state.current_frame(); dirty = true; },
                Event::ButtonPress(e) if e.detail == 3 => super::context_menu(tx),
                Event::ButtonPress(e) if e.detail == 1 => { pressed = Some((e.root_x as i32,e.root_y as i32)); press_origin = position; dragging = false; }
                Event::MotionNotify(e) => if let Some((px,py)) = pressed {
                    let (dx,dy) = (e.root_x as i32-px,e.root_y as i32-py);
                    dragging |= dx.abs() >= 4 || dy.abs() >= 4;
                    if dragging {
                        areas = monitors(&conn, root, screen_num);
                        let area = areas.iter().copied().find(|a| a.contains(e.root_x as i32,e.root_y as i32,0)).unwrap_or(areas[0]);
                        position = working_area(&conn, root, area).clamp(press_origin.0+dx,press_origin.1+dy);
                        conn.configure_window(window, &ConfigureWindowAux::new().x(position.0).y(position.1))?;
                        if let Some(next) = state.set_pose(if dx >= 0 { PetPose::RunningRight } else { PetPose::RunningLeft }, Instant::now()) { frame = Some(next); dirty = true; }
                    }
                },
                Event::ButtonRelease(e) if e.detail == 1 => {
                    pressed = None;
                    if dragging {
                        crate::save_desktop_pet_position(position.0,position.1);
                        if let Some(next) = state.set_pose(PetPose::Idle,Instant::now()) { frame = Some(next); dirty = true; }
                    } else { let _ = tx.send(Msg::TogglePanel); }
                    dragging = false;
                }
                Event::DestroyNotify(e) if e.window == window => return Ok(()),
                _ => {}
            }
        }
        if let Some(next) = state.tick(Instant::now()) { frame = Some(next); dirty = true; }
        // 非 selection owner 收不到 SelectionClear；定期重查才能覆盖合成器退出/重启。
        if Instant::now() >= next_compositor_check {
            let new = conn.get_selection_owner(compositor_atom)?.reply()?.owner != 0;
            dirty |= new != composited;
            composited = new;
            next_compositor_check = Instant::now() + Duration::from_secs(1);
        }
        if dirty {
            if let Some(source) = frame.as_ref() {
                let scaled = cache.get(source);
                let key = std::sync::Arc::as_ptr(source) as usize;
                if !images.contains_key(&key) {
                    let image = Image::new(WIDTH, HEIGHT, ScanlinePad::Pad32, 32, BitsPerPixel::B32, ImageOrder::LsbFirst, Cow::Owned(scaled.pixels.clone()))?;
                    images.insert(key, image.native(conn.setup())?.into_owned());
                }
                for cookie in images[&key].put(&conn,pixmap,gc,0,0)? { cookie.check()?; }
                let rectangles: Vec<_> = scaled.spans.iter().map(|&(x,y,w)| Rectangle { x:x as i16,y:y as i16,width:w as u16,height:1 }).collect();
                shape::rectangles(&conn, shape::SO::SET, shape::SK::INPUT, ClipOrdering::UNSORTED,window,0,0,&rectangles)?.check()?;
                if composited {
                    shape::mask(&conn,shape::SO::SET,shape::SK::BOUNDING,window,0,0,x11rb::NONE)?;
                } else {
                    shape::rectangles(&conn,shape::SO::SET,shape::SK::BOUNDING,ClipOrdering::UNSORTED,window,0,0,&rectangles)?;
                }
                conn.copy_area(pixmap,window,gc,0,0,0,0,WIDTH,HEIGHT)?;
            } else {
                shape::rectangles(&conn,shape::SO::SET,shape::SK::INPUT,ClipOrdering::UNSORTED,window,0,0,&[])?;
            }
            dirty = false;
        }
        conn.flush()?;
        std::thread::sleep(state.time_to_next_frame().min(Duration::from_millis(16)));
    }
}
