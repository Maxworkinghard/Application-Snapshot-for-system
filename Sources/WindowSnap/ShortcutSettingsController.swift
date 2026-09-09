import AppKit
import Carbon

final class ShortcutSettingsController: NSObject, NSWindowDelegate {
    var currentSet: ShortcutSet
    private let onSave: (ShortcutSet) -> Bool
    private var window: NSWindow?
    private var recorders: [ShortcutRecorderView] = []
    private var statusLabel: NSTextField?

    private static let functionNames = [
        "截取当前应用窗口",
        "录制当前应用窗口",
        "截取上一个应用窗口",
        "润色提示词",
    ]

    init(
        currentSet: ShortcutSet,
        onSave: @escaping (ShortcutSet) -> Bool
    ) {
        self.currentSet = currentSet
        self.onSave = onSave
    }

    func show() {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let configurations = [
            currentSet.capture,
            currentSet.record,
            currentSet.capturePrevious,
            currentSet.polish,
        ]

        var rowViews: [NSView] = []
        recorders = []
        for (index, name) in Self.functionNames.enumerated() {
            let recorder = ShortcutRecorderView(configuration: configurations[index])
            recorder.translatesAutoresizingMaskIntoConstraints = false
            recorders.append(recorder)

            let label = NSTextField(labelWithString: name)
            label.font = .systemFont(ofSize: 13)
            label.textColor = .labelColor
            label.translatesAutoresizingMaskIntoConstraints = false

            let row = NSView()
            row.translatesAutoresizingMaskIntoConstraints = false
            row.addSubview(label)
            row.addSubview(recorder)
            NSLayoutConstraint.activate([
                label.leadingAnchor.constraint(equalTo: row.leadingAnchor),
                label.centerYAnchor.constraint(equalTo: row.centerYAnchor),
                recorder.trailingAnchor.constraint(equalTo: row.trailingAnchor),
                recorder.centerYAnchor.constraint(equalTo: row.centerYAnchor),
                recorder.leadingAnchor.constraint(greaterThanOrEqualTo: label.trailingAnchor, constant: 16),
                row.heightAnchor.constraint(equalToConstant: 44)
            ])
            rowViews.append(row)
        }

        let title = NSTextField(labelWithString: "快捷键")
        title.font = .systemFont(ofSize: 20, weight: .semibold)

        let description = NSTextField(labelWithString: "点击右侧组合键，再按下想使用的新组合")
        description.textColor = .secondaryLabelColor
        description.font = .systemFont(ofSize: 13)

        let status = NSTextField(labelWithString: "")
        status.font = .systemFont(ofSize: 12, weight: .medium)
        status.textColor = .systemRed
        status.translatesAutoresizingMaskIntoConstraints = false

        let cancelButton = NSButton(
            title: "取消",
            target: self,
            action: #selector(cancel)
        )
        cancelButton.bezelStyle = .rounded

        let saveButton = NSButton(
            title: "保存",
            target: self,
            action: #selector(save)
        )
        saveButton.bezelStyle = .rounded
        saveButton.keyEquivalent = "\r"

        let buttons = NSStackView(views: [cancelButton, saveButton])
        buttons.orientation = .horizontal
        buttons.spacing = 8
        buttons.alignment = .centerY

        let rows = NSStackView(views: rowViews)
        rows.orientation = .vertical
        rows.spacing = 6
        rows.translatesAutoresizingMaskIntoConstraints = false

        let content = NSView()
        content.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(title)
        content.addSubview(description)
        content.addSubview(rows)
        content.addSubview(status)
        content.addSubview(buttons)

        NSLayoutConstraint.activate([
            title.topAnchor.constraint(equalTo: content.topAnchor, constant: 24),
            title.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 24),
            title.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -24),
            description.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 6),
            description.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            description.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            rows.topAnchor.constraint(equalTo: description.bottomAnchor, constant: 18),
            rows.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            rows.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            status.topAnchor.constraint(equalTo: rows.bottomAnchor, constant: 10),
            status.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            status.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            status.heightAnchor.constraint(equalToConstant: 16),
            buttons.topAnchor.constraint(equalTo: status.bottomAnchor, constant: 14),
            buttons.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            buttons.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20),
            content.widthAnchor.constraint(equalToConstant: 380)
        ])

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 380, height: 400),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "应用快照快捷键"
        window.contentView = content
        window.delegate = self
        window.isReleasedWhenClosed = false
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
        self.statusLabel = status
    }

    @objc private func cancel() {
        window?.close()
    }

    @objc private func save() {
        guard recorders.count == 4 else { return }
        var set = currentSet
        set.capture = recorders[0].configuration
        set.record = recorders[1].configuration
        set.capturePrevious = recorders[2].configuration
        set.polish = recorders[3].configuration

        if set.hasConflict {
            showStatus("存在重复的快捷键组合，请调整")
            return
        }
        guard onSave(set) else {
            showStatus("有快捷键已被其他应用占用，请换一个组合")
            return
        }
        window?.close()
    }

    private func showStatus(_ message: String) {
        statusLabel?.stringValue = message
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
        recorders = []
        statusLabel = nil
    }
}

