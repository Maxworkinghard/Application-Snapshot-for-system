import AppKit
import ImageIO

/// GIF 桌宠视图：逐帧渲染素材目录里的姿势 GIF。
/// 姿势语义与 Windows 端 Pet.cs 对齐——idle / 跑动循环，
/// 挥手 / 起跳 / 失败 / 等待单次播完回 idle；拖动触发跑动，单击由调用方接管。
final class DesktopPetGifView: NSView {
    enum Pose: Int, CaseIterable {
        case idle
        case waving
        case jumping
        case failed
        case waiting
        case runningLeft
        case runningRight

        var fileName: String {
            switch self {
            case .idle: return "idle"
            case .waving: return "waving"
            case .jumping: return "jumping"
            case .failed: return "failed"
            case .waiting: return "waiting"
            case .runningLeft: return "running-left"
            case .runningRight: return "running-right"
            }
        }

        var loops: Bool {
            switch self {
            case .idle, .runningLeft, .runningRight: return true
            case .waving, .jumping, .failed, .waiting: return false
            }
        }
    }

    private struct PoseClip {
        let frames: [(image: CGImage, delay: TimeInterval)]
        let loops: Bool
    }

    /// 96 DPI 设计值；GIF 原始 192×208，等比缩放显示（与 Windows 端一致）。
    static let petSize = NSSize(width: 155, height: 168)

    let skin: String

    var onClick: () -> Void = {}
    var onDragged: (NSPoint) -> Void = { _ in }
    var onRightClick: (NSEvent) -> Void = { _ in }

    private var clips: [Pose: PoseClip] = [:]
    private var currentPose: Pose = .idle
    private var frameIndex = 0
    private var timer: Timer?
    private var mouseDownLocation: NSPoint = .zero
    private var didDrag = false
    private let dragThreshold: CGFloat = 4

    init?(skin: String) {
        self.skin = skin
        super.init(frame: NSRect(origin: .zero, size: Self.petSize))
        guard loadClips() else { return nil }
        toolTip = "点击打开功能菜单，拖动移动"
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    deinit {
        timer?.invalidate()
    }

    // MARK: - 素材加载

    private func loadClips() -> Bool {
        clips = [:]
        for pose in Pose.allCases {
            if let clip = Self.loadClip(skin: skin, pose: pose) {
                clips[pose] = clip
            }
        }
        // idle 缺失（素材不完整）时退化到任意可用的姿势，避免空窗口
        if clips[.idle] == nil, let fallback = clips.first {
            clips[.idle] = fallback.value
        }
        return clips[.idle] != nil
    }

    private static func loadClip(skin: String, pose: Pose) -> PoseClip? {
        let url = PetAssets.poseURL(skin: skin, pose: pose.fileName)
        guard let source = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
        let count = CGImageSourceGetCount(source)
        guard count > 0 else { return nil }

        var frames: [(CGImage, TimeInterval)] = []
        for index in 0..<count {
            guard let image = CGImageSourceCreateImageAtIndex(source, index, nil) else { continue }
            var delay = 0.1
            if let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any],
               let gifProperties = properties[kCGImagePropertyGIFDictionary] as? [CFString: Any],
               let gifDelay = gifProperties[kCGImagePropertyGIFDelayTime] as? Double,
               gifDelay > 0 {
                delay = gifDelay
            }
            frames.append((image, max(delay, 0.02)))
        }
        guard !frames.isEmpty else { return nil }
        return PoseClip(frames: frames, loops: pose.loops)
    }

    // MARK: - 动画

    func startAnimating() {
        play(.idle)
    }

    func stopAnimating() {
        timer?.invalidate()
        timer = nil
    }

    /// 切换姿势。循环态重复调用不重置帧（拖动中持续跑动）；一次性动画重播。
    func setPose(_ pose: Pose) {
        guard clips[pose] != nil else { return }
        if pose == currentPose, let clip = clips[pose], clip.loops {
            return
        }
        play(pose)
    }

    private func play(_ pose: Pose) {
        guard let clip = clips[pose] else { return }
        currentPose = pose
        frameIndex = 0
        needsDisplay = true
        scheduleNextFrame(after: clip.frames[0].delay)
    }

    private func scheduleNextFrame(after delay: TimeInterval) {
        timer?.invalidate()
        let timer = Timer(timeInterval: delay, repeats: false) { [weak self] _ in
            self?.advance()
        }
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    private func advance() {
        guard let clip = clips[currentPose] else { return }
        frameIndex += 1
        if frameIndex >= clip.frames.count {
            if clip.loops {
                frameIndex = 0
            } else {
                // 单次动画播完回待机
                play(.idle)
                return
            }
        }
        needsDisplay = true
        scheduleNextFrame(after: clip.frames[frameIndex].delay)
    }

    override var isOpaque: Bool { false }

    override func draw(_ dirtyRect: NSRect) {
        guard let context = NSGraphicsContext.current?.cgContext,
              let frame = clips[currentPose]?.frames[frameIndex] else { return }
        context.clear(bounds)
        context.interpolationQuality = .high
        context.draw(frame.image, in: bounds)
    }

    // MARK: - 鼠标交互（与悬浮球 DesktopPetView 同规则）

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    override func mouseDown(with event: NSEvent) {
        mouseDownLocation = NSEvent.mouseLocation
        didDrag = false
    }

    override func mouseDragged(with event: NSEvent) {
        let current = NSEvent.mouseLocation
        let deltaX = current.x - mouseDownLocation.x
        let deltaY = current.y - mouseDownLocation.y
        if !didDrag && (abs(deltaX) > dragThreshold || abs(deltaY) > dragThreshold) {
            didDrag = true
        }

        guard let window = self.window else { return }
        let newOrigin = NSPoint(
            x: current.x - window.frame.width / 2,
            y: current.y - window.frame.height / 2
        )
        window.setFrameOrigin(clampToVisibleArea(newOrigin, size: window.frame.size, around: current))
        if didDrag {
            setPose(deltaX >= 0 ? .runningRight : .runningLeft)
        }
    }

    override func mouseUp(with event: NSEvent) {
        if didDrag, let window = self.window {
            onDragged(window.frame.origin)
            setPose(.idle)
        } else {
            onClick()
        }
    }

    override func rightMouseDown(with event: NSEvent) {
        onRightClick(event)
    }
}
