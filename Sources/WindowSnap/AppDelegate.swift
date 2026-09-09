import AppKit
import Carbon

enum RecordingState: Equatable {
    case idle
    case starting
    case recording
    case stopping
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let captureService = WindowCaptureService()
    private let recordingService = WindowRecordingService()
    private let toastController = ToastController()
    private let shortcutStore = ShortcutStore()
    private let saveDirectoryStore = SaveDirectoryStore()
    private let capturableApplicationService = CapturableApplicationService()
    /// 润色调用「润色设置…」中配置的真实模型；未配置完整时直接报错引导配置，不再回退本地模板。
    private let polishConfigurationStore = PolishBackendConfigurationStore()
    private lazy var promptPolishingService: PromptPolishingService = RemotePromptPolishingService(
        configurationStore: polishConfigurationStore
    )
    private var statusItem: NSStatusItem?
    private var captureMenuItem: NSMenuItem?
    private var recordMenuItem: NSMenuItem?
    private var hotKey: GlobalHotKey?
    private var recordingHotKey: GlobalHotKey?
    private var shortcutConfiguration = ShortcutConfiguration.default
    private var shortcutSettingsController: ShortcutSettingsController?
    private var promptPolishSettingsController: PromptPolishSettingsController?
    private var workspaceObserver: NSObjectProtocol?
    private var terminationObserver: NSObjectProtocol?
    private var lastExternalApplication: NSRunningApplication?
    private var previousExternalApplication: NSRunningApplication?
    private var petController: DesktopPetController?
    private var recordingControlsPanel: RecordingControlsPanel?
    private var windowPickerProcess: Process?
    private var rightClickMonitor: Any?
    private var windowPickerWasCancelled = false
    private var isCapturing = false
    private var recordingState: RecordingState = .idle {
        didSet {
            updateRecordingUI()
        }
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        rememberFrontmostApplication()
        observeApplicationChanges()
        observeApplicationTermination()
        shortcutConfiguration = shortcutStore.configuration
        configureStatusItem()
        configureDesktopPet()

        if !registerShortcut(shortcutConfiguration) {
            shortcutConfiguration = .default
            shortcutStore.save(shortcutConfiguration)
            updateCaptureMenuShortcut()

            if !registerShortcut(shortcutConfiguration) {
                toastController.show(message: "快捷键注册失败，请从菜单栏截取", symbolName: "exclamationmark.triangle")
            } else {
                toastController.show(message: "原快捷键被占用，已恢复默认快捷键", symbolName: "exclamationmark.triangle")
            }
        }

        _ = registerRecordingShortcut()
    }

    func applicationWillTerminate(_ notification: Notification) {
        removeWindowSelectionMonitor()
        windowPickerProcess?.terminate()

        if let workspaceObserver {
            NSWorkspace.shared.notificationCenter.removeObserver(workspaceObserver)
        }
        if let terminationObserver {
            NSWorkspace.shared.notificationCenter.removeObserver(terminationObserver)
        }
    }

    @objc private func captureFrontWindow() {
        captureWindow(of: currentTargetApplication())
    }

