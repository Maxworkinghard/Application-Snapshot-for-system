import AppKit

final class DesktopPetController: NSObject {
    /// 桌面呈现形式：悬浮球（默认）或桌宠，二者互斥，切换时重建控制器的窗体。
    enum Presentation {
        case ball
        case pet(skin: String)
    }

    var onCapture: (NSRunningApplication) -> Void = { _ in }
    var onRecord: (NSRunningApplication) -> Void = { _ in }
    var onStopRecording: () -> Void = {}
    var onRecordingState: () -> RecordingState = { .idle }
    var onPolishPrompt: () -> Void = {}
    var onPolishBusy: () -> Bool = { false }
    var onOpenScreenRecordingSettings: () -> Void = {}
    var onOpenSettings: () -> Void = {}
    var onQuit: () -> Void = {}

    private let applicationService: CapturableApplicationService
    private let presentation: Presentation
    private var panel: NSPanel?
    private var petView: DesktopPetView?
    private var gifView: PetGifView?
    private var actionPanel: NSPanel?
    private var snapshotListController: ApplicationSnapshotViewController?
    private weak var recordButton: NSButton?
    private var target: NSRunningApplication?
    private var hiddenTemporarily = false
    private var hiddenForRecording = false

    /// 桌宠与悬浮球尺寸不同，位置键必须分开：共用会把按另一种尺寸夹过的原点带过来。
    private let positionXKey: String
    private let positionYKey: String
    /// GIF 原始 192x208，等比缩到 155x168 展示；单位是点，Retina 由 AppKit 背景层处理，没有 DPI 倍率。
    private let panelSize: NSSize

    /// 一级菜单宽度固定，高度随按钮内容自然撑开。
    private static let menuPageWidth: CGFloat = 248
    /// 二级应用列表页固定尺寸，列表过长时内部滚动。
    private static let applicationsPageSize = NSSize(width: 304, height: 400)

    init(applicationService: CapturableApplicationService, presentation: Presentation) {
        self.applicationService = applicationService
        self.presentation = presentation
        switch presentation {
        case .ball:
            positionXKey = "pet.position.x"
            positionYKey = "pet.position.y"
            panelSize = NSSize(width: 56, height: 56)
        case .pet:
            positionXKey = "desktoppet.x"
            positionYKey = "desktoppet.y"
            panelSize = NSSize(width: 155, height: 168)
        }
    }

    var currentBounds: NSRect? { panel?.frame }
    var isPetMode: Bool { isPetPresentation }

    private var isPetPresentation: Bool {
        if case .pet = presentation { return true }
        return false
    }

    func show() {
        if panel == nil {
            let contentView: NSView
            switch presentation {
            case .ball:
                let petView = DesktopPetView(frame: NSRect(origin: .zero, size: panelSize))
                petView.onClick = { [weak self] in self?.toggleActionPanel() }
                petView.onDragged = { [weak self] origin in self?.savePosition(origin) }
                petView.onRightClick = { [weak self] event in self?.showContextMenu(event) }
                self.petView = petView
                contentView = petView
            case .pet(let skin):
                let gifView = PetGifView(frame: NSRect(origin: .zero, size: panelSize), skin: skin)
                gifView.toolTip = "点击打开功能菜单，拖动移动"
                gifView.onClick = { [weak self] in self?.toggleActionPanel() }
                gifView.onDragged = { [weak self] origin in self?.savePosition(origin) }
                gifView.onDragMoved = { [weak self] in self?.positionActionPanel() }
                gifView.onRightClick = { [weak self] event in self?.showContextMenu(event) }
                self.gifView = gifView
                contentView = gifView
            }

            let panel = NSPanel(
                contentRect: contentView.bounds,
                styleMask: [.borderless, .nonactivatingPanel],
                backing: .buffered,
                defer: false
            )
            panel.isOpaque = false
            panel.backgroundColor = .clear
            // 桌宠逐帧换图，带阴影会每帧重算 alpha 轮廓，Windows 分层窗也没有阴影。
            panel.hasShadow = !isPetPresentation
            panel.level = .floating
            panel.collectionBehavior = [.canJoinAllSpaces, .stationary]
            panel.ignoresMouseEvents = false
            panel.hidesOnDeactivate = false
            panel.contentView = contentView
            if isPetPresentation {
                // 截图/录制走单窗口合成拍不到桌宠；这里只是防住日后可能出现的整屏路径。
                panel.sharingType = .none
            }
            self.panel = panel
        }

        panel?.setFrameOrigin(savedOrDefaulPosition())
        panel?.orderFrontRegardless()
        gifView?.playPose(.idle)
    }

