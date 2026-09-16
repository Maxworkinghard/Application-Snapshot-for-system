import AppKit

/// 把悬浮窗原点夹回「锚点所在屏幕」的可见区域（Dock、菜单栏之外）。
/// 历史位置可能停在屏幕边缘外或 Dock 后面（isPointOnScreen 有 ±200 容差），
/// 恢复与拖拽时都做约束，避免悬浮窗从视野里消失。
func clampToVisibleArea(_ origin: NSPoint, size: NSSize, around anchor: NSPoint? = nil) -> NSPoint {
    let reference = anchor ?? origin
    let screen = NSScreen.screens.first { $0.frame.contains(reference) } ?? NSScreen.main
    guard let visible = screen?.visibleFrame else { return origin }
    let margin: CGFloat = 8
    return NSPoint(
        x: min(max(origin.x, visible.minX + margin), visible.maxX - size.width - margin),
        y: min(max(origin.y, visible.minY + margin), visible.maxY - size.height - margin)
    )
}

/// 悬浮窗与桌宠共用的鼠标行为：单击弹功能面板、拖动移动（超过阈值才算拖动）、右键菜单。
class FloatingPetBaseView: NSView {
    var onClick: () -> Void = {}
    var onDragged: (NSPoint) -> Void = { _ in }
    var onRightClick: (NSEvent) -> Void = { _ in }

    private var mouseDownLocation: NSPoint = .zero
    private var didDrag = false
    private let dragThreshold: CGFloat = 4

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func mouseDown(with event: NSEvent) {
        mouseDownLocation = NSEvent.mouseLocation
        didDrag = false
    }

    override func mouseDragged(with event: NSEvent) {
        let current = NSEvent.mouseLocation
        let dx = abs(current.x - mouseDownLocation.x)
        let dy = abs(current.y - mouseDownLocation.y)
        if dx > dragThreshold || dy > dragThreshold {
            didDrag = true
        }

        guard let window = self.window else { return }
        let newOrigin = NSPoint(
            x: current.x - window.frame.width / 2,
            y: current.y - window.frame.height / 2
        )
        window.setFrameOrigin(clampToVisibleArea(newOrigin, size: window.frame.size, around: current))
    }

    override func mouseUp(with event: NSEvent) {
        if didDrag, let window = self.window {
            onDragged(window.frame.origin)
        } else {
            onClick()
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        onRightClick(event)
    }
}

/// 桌宠视图：按 GIF 自带延迟逐帧播放当前动作，透明区域自动穿透鼠标之外的部分由窗口负责。
/// 单次动画（挥手/起跳/失败/等待）播完回 idle，循环动画（idle/跑动）持续循环；
/// 与 Windows 端 PetForm 的播放规则一致。
final class PetAnimationView: FloatingPetBaseView {
    private let clips: [PetPose: PetClip]
    private var currentPose: PetPose = .idle
    private var frameIndex = 0
    private var frameTimer: Timer?

    /// 至少要有一个动作可播才构造得出来；idle 缺失（素材不完整）时退化到任意可用动作。
    init?(skin: String, height: CGFloat) {
        var clips = PetAssets.loadClips(skin: skin)
        guard let fallback = clips[.idle] ?? clips.values.first else {
            return nil
        }
        if clips[.idle] == nil {
            clips[.idle] = fallback
        }
        self.clips = clips

        let size = fallback.pixelSize
        let scale = size.height > 0 ? height / size.height : 1
        super.init(frame: NSRect(x: 0, y: 0, width: (size.width * scale).rounded(), height: height))
        wantsLayer = true
        play(.idle)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        frameTimer?.invalidate()
    }

    /// 停表；切换形式或关闭悬浮窗时调用，避免定时器留着空转。
    func stop() {
        frameTimer?.invalidate()
        frameTimer = nil
    }

    /// 切换动作；已经在播的循环动作不打断（否则 idle 会被反复重置回第一帧）。
    func setPose(_ pose: PetPose) {
        guard let clip = clips[pose] else { return }
        if currentPose == pose, clip.loops {
            return
        }
        play(pose)
    }

    private func play(_ pose: PetPose) {
        guard let clip = clips[pose] else { return }
        currentPose = pose
        frameIndex = 0
        clip.seek(to: 0)
        needsDisplay = true
        scheduleNextFrame(after: clip.delays[0])
    }

    private func scheduleNextFrame(after delay: TimeInterval) {
        frameTimer?.invalidate()
        let timer = Timer(timeInterval: delay, repeats: false) { [weak self] _ in
            self?.advanceFrame()
        }
        // .common 模式：拖动桌宠或弹菜单时动画不会停住
        RunLoop.main.add(timer, forMode: .common)
        frameTimer = timer
    }

    private func advanceFrame() {
        guard let clip = clips[currentPose] else { return }
        frameIndex += 1
        if frameIndex >= clip.frameCount {
            if clip.loops {
                frameIndex = 0
            } else {
                // 单次动画播完回待机
                play(.idle)
                return
            }
        }
        clip.seek(to: frameIndex)
        needsDisplay = true
        scheduleNextFrame(after: clip.delays[frameIndex])
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let clip = clips[currentPose] else { return }
        clip.seek(to: frameIndex)
        NSGraphicsContext.current?.imageInterpolation = .high
        _ = clip.rep.draw(
            in: bounds,
            from: .zero,
            operation: .sourceOver,
            fraction: 1.0,
            respectFlipped: true,
            hints: [.interpolation: NSImageInterpolation.high]
        )
    }
}
