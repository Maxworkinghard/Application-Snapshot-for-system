//! 桌宠素材：`$XDG_DATA_HOME/windowsnap/pet/<形象>/<动作>.gif`（默认 `~/.local/share/...`）。
//!
//! 素材不随应用内置或分发（素材并非本项目制作，避免版权问题），由用户自行放入；
//! 目录为空时桌宠模式不可用，启动与切换都回退悬浮球。动作名与 Windows / macOS 端一致，
//! 同一套素材三端可直接复用。
//!
//! GIF 只有全透明 / 不透明两种像素，所以帧解码后直接按 alpha 生成 X11 shape 掩码，
//! 无需合成器也能得到正确的镂空边缘（见 pet.rs 的 apply_frame_shape）。

use std::path::{Path, PathBuf};

/// 桌宠显示高度（px），宽度按素材比例换算；与 Windows / macOS 端取值一致。
pub const DISPLAY_HEIGHT: u32 = 168;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PetPose {
    Idle,
    Waving,
    Jumping,
    Failed,
    Waiting,
    RunningLeft,
    RunningRight,
}

impl PetPose {
    pub const ALL: [PetPose; 7] = [
        PetPose::Idle,
        PetPose::Waving,
        PetPose::Jumping,
        PetPose::Failed,
        PetPose::Waiting,
        PetPose::RunningLeft,
        PetPose::RunningRight,
    ];

    pub fn name(self) -> &'static str {
        match self {
            PetPose::Idle => "idle",
            PetPose::Waving => "waving",
            PetPose::Jumping => "jumping",
            PetPose::Failed => "failed",
            PetPose::Waiting => "waiting",
            PetPose::RunningLeft => "running-left",
            PetPose::RunningRight => "running-right",
        }
    }

    /// idle 与跑动持续循环，其余单次动画播完回 idle。
    pub fn loops(self) -> bool {
        matches!(self, PetPose::Idle | PetPose::RunningLeft | PetPose::RunningRight)
    }

    pub fn index(self) -> usize {
        PetPose::ALL.iter().position(|pose| *pose == self).unwrap_or(0)
    }
}

/// 一帧：已缩放到显示尺寸的 RGBA，以及这一帧的停留时长。
pub struct Frame {
    pub rgba: Vec<u8>,
    pub delay_ms: u32,
}

pub struct Clip {
    pub frames: Vec<Frame>,
    pub loops: bool,
}

/// 一个形象的全部动作，统一缩放到同一显示尺寸（窗口尺寸据此确定）。
pub struct Clips {
    pub width: u32,
    pub height: u32,
    slots: Vec<Option<Clip>>,
}

impl Clips {
    /// 该动作是否有独立素材（没有就别切，保持当前动画，与 Windows 端 SetPose 同规则）。
    pub fn has(&self, pose: PetPose) -> bool {
        self.slots[pose.index()].is_some()
    }

    /// 取动作素材；缺失时退化到 idle，idle 也缺失就用任意可用动作，避免空窗口。
    pub fn clip(&self, pose: PetPose) -> Option<&Clip> {
        self.slots[pose.index()].as_ref().or_else(|| self.any())
    }

    fn any(&self) -> Option<&Clip> {
        self.slots[PetPose::Idle.index()]
            .as_ref()
            .or_else(|| self.slots.iter().flatten().next())
    }
}

/// 素材根目录：$XDG_DATA_HOME/windowsnap/pet，回退 ~/.local/share/windowsnap/pet。
pub fn root() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map(|base| base.join("windowsnap/pet"))
}

pub fn root_display() -> String {
    root()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| "~/.local/share/windowsnap/pet".to_string())
}

/// 可用形象 = pet 目录下含至少一个 gif 的子目录，按名称排序。
pub fn available_skins() -> Vec<String> {
    let Some(root) = root() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&root) else { return Vec::new() };
    let mut skins: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir() && has_gif(&entry.path()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect();
    skins.sort();
    skins
}

fn has_gif(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("gif"))
        })
    })
}

/// 解析当前选择：选中的形象不可用时退回第一个可用形象，一个都没有则返回 None。
pub fn resolve_skin(preferred: &str) -> Option<String> {
    let skins = available_skins();
    if skins.iter().any(|skin| skin == preferred) {
        return Some(preferred.to_string());
    }
    skins.into_iter().next()
}