    /// 切换桌面形式时拆掉当前呈现；截图/追踪机制不受影响。
    func hide() {
        closeActionPanel()
        gifView?.stopAnimating()
        panel?.orderOut(nil)
        panel = nil
        petView = nil
        gifView = nil
    }

    func setPose(_ pose: PetPose) {
        gifView?.setPose(pose)
    }

    /// 截图流程期间短暂离场（只有 screencapture 选窗这条路径会拍到它）。
    func setHiddenTemporarily(_ hidden: Bool) {
        hiddenTemporarily = hidden
        applyHiddenState()
    }

    /// macOS 录制走 SCContentFilter 单窗口合成，拍不到桌宠，此开关仅为三端行为对齐而保留，未接入。
    func setHiddenForRecording(_ hidden: Bool) {
        hiddenForRecording = hidden
        applyHiddenState()
    }

    /// 两个隐藏标志各自独立：任一为真就离场，都清空后才回来。
    private func applyHiddenState() {
        guard let panel else { return }
        if hiddenTemporarily || hiddenForRecording {
            closeActionPanel()
            panel.orderOut(nil)
        } else {
            panel.orderFrontRegardless()
        }
    }

    func updateTarget(_ application: NSRunningApplication?) {
        target = application
        petView?.applyIcon(for: application)
        updateRecordingState()
    }

    func updateRecordingState() {
        if let recordButton {
            configureRecordButton(recordButton)
        }
    }

    /// 右键悬浮窗：弹出「设置…」菜单（统一设置：快捷键 + 润色服务）；
    /// 桌宠没有菜单栏入口可依靠，再追加一条退出。
    private func showContextMenu(_ event: NSEvent) {
        guard let view = panel?.contentView else { return }
        let menu = NSMenu()
        let item = NSMenuItem(
            title: "设置…",
            action: #selector(openSettingsFromMenu),
            keyEquivalent: ""
        )
        item.target = self
        menu.addItem(item)
        if isPetPresentation {
            menu.addItem(.separator())
            let quitItem = NSMenuItem(
                title: "退出应用快照",
                action: #selector(quitFromMenu),
                keyEquivalent: ""
            )
            quitItem.target = self
            menu.addItem(quitItem)
        }
        NSMenu.popUpContextMenu(menu, with: event, for: view)
    }

    @objc private func openSettingsFromMenu() {
        closeActionPanel()
        onOpenSettings()
    }

    @objc private func quitFromMenu() {
        onQuit()
    }

