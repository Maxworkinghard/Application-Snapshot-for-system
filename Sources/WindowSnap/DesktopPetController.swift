import AppKit

final class DesktopPetController {
    var onRequestTarget: () -> NSRunningApplication? = { nil }
    var onCapture: (NSRunningApplication) -> Void = { _ in }
    var onRecord: (NSRunningApplication) -> Void = { _ in }

    private var panel: NSPanel?
    private var petView: DesktopPetView?
    private var popover: NSPopover?
    private var target: NSRunningApplication?

    private let positionXKey = "pet.position.x"
    private let positionYKey = "pet.position.y"

    func show() {
        if panel == nil {
            let petView = DesktopPetView(frame: NSRect(x: 0, y: 0, width: 56, height: 56))
            petView.onClick = { [weak self] in self?.showPopover() }
            petView.onDragged = { [weak self] origin in self?.savePosition(origin) }
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
    }

    private func showPopover() {
        closePopover()

        let stack = NSStackView()
        stack.orientation = .vertical
        stack.spacing = 10
        stack.edgeInsets = NSEdgeInsets(top: 16, left: 16, bottom: 16, right: 16)

        let name = target?.localizedName ?? "未识别到上一个应用"
        let titleLabel = NSTextField(labelWithString: name)
        titleLabel.font = .systemFont(ofSize: 13, weight: .semibold)
        titleLabel.alignment = .center

        let button = NSButton(
            title: "截取 \(name) 窗口",
            target: self,
            action: #selector(performCapture)
        )
        button.bezelStyle = .rounded
        button.controlSize = .regular
        button.keyEquivalent = "\r"
        button.isEnabled = target != nil

        let recordButton = NSButton(
            title: "录制 \(name) 窗口",
            target: self,
            action: #selector(performRecord)
        )
        recordButton.bezelStyle = .rounded
        recordButton.controlSize = .regular
        recordButton.isEnabled = target != nil

        stack.addArrangedSubview(titleLabel)
        stack.addArrangedSubview(button)
        stack.addArrangedSubview(recordButton)
        stack.translatesAutoresizingMaskIntoConstraints = false

        let container = NSView()
        container.translatesAutoresizingMaskIntoConstraints = false
        container.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: container.topAnchor),
            stack.leadingAnchor.constraint(equalTo: container.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: container.trailingAnchor),
            stack.bottomAnchor.constraint(equalTo: container.bottomAnchor)
        ])

        let popover = NSPopover()
        popover.behavior = .transient
        popover.contentViewController = NSViewController()
        popover.contentViewController?.view = container
        popover.contentViewController?.view.frame = NSRect(x: 0, y: 0, width: 200, height: 120)
        self.popover = popover

        guard let petView else { return }
        popover.show(relativeTo: petView.bounds, of: petView, preferredEdge: .maxY)
    }

    private func closePopover() {
        popover?.close()
        popover = nil
    }

    @objc private func performCapture() {
        closePopover()
        guard let target else { return }
        onCapture(target)
    }

    @objc private func performRecord() {
        closePopover()
        guard let target else { return }
        onRecord(target)
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
                return point
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

private final class DesktopPetView: NSView {
    var onClick: () -> Void = {}
    var onDragged: (NSPoint) -> Void = { _ in }

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
        window.setFrameOrigin(newOrigin)
    }

    override func mouseUp(with event: NSEvent) {
        if didDrag, let window = self.window {
            onDragged(window.frame.origin)
        } else {
            onClick()
        }
    }
}
