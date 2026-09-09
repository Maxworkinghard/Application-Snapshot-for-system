import AppKit
import Carbon

/// 统一设置窗口：全局快捷键绑定 + 润色 LLM Provider 配置。
/// 右键悬浮球「设置…」与菜单栏「设置…」都打开此窗口。
final class AppSettingsController: NSObject, NSWindowDelegate {
    var currentSet: ShortcutSet
    private let onSaveShortcuts: (ShortcutSet) -> Bool
    private let polishStore: PolishBackendConfigurationStore
    private var window: NSWindow?
    private var recorders: [ShortcutRecorderView] = []
    private var statusLabel: NSTextField?
    private var kindPopUp: NSPopUpButton?
    private var baseURLField: NSTextField?
    private var modelField: NSTextField?
    private var apiKeyField: NSSecureTextField?

    private static let shortcutNames = [
        "截取当前应用窗口",
        "录制当前应用窗口",
        "截取上一个应用窗口",
        "润色提示词",
    ]

    init(
        currentSet: ShortcutSet,
        polishStore: PolishBackendConfigurationStore,
        onSaveShortcuts: @escaping (ShortcutSet) -> Bool
    ) {
        self.currentSet = currentSet
        self.polishStore = polishStore
        self.onSaveShortcuts = onSaveShortcuts
    }