    private func toggleActionPanel() {
        if actionPanel != nil {
            closeActionPanel()
            return
        }

        let panel = NSPanel(
            contentRect: .zero,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.isOpaque = true
        panel.backgroundColor = .windowBackgroundColor
        panel.hasShadow = true
        panel.level = .floating
        panel.collectionBehavior = [.canJoinAllSpaces, .stationary, .ignoresCycle]
        panel.hidesOnDeactivate = false
        panel.becomesKeyOnlyIfNeeded = true
        self.panel?.addChildWindow(panel, ordered: .above)
        actionPanel = panel
        installMenuPage()
        panel.makeKeyAndOrderFront(nil)
    }

    private func closeActionPanel() {
        if let actionPanel {
            panel?.removeChildWindow(actionPanel)
            actionPanel.orderOut(nil)
        }
        actionPanel = nil
        recordButton = nil
    }

    private func positionActionPanel() {
        guard let actionPanel, let panel else { return }
        let screen = NSScreen.screens.first { $0.frame.intersects(panel.frame) }
            ?? NSScreen.main
            ?? NSScreen.screens.first
        guard let visibleFrame = screen?.visibleFrame else { return }

        let horizontalInset: CGFloat = 8
        let verticalInset: CGFloat = 8
        let x = min(
            max(panel.frame.midX - actionPanel.frame.width / 2, visibleFrame.minX + horizontalInset),
            visibleFrame.maxX - actionPanel.frame.width - horizontalInset
        )
        var y = panel.frame.maxY + verticalInset
        if y + actionPanel.frame.height > visibleFrame.maxY - verticalInset {
            y = panel.frame.minY - actionPanel.frame.height - verticalInset
        }
        y = min(
            max(y, visibleFrame.minY + verticalInset),
            visibleFrame.maxY - actionPanel.frame.height - verticalInset
        )
        actionPanel.setFrameOrigin(NSPoint(x: x, y: y))
    }

    // MARK: - 一级菜单页

    private func installMenuPage() {
        guard let actionPanel else { return }
        let container = makeMenuContentView()
        let size = NSSize(width: Self.menuPageWidth, height: container.fittingSize.height)
        container.frame = NSRect(origin: .zero, size: size)
        actionPanel.contentView = container
        actionPanel.setContentSize(size)
        positionActionPanel()
    }

    private func makeMenuContentView() -> NSView {
        let snapshotButton = NSButton(
            title: "应用快照",
            target: self,
            action: #selector(openApplicationSnapshot)
        )
        snapshotButton.bezelStyle = .rounded

        let recordButton = NSButton(
            title: "录制",
            target: self,
            action: #selector(performRecord)
        )
        recordButton.bezelStyle = .rounded
        self.recordButton = recordButton
        configureRecordButton(recordButton)

        let isPolishing = onPolishBusy()
        let promptButton = NSButton(
            title: isPolishing ? "停止润色" : "润色 Prompt",
            target: self,
            action: #selector(polishPrompt)
        )
        promptButton.bezelStyle = .rounded
        promptButton.toolTip = isPolishing
            ? "取消当前润色请求，剪切板不会被改动"
            : "读取剪切板文字，确认后润色并写回剪切板"

        let buttons = [snapshotButton, recordButton, promptButton]

        let stack = NSStackView(views: buttons)
        stack.orientation = .vertical
        stack.spacing = 10
        stack.alignment = .centerX
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)
        stack.translatesAutoresizingMaskIntoConstraints = false

        // 等宽约束必须在按钮加入 stack 之后激活，否则两个按钮没有共同祖先，
        // AppKit 会抛 NSGenericException，导致整个功能菜单建不出来。
        for button in buttons.dropFirst() {
            button.widthAnchor.constraint(equalTo: buttons[0].widthAnchor).isActive = true
        }
        buttons[0].widthAnchor.constraint(greaterThanOrEqualToConstant: 208).isActive = true

        let container = NSView()
        container.addSubview(stack)
        NSLayoutConstraint.activate([
            container.widthAnchor.constraint(equalToConstant: Self.menuPageWidth),
            stack.topAnchor.constraint(equalTo: container.topAnchor),
            stack.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            stack.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])
        return container
    }

    private func configureRecordButton(_ button: NSButton) {
        let title: String
        let subtitle: String
        let toolTip: String?
        let isEnabled: Bool

        switch onRecordingState() {
        case .idle:
            title = "录制"
            if let target {
                let name = target.localizedName ?? "应用"
                subtitle = "\(name) 的窗口"
                toolTip = "录制 \(name) 窗口"
                isEnabled = true
            } else {
                subtitle = "未找到可录制的应用"
                toolTip = nil
                isEnabled = false
            }
        case .starting:
            title = "录制"
            subtitle = "正在开始录制…"
            toolTip = nil
            isEnabled = false
        case .recording:
            title = "停止录制"
            subtitle = "结束当前录制"
            toolTip = "停止录制"
            isEnabled = true
        case .stopping:
            title = "录制"
            subtitle = "正在保存录制…"
            toolTip = nil
            isEnabled = false
        }

        let paragraph = NSMutableParagraphStyle()
        paragraph.alignment = .center
        let attributedTitle = NSMutableAttributedString(
            string: "\(title)\n",
            attributes: [
                .font: NSFont.systemFont(ofSize: 13, weight: .semibold),
                .foregroundColor: NSColor.labelColor,
                .paragraphStyle: paragraph
            ]
        )
        attributedTitle.append(NSAttributedString(
            string: subtitle,
            attributes: [
                .font: NSFont.systemFont(ofSize: 10),
                .foregroundColor: NSColor.secondaryLabelColor,
                .paragraphStyle: paragraph
            ]
        ))
        button.attributedTitle = attributedTitle
        button.toolTip = toolTip
        button.isEnabled = isEnabled
    }

