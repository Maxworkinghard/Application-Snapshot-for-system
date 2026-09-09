import AppKit

final class DesktopPetController: NSObject {
    var onCapture: (NSRunningApplication) -> Void = { _ in }
    var onRecord: (NSRunningApplication) -> Void = { _ in }
    var onStopRecording: () -> Void = {}
    var onRecordingState: () -> RecordingState = { .idle }
    var onPolishPrompt: () -> Void = {}
    var onPolishBusy: () -> Bool = { false }
    var onOpenScreenRecordingSettings: () -> Void = {}
    var onOpenSettings: () -> Void = {}

    private let applicationService: CapturableApplicationService
    private var panel: NSPanel?
    private var petView: DesktopPetView?
    private var actionPanel: NSPanel?
    private var snapshotListController: ApplicationSnapshotViewController?
    private weak var recordButton: NSButton?
    private var target: NSRunningApplication?

    private let positionXKey = "pet.position.x"
    private let positionYKey = "pet.position.y"

    /// 一级菜单宽度固定，高度随按钮内容自然撑开。
    private static let menuPageWidth: CGFloat = 248
    /// 二级应用列表页固定尺寸，列表过长时内部滚动。
    private static let applicationsPageSize = NSSize(width: 304, height: 400)

    init(applicationService: CapturableApplicationService) {
        self.applicationService = applicationService
    }

    func show() {
        if panel == nil {
            let petView = DesktopPetView(frame: NSRect(x: 0, y: 0, width: 56, height: 56))
            petView.onClick = { [weak self] in self?.toggleActionPanel() }
            petView.onDragged = { [weak self] origin in self?.savePosition(origin) }
            petView.onRightClick = { [weak self] event in self?.showContextMenu(event) }
            self.petView = petView

            let panel = NSPanel(
                contentRect: petView.bounds,
                styleMask: [.borderless, .nonactivatingPanel],
                backing: .buffered,
                defer: false
            )
            panel.isOpaque = false
            panel.backgroundColor = .clear
            panel.hasShadow = true
            panel.level = .floating
            panel.collectionBehavior = [.canJoinAllSpaces, .stationary]
            panel.ignoresMouseEvents = false
            panel.hidesOnDeactivate = false
            panel.contentView = petView
            self.panel = panel
        }

        panel?.setFrameOrigin(savedOrDefaulPosition())
        panel?.orderFrontRegardless()
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

    /// 右键悬浮窗：弹出「设置…」菜单（统一设置：快捷键 + 润色服务）。
    private func showContextMenu(_ event: NSEvent) {
        guard let petView else { return }
        let menu = NSMenu()
        let item = NSMenuItem(
            title: "设置…",
            action: #selector(openSettingsFromMenu),
            keyEquivalent: ""
        )
        item.target = self
        menu.addItem(item)
        NSMenu.popUpContextMenu(menu, with: event, for: petView)
    }

    @objc private func openSettingsFromMenu() {
        closeActionPanel()
        onOpenSettings()
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
                return clampToVisibleArea(point, size: NSSize(width: 56, height: 56))
            }
        }
        return defaultPosition()
    }

    private func defaultPosition() -> NSPoint {
        guard let screen = NSScreen.main ?? NSScreen.screens.first else {
            return NSPoint(x: 100, y: 100)
        }
        let frame = screen.visibleFrame
        let size: CGFloat = 56
        return NSPoint(
            x: frame.maxX - size - 16,
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