    func show() {
        if let window {
            refreshShortcutFields()
            refreshPolishFields()
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        buildWindow()
        refreshShortcutFields()
        refreshPolishFields()
    }

    private func buildWindow() {
        let title = NSTextField(labelWithString: "设置")
        title.font = .systemFont(ofSize: 20, weight: .semibold)
        title.translatesAutoresizingMaskIntoConstraints = false

        let content = NSView()
        content.translatesAutoresizingMaskIntoConstraints = false

        // MARK: 快捷键区块

        let shortcutHeader = NSTextField(labelWithString: "快捷键")
        shortcutHeader.font = .systemFont(ofSize: 15, weight: .semibold)
        shortcutHeader.translatesAutoresizingMaskIntoConstraints = false

        let shortcutHint = NSTextField(labelWithString: "点击组合键框后按下新组合；「×」清除绑定（留空即不启用）")
        shortcutHint.textColor = .secondaryLabelColor
        shortcutHint.font = .systemFont(ofSize: 12)
        shortcutHint.translatesAutoresizingMaskIntoConstraints = false

        var rows: [NSView] = []
        recorders = []
        for name in Self.shortcutNames {
            let recorder = ShortcutRecorderView()
            recorder.translatesAutoresizingMaskIntoConstraints = false
            recorders.append(recorder)

            let clearButton = NSButton(
                title: "×",
                target: self,
                action: #selector(clearShortcut)
            )
            clearButton.bezelStyle = .smallSquare
            clearButton.controlSize = .small
            clearButton.font = .systemFont(ofSize: 14)
            clearButton.translatesAutoresizingMaskIntoConstraints = false
            clearButton.identifier = NSUserInterfaceItemIdentifier(rawValue: name)

            let label = NSTextField(labelWithString: name)
            label.font = .systemFont(ofSize: 13)
            label.translatesAutoresizingMaskIntoConstraints = false

            let row = NSView()
            row.translatesAutoresizingMaskIntoConstraints = false
            row.addSubview(label)
            row.addSubview(recorder)
            row.addSubview(clearButton)
            NSLayoutConstraint.activate([
                label.leadingAnchor.constraint(equalTo: row.leadingAnchor),
                label.centerYAnchor.constraint(equalTo: row.centerYAnchor),
                recorder.trailingAnchor.constraint(equalTo: clearButton.leadingAnchor, constant: 6),
                recorder.centerYAnchor.constraint(equalTo: row.centerYAnchor),
                recorder.leadingAnchor.constraint(greaterThanOrEqualTo: label.trailingAnchor, constant: 16),
                clearButton.trailingAnchor.constraint(equalTo: row.trailingAnchor),
                clearButton.centerYAnchor.constraint(equalTo: row.centerYAnchor),
                clearButton.widthAnchor.constraint(equalToConstant: 24),
                clearButton.heightAnchor.constraint(equalToConstant: 24),
                row.heightAnchor.constraint(equalToConstant: 44)
            ])
            rows.append(row)
        }

        let shortcutRows = NSStackView(views: rows)
        shortcutRows.orientation = .vertical
        shortcutRows.spacing = 6
        shortcutRows.translatesAutoresizingMaskIntoConstraints = false

        let shortcutDivider = NSBox()
        shortcutDivider.boxType = .separator
        shortcutDivider.translatesAutoresizingMaskIntoConstraints = false

        // MARK: 润色服务区块

        let polishHeader = NSTextField(labelWithString: "润色服务（LLM Provider）")
        polishHeader.font = .systemFont(ofSize: 15, weight: .semibold)
        polishHeader.translatesAutoresizingMaskIntoConstraints = false

        let polishHint = NSTextField(wrappingLabelWithString: "三项都填写后，「润色」才会调用所配置的模型；未配置时润色会提示先完成配置。API Key 保存在系统 Keychain 中。")
        polishHint.textColor = .secondaryLabelColor
        polishHint.font = .systemFont(ofSize: 12)
        polishHint.translatesAutoresizingMaskIntoConstraints = false

        let kindPopUp = NSPopUpButton()
        kindPopUp.addItem(withTitle: PolishBackendProtocolKind.openAICompatible.displayName)
        kindPopUp.addItem(withTitle: PolishBackendProtocolKind.anthropic.displayName)
        self.kindPopUp = kindPopUp

        let baseURLField = NSTextField(string: "")
        baseURLField.placeholderString = "https://api.openai.com/v1"
        self.baseURLField = baseURLField

        let modelField = NSTextField(string: "")
        modelField.placeholderString = "gpt-4o-mini"
        self.modelField = modelField

        let apiKeyField = NSSecureTextField(string: "")
        self.apiKeyField = apiKeyField

        let polishForm = NSStackView(views: [
            makeRow(label: "协议", field: kindPopUp),
            makeRow(label: "Base URL", field: baseURLField),
            makeRow(label: "模型", field: modelField),
            makeRow(label: "API Key", field: apiKeyField)
        ])
        polishForm.orientation = .vertical
        polishForm.spacing = 10
        polishForm.alignment = .leading
        polishForm.translatesAutoresizingMaskIntoConstraints = false

        let status = NSTextField(labelWithString: "")
        status.font = .systemFont(ofSize: 12, weight: .medium)
        status.textColor = .systemRed
        status.translatesAutoresizingMaskIntoConstraints = false
        self.statusLabel = status

        // MARK: 底部按钮

        let clearPolishButton = NSButton(title: "清除润色配置", target: self, action: #selector(clearPolish))
        clearPolishButton.bezelStyle = .rounded
        clearPolishButton.translatesAutoresizingMaskIntoConstraints = false

        let cancelButton = NSButton(title: "取消", target: self, action: #selector(cancel))
        cancelButton.bezelStyle = .rounded

        let saveButton = NSButton(title: "保存", target: self, action: #selector(save))
        saveButton.bezelStyle = .rounded
        saveButton.keyEquivalent = "\r"

        let rightButtons = NSStackView(views: [cancelButton, saveButton])
        rightButtons.orientation = .horizontal
        rightButtons.spacing = 8
        rightButtons.translatesAutoresizingMaskIntoConstraints = false

        content.addSubview(title)
        content.addSubview(shortcutHeader)
        content.addSubview(shortcutHint)
        content.addSubview(shortcutRows)
        content.addSubview(shortcutDivider)
        content.addSubview(polishHeader)
        content.addSubview(polishHint)
        content.addSubview(polishForm)
        content.addSubview(status)
        content.addSubview(clearPolishButton)
        content.addSubview(rightButtons)

        NSLayoutConstraint.activate([
            title.topAnchor.constraint(equalTo: content.topAnchor, constant: 24),
            title.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 24),
            title.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -24),

            shortcutHeader.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 18),
            shortcutHeader.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            shortcutHeader.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            shortcutHint.topAnchor.constraint(equalTo: shortcutHeader.bottomAnchor, constant: 4),
            shortcutHint.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            shortcutHint.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            shortcutRows.topAnchor.constraint(equalTo: shortcutHint.bottomAnchor, constant: 10),
            shortcutRows.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            shortcutRows.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            shortcutDivider.topAnchor.constraint(equalTo: shortcutRows.bottomAnchor, constant: 16),
            shortcutDivider.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            shortcutDivider.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            polishHeader.topAnchor.constraint(equalTo: shortcutDivider.bottomAnchor, constant: 16),
            polishHeader.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            polishHeader.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            polishHint.topAnchor.constraint(equalTo: polishHeader.bottomAnchor, constant: 4),
            polishHint.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            polishHint.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            polishForm.topAnchor.constraint(equalTo: polishHint.bottomAnchor, constant: 12),
            polishForm.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            polishForm.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            status.topAnchor.constraint(equalTo: polishForm.bottomAnchor, constant: 10),
            status.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            status.trailingAnchor.constraint(equalTo: title.trailingAnchor),
            status.heightAnchor.constraint(equalToConstant: 16),

            clearPolishButton.topAnchor.constraint(equalTo: status.bottomAnchor, constant: 14),
            clearPolishButton.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            clearPolishButton.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20),

            rightButtons.centerYAnchor.constraint(equalTo: clearPolishButton.centerYAnchor),
            rightButtons.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            content.widthAnchor.constraint(equalToConstant: 460)
        ])

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 460, height: 640),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "应用快照设置"
        window.contentView = content
        window.delegate = self
        window.isReleasedWhenClosed = false
        window.center()
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        self.window = window
    }

    func windowWillClose(_ notification: Notification) {
        window = nil
        recorders = []
        statusLabel = nil
        kindPopUp = nil
        baseURLField = nil
        modelField = nil
        apiKeyField = nil
    }

    // MARK: - 动作

    @objc private func cancel() {
        window?.close()
    }

    @objc private func clearShortcut(_ sender: NSButton) {
        guard let name = sender.identifier?.rawValue,
              let index = Self.shortcutNames.firstIndex(of: name),
              recorders.indices.contains(index) else { return }
        recorders[index].clear()
    }

    @objc private func clearPolish() {
        polishStore.clear()
        refreshPolishFields()
        showStatus("润色配置已清除")
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
        guard onSaveShortcuts(set) else {
            showStatus("有快捷键已被其他应用占用，请换一个组合")
            return
        }

        if let error = validateAndSavePolish() {
            showStatus(error)
            return
        }
        window?.close()
    }

    /// 润色配置校验：全空视为「不启用」直接放行；填了部分则要求完整。
    private func validateAndSavePolish() -> String? {
        let kind: PolishBackendProtocolKind = kindPopUp?.indexOfSelectedItem == 1 ? .anthropic : .openAICompatible
        let baseURL = (baseURLField?.stringValue ?? "").trimmingCharacters(in: .whitespaces)
        let model = (modelField?.stringValue ?? "").trimmingCharacters(in: .whitespaces)
        let enteredKey = apiKeyField?.stringValue ?? ""

        let existingKey = polishStore.configuration.apiKey
        let allEmpty = baseURL.isEmpty && model.isEmpty && enteredKey.isEmpty && existingKey.isEmpty

        if allEmpty {
            return nil
        }
        guard !baseURL.isEmpty, let url = URL(string: baseURL), url.scheme == "http" || url.scheme == "https" else {
            return "请输入有效的 Base URL（以 http/https 开头）"
        }
        guard !model.isEmpty else {
            return "请输入模型名称"
        }
        // 已保存过 Key 时，输入框留空表示「沿用原值」；清空配置一律走「清除润色配置」按钮。
        guard !enteredKey.isEmpty || !existingKey.isEmpty else {
            return "请输入 API Key"
        }
        let apiKey = enteredKey.isEmpty ? existingKey : enteredKey

        polishStore.save(PolishBackendConfiguration(kind: kind, baseURL: baseURL, model: model, apiKey: apiKey))
        return nil
    }

    // MARK: - 刷新

    private func refreshShortcutFields() {
        guard recorders.count == 4 else { return }
        recorders[0].configuration = currentSet.capture
        recorders[1].configuration = currentSet.record
        recorders[2].configuration = currentSet.capturePrevious
        recorders[3].configuration = currentSet.polish
    }

    private func refreshPolishFields() {
        let configuration = polishStore.configuration
        kindPopUp?.selectItem(at: configuration.kind == .anthropic ? 1 : 0)
        baseURLField?.stringValue = configuration.baseURL
        modelField?.stringValue = configuration.model
        apiKeyField?.stringValue = ""
        apiKeyField?.placeholderString = configuration.apiKey.isEmpty ? "sk-..." : "已保存，留空则保持不变"
    }

    private func showStatus(_ message: String) {
        statusLabel?.stringValue = message
    }

    private func makeRow(label labelText: String, field: NSView) -> NSView {
        let label = NSTextField(labelWithString: labelText)
        label.font = .systemFont(ofSize: 13)
        label.alignment = .right
        label.translatesAutoresizingMaskIntoConstraints = false
        label.widthAnchor.constraint(equalToConstant: 72).isActive = true

        field.translatesAutoresizingMaskIntoConstraints = false

        let row = NSStackView(views: [label, field])
        row.orientation = .horizontal
        row.spacing = 10
        row.alignment = .centerY
        return row
    }
}