    // MARK: - 应用快照列表页

    private func installApplicationsPage() {
        guard let actionPanel else { return }
        let controller = snapshotListController ?? makeSnapshotListController()
        snapshotListController = controller

        let size = Self.applicationsPageSize
        controller.view.frame = NSRect(origin: .zero, size: size)
        actionPanel.contentView = controller.view
        actionPanel.setContentSize(size)
        positionActionPanel()
        controller.reload()
    }

    private func makeSnapshotListController() -> ApplicationSnapshotViewController {
        let controller = ApplicationSnapshotViewController(applicationService: applicationService)
        controller.onSelect = { [weak self] application in
            self?.handleSelectApplication(application)
        }
        controller.onBack = { [weak self] in
            self?.installMenuPage()
        }
        controller.onOpenScreenRecordingSettings = { [weak self] in
            self?.handleOpenScreenRecordingSettings()
        }
        return controller
    }

    private func handleSelectApplication(_ application: CapturableApplication) {
        closeActionPanel()
        onCapture(application.runningApplication)
    }

    private func handleOpenScreenRecordingSettings() {
        closeActionPanel()
        onOpenScreenRecordingSettings()
    }

    @objc private func openApplicationSnapshot() {
        installApplicationsPage()
    }

    @objc private func polishPrompt() {
        closeActionPanel()
        onPolishPrompt()
    }

    @objc private func performRecord() {
        switch onRecordingState() {
        case .idle:
            guard let target else { return }
            closeActionPanel()
            onRecord(target)
        case .recording:
            closeActionPanel()
            onStopRecording()
        case .starting, .stopping:
            break
        }
    }

    private func savedOrDefaulPosition() -> NSPoint {
        let defaults = UserDefaults.standard
        if defaults.object(forKey: positionXKey) != nil,
           defaults.object(forKey: positionYKey) != nil {
            let point = NSPoint(
                x: CGFloat(defaults.double(forKey: positionXKey)),
                y: CGFloat(defaults.double(forKey: positionYKey))
            )
            if isPointOnScreen(point) {
                return clampToVisibleArea(point, size: panelSize)
            }
        }
        return defaultPosition()
    }

    private func defaultPosition() -> NSPoint {
        guard let screen = NSScreen.main ?? NSScreen.screens.first else {
            return NSPoint(x: 100, y: 100)
        }
        let frame = screen.visibleFrame
        return NSPoint(
            x: frame.maxX - panelSize.width - 16,
            y: frame.minY + 16
        )
    }

    private func isPointOnScreen(_ point: NSPoint) -> Bool {
        NSScreen.screens.contains { screen in
            screen.visibleFrame.insetBy(dx: -200, dy: -200).contains(point)
        }
    }

    private func savePosition(_ origin: NSPoint) {
        let defaults = UserDefaults.standard
        defaults.set(Double(origin.x), forKey: positionXKey)
        defaults.set(Double(origin.y), forKey: positionYKey)
    }
}

/// 把悬浮窗原点夹回「锚点所在屏幕」的可见区域（Dock、菜单栏之外）。
/// 历史位置可能停在屏幕边缘外或 Dock 后面（isPointOnScreen 有 ±200 容差），
/// 恢复与拖拽时都做约束，避免悬浮窗从视野里消失。
private func clampToVisibleArea(_ origin: NSPoint, size: NSSize, around anchor: NSPoint? = nil) -> NSPoint {
    let reference = anchor ?? origin
    let screen = NSScreen.screens.first { $0.frame.contains(reference) } ?? NSScreen.main
    guard let visible = screen?.visibleFrame else { return origin }
    let margin: CGFloat = 8
    return NSPoint(
        x: min(max(origin.x, visible.minX + margin), visible.maxX - size.width - margin),
        y: min(max(origin.y, visible.minY + margin), visible.maxY - size.height - margin)
    )
}

