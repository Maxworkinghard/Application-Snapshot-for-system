//! 姿势状态机与帧时钟，等价于 Windows 端 Pet.cs 的 SetPose / PlayPose / OnFrameTick。
//! 本文件不碰任何 x11rb / wayland 类型，可以无头单测。

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::decode::{load_clip, Clip, Rgba};

/// 7 个姿势，序号与 Windows 端一致。
/// Waving 会被加载但目前没有任何触发点，保留不可达——移植以对齐 Windows 为准。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PetPose {
    Idle = 0,
    Waving = 1,
    Jumping = 2,
    Failed = 3,
    Waiting = 4,
    RunningLeft = 5,
    RunningRight = 6,
}

impl PetPose {
    /// 枚举顺序，缺 idle 时的兜底扫描也按这个顺序。
    pub const ALL: [PetPose; 7] = [
        PetPose::Idle,
        PetPose::Waving,
        PetPose::Jumping,
        PetPose::Failed,
        PetPose::Waiting,
        PetPose::RunningLeft,
        PetPose::RunningRight,
    ];

    /// 素材文件名（不含 .gif）。
    pub fn file_name(self) -> &'static str {
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

    /// 待机与跑动循环播放，其余一次性。
    pub fn loops(self) -> bool {
        matches!(
            self,
            PetPose::Idle | PetPose::RunningLeft | PetPose::RunningRight
        )
    }
}

pub struct PetState {
    clips: [Option<Arc<Clip>>; 7],
    pose: PetPose,
    frame: usize,
    next_at: Instant,
    /// 窗口已拆除，后续姿势一律丢弃。
    disposed: bool,
}

impl PetState {
    /// 从 <素材根>/<形象>/ 载入 7 个姿势；缺的姿势留空，对它 set_pose 会被静默忽略。
    pub fn load(skin_dir: &Path) -> Self {
        let mut clips: [Option<Arc<Clip>>; 7] = std::array::from_fn(|_| None);
        for pose in PetPose::ALL {
            let path = skin_dir.join(format!("{}.gif", pose.file_name()));
            clips[pose as usize] = load_clip(&path, pose.loops()).map(Arc::new);
        }
        // idle.gif 缺失时把待机指向枚举顺序上第一个有效动画，免得窗口一片空白。
        // Windows 在这里存了两份指针并各释放一次，这里改用 Arc 共享，行为一致但不会二次释放。
        if clips[PetPose::Idle as usize].is_none() {
            if let Some(clip) = PetPose::ALL[1..]
                .iter()
                .find_map(|pose| clips[*pose as usize].clone())
            {
                clips[PetPose::Idle as usize] = Some(clip);
            }
        }
        Self {
            clips,
            pose: PetPose::Idle,
            frame: 0,
            next_at: Instant::now(),
            disposed: false,
        }
    }

    /// 首次显示：无条件播放待机（此时 pose 已经是 Idle，走 set_pose 会被循环守卫挡掉）。
    pub fn start(&mut self, now: Instant) -> Option<Arc<Rgba>> {
        self.play_pose(PetPose::Idle, now)
    }

    /// 窗口拆除，对应 Windows 的 IsDisposed 判断。
    pub fn dispose(&mut self) {
        self.disposed = true;
    }

    /// 切换姿势。循环态重复调用不重置帧（拖动中持续跑动），一次性动画重播。
    /// 没有优先级，严格后来者覆盖。
    /// 「回到 UI 线程」那道守卫在这里由通道结构性满足：状态机只被桌宠线程自己碰。
    pub fn set_pose(&mut self, pose: PetPose, now: Instant) -> Option<Arc<Rgba>> {
        if self.disposed {
            return None;
        }
        let clip = self.clips[pose as usize].as_ref()?;
        if self.pose == pose && clip.loops {
            return None;
        }
        self.play_pose(pose, now)
    }

    pub fn time_to_next_frame(&self) -> Duration {
        if self.disposed || self.clips[self.pose as usize].is_none() { return Duration::from_millis(16); }
        self.next_at.saturating_duration_since(Instant::now())
    }

    /// 当前该显示的那一帧（重绘用）。
    pub fn current_frame(&self) -> Option<Arc<Rgba>> {
        self.clips[self.pose as usize]
            .as_ref()
            .map(|clip| clip.frames[self.frame].clone())
    }

