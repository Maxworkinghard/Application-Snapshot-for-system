import AppKit

final class ToastController {
    private var panel: NSPanel?
    private var closeWorkItem: DispatchWorkItem?

    func show(message: String, symbolName: String) {
        closeWorkItem?.cancel()
        panel?.close()

        let content = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: 320, height: 56))
        content.material = .hudWindow
        content.blendingMode = .behindWindow
        content.state = .active
        content.wantsLayer = true
        content.layer?.cornerRadius = 8
        content.layer?.cornerCurve = .continuous

        let symbol = NSImageView(image: NSImage(
            systemSymbolName: symbolName,
            accessibilityDescription: nil
        ) ?? NSImage())
        symbol.symbolConfiguration = NSImage.SymbolConfiguration(pointSize: 17, weight: .semibold)
        symbol.contentTintColor = .labelColor
        symbol.translatesAutoresizingMaskIntoConstraints = false

        let label = NSTextField(labelWithString: message)
        label.font = .systemFont(ofSize: 13, weight: .medium)
        label.textColor = .labelColor
        label.lineBreakMode = .byTruncatingTail
        label.translatesAutoresizingMaskIntoConstraints = false

        content.addSubview(symbol)
        content.addSubview(label)
        NSLayoutConstraint.activate([
            symbol.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
            symbol.centerYAnchor.constraint(equalTo: content.centerYAnchor),
            symbol.widthAnchor.constraint(equalToConstant: 20),
            symbol.heightAnchor.constraint(equalToConstant: 20),
            label.leadingAnchor.constraint(equalTo: symbol.trailingAnchor, constant: 12),
            label.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -16),
            label.centerYAnchor.constraint(equalTo: content.centerYAnchor)
        ])

        let panel = NSPanel(
            contentRect: content.bounds,
            styleMask: [.borderless, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.contentView = content
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.level = .statusBar
        panel.collectionBehavior = [.canJoinAllSpaces, .transient, .ignoresCycle]
        panel.ignoresMouseEvents = true

        let screen = NSScreen.main ?? NSScreen.screens.first
        if let visibleFrame = screen?.visibleFrame {
            panel.setFrameOrigin(NSPoint(
                x: visibleFrame.maxX - panel.frame.width - 16,
                y: visibleFrame.maxY - panel.frame.height - 16
            ))
        }

        panel.orderFrontRegardless()
        self.panel = panel

        let closeWorkItem = DispatchWorkItem { [weak self, weak panel] in
            panel?.orderOut(nil)
            if self?.panel === panel {
                self?.panel = nil
            }
        }
        self.closeWorkItem = closeWorkItem
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.8, execute: closeWorkItem)
    }
}
