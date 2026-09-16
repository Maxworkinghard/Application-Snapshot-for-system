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
    private let desktopModeStore = DesktopModeStore()
    /// 润色调用「润色设置…」中配置的真实模型；未配置完整时直接报错引导配置，不再回退本地模板。
    private let polishConfigurationStore = PolishBackendConfigurationStore()
    private lazy var promptPolishingService: PromptPolishingService = RemotePromptPolishingService(
        configurationStore: polishConfigurationStore
    )
    private var statusItem: NSStatusItem?
    private var captureMenuItem: NSMenuItem?
    private var recordMenuItem: NSMenuItem?
    private var capturePreviousMenuItem: NSMenuItem?
    private var polishMenuItem: NSMenuItem?
    private var hotKey: GlobalHotKey?
    private var recordingHotKey: GlobalHotKey?
    private var previousAppHotKey: GlobalHotKey?
    private var polishHotKey: GlobalHotKey?
    private var shortcutSet = ShortcutSet.default
    private var settingsController: AppSettingsController?
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
        shortcutSet = shortcutStore.configuration
        configureStatusItem()
        configureDesktopPet()
        // 所有成败结果都汇入 Toast，这里一处订阅即可覆盖截图/录制/润色
        toastController.anchorBounds = { [weak self] in self?.petController?.currentBounds }
        toastController.onNotify = { [weak self] kind in
            switch kind {
            case .success:
                self?.petController?.setPose(.jumping)
            case .error:
                self?.petController?.setPose(.failed)
            case .warning:
                self?.petController?.setPose(.waiting)
            }
        }

        if let capture = shortcutSet.capture {
            if !registerCaptureShortcut(capture) {
                shortcutSet.capture = ShortcutConfiguration.default
                shortcutStore.save(shortcutSet)

                if !registerCaptureShortcut(shortcutSet.capture!) {
                    toastController.show(message: "快捷键注册失败，请从菜单栏截取", symbolName: "exclamationmark.triangle", kind: .error)
                } else {
                    toastController.show(message: "原快捷键被占用，已恢复默认快捷键", symbolName: "exclamationmark.triangle", kind: .error)
                }
            }
        }

        if let record = shortcutSet.record, !registerRecordShortcut(record) {
            toastController.show(message: "录制快捷键 \(record.displayString) 被占用", symbolName: "exclamationmark.triangle", kind: .error)
        }
        if let capturePrevious = shortcutSet.capturePrevious, !registerPreviousAppShortcut(capturePrevious) {
            toastController.show(message: "截取上一个应用的快捷键被占用", symbolName: "exclamationmark.triangle", kind: .error)
        }
        if let polish = shortcutSet.polish, !registerPolishShortcut(polish) {
            toastController.show(message: "润色快捷键被占用", symbolName: "exclamationmark.triangle", kind: .error)
        }
        updateMenuShortcuts()
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

    /// 系统截屏同款快门声；文件缺失时回退系统提示音。
    private static let shutterSound = NSSound(
        contentsOfFile: "/System/Library/Components/CoreAudio.component/Contents/SharedSupport/SystemSounds/system/Shutter.aif",
        byReference: true
    )

    private func playShutterSound() {
        if let shutterSound = Self.shutterSound, shutterSound.play() {
            return
        }
        NSSound(named: NSSound.Name("Tink"))?.play()
    }

    /// 截取「上一个前台应用」窗口（悬浮球当前显示图标的目标应用）。
    @objc private func capturePreviousAppWindow() {
        guard let application = previousExternalApplication else {
            toastController.show(message: "还没有上一个应用可截取", symbolName: "exclamationmark.triangle", kind: .error)
            return
        }
        captureWindow(of: application)
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
                self.playShutterSound()
                self.toastController.show(
                    message: "已复制 \(capturedWindow.applicationName) 窗口，60 秒后自动清空",
                    symbolName: "checkmark",
                    kind: .success
                )
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle",
                    kind: .error
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
            toastController.show(message: "没有找到可录制的应用", symbolName: "exclamationmark.triangle", kind: .error)
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
                            symbolName: "exclamationmark.triangle",
                            kind: .error
                        )
                    }
                }
            case .failure(let error):
                self.recordingState = .idle
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle",
                    kind: .error
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
                    symbolName: "checkmark",
                    kind: .success
                )
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle",
                    kind: .error
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
        if let record = shortcutSet.record {
            recordMenuItem?.keyEquivalentModifierMask = record.menuModifierMask
            recordMenuItem?.keyEquivalent = record.menuKeyEquivalent
        } else {
            recordMenuItem?.keyEquivalent = ""
        }
        petController?.updateRecordingState()
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
                self.petController?.setHiddenTemporarily(false)

                guard !self.windowPickerWasCancelled,
                      process.terminationStatus == 0 else {
                    return
                }

                self.playShutterSound()
                self.toastController.show(message: "窗口已复制，可直接 ⌘V", symbolName: "checkmark", kind: .success)
            }
        }

        do {
            // 选窗走系统 screencapture 拍屏幕，桌宠/悬浮球必须先离场
            petController?.setHiddenTemporarily(true)
            try process.run()
        } catch {
            removeWindowSelectionMonitor()
            windowPickerProcess = nil
            petController?.setHiddenTemporarily(false)
            toastController.show(message: error.localizedDescription, symbolName: "exclamationmark.triangle", kind: .error)
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
        toastController.show(message: "录制将保存到 \(display)", symbolName: "checkmark", kind: .success)
    }

    /// 右键悬浮球「设置…」与菜单栏「设置…」的统一入口：快捷键绑定 + 润色服务配置。
    @objc private func openSettings() {
        if settingsController == nil {
            settingsController = AppSettingsController(
                currentSet: shortcutSet,
                polishStore: polishConfigurationStore,
                desktopModeStore: desktopModeStore,
                onSaveShortcuts: { [weak self] set in
                    self?.applyShortcuts(set) ?? false
                },
                onSwitchDesktopMode: { [weak self] toPet in
                    self?.switchDesktopMode(toPet: toPet)
                },
                onSwitchPetSkin: { [weak self] skin in
                    self?.switchPetSkin(skin)
                }
            )
        }
        settingsController?.currentSet = shortcutSet
        settingsController?.show()
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

        let capturePreviousItem = NSMenuItem(
            title: "截取上一个应用窗口",
            action: #selector(capturePreviousAppWindow),
            keyEquivalent: "p"
        )
        capturePreviousItem.keyEquivalentModifierMask = [.option, .shift]
        capturePreviousItem.target = self
        menu.addItem(capturePreviousItem)
        capturePreviousMenuItem = capturePreviousItem

        let polishItem = NSMenuItem(
            title: "润色提示词",
            action: #selector(polishPromptFromClipboard),
            keyEquivalent: "m"
        )
        polishItem.keyEquivalentModifierMask = [.option, .shift]
        polishItem.target = self
        menu.addItem(polishItem)
        polishMenuItem = polishItem

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

        let settingsItem = NSMenuItem(
            title: "设置…",
            action: #selector(openSettings),
            keyEquivalent: ","
        )
        settingsItem.keyEquivalentModifierMask = [.command]
        settingsItem.target = self
        menu.addItem(settingsItem)

        let saveDirectoryItem = NSMenuItem(
            title: "设置保存目录…",
            action: #selector(chooseSaveDirectory),
            keyEquivalent: ""
        )
        saveDirectoryItem.target = self
        menu.addItem(saveDirectoryItem)

        let screenRecordingSettingsItem = NSMenuItem(
            title: "屏幕录制设置…",
            action: #selector(openScreenRecordingSettings),
            keyEquivalent: ""
        )
        screenRecordingSettingsItem.target = self
        menu.addItem(screenRecordingSettingsItem)

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

    private func registerCaptureShortcut(_ configuration: ShortcutConfiguration) -> Bool {
        guard let hotKey = GlobalHotKey(
            configuration: configuration,
            action: { [weak self] in self?.captureFrontWindow() }
        ) else {
            return false
        }
        self.hotKey = hotKey
        return true
    }

    private func registerRecordShortcut(_ configuration: ShortcutConfiguration) -> Bool {
        guard let hotKey = GlobalHotKey(
            configuration: configuration,
            action: { [weak self] in self?.toggleRecording() }
        ) else {
            return false
        }
        recordingHotKey = hotKey
        return true
    }

    private func registerPreviousAppShortcut(_ configuration: ShortcutConfiguration) -> Bool {
        guard let hotKey = GlobalHotKey(
            configuration: configuration,
            action: { [weak self] in self?.capturePreviousAppWindow() }
        ) else {
            return false
        }
        previousAppHotKey = hotKey
        return true
    }

    private func registerPolishShortcut(_ configuration: ShortcutConfiguration) -> Bool {
        guard let hotKey = GlobalHotKey(
            configuration: configuration,
            action: { [weak self] in self?.polishPromptFromClipboard() }
        ) else {
            return false
        }
        polishHotKey = hotKey
        return true
    }

    /// 设置窗口保存时整组应用：有绑定的组合全部注册成功才替换旧热键并落盘；
    /// 任一失败则丢弃本次新建的热键（deinit 自动注销），旧快捷键原样保留。
    /// 清除绑定的项会注销对应热键（置 nil 即释放）。
    private func applyShortcuts(_ set: ShortcutSet) -> Bool {
        guard !set.hasConflict else { return false }
        guard let capture = makeHotKey(set.capture, action: { [weak self] in self?.captureFrontWindow() }),
              let record = makeHotKey(set.record, action: { [weak self] in self?.toggleRecording() }),
              let previous = makeHotKey(set.capturePrevious, action: { [weak self] in self?.capturePreviousAppWindow() }),
              let polish = makeHotKey(set.polish, action: { [weak self] in self?.polishPromptFromClipboard() })
        else {
            return false
        }

        hotKey = capture
        recordingHotKey = record
        previousAppHotKey = previous
        polishHotKey = polish

        shortcutSet = set
        shortcutStore.save(set)
        updateMenuShortcuts()
        return true
    }

    /// 未绑定的组合返回 nil 而不是失败——nil 合法表示「不启用该快捷键」。
    private func makeHotKey(
        _ configuration: ShortcutConfiguration?,
        action: @escaping () -> Void
    ) -> GlobalHotKey? {
        guard let configuration else { return nil }
        return GlobalHotKey(configuration: configuration, action: action)
    }

    private func updateMenuShortcuts() {
        if let capture = shortcutSet.capture {
            captureMenuItem?.keyEquivalent = capture.menuKeyEquivalent
            captureMenuItem?.keyEquivalentModifierMask = capture.menuModifierMask
        } else {
            captureMenuItem?.keyEquivalent = ""
        }

        if let record = shortcutSet.record {
            recordMenuItem?.keyEquivalent = record.menuKeyEquivalent
            recordMenuItem?.keyEquivalentModifierMask = record.menuModifierMask
        } else {
            recordMenuItem?.keyEquivalent = ""
        }

        if let capturePrevious = shortcutSet.capturePrevious {
            capturePreviousMenuItem?.keyEquivalent = capturePrevious.menuKeyEquivalent
            capturePreviousMenuItem?.keyEquivalentModifierMask = capturePrevious.menuModifierMask
        } else {
            capturePreviousMenuItem?.keyEquivalent = ""
        }

        if let polish = shortcutSet.polish {
            polishMenuItem?.keyEquivalent = polish.menuKeyEquivalent
            polishMenuItem?.keyEquivalentModifierMask = polish.menuModifierMask
        } else {
            polishMenuItem?.keyEquivalent = ""
        }
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
        petController?.hide()
        petController = nil
        // 默认悬浮球；仅当设置选了桌宠且本地素材可用时才用桌宠，素材消失会自动退回悬浮球
        let presentation: DesktopPetController.Presentation
        if desktopModeStore.isPetMode, let skin = desktopModeStore.resolveSkin() {
            presentation = .pet(skin: skin)
        } else {
            presentation = .ball
        }

        let controller = DesktopPetController(
            applicationService: capturableApplicationService,
            presentation: presentation
        )
        controller.onCapture = { [weak self] app in self?.captureWindow(of: app) }
        controller.onRecord = { [weak self] app in self?.startRecording(of: app) }
        controller.onStopRecording = { [weak self] in self?.stopRecording() }
        controller.onRecordingState = { [weak self] in self?.recordingState ?? .idle }
        controller.onPolishPrompt = { [weak self] in self?.polishPromptFromClipboard() }
        controller.onPolishBusy = { [weak self] in self?.isPolishBusy() ?? false }
        controller.onOpenScreenRecordingSettings = { [weak self] in self?.openScreenRecordingSettings() }
        controller.onOpenSettings = { [weak self] in self?.openSettings() }
        controller.onQuit = { [weak self] in self?.quit() }
        controller.show()
        controller.setHiddenTemporarily(windowPickerProcess?.isRunning == true)
        controller.updateTarget(previousExternalApplication)
        petController = controller
    }

    /// 切换悬浮球/桌宠形式，设置窗口保存时调用；无素材可切桌宠时拒绝并提示。
    private func switchDesktopMode(toPet: Bool) {
        guard toPet != desktopModeStore.isPetMode || toPet != (petController?.isPetMode ?? false) else { return }
        if toPet, desktopModeStore.resolveSkin() == nil {
            toastController.show(
                message: "未找到桌宠素材(\(PetAssets.root.path)/<形象>/*.gif)，无法启用桌宠",
                symbolName: "exclamationmark.triangle",
                kind: .warning
            )
            return
        }
        desktopModeStore.isPetMode = toPet
        configureDesktopPet()
        toastController.show(
            message: toPet ? "已切换到桌宠模式" : "已切换到悬浮球模式",
            symbolName: "checkmark",
            kind: .success
        )
    }

    /// 切换桌宠形象；桌宠显示中时立即换装（位置沿用 desktoppet.x/y）。
    private func switchPetSkin(_ skin: String) {
        guard skin != desktopModeStore.skin else { return }
        desktopModeStore.skin = skin
        guard desktopModeStore.isPetMode else { return }
        configureDesktopPet()
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
    @objc private func polishPromptFromClipboard() {
        if case .polishing(let task, _) = polishRuntimeState {
            task.cancel()
            polishRuntimeState = .idle
            toastController.show(message: "已停止润色，剪切板未改动", symbolName: "xmark", kind: .warning)
            return
        }

        let pasteboard = NSPasteboard.general
        let text = (pasteboard.string(forType: .string) ?? "")
            .trimmingCharacters(in: .whitespacesAndNewlines)

        guard !text.isEmpty else {
            toastController.show(message: "剪切板没有文字，请先复制 Prompt", symbolName: "exclamationmark.triangle", kind: .error)
            return
        }
        guard text.count <= Self.maxPolishInputLength else {
            toastController.show(message: "剪切板内容过长（上限 \(Self.maxPolishInputLength) 字）", symbolName: "exclamationmark.triangle", kind: .error)
            return
        }

        let baselineChangeCount = pasteboard.changeCount
        guard confirmPolish(previewText: text) else { return }
        guard pasteboard.changeCount == baselineChangeCount else {
            toastController.show(message: "剪切板内容已变化，请重新点击润色", symbolName: "exclamationmark.triangle", kind: .error)
            return
        }

        toastController.show(message: "正在润色…", symbolName: "wand.and.stars", kind: .warning)

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
                    self.toastController.show(message: "剪切板内容已变化，润色结果未写入", symbolName: "exclamationmark.triangle", kind: .error)
                    return
                }
                pasteboard.clearContents()
                pasteboard.setString(response.polishedText, forType: .string)
                self.toastController.show(
                    message: "润色完成，结果已替换剪切板",
                    symbolName: "checkmark",
                    kind: .success
                )
            case .failure(let error):
                self.toastController.show(
                    message: error.localizedDescription,
                    symbolName: "exclamationmark.triangle",
                    kind: .error
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