    /// 帧时钟：到点就前进一帧并返回要画的位图，没到点返回 None。
    /// 每帧按「正在显示的那一帧」的延时重新定时，没有任何时长常量或超时。
    pub fn tick(&mut self, now: Instant) -> Option<Arc<Rgba>> {
        if self.disposed || now < self.next_at {
            return None;
        }
        let clip = self.clips[self.pose as usize].clone()?;
        self.frame += 1;
        if self.frame >= clip.frames.len() {
            if clip.loops {
                self.frame = 0;
            } else {
                // 一次性动画播完回待机
                return self.play_pose(PetPose::Idle, now);
            }
        }
        self.next_at = now + Duration::from_millis(clip.delays_ms[self.frame] as u64);
        Some(clip.frames[self.frame].clone())
    }

    /// 无条件从第 0 帧开始播；clip 为空时连当前姿势都不动。
    fn play_pose(&mut self, pose: PetPose, now: Instant) -> Option<Arc<Rgba>> {
        let clip = self.clips[pose as usize].clone()?;
        self.pose = pose;
        self.frame = 0;
        self.next_at = now + Duration::from_millis(clip.delays_ms[0] as u64);
        Some(clip.frames[0].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一段 frames 帧、每帧 100ms 的假动画。
    fn clip(frames: usize, loops: bool) -> Arc<Clip> {
        Arc::new(Clip {
            frames: (0..frames)
                .map(|_| {
                    Arc::new(Rgba {
                        w: 1,
                        h: 1,
                        premul_bgra: vec![0, 0, 0, 0],
                    })
                })
                .collect(),
            delays_ms: vec![100; frames],
            loops,
        })
    }

    fn state(entries: &[(PetPose, Arc<Clip>)]) -> PetState {
        let mut clips: [Option<Arc<Clip>>; 7] = std::array::from_fn(|_| None);
        for (pose, clip) in entries {
            clips[*pose as usize] = Some(clip.clone());
        }
        PetState {
            clips,
            pose: PetPose::Idle,
            frame: 0,
            next_at: Instant::now(),
            disposed: false,
        }
    }

    /// 循环姿势重复设置不重置帧。
    #[test]
    fn loop_pose_repeat_keeps_frame() {
        let now = Instant::now();
        let mut pet = state(&[
            (PetPose::Idle, clip(2, true)),
            (PetPose::RunningRight, clip(3, true)),
        ]);
        pet.set_pose(PetPose::RunningRight, now);
        pet.tick(now + Duration::from_millis(100));
        assert_eq!(pet.frame, 1);
        pet.set_pose(PetPose::RunningRight, now + Duration::from_millis(150));
        assert_eq!(pet.frame, 1);
    }

    /// 一次性姿势重复设置从头重播。
    #[test]
    fn oneshot_pose_repeat_restarts() {
        let now = Instant::now();
        let mut pet = state(&[
            (PetPose::Idle, clip(2, true)),
            (PetPose::Jumping, clip(3, false)),
        ]);
        pet.set_pose(PetPose::Jumping, now);
        pet.tick(now + Duration::from_millis(100));
        assert_eq!(pet.frame, 1);
        pet.set_pose(PetPose::Jumping, now + Duration::from_millis(150));
        assert_eq!(pet.frame, 0);
    }

    /// 一次性动画播完自动回待机。
    #[test]
    fn oneshot_finishes_to_idle() {
        let now = Instant::now();
        let mut pet = state(&[
            (PetPose::Idle, clip(2, true)),
            (PetPose::Failed, clip(2, false)),
        ]);
        pet.set_pose(PetPose::Failed, now);
        pet.tick(now + Duration::from_millis(100));
        pet.tick(now + Duration::from_millis(200));
        assert_eq!(pet.pose, PetPose::Idle);
        assert_eq!(pet.frame, 0);
    }

    /// 缺素材的姿势静默忽略，当前姿势不变。
    #[test]
    fn missing_clip_is_silent_noop() {
        let now = Instant::now();
        let mut pet = state(&[(PetPose::Idle, clip(2, true))]);
        pet.start(now);
        assert!(pet.set_pose(PetPose::Waiting, now).is_none());
        assert_eq!(pet.pose, PetPose::Idle);
    }

    /// 拆除之后不再换姿势。
    #[test]
    fn disposed_ignores_set_pose() {
        let now = Instant::now();
        let mut pet = state(&[
            (PetPose::Idle, clip(2, true)),
            (PetPose::Jumping, clip(2, false)),
        ]);
        pet.dispose();
        assert!(pet.set_pose(PetPose::Jumping, now).is_none());
        assert_eq!(pet.pose, PetPose::Idle);
    }
}
