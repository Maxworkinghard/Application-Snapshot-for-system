import AppKit
import CoreGraphics
import ScreenCaptureKit

enum WindowCaptureError: LocalizedError {
    case noWindow
    case imageUnavailable
    case clipboardWriteFailed

    var errorDescription: String? {
        switch self {
        case .noWindow:
            return "没有找到可截取的应用窗口"
        case .imageUnavailable:
            return "未能生成窗口快照"
        case .clipboardWriteFailed:
            return "未能写入系统剪贴板"
        }
    }
}

struct CapturedWindow {
    let applicationName: String
    let title: String
}

final class WindowCaptureService {
    /// 找到目标 PID 的最前窗口，回调主线程。
    func frontWindow(
        processID: pid_t?,
        completion: @escaping (Result<SCWindow, Error>) -> Void
    ) {
        SCShareableContent.getExcludingDesktopWindows(
            true,
            onScreenWindowsOnly: true
        ) { [weak self] content, error in
            if let error {
                DispatchQueue.main.async { completion(.failure(error)) }
                return
            }

            guard let content,
                  let window = self?.frontmostWindow(in: content, processID: processID) else {
                DispatchQueue.main.async { completion(.failure(WindowCaptureError.noWindow)) }
                return
            }
            DispatchQueue.main.async { completion(.success(window)) }
        }
    }

    func captureFrontWindow(
        processID: pid_t?,
        completion: @escaping (Result<CapturedWindow, Error>) -> Void
    ) {
        frontWindow(processID: processID) { [weak self] result in
            switch result {
            case .success(let window):
                self?.capture(window: window, completion: completion)
            case .failure(let error):
                completion(.failure(error))
            }
        }
    }

    /// 「可截取窗口」判定：在屏、layer 0、尺寸达标，且不属于本应用进程。
    /// 截图链路与应用列表枚举（CapturableApplicationService）共用这一处实现，避免两条链路的过滤条件漂移。
    static func isCapturableWindow(_ window: SCWindow) -> Bool {
        window.isOnScreen
            && window.windowLayer == 0
            && window.frame.width >= 80
            && window.frame.height >= 80
            && window.owningApplication?.processID != ProcessInfo.processInfo.processIdentifier
    }

    private func frontmostWindow(
        in content: SCShareableContent,
        processID: pid_t?
    ) -> SCWindow? {
        let windows = content.windows.filter { window in
            guard Self.isCapturableWindow(window) else {
                return false
            }

            if let processID {
                return window.owningApplication?.processID == processID
            }
            return true
        }

        let orderedIDs = frontToBackWindowIDs()
        return windows.min { first, second in
            let firstIndex = orderedIDs[first.windowID] ?? Int.max
            let secondIndex = orderedIDs[second.windowID] ?? Int.max
            return firstIndex < secondIndex
        }
    }

    private func frontToBackWindowIDs() -> [CGWindowID: Int] {
        guard let rawWindows = CGWindowListCopyWindowInfo(
            [.optionOnScreenOnly, .excludeDesktopElements],
            kCGNullWindowID
        ) as? [[CFString: Any]] else {
            return [:]
        }

        var result: [CGWindowID: Int] = [:]
        for (index, window) in rawWindows.enumerated() {
            guard let number = window[kCGWindowNumber] as? NSNumber else { continue }
            result[CGWindowID(number.uint32Value)] = index
        }
        return result
    }

    private func capture(
        window: SCWindow,
        completion: @escaping (Result<CapturedWindow, Error>) -> Void
    ) {
        let filter = SCContentFilter(desktopIndependentWindow: window)
        let information = SCShareableContent.info(for: filter)
        let configuration = SCStreamConfiguration()
        configuration.width = Int(window.frame.width * CGFloat(information.pointPixelScale))
        configuration.height = Int(window.frame.height * CGFloat(information.pointPixelScale))
        configuration.captureResolution = .best
        configuration.ignoreShadowsSingleWindow = false
        configuration.showsCursor = false

        SCScreenshotManager.captureImage(
            contentFilter: filter,
            configuration: configuration
        ) { image, error in
            if let error {
                DispatchQueue.main.async { completion(.failure(error)) }
                return
            }

            guard let image else {
                DispatchQueue.main.async {
                    completion(.failure(WindowCaptureError.imageUnavailable))
                }
                return
            }

            DispatchQueue.main.async {
                guard Self.writeToClipboard(image) else {
                    completion(.failure(WindowCaptureError.clipboardWriteFailed))
                    return
                }

                let applicationName = window.owningApplication?.applicationName ?? "应用"
                completion(.success(CapturedWindow(
                    applicationName: applicationName,
                    title: window.title ?? ""
                )))
            }
        }
    }

    private static func writeToClipboard(_ image: CGImage) -> Bool {
        let (pngData, tiffData): (Data?, Data?) = autoreleasepool {
            let bitmap = NSBitmapImageRep(cgImage: image)
            return (bitmap.representation(using: .png, properties: [:]), bitmap.tiffRepresentation)
        }
        guard let pngData else {
            return false
        }

        let item = NSPasteboardItem()
        item.setData(pngData, forType: .png)
        if let tiffData {
            item.setData(tiffData, forType: .tiff)
        }

        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        let success = pasteboard.writeObjects([item])
        if success {
            scheduleClipboardAutoClear(changeCount: pasteboard.changeCount)
        }
        return success
    }

    private static var autoClearWorkItem: DispatchWorkItem?
    private static let autoClearInterval: TimeInterval = 60

    private static func scheduleClipboardAutoClear(changeCount: Int) {
        autoClearWorkItem?.cancel()
        let work = DispatchWorkItem {
            let pasteboard = NSPasteboard.general
            // 仅当剪贴板自上次写入后未被其他内容覆盖时清空，避免抹掉用户后续复制的东西
            guard pasteboard.changeCount == changeCount else { return }
            pasteboard.clearContents()
        }
        autoClearWorkItem = work
        DispatchQueue.main.asyncAfter(deadline: .now() + autoClearInterval, execute: work)
    }
}