private final class ShortcutRecorderView: NSView {
    var configuration: ShortcutConfiguration? {
        didSet { updateLabel() }
    }

    private let label = NSTextField(labelWithString: "")

    init() {
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

    func clear() {
        configuration = nil
    }

    private func record(_ event: NSEvent) {
        let modifiers = carbonModifiers(from: event.modifierFlags)
        guard modifiers != 0 else {
            NSSound.beep()
            showError("至少一个修饰键")
            return
        }

        configuration = ShortcutConfiguration(
            keyCode: UInt32(event.keyCode),
            modifiers: modifiers
        )
    }

    override func flagsChanged(with event: NSEvent) {
        if event.modifierFlags.intersection([.command, .option, .control, .shift]).isEmpty {
            return
        }
        label.stringValue = "按下组合键…"
    }

    private func showError(_ message: String) {
        label.stringValue = message
        label.textColor = .systemRed
        label.font = .systemFont(ofSize: 11, weight: .medium)
    }

    private func updateLabel() {
        if let configuration {
            label.stringValue = configuration.displayString
            label.textColor = .labelColor
            label.font = .systemFont(ofSize: 15, weight: .medium)
        } else {
            label.stringValue = "未设置"
            label.textColor = .secondaryLabelColor
            label.font = .systemFont(ofSize: 13, weight: .regular)
        }
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