/// 载入一个形象的全部动作。先按 idle（或任意可用动作）定下显示尺寸，
/// 其余动作统一缩放到同一尺寸，这样窗口尺寸在整个会话内固定。
pub fn load(skin: &str) -> Option<Clips> {
    let root = root()?;
    let dir = root.join(skin);

    // 先定尺寸：优先 idle，其次任意能解码的动作
    let mut size: Option<(u32, u32)> = None;
    for pose in PetPose::ALL {
        if let Some((width, height)) = probe(&dir.join(format!("{}.gif", pose.name()))) {
            let display_height = DISPLAY_HEIGHT;
            let display_width = (width * display_height).div_ceil(height.max(1)).max(1);
            size = Some((display_width, display_height));
            if pose == PetPose::Idle {
                break;
            }
        }
    }
    let (width, height) = size?;

    let mut slots: Vec<Option<Clip>> = Vec::with_capacity(PetPose::ALL.len());
    for pose in PetPose::ALL {
        let path = dir.join(format!("{}.gif", pose.name()));
        slots.push(decode(&path, width, height).map(|frames| Clip {
            frames,
            loops: pose.loops(),
        }));
    }
    if slots.iter().all(Option::is_none) {
        return None;
    }
    Some(Clips {
        width,
        height,
        slots,
    })
}

/// 只读 GIF 头，拿逻辑画布尺寸。
fn probe(path: &Path) -> Option<(u32, u32)> {
    let file = std::fs::File::open(path).ok()?;
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let decoder = options.read_info(std::io::BufReader::new(file)).ok()?;
    let (width, height) = (decoder.width() as u32, decoder.height() as u32);
    (width > 0 && height > 0).then_some((width, height))
}

/// 逐帧解码并按 GIF 的 disposal 规则合成到画布，再最近邻缩放到显示尺寸。
/// 最近邻而非插值：GIF 只有全透明/不透明，插值会在边缘造出半透明像素，
/// 而 shape 掩码只认二值 alpha，半透明边缘只会变成锯齿和脏色。
fn decode(path: &Path, out_width: u32, out_height: u32) -> Option<Vec<Frame>> {
    let file = std::fs::File::open(path).ok()?;
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut decoder = options.read_info(std::io::BufReader::new(file)).ok()?;

    let canvas_width = decoder.width() as u32;
    let canvas_height = decoder.height() as u32;
    if canvas_width == 0 || canvas_height == 0 {
        return None;
    }

    let mut canvas = vec![0u8; (canvas_width * canvas_height * 4) as usize];
    let mut saved: Option<Vec<u8>> = None;
    let mut frames: Vec<Frame> = Vec::new();

    loop {
        let frame = match decoder.read_next_frame() {
            Ok(Some(frame)) => frame,
            Ok(None) => break,
            Err(_) => break,
        };
        let dispose = frame.dispose;
        let (left, top) = (frame.left as u32, frame.top as u32);
        let (frame_width, frame_height) = (frame.width as u32, frame.height as u32);

        if dispose == gif::DisposalMethod::Previous {
            saved = Some(canvas.clone());
        }

        for y in 0..frame_height {
            let canvas_y = top + y;
            if canvas_y >= canvas_height {
                break;
            }
            for x in 0..frame_width {
                let canvas_x = left + x;
                if canvas_x >= canvas_width {
                    break;
                }
                let source = ((y * frame_width + x) * 4) as usize;
                if frame.buffer[source + 3] == 0 {
                    // GIF 的透明像素表示「沿用画布」，不覆盖
                    continue;
                }
                let target = ((canvas_y * canvas_width + canvas_x) * 4) as usize;
                canvas[target..target + 4].copy_from_slice(&frame.buffer[source..source + 4]);
            }
        }

        // GIF 里 0 / 过小的延迟按浏览器惯例抬到 20ms，与 Windows / macOS 端同规则
        let delay_ms = (frame.delay as u32 * 10).max(20);
        frames.push(Frame {
            rgba: scale_nearest(&canvas, canvas_width, canvas_height, out_width, out_height),
            delay_ms,
        });

        match dispose {
            gif::DisposalMethod::Background => {
                for y in 0..frame_height {
                    let canvas_y = top + y;
                    if canvas_y >= canvas_height {
                        break;
                    }
                    for x in 0..frame_width {
                        let canvas_x = left + x;
                        if canvas_x >= canvas_width {
                            break;
                        }
                        let target = ((canvas_y * canvas_width + canvas_x) * 4) as usize;
                        canvas[target..target + 4].copy_from_slice(&[0, 0, 0, 0]);
                    }
                }
            }
            gif::DisposalMethod::Previous => {
                if let Some(previous) = saved.take() {
                    canvas = previous;
                }
            }
            _ => {}
        }
    }

    (!frames.is_empty()).then_some(frames)
}

fn scale_nearest(src: &[u8], src_width: u32, src_height: u32, width: u32, height: u32) -> Vec<u8> {
    let mut out = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        let source_y = (y * src_height / height).min(src_height - 1);
        for x in 0..width {
            let source_x = (x * src_width / width).min(src_width - 1);
            let source = ((source_y * src_width + source_x) * 4) as usize;
            let target = ((y * width + x) * 4) as usize;
            out[target..target + 4].copy_from_slice(&src[source..source + 4]);
        }
    }
    out
}