private final class DesktopPetView: NSView {
    var onClick: () -> Void = {}
    var onDragged: (NSPoint) -> Void = { _ in }
    var onRightClick: (NSEvent) -> Void = { _ in }

    private var icon: NSImage?
    private var mouseDownLocation: NSPoint = .zero
    private var didDrag = false
    private let dragThreshold: CGFloat = 4

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.cornerRadius = frame.width / 2
        layer?.cornerCurve = .continuous
        layer?.borderWidth = 1
        layer?.borderColor = NSColor.separatorColor.cgColor
        layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor
        layer?.masksToBounds = true
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func applyIcon(for application: NSRunningApplication?) {
        icon = application?.icon
        needsDisplay = true
    }

    override func draw(_ dirtyRect: NSRect) {
        NSColor.controlBackgroundColor.setFill()
        dirtyRect.fill()

        let inset: CGFloat = 6
        let rect = bounds.insetBy(dx: inset, dy: inset)
        if let icon {
            icon.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1.0, respectFlipped: true, hints: nil)
        } else if let fallback = NSImage(systemSymbolName: "camera.viewfinder", accessibilityDescription: "应用快照") {
            let config = NSImage.SymbolConfiguration(pointSize: rect.width, weight: .regular)
            let rendered = fallback.withSymbolConfiguration(config) ?? fallback
            rendered.draw(in: rect, from: .zero, operation: .sourceOver, fraction: 1.0, respectFlipped: true, hints: nil)
        }
    }

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

/// GIF 桌宠视图：逐帧按 GIF 自带延迟推进，透明背景直接落在无背景色的面板上。
/// 单次动画（起跳/失败/等待）播完回 idle，循环动画（idle/跑动）持续循环。
private final class PetGifView: NSView {
    var onClick: () -> Void = {}
    var onDragged: (NSPoint) -> Void = { _ in }
    var onDragMoved: () -> Void = {}
    var onRightClick: (NSEvent) -> Void = { _ in }

    private var clips: [PetClip?]
    private var currentPose: PetPose = .idle
    private var frameIndex = 0
    private var frameTimer: Timer?
    private var pressCursor: NSPoint = .zero
    private var pressOrigin: NSPoint = .zero
    private var dragging = false
    private let dragThreshold: CGFloat = 4
    private var alphaMask: [UInt8]?
    private var alphaMaskKey: (pose: Int, frame: Int)?

    init(frame: NSRect, skin: String) {
        clips = PetPose.allCases.map {
            PetClip.load(url: PetAssets.posePath(skin: skin, pose: $0), loops: $0.loops)
        }
        super.init(frame: frame)
        wantsLayer = true
        // idle 缺失（素材不完整）时退化到任意可用的动画，避免空窗口
        if clips[PetPose.idle.rawValue] == nil {
            clips[PetPose.idle.rawValue] = clips.compactMap { $0 }.first
        }
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let clip = clips[currentPose.rawValue],
              frameIndex < clip.frames.count,
              let context = NSGraphicsContext.current?.cgContext else {
            return
        }
        context.interpolationQuality = .high
        context.draw(clip.frames[frameIndex], in: bounds)
    }

    /// 切换动画。循环态重复调用不重置帧（拖动中持续跑动）；一次性动画重播。
    func setPose(_ pose: PetPose) {
        guard Thread.isMainThread else {
            DispatchQueue.main.async { [weak self] in self?.setPose(pose) }
            return
        }
        guard let clip = clips[pose.rawValue] else { return }
        if currentPose == pose && clip.loops { return }
        playPose(pose)
    }

    func playPose(_ pose: PetPose) {
        guard let clip = clips[pose.rawValue] else { return }
        currentPose = pose
        frameIndex = 0
        needsDisplay = true
        scheduleNextFrame(after: clip.delays[0])
    }

    private func advanceFrame() {
        guard let clip = clips[currentPose.rawValue] else { return }
        frameIndex += 1
        if frameIndex >= clip.frames.count {
            if clip.loops {
                frameIndex = 0
            } else {
                // 单次动画播完回待机
                playPose(.idle)
                return
            }
        }
        needsDisplay = true
        scheduleNextFrame(after: clip.delays[frameIndex])
    }

