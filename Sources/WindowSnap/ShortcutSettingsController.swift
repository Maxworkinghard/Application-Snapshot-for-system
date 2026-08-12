import AppKit
import Carbon

final class ShortcutSettingsController: NSObject, NSWindowDelegate {
    var currentConfiguration: ShortcutConfiguration
    private let onSave: (ShortcutConfiguration) -> Bool
    private var window: NSWindow?
    private var recorder: ShortcutRecorderView?

    init(
        currentConfiguration: ShortcutConfiguration,
        onSave: @escaping (ShortcutConfiguration) -> Bool
    ) {
        self.currentConfiguration = currentConfiguration
        self.onSave = onSave
    }

    func show() {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let recorder = ShortcutRecorderView(configuration: currentConfiguration)
        recorder.translatesAutoresizingMaskIntoConstraints = false
        self.recorder = recorder

        let title = NSTextField(labelWithString: "快捷键")
        title.font = .systemFont(ofSize: 20, weight: .semibold)

        let description = NSTextField(labelWithString: "按下你想使用的组合键")
        description.textColor = .secondaryLabelColor
        description.font = .systemFont(ofSize: 13)

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

        let content = NSView()
        content.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(title)
        content.addSubview(description)
        content.addSubview(recorder)
        content.addSubview(buttons)

        NSLayoutConstraint.activate([
            title.topAnchor.constraint(equalTo: content.topAnchor, constant: 24),
            title.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 24),
            title.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -24),
            description.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 6),
            description.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            description.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            recorder.topAnchor.constraint(equalTo: description.bottomAnchor, constant: 20),
            recorder.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            recorder.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            recorder.heightAnchor.constraint(equalToConstant: 48),
            buttons.topAnchor.constraint(equalTo: recorder.bottomAnchor, constant: 24),
            buttons.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            buttons.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20),
            content.widthAnchor.constraint(equalToConstant: 360)
        ])

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 360, height: 196),
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
        recorder.window?.makeFirstResponder(recorder)
    }

    @objc private func cancel() {
        window?.close()
    }

    @objc private func save() {
        guard let configuration = recorder?.configuration else { return }
        guard onSave(configuration) else {
            NSSound.beep()
            recorder?.showError("这个快捷键已被占用，请换一个组合")
            return
        }
        window?.close()
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
        recorder = nil
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
        label.font = .systemFont(ofSize: 17, weight: .medium)
        label.translatesAutoresizingMaskIntoConstraints = false
        addSubview(label)
        NSLayoutConstraint.activate([
            label.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 12),
            label.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -12),
            label.centerYAnchor.constraint(equalTo: centerYAnchor)
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
        label.font = .systemFont(ofSize: 12, weight: .medium)
    }

    private func updateLabel() {
        label.stringValue = configuration.displayString
        label.textColor = .labelColor
        label.font = .systemFont(ofSize: 17, weight: .medium)
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
