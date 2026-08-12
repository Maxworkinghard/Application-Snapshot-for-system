import AppKit

/// 录制中浮动控制条：屏幕中下方的小药丸，显示已录时长 + 停止按钮。
/// 对应 macOS 端 Toast 的视觉语言（毛玻璃 HUD），但可点击。
final class RecordingControlsPanel {
    private var panel: NSPanel?
    private var timer: DispatchSourceTimer?
    private var startTime: Date?
    private var onStop: (() -> Void)?

    func show(startTime: Date, onStop: @escaping () -> Void) {
        close()
        self.startTime = startTime
        self.onStop = onStop

        let content = NSVisualEffectView(frame: NSRect(x: 0, y: 0, width: 220, height: 40))
        content.material = .hudWindow
        content.blendingMode = .behindWindow
        content.state = .active
        content.wantsLayer = true
        content.layer?.cornerRadius = 20
        content.layer?.cornerCurve = .continuous

        let redDot = NSView(frame: NSRect(x: 0, y: 0, width: 10, height: 10))
        redDot.wantsLayer = true
        redDot.layer?.backgroundColor = NSColor.systemRed.cgColor
        redDot.layer?.cornerRadius = 5
        redDot.translatesAutoresizingMaskIntoConstraints = false

        let timeLabel = NSTextField(labelWithString: "00:00")
        timeLabel.font = .monospacedDigitSystemFont(ofSize: 13, weight: .semibold)
        timeLabel.textColor = .labelColor
        timeLabel.translatesAutoresizingMaskIntoConstraints = false

        let stopButton = NSButton(
            title: "停止",
            target: self,
            action: #selector(handleStop)
        )
        stopButton.bezelStyle = .rounded
        stopButton.controlSize = .small
        stopButton.translatesAutoresizingMaskIntoConstraints = false

        content.addSubview(redDot)
        content.addSubview(timeLabel)
        content.addSubview(stopButton)
        NSLayoutConstraint.activate([
            redDot.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 16),
            redDot.centerYAnchor.constraint(equalTo: content.centerYAnchor),
            redDot.widthAnchor.constraint(equalToConstant: 10),
            redDot.heightAnchor.constraint(equalToConstant: 10),
            timeLabel.leadingAnchor.constraint(equalTo: redDot.trailingAnchor, constant: 8),
            timeLabel.centerYAnchor.constraint(equalTo: content.centerYAnchor),
            stopButton.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -12),
            stopButton.centerYAnchor.constraint(equalTo: content.centerYAnchor),
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
        panel.ignoresMouseEvents = false
        panel.hidesOnDeactivate = false

        if let visibleFrame = (NSScreen.main ?? NSScreen.screens.first)?.visibleFrame {
            panel.setFrameOrigin(NSPoint(
                x: visibleFrame.midX - panel.frame.width / 2,
                y: visibleFrame.minY + 48
            ))
        }
        panel.orderFrontRegardless()
        self.panel = panel

        let timer = DispatchSource.makeTimerSource(queue: .main)
        timer.schedule(deadline: .now(), repeating: 1.0)
        timer.setEventHandler { [weak self, weak timeLabel] in
            guard let self, let start = self.startTime, let timeLabel else { return }
            let elapsed = Date().timeIntervalSince(start)
            let secs = Int(elapsed)
            timeLabel.stringValue = String(format: "%02d:%02d", secs / 60, secs % 60)
        }
        timer.resume()
        self.timer = timer
    }

    func close() {
        timer?.cancel()
        timer = nil
        panel?.orderOut(nil)
        panel = nil
        onStop = nil
        startTime = nil
    }

    @objc private func handleStop() {
        onStop?()
    }
}