private final class ShortcutRecorderView: NSView {
    private(set) var configuration: ShortcutConfiguration
    private let label = NSTextField(labelWithString: "")

    init(configuration: ShortcutConfiguration) {
        self.configuration = configuration
        super.init(frame: .zero)

        wantsLayer = true
        layer?.cornerRadius = 8
        layer?.cornerCurve = .continuous
        layer?.borderWidth = 1
        layer?.borderColor = NSColor.separatorColor.cgColor
        layer?.backgroundColor = NSColor.controlBackgroundColor.cgColor

        label.alignment = .center
        label.font = .systemFont(ofSize: 15, weight: .medium)
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 10),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -10),
            label.centerYAnchor.constraint(equalTo: centerYAnchor),
            heightAnchor.constraint(equalToConstant: 34),
            widthAnchor.constraint(equalToConstant: 140)
        ])
        updateLabel()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var acceptsFirstResponder: Bool { true }

    override func becomeFirstResponder() -> Bool {
        layer?.borderColor = NSColor.controlAccentColor.cgColor
        return super.becomeFirstResponder()
    }

    override func resignFirstResponder() -> Bool {
        layer?.borderColor = NSColor.separatorColor.cgColor
        return super.resignFirstResponder()
    }

    override func keyDown(with event: NSEvent) {
        record(event)
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard carbonModifiers(from: event.modifierFlags) != 0 else {
            return super.performKeyEquivalent(with: event)
        }
        record(event)
        return true
    }

    private func record(_ event: NSEvent) {
        let modifiers = carbonModifiers(from: event.modifierFlags)
        guard modifiers != 0 else {
            NSSound.beep()
            showError("请使用至少一个修饰键，例如 ⌘、⌥、⌃ 或 ⇧")
            return
        }

        configuration = ShortcutConfiguration(
            keyCode: UInt32(event.keyCode),
            modifiers: modifiers
        )
        updateLabel()
    }

    override func flagsChanged(with event: NSEvent) {
        if event.modifierFlags.intersection([.command, .option, .control, .shift]).isEmpty {
            return
        }
        label.stringValue = "按下组合键…"
    }

    func showError(_ message: String) {
        label.stringValue = message
        label.textColor = .systemRed
        label.font = .systemFont(ofSize: 11, weight: .medium)
    }

    private func updateLabel() {
        label.stringValue = configuration.displayString
        label.textColor = .labelColor
        label.font = .systemFont(ofSize: 15, weight: .medium)
    }

    private func carbonModifiers(from flags: NSEvent.ModifierFlags) -> UInt32 {
        var modifiers: UInt32 = 0
        if flags.contains(.command) { modifiers |= UInt32(cmdKey) }
        if flags.contains(.option) { modifiers |= UInt32(optionKey) }
        if flags.contains(.control) { modifiers |= UInt32(controlKey) }
        if flags.contains(.shift) { modifiers |= UInt32(shiftKey) }
        return modifiers
    }
}
