import AppKit

/// 应用快照二级页面：可截取应用列表，含 加载中 / 有结果 / 空 / 失败（含屏幕录制权限缺失）四态。
/// 只负责展示与选择，截图经 onSelect 回调交回既有链路按 PID 截取。
final class ApplicationSnapshotViewController: NSViewController, NSTableViewDataSource, NSTableViewDelegate {
    var onSelect: (CapturableApplication) -> Void = { _ in }
    var onBack: () -> Void = {}
    var onOpenScreenRecordingSettings: () -> Void = {}

    private let applicationService: CapturableApplicationService
    private var applications: [CapturableApplication] = []
    /// 递增代数：丢弃过期的异步回调（离开页面后再进入、连续刷新等场景）。
    private var loadGeneration = 0

    private var tableView: NSTableView?
    private var stateContainer: NSView?
    private var stateView: NSView?
    private var stateViewConstraints: [NSLayoutConstraint] = []
    private var listScrollView: NSScrollView!
    private var loadingView: NSView!
    private var emptyView: NSView!
    private var failureView: NSView!
    private var failureMessageLabel: NSTextField!
    private var permissionHintLabel: NSTextField!
    private var permissionButton: NSButton!
    private var retryButton: NSButton!

    private static let columnIdentifier = NSUserInterfaceItemIdentifier("application")
    private static let cellIdentifier = NSUserInterfaceItemIdentifier("applicationCell")