    /// 每帧按当前帧自己的延迟重新排一次，不用固定帧率。
    /// 挂 .common 模式，否则拖动/菜单跟踪期间动画会停住。
    private func scheduleNextFrame(after milliseconds: Int) {
        frameTimer?.invalidate()
        let timer = Timer(timeInterval: Double(milliseconds) / 1000, repeats: false) { [weak self] _ in
            self?.advanceFrame()
        }
        RunLoop.main.add(timer, forMode: .common)
        frameTimer = timer
    }

    /// 拆掉桌宠时停表，否则已离屏的视图会继续按帧醒来。
    func stopAnimating() {
        frameTimer?.invalidate()
        frameTimer = nil
    }

    override func acceptsFirstMouse(for event: NSEvent?) -> Bool {
        true
    }

    /// 透明像素把点击让给下面的窗口，与 Windows UpdateLayeredWindow / Linux ShapeInput 对齐。
    override func hitTest(_ point: NSPoint) -> NSView? {
        isOpaque(at: point) ? self : nil
    }

    private func isOpaque(at point: NSPoint) -> Bool {
        guard bounds.contains(point),
              let clip = clips[currentPose.rawValue],
              frameIndex < clip.frames.count else {
            return false
        }
        let image = clip.frames[frameIndex]
        refreshAlphaMask(image: image)
        guard let mask = alphaMask else { return true }
        let width = image.width
        let height = image.height
        guard width > 0, height > 0 else { return false }
        let x = min(max(Int((point.x / bounds.width) * CGFloat(width)), 0), width - 1)
        let y = min(max(Int(((bounds.height - point.y) / bounds.height) * CGFloat(height)), 0), height - 1)
        return mask[y * width + x] >= 128
    }

    private func refreshAlphaMask(image: CGImage) {
        if alphaMaskKey?.pose == currentPose.rawValue, alphaMaskKey?.frame == frameIndex {
            return
        }
        let width = image.width
        let height = image.height
        var pixels = [UInt8](repeating: 0, count: width * height * 4)
        let drawn = pixels.withUnsafeMutableBytes { raw -> Bool in
            guard let ctx = CGContext(
                data: raw.baseAddress,
                width: width,
                height: height,
                bitsPerComponent: 8,
                bytesPerRow: width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            ) else {
                return false
            }
            ctx.translateBy(x: 0, y: CGFloat(height))
            ctx.scaleBy(x: 1, y: -1)
            ctx.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        guard drawn else {
            alphaMask = nil
            alphaMaskKey = (currentPose.rawValue, frameIndex)
            return
        }
        var mask = [UInt8](repeating: 0, count: width * height)
        for index in 0..<(width * height) {
            mask[index] = pixels[index * 4 + 3]
        }
        alphaMask = mask
        alphaMaskKey = (currentPose.rawValue, frameIndex)
    }

    override func mouseDown(with event: NSEvent) {
        pressCursor = NSEvent.mouseLocation
        pressOrigin = window?.frame.origin ?? .zero
        dragging = false
    }

    override func mouseDragged(with event: NSEvent) {
        guard let window = self.window else { return }
        let current = NSEvent.mouseLocation
        let deltaX = current.x - pressCursor.x
        let deltaY = current.y - pressCursor.y
        if abs(deltaX) >= dragThreshold || abs(deltaY) >= dragThreshold {
            dragging = true
        }
        guard dragging else { return }

        // 按「相对按下点的位移」移动，不是把窗口居中到光标——桌宠比悬浮球大得多，居中会瞬移
        let newOrigin = NSPoint(x: pressOrigin.x + deltaX, y: pressOrigin.y + deltaY)
        window.setFrameOrigin(clampToVisibleArea(newOrigin, size: window.frame.size, around: current))
        // 朝向按累计位移的符号定，不看瞬时方向
        setPose(deltaX >= 0 ? .runningRight : .runningLeft)
        onDragMoved()
    }

    override func mouseUp(with event: NSEvent) {
        if dragging, let window = self.window {
            dragging = false
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
