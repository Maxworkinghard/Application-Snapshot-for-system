import AppKit

/// 润色服务连接设置：三项填写完整后「润色 Prompt」才会调用真实模型；未配置时润色会直接提示先完成配置。
final class PromptPolishSettingsController: NSObject, NSWindowDelegate {
    private let configurationStore: PolishBackendConfigurationStore
    private var window: NSWindow?
    private var kindPopUp: NSPopUpButton?
    private var baseURLField: NSTextField?
    private var modelField: NSTextField?
    private var apiKeyField: NSSecureTextField?
    private var errorLabel: NSTextField?

    init(configurationStore: PolishBackendConfigurationStore) {
        self.configurationStore = configurationStore
    }

    func show() {
        if let window {
            refreshFields()
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }

        let title = NSTextField(labelWithString: "润色服务设置")
        title.font = .systemFont(ofSize: 20, weight: .semibold)
        title.translatesAutoresizingMaskIntoConstraints = false

        let description = NSTextField(wrappingLabelWithString: "三项都填写后，「润色 Prompt」将调用所配置的模型润色；未配置或清除配置后，润色会提示先完成配置。API Key 保存在系统 Keychain 中。")
        description.textColor = .secondaryLabelColor
        description.font = .systemFont(ofSize: 12)
        description.translatesAutoresizingMaskIntoConstraints = false

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

        let errorLabel = NSTextField(labelWithString: "")
        errorLabel.font = .systemFont(ofSize: 12)
        errorLabel.textColor = .systemRed
        errorLabel.translatesAutoresizingMaskIntoConstraints = false
        self.errorLabel = errorLabel

        refreshFields()

        let form = NSStackView(views: [
            makeRow(label: "协议", field: kindPopUp),
            makeRow(label: "Base URL", field: baseURLField),
            makeRow(label: "模型", field: modelField),
            makeRow(label: "API Key", field: apiKeyField)
        ])
        form.orientation = .vertical
        form.spacing = 10
        form.alignment = .leading
        form.translatesAutoresizingMaskIntoConstraints = false

        let clearButton = NSButton(title: "清除配置", target: self, action: #selector(clear))
        clearButton.bezelStyle = .rounded
        clearButton.translatesAutoresizingMaskIntoConstraints = false

        let cancelButton = NSButton(title: "取消", target: self, action: #selector(cancel))
        cancelButton.bezelStyle = .rounded

        let saveButton = NSButton(title: "保存", target: self, action: #selector(save))
        saveButton.bezelStyle = .rounded
        saveButton.keyEquivalent = "\r"

        let rightButtons = NSStackView(views: [cancelButton, saveButton])
        rightButtons.orientation = .horizontal
        rightButtons.spacing = 8
        rightButtons.translatesAutoresizingMaskIntoConstraints = false

        let content = NSView()
        content.translatesAutoresizingMaskIntoConstraints = false
        content.addSubview(title)
        content.addSubview(description)
        content.addSubview(form)
        content.addSubview(errorLabel)
        content.addSubview(clearButton)
        content.addSubview(rightButtons)

        NSLayoutConstraint.activate([
            title.topAnchor.constraint(equalTo: content.topAnchor, constant: 24),
            title.leadingAnchor.constraint(equalTo: content.leadingAnchor, constant: 24),
            title.trailingAnchor.constraint(equalTo: content.trailingAnchor, constant: -24),

            description.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 6),
            description.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            description.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            form.topAnchor.constraint(equalTo: description.bottomAnchor, constant: 20),
            form.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            form.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            errorLabel.topAnchor.constraint(equalTo: form.bottomAnchor, constant: 12),
            errorLabel.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            errorLabel.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            clearButton.topAnchor.constraint(equalTo: errorLabel.bottomAnchor, constant: 16),
            clearButton.leadingAnchor.constraint(equalTo: title.leadingAnchor),
            clearButton.bottomAnchor.constraint(equalTo: content.bottomAnchor, constant: -20),

            rightButtons.centerYAnchor.constraint(equalTo: clearButton.centerYAnchor),
            rightButtons.trailingAnchor.constraint(equalTo: title.trailingAnchor),

            content.widthAnchor.constraint(equalToConstant: 420)
        ])

        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 420, height: 320),
            styleMask: [.titled, .closable],
            backing: .buffered,
            defer: false
        )
        window.title = "润色服务设置"
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
        kindPopUp = nil
        baseURLField = nil
        modelField = nil
        apiKeyField = nil
        errorLabel = nil
    }

    // MARK: - 动作

    @objc private func cancel() {
        window?.close()
    }

    @objc private func clear() {
        configurationStore.clear()
        window?.close()
    }

    @objc private func save() {
        let kind: PolishBackendProtocolKind = kindPopUp?.indexOfSelectedItem == 1 ? .anthropic : .openAICompatible
        let baseURL = (baseURLField?.stringValue ?? "").trimmingCharacters(in: .whitespaces)
        let model = (modelField?.stringValue ?? "").trimmingCharacters(in: .whitespaces)
        let enteredKey = apiKeyField?.stringValue ?? ""
        // 已保存过 Key 时，输入框留空表示“沿用原值”，而不是清空——所以清空配置一律走「清除配置」按钮，
        // 不在这里做「三项都空就清除」的隐式判断（那样在已保存 Key 时永远不会成立，容易让人困惑）。
        let apiKey = enteredKey.isEmpty ? configurationStore.configuration.apiKey : enteredKey

        guard !baseURL.isEmpty, let url = URL(string: baseURL), url.scheme == "http" || url.scheme == "https" else {
            errorLabel?.stringValue = "请输入有效的 Base URL（以 http/https 开头）"
            return
        }
        guard !model.isEmpty else {
            errorLabel?.stringValue = "请输入模型名称"
            return
        }
        guard !apiKey.isEmpty else {
            errorLabel?.stringValue = "请输入 API Key"
            return
        }

        configurationStore.save(PolishBackendConfiguration(kind: kind, baseURL: baseURL, model: model, apiKey: apiKey))
        window?.close()
    }

    // MARK: - 辅助

    private func refreshFields() {
        let configuration = configurationStore.configuration
        kindPopUp?.selectItem(at: configuration.kind == .anthropic ? 1 : 0)
        baseURLField?.stringValue = configuration.baseURL
        modelField?.stringValue = configuration.model
        apiKeyField?.stringValue = ""
        apiKeyField?.placeholderString = configuration.apiKey.isEmpty ? "sk-..." : "已保存，留空则保持不变"
        errorLabel?.stringValue = ""
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