    init(applicationService: CapturableApplicationService) {
        self.applicationService = applicationService
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override func loadView() {
        let backButton = NSButton(title: "返回", target: self, action: #selector(backTapped))
        backButton.bezelStyle = .rounded
        backButton.controlSize = .small
        backButton.translatesAutoresizingMaskIntoConstraints = false

        let titleLabel = NSTextField(labelWithString: "选择要截取的应用")
        titleLabel.font = .systemFont(ofSize: 13, weight: .semibold)
        titleLabel.translatesAutoresizingMaskIntoConstraints = false

        let refreshButton = NSButton(title: "刷新", target: self, action: #selector(reloadTapped))
        refreshButton.bezelStyle = .rounded
        refreshButton.controlSize = .small
        refreshButton.translatesAutoresizingMaskIntoConstraints = false

        let stateContainer = NSView()
        stateContainer.translatesAutoresizingMaskIntoConstraints = false

        let root = NSView()
        root.addSubview(backButton)
        root.addSubview(titleLabel)
        root.addSubview(refreshButton)
        root.addSubview(stateContainer)

        NSLayoutConstraint.activate([
            backButton.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 20),
            backButton.topAnchor.constraint(equalTo: root.topAnchor, constant: 16),
            titleLabel.centerXAnchor.constraint(equalTo: root.centerXAnchor),
            titleLabel.centerYAnchor.constraint(equalTo: backButton.centerYAnchor),
            refreshButton.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -20),
            refreshButton.centerYAnchor.constraint(equalTo: backButton.centerYAnchor),
            stateContainer.topAnchor.constraint(equalTo: backButton.bottomAnchor, constant: 14),
            stateContainer.leadingAnchor.constraint(equalTo: root.leadingAnchor, constant: 20),
            stateContainer.trailingAnchor.constraint(equalTo: root.trailingAnchor, constant: -20),
            stateContainer.bottomAnchor.constraint(equalTo: root.bottomAnchor, constant: -20)
        ])

        self.stateContainer = stateContainer
        listScrollView = makeListScrollView()
        loadingView = makeLoadingView()
        emptyView = makeEmptyView()
        failureView = makeFailureView()
        view = root
    }

    /// 进入页面或点「刷新 / 重新查找 / 重试」时实时查询，不做长期缓存。
    func reload() {
        loadGeneration += 1
        let generation = loadGeneration

        applications = []
        tableView?.reloadData()
        installStateView(loadingView)

        applicationService.loadApplications { [weak self] result in
            guard let self, self.loadGeneration == generation else { return }
            switch result {
            case .success(let applications):
                self.applications = applications
                self.tableView?.reloadData()
                self.installStateView(applications.isEmpty ? self.emptyView : self.listScrollView)
            case .failure(let error):
                self.showFailureState(for: error)
            }
        }
    }

    // MARK: - 状态切换

    private func installStateView(_ newState: NSView) {
        NSLayoutConstraint.deactivate(stateViewConstraints)
        stateViewConstraints = []
        stateView?.removeFromSuperview()

        guard let stateContainer else { return }
        newState.translatesAutoresizingMaskIntoConstraints = false
        stateContainer.addSubview(newState)
        let constraints = [
            newState.topAnchor.constraint(equalTo: stateContainer.topAnchor),
            newState.leadingAnchor.constraint(equalTo: stateContainer.leadingAnchor),
            newState.trailingAnchor.constraint(equalTo: stateContainer.trailingAnchor),
            newState.bottomAnchor.constraint(equalTo: stateContainer.bottomAnchor)
        ]
        NSLayoutConstraint.activate(constraints)
        stateViewConstraints = constraints
        stateView = newState
    }

    private func showFailureState(for error: Error) {
        let permissionDenied: Bool
        if let enumerationError = error as? ApplicationEnumerationError,
           case .screenRecordingPermissionDenied = enumerationError {
            permissionDenied = true
        } else {
            permissionDenied = false
        }

        failureMessageLabel.stringValue = error.localizedDescription
        permissionHintLabel.isHidden = !permissionDenied
        permissionButton.isHidden = !permissionDenied
        retryButton.isHidden = permissionDenied
        installStateView(failureView)
    }

    // MARK: - 子视图构建

    private func makeListScrollView() -> NSScrollView {
        let tableView = NSTableView()
        tableView.addTableColumn(NSTableColumn(identifier: Self.columnIdentifier))
        tableView.headerView = nil
        tableView.rowHeight = 36
        tableView.backgroundColor = .clear
        tableView.target = self
        tableView.action = #selector(rowClicked)
        tableView.dataSource = self
        tableView.delegate = self
        self.tableView = tableView

        let scrollView = NSScrollView()
        scrollView.documentView = tableView
        scrollView.hasVerticalScroller = true
        scrollView.hasHorizontalScroller = false
        scrollView.drawsBackground = false
        return scrollView
    }

    private func makeLoadingView() -> NSView {
        let progressIndicator = NSProgressIndicator()
        progressIndicator.style = .spinning
        progressIndicator.controlSize = .small
        progressIndicator.startAnimation(nil)

        let label = NSTextField(labelWithString: "正在查找可截取的应用…")
        label.font = .systemFont(ofSize: 13)
        label.textColor = .secondaryLabelColor

        return centeredContainer(for: NSStackView(views: [progressIndicator, label]), spacing: 8)
    }

    private func makeEmptyView() -> NSView {
        let label = NSTextField(labelWithString: "当前没有可截取的应用窗口")
        label.font = .systemFont(ofSize: 13, weight: .medium)

        let hint = NSTextField(labelWithString: "打开一个普通应用窗口后，再重新查找")
        hint.font = .systemFont(ofSize: 12)
        hint.textColor = .secondaryLabelColor

        let button = NSButton(title: "重新查找", target: self, action: #selector(reloadTapped))
        button.bezelStyle = .rounded
        button.controlSize = .small

        return centeredContainer(for: NSStackView(views: [label, hint, button]), spacing: 8)
    }

    private func makeFailureView() -> NSView {
        let messageLabel = NSTextField(labelWithString: "")
        messageLabel.font = .systemFont(ofSize: 13, weight: .medium)

        let hintLabel = NSTextField(labelWithString: "请在系统设置中授予屏幕录制权限")
        hintLabel.font = .systemFont(ofSize: 12)
        hintLabel.textColor = .secondaryLabelColor

        let openSettingsButton = NSButton(
            title: "打开屏幕录制设置",
            target: self,
            action: #selector(openScreenRecordingSettingsTapped)
        )
        openSettingsButton.bezelStyle = .rounded
        openSettingsButton.controlSize = .small

        let retry = NSButton(title: "重试", target: self, action: #selector(reloadTapped))
        retry.bezelStyle = .rounded
        retry.controlSize = .small

        failureMessageLabel = messageLabel
        permissionHintLabel = hintLabel
        permissionButton = openSettingsButton
        retryButton = retry
        return centeredContainer(for: NSStackView(views: [messageLabel, hintLabel, openSettingsButton, retry]), spacing: 8)
    }

    private func centeredContainer(for stack: NSStackView, spacing: CGFloat) -> NSView {
        stack.orientation = .vertical
        stack.spacing = spacing
        stack.alignment = .centerX
        stack.translatesAutoresizingMaskIntoConstraints = false

        let container = NSView()
        container.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.centerXAnchor.constraint(equalTo: container.centerXAnchor),
            stack.centerYAnchor.constraint(equalTo: container.centerYAnchor),
            stack.leadingAnchor.constraint(greaterThanOrEqualTo: container.leadingAnchor),
            stack.trailingAnchor.constraint(lessThanOrEqualTo: container.trailingAnchor)
        ])
        return container
    }

    // MARK: - 动作

    @objc private func backTapped() {
        onBack()
    }

    @objc private func reloadTapped() {
        reload()
    }

    @objc private func openScreenRecordingSettingsTapped() {
        onOpenScreenRecordingSettings()
    }

    @objc private func rowClicked() {
        guard let tableView else { return }
        let row = tableView.clickedRow
        guard row >= 0, row < applications.count else { return }
        let application = applications[row]
        guard !application.runningApplication.isTerminated else {
            reload()
            return
        }
        onSelect(application)
    }

    // MARK: - NSTableViewDataSource / Delegate

    func numberOfRows(in tableView: NSTableView) -> Int {
        applications.count
    }

    func tableView(_ tableView: NSTableView, viewFor tableColumn: NSTableColumn?, row: Int) -> NSView? {
        let cell = (tableView.makeView(withIdentifier: Self.cellIdentifier, owner: self) as? NSTableCellView)
            ?? Self.makeApplicationCell()
        let application = applications[row]
        cell.imageView?.image = application.icon
        cell.textField?.stringValue = application.displayName
        return cell
    }

    private static func makeApplicationCell() -> NSTableCellView {
        let cell = NSTableCellView()
        cell.identifier = Self.cellIdentifier

        let imageView = NSImageView()
        imageView.translatesAutoresizingMaskIntoConstraints = false
        imageView.imageScaling = .scaleProportionallyDown

        let textField = NSTextField(labelWithString: "")
        textField.translatesAutoresizingMaskIntoConstraints = false
        textField.font = .systemFont(ofSize: 13)
        textField.lineBreakMode = .byTruncatingMiddle

        cell.addSubview(imageView)
        cell.addSubview(textField)
        cell.imageView = imageView
        cell.textField = textField
        NSLayoutConstraint.activate([
            imageView.leadingAnchor.constraint(equalTo: cell.leadingAnchor, constant: 4),
            imageView.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            imageView.widthAnchor.constraint(equalToConstant: 24),
            imageView.heightAnchor.constraint(equalToConstant: 24),
            textField.leadingAnchor.constraint(equalTo: imageView.trailingAnchor, constant: 10),
            textField.centerYAnchor.constraint(equalTo: cell.centerYAnchor),
            textField.trailingAnchor.constraint(lessThanOrEqualTo: cell.trailingAnchor, constant: -4)
        ])
        return cell
    }
}
