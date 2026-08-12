import AppKit
import Carbon

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let captureService = WindowCaptureService()
    private let recordingService = WindowRecordingService()
    private let toastController = ToastController()
    private let shortcutStore = ShortcutStore()
    private let saveDirectoryStore = SaveDirectoryStore()
    private var statusItem: NSStatusItem?
    private var captureMenuItem: NSMenuItem?
    private var recordMenuItem: NSMenuItem?
    private var hotKey: GlobalHotKey?
    private var recordingHotKey: GlobalHotKey?
    private var shortcutConfiguration = ShortcutConfiguration.default
    private var shortcutSettingsController: ShortcutSettingsController?
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
    private var isRecording = false

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
        if isRecording {
            stopRecording()
        } else {
            startRecording(of: currentTargetApplication())
        }
    }

    private func startRecording(of application: NSRunningApplication?) {
        guard !isRecording else { return }
        guard let application else {
            toastController.show(message: "没有找到可录制的应用", symbolName: "exclamationmark.triangle")
            return
        }

        captureService.frontWindow(processID: application.processIdentifier) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let window):
                let appName = window.owningApplication?.applicationName ?? "应用"
                self.recordingService.startRecording(
                    window: window,
                    applicationName: appName
                ) { result in
                    switch result {
                    case .success:
                        self.beginRecordingUI()
                    case .failure(let error):
                        self.toastController.show(
                            message: error.localizedDescription,
                            symbolName: "exclamationmark.triangle"
                        )
                    }
                }
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle"
                )
            }
        }
    }

    private func beginRecordingUI() {
        isRecording = true
        recordMenuItem?.title = "停止录制"
        recordMenuItem?.keyEquivalentModifierMask = [.option, .shift]
        recordMenuItem?.keyEquivalent = "r"

        let panel = RecordingControlsPanel()
        panel.show(startTime: Date()) { [weak self] in
            self?.stopRecording()
        }
        recordingControlsPanel = panel
    }

    private func stopRecording() {
        guard isRecording else { return }
        isRecording = false
        recordMenuItem?.title = "录制当前应用窗口"
        recordingControlsPanel?.close()
        recordingControlsPanel = nil

        recordingService.stopRecording { [weak self] result in
            guard let self else { return }
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
        let controller = DesktopPetController()
        controller.onRequestTarget = { [weak self] in self?.previousExternalApplication }
        controller.onCapture = { [weak self] app in self?.captureWindow(of: app) }
        controller.onRecord = { [weak self] app in self?.startRecording(of: app) }
        controller.show()
        controller.updateTarget(previousExternalApplication)
        petController = controller
    }

    private func rememberFrontmostApplication() {
        guard let application = NSWorkspace.shared.frontmostApplication,
              application.processIdentifier != ProcessInfo.processInfo.processIdentifier else {
            return
        }
        lastExternalApplication = application
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