    private func captureWindow(of application: NSRunningApplication?) {
        guard !isCapturing else { return }
        isCapturing = true

        let processID = application?.processIdentifier
        captureService.captureFrontWindow(processID: processID) { [weak self] result in
            guard let self else { return }
            self.isCapturing = false

            switch result {
            case .success(let capturedWindow):
                self.toastController.show(
                    message: "已复制 \(capturedWindow.applicationName) 窗口，60 秒后自动清空",
                    symbolName: "checkmark"
                )
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle"
                )
            }
        }
    }

    @objc private func toggleRecording() {
        switch recordingState {
        case .idle:
            startRecording(of: currentTargetApplication())
        case .recording:
            stopRecording()
        case .starting, .stopping:
            break
        }
    }

    private func startRecording(of application: NSRunningApplication?) {
        guard recordingState == .idle else { return }
        guard let application else {
            toastController.show(message: "没有找到可录制的应用", symbolName: "exclamationmark.triangle")
            return
        }
        recordingState = .starting

        captureService.frontWindow(processID: application.processIdentifier) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let window):
                let appName = window.owningApplication?.applicationName ?? "应用"
                self.recordingService.startRecording(
                    window: window,
                    applicationName: appName
                ) { [weak self] result in
                    guard let self else { return }
                    switch result {
                    case .success:
                        self.beginRecordingUI()
                    case .failure(let error):
                        self.recordingState = .idle
                        self.toastController.show(
                            message: error.localizedDescription,
                            symbolName: "exclamationmark.triangle"
                        )
                    }
                }
            case .failure(let error):
                self.recordingState = .idle
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle"
                )
            }
        }
    }

    private func beginRecordingUI() {
        recordingState = .recording

        let panel = RecordingControlsPanel()
        panel.show(startTime: Date()) { [weak self] in
            self?.stopRecording()
        }
        recordingControlsPanel = panel
    }

    private func stopRecording() {
        guard recordingState == .recording else { return }
        recordingState = .stopping
        recordingControlsPanel?.close()
        recordingControlsPanel = nil

        recordingService.stopRecording { [weak self] result in
            guard let self else { return }
            self.recordingState = .idle
            switch result {
            case .success(let recording):
                self.toastController.show(
                    message: "已保存到 \(recording.url.lastPathComponent)",
                    symbolName: "checkmark"
                )
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle"
                )
            }
        }
    }

    private func updateRecordingUI() {
        let title: String
        let isEnabled: Bool
        switch recordingState {
        case .idle:
            title = "录制当前应用窗口"
            isEnabled = true
        case .starting:
            title = "正在开始录制…"
            isEnabled = false
        case .recording:
            title = "停止录制"
            isEnabled = true
        case .stopping:
            title = "正在保存录制…"
            isEnabled = false
        }

        recordMenuItem?.title = title
        recordMenuItem?.isEnabled = isEnabled
        recordMenuItem?.keyEquivalentModifierMask = [.option, .shift]
        recordMenuItem?.keyEquivalent = "r"
        petController?.updateRecordingState()
    }

    private func registerRecordingShortcut() -> Bool {
        let config = ShortcutConfiguration(
            keyCode: UInt32(kVK_ANSI_R),
            modifiers: UInt32(optionKey | shiftKey)
        )
        let hotKey = GlobalHotKey(configuration: config) { [weak self] in
            self?.toggleRecording()
        }
        guard let hotKey else { return false }
        recordingHotKey = hotKey
        return true
    }

    @objc private func chooseWindow() {
        guard windowPickerProcess == nil else { return }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
        process.arguments = ["-i", "-w", "-c"]
        windowPickerWasCancelled = false
        windowPickerProcess = process
        rightClickMonitor = NSEvent.addGlobalMonitorForEvents(
            matching: .rightMouseDown
        ) { [weak self] _ in
            self?.cancelWindowSelection()
        }
        process.terminationHandler = { [weak self] process in
            DispatchQueue.main.async {
                guard let self else { return }
                self.removeWindowSelectionMonitor()
                self.windowPickerProcess = nil

                guard !self.windowPickerWasCancelled,
                      process.terminationStatus == 0 else {
                    return
                }

                self.toastController.show(message: "窗口已复制，可直接 ⌘V", symbolName: "checkmark")
            }
        }

        do {
            try process.run()
        } catch {
            removeWindowSelectionMonitor()
            windowPickerProcess = nil
            toastController.show(message: error.localizedDescription, symbolName: "exclamationmark.triangle")
        }
    }

    private func cancelWindowSelection() {
        guard let process = windowPickerProcess, process.isRunning else { return }
        windowPickerWasCancelled = true
        process.terminate()
    }

    private func removeWindowSelectionMonitor() {
        if let rightClickMonitor {
            NSEvent.removeMonitor(rightClickMonitor)
            self.rightClickMonitor = nil
        }
    }

    @objc private func openScreenRecordingSettings() {
        guard let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture") else {
            return
        }
        NSWorkspace.shared.open(url)
    }

    @objc private func chooseSaveDirectory() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.canCreateDirectories = true
        panel.allowsMultipleSelection = false
        panel.directoryURL = saveDirectoryStore.directory
        panel.message = "选择录制文件的保存目录（截图只进剪贴板，不产生文件）"
        panel.prompt = "选择"

        NSApp.activate(ignoringOtherApps: true)
        guard panel.runModal() == .OK, let url = panel.url else { return }
        saveDirectoryStore.save(url)
        let display = (url.path as NSString).abbreviatingWithTildeInPath
        toastController.show(message: "录制将保存到 \(display)", symbolName: "checkmark")
    }

    @objc private func openShortcutSettings() {
        if shortcutSettingsController == nil {
            shortcutSettingsController = ShortcutSettingsController(
                currentConfiguration: shortcutConfiguration
            ) { [weak self] configuration in
                self?.applyShortcut(configuration) ?? false
            }
        }
        shortcutSettingsController?.currentConfiguration = shortcutConfiguration
        shortcutSettingsController?.show()
    }

    @objc private func openPromptPolishSettings() {
        if promptPolishSettingsController == nil {
            promptPolishSettingsController = PromptPolishSettingsController(configurationStore: polishConfigurationStore)
        }
        promptPolishSettingsController?.show()
    }

    @objc private func quit() {
        NSApplication.shared.terminate(nil)
    }

    private func configureStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = NSImage(
            systemSymbolName: "camera.viewfinder",
            accessibilityDescription: "应用快照"
        )
        item.button?.toolTip = "应用快照"

        let menu = NSMenu()
        let captureItem = NSMenuItem(
            title: "截取当前应用窗口",
            action: #selector(captureFrontWindow),
            keyEquivalent: "2"
        )
        captureItem.keyEquivalentModifierMask = [.option, .shift]
        captureItem.target = self
        menu.addItem(captureItem)
        captureMenuItem = captureItem
        updateCaptureMenuShortcut()

        let recordItem = NSMenuItem(
            title: "录制当前应用窗口",
            action: #selector(toggleRecording),
            keyEquivalent: "r"
        )
        recordItem.keyEquivalentModifierMask = [.option, .shift]
        recordItem.target = self
        menu.addItem(recordItem)
        recordMenuItem = recordItem

        let chooseItem = NSMenuItem(
            title: "选择其他窗口…",
            action: #selector(chooseWindow),
            keyEquivalent: ""
        )
        chooseItem.target = self
        menu.addItem(chooseItem)

        menu.addItem(.separator())

        let shortcutItem = NSMenuItem(
            title: "设置快捷键…",
            action: #selector(openShortcutSettings),
            keyEquivalent: ""
        )
        shortcutItem.target = self
        menu.addItem(shortcutItem)

        let saveDirectoryItem = NSMenuItem(
            title: "设置保存目录…",
            action: #selector(chooseSaveDirectory),
            keyEquivalent: ""
        )
        saveDirectoryItem.target = self
        menu.addItem(saveDirectoryItem)

        let polishSettingsItem = NSMenuItem(
            title: "润色设置…",
            action: #selector(openPromptPolishSettings),
            keyEquivalent: ""
        )
        polishSettingsItem.target = self
        menu.addItem(polishSettingsItem)

        let settingsItem = NSMenuItem(
            title: "屏幕录制设置…",
            action: #selector(openScreenRecordingSettings),
            keyEquivalent: ""
        )
        settingsItem.target = self
        menu.addItem(settingsItem)

        menu.addItem(.separator())

        let quitItem = NSMenuItem(
            title: "退出应用快照",
            action: #selector(quit),
            keyEquivalent: "q"
        )
        quitItem.target = self
        menu.addItem(quitItem)

        item.menu = menu
        statusItem = item
    }

    private func registerShortcut(_ configuration: ShortcutConfiguration) -> Bool {
        guard let newHotKey = GlobalHotKey(
            configuration: configuration,
            action: { [weak self] in self?.captureFrontWindow() }
        ) else {
            return false
        }
        hotKey = newHotKey
        return true
    }

    private func applyShortcut(_ configuration: ShortcutConfiguration) -> Bool {
        guard configuration != shortcutConfiguration else { return true }
        guard registerShortcut(configuration) else { return false }

        shortcutConfiguration = configuration
        shortcutStore.save(configuration)
        updateCaptureMenuShortcut()
        toastController.show(message: "快捷键已设为 \(configuration.displayString)", symbolName: "checkmark")
        return true
    }

    private func updateCaptureMenuShortcut() {
        captureMenuItem?.keyEquivalent = shortcutConfiguration.menuKeyEquivalent
        captureMenuItem?.keyEquivalentModifierMask = shortcutConfiguration.menuModifierMask
    }

    private func observeApplicationChanges() {
        workspaceObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didActivateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let application = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication,
                  application.processIdentifier != ProcessInfo.processInfo.processIdentifier else {
                return
            }
            self?.recordFrontmostApplication(application)
        }
    }

    private func observeApplicationTermination() {
        terminationObserver = NSWorkspace.shared.notificationCenter.addObserver(
            forName: NSWorkspace.didTerminateApplicationNotification,
            object: nil,
            queue: .main
        ) { [weak self] notification in
            guard let application = notification.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication else {
                return
            }
            self?.clearTerminatedApplication(application)
        }
    }

    private func recordFrontmostApplication(_ application: NSRunningApplication) {
        guard application != lastExternalApplication else { return }
        previousExternalApplication = lastExternalApplication
        lastExternalApplication = application
        petController?.updateTarget(previousExternalApplication)
    }

    private func clearTerminatedApplication(_ application: NSRunningApplication) {
        var changed = false
        if lastExternalApplication == application {
            lastExternalApplication = nil
            changed = true
        }
        if previousExternalApplication == application {
            previousExternalApplication = nil
            changed = true
        }
        if changed {
            petController?.updateTarget(previousExternalApplication)
        }
    }

    private func configureDesktopPet() {
        let controller = DesktopPetController(applicationService: capturableApplicationService)
        controller.onCapture = { [weak self] app in self?.captureWindow(of: app) }
        controller.onRecord = { [weak self] app in self?.startRecording(of: app) }
        controller.onStopRecording = { [weak self] in self?.stopRecording() }
        controller.onRecordingState = { [weak self] in self?.recordingState ?? .idle }
        controller.onPolishPrompt = { [weak self] in self?.polishPromptFromClipboard() }
        controller.onPolishBusy = { [weak self] in self?.isPolishBusy() ?? false }
        controller.onOpenScreenRecordingSettings = { [weak self] in self?.openScreenRecordingSettings() }
        controller.show()
        controller.updateTarget(previousExternalApplication)
        petController = controller
    }

    private static let maxPolishInputLength = 12_000
    private static let confirmPreviewLength = 200

    private enum PolishRuntimeState {
        case idle
        case polishing(task: PromptPolishingTask, requestID: UUID)
    }

    private var polishRuntimeState: PolishRuntimeState = .idle

    private func isPolishBusy() -> Bool {
        if case .polishing = polishRuntimeState { return true }
        return false
    }

    /// 润色流程：剪切板文字 → 用户确认 → 服务润色 → 结果写回剪切板（替换原文）。
    /// 处理中再次点击视为取消；写回前会校验剪切板未被外部改动，避免用旧结果覆盖用户新复制的内容。
    private func polishPromptFromClipboard() {
        if case .polishing(let task, _) = polishRuntimeState {
            task.cancel()
            polishRuntimeState = .idle
            toastController.show(message: "已停止润色，剪切板未改动", symbolName: "xmark")
            return
        }

        let pasteboard = NSPasteboard.general
        let text = (pasteboard.string(forType: .string) ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        guard !text.isEmpty else {
            toastController.show(message: "剪切板没有文字，请先复制 Prompt", symbolName: "exclamationmark.triangle")
            return
        }
        guard text.count <= Self.maxPolishInputLength else {
            toastController.show(message: "剪切板内容过长（上限 \(Self.maxPolishInputLength) 字）", symbolName: "exclamationmark.triangle")
            return
        }

        let baselineChangeCount = pasteboard.changeCount
        guard confirmPolish(previewText: text) else { return }
        guard pasteboard.changeCount == baselineChangeCount else {
            toastController.show(message: "剪切板内容已变化，请重新点击润色", symbolName: "exclamationmark.triangle")
            return
        }

        toastController.show(message: "正在润色…", symbolName: "wand.and.stars")

        let requestID = UUID()
        let task = promptPolishingService.polish(
            request: PromptPolishRequest(text: text)
        ) { [weak self] result in
            guard let self else { return }
            guard case .polishing(_, let activeID) = self.polishRuntimeState, activeID == requestID else {
                return // 已取消或已被更新的状态覆盖，忽略过期回调
            }
            self.polishRuntimeState = .idle

            switch result {
            case .success(let response):
                guard pasteboard.changeCount == baselineChangeCount else {
                    self.toastController.show(message: "剪切板内容已变化，润色结果未写入", symbolName: "exclamationmark.triangle")
                    return
                }
                pasteboard.clearContents()
                pasteboard.setString(response.polishedText, forType: .string)
                self.toastController.show(
                    message: "润色完成，结果已替换剪切板",
                    symbolName: "checkmark"
                )
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle"
                )
            }
        }
        polishRuntimeState = .polishing(task: task, requestID: requestID)
    }

    /// 系统确认弹窗（非独立编辑窗口）：用户确认后才会覆盖剪切板内容。
    private func confirmPolish(previewText: String) -> Bool {
        let alert = NSAlert()
        alert.alertStyle = .informational
        alert.messageText = "润色剪切板中的 Prompt？"
        alert.informativeText = "确认后将用润色结果替换剪切板内容：\n\n\(truncatedPreview(previewText))"
        alert.addButton(withTitle: "润色并替换")
        alert.addButton(withTitle: "取消")
        NSApp.activate(ignoringOtherApps: true)
        return alert.runModal() == .alertFirstButtonReturn
    }

    private func truncatedPreview(_ text: String) -> String {
        guard text.count > Self.confirmPreviewLength else { return text }
        return String(text.prefix(Self.confirmPreviewLength)) + "…"
    }

    private func rememberFrontmostApplication() {
        guard let application = NSWorkspace.shared.frontmostApplication,
              application.processIdentifier != ProcessInfo.processInfo.processIdentifier else {
            return
        }
        lastExternalApplication = application
        previousExternalApplication = application
    }

    private func currentTargetApplication() -> NSRunningApplication? {
        if let application = NSWorkspace.shared.frontmostApplication,
           application.processIdentifier != ProcessInfo.processInfo.processIdentifier {
            lastExternalApplication = application
            return application
        }
        return lastExternalApplication
    }
}
