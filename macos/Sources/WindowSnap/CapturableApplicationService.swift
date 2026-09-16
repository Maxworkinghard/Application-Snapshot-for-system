import AppKit
import CoreGraphics
import ScreenCaptureKit

/// 列表展示用的可截取应用。包装 NSRunningApplication，列表存活期间以 processIdentifier 区分同名进程：
/// 点击前会重新确认应用未终止，再沿既有 onCapture 链路按 PID 截取。
struct CapturableApplication {
    let runningApplication: NSRunningApplication

    var displayName: String {
        runningApplication.localizedName ?? "应用"
    }

    var icon: NSImage? {
        runningApplication.icon
    }
}

enum ApplicationEnumerationError: LocalizedError {
    case screenRecordingPermissionDenied
    case enumerationFailed

    var errorDescription: String? {
        switch self {
        case .screenRecordingPermissionDenied:
            return "未获得屏幕录制权限，无法读取应用窗口"
        case .enumerationFailed:
            return "暂时无法读取应用窗口，请稍后重试"
        }
    }
}

/// 枚举「正在运行且确实可被截取」的用户应用：
/// activationPolicy == .regular 滤掉系统/后台进程，再要求该应用在屏上存在满足截图条件的窗口
/// （复用 WindowCaptureService.isCapturableWindow），保证列表里每一项点击即可截取。
/// 需在主线程调用；completion 统一回调主线程。
final class CapturableApplicationService {
    func loadApplications(
        completion: @escaping (Result<[CapturableApplication], Error>) -> Void
    ) {
        let ownProcessIdentifier = ProcessInfo.processInfo.processIdentifier
        let ownBundleIdentifier = Bundle.main.bundleIdentifier
        let candidates = NSWorkspace.shared.runningApplications.filter { application in
            guard application.activationPolicy == .regular,
                  application.processIdentifier != ownProcessIdentifier else {
                return false
            }
            if let ownBundleIdentifier, application.bundleIdentifier == ownBundleIdentifier {
                return false
            }
            return true
        }

        SCShareableContent.getExcludingDesktopWindows(
            true,
            onScreenWindowsOnly: true
        ) { content, error in
            DispatchQueue.main.async {
                if error != nil {
                    if CGPreflightScreenCaptureAccess() {
                        completion(.failure(ApplicationEnumerationError.enumerationFailed))
                    } else {
                        completion(.failure(ApplicationEnumerationError.screenRecordingPermissionDenied))
                    }
                    return
                }

                guard let content else {
                    completion(.failure(ApplicationEnumerationError.enumerationFailed))
                    return
                }

                let capturableProcessIDs = Set(
                    content.windows
                        .filter(WindowCaptureService.isCapturableWindow)
                        .compactMap { $0.owningApplication?.processID }
                )
                let applications = candidates
                    .filter { capturableProcessIDs.contains($0.processIdentifier) }
                    .map(CapturableApplication.init)
                    .sorted {
                        $0.displayName.localizedStandardCompare($1.displayName) == .orderedAscending
                    }
                completion(.success(applications))
            }
        }
    }
}
