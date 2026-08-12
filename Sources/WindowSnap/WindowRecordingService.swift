import AppKit
import AVFoundation
import ScreenCaptureKit

enum WindowRecordingError: LocalizedError {
    case noWindow
    case writerSetupFailed
    case captureStartFailed

    var errorDescription: String? {
        switch self {
        case .noWindow:
            return "没有找到可录制的应用窗口"
        case .writerSetupFailed:
            return "录制写入器初始化失败"
        case .captureStartFailed:
            return "启动屏幕流失败"
        }
    }
}

struct RecordingResult {
    let url: URL
    let applicationName: String
}

/// 录制文件保存目录：内置默认「下载」，可在菜单栏「设置保存目录…」更改。
final class SaveDirectoryStore {
    private let key = "recording.saveDirectory"

    static let defaultDirectory: URL = FileManager.default
        .urls(for: .downloadsDirectory, in: .userDomainMask).first
        ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Downloads")

    /// 用户设置的目录；未设置时返回默认目录。
    var directory: URL {
        guard let path = UserDefaults.standard.string(forKey: key), !path.isEmpty else {
            return Self.defaultDirectory
        }
        return URL(fileURLWithPath: path)
    }

    func save(_ url: URL) {
        UserDefaults.standard.set(url.path, forKey: key)
    }
}

final class WindowRecordingService: NSObject {
    private var stream: SCStream?
    private var assetWriter: AVAssetWriter?
    private var writerInput: AVAssetWriterInput?
    private let outputQueue = DispatchQueue(label: "WindowSnap.recording.output")
    private let sessionQueue = DispatchQueue(label: "WindowSnap.recording.session")
    private var startedAt: Date?
    private var outputURL: URL?
    private var applicationName: String = ""
    // 以下两个状态只在 outputQueue 上读写（控制路径通过 outputQueue.sync 访问）
    private var didStartSession = false
    private var isFinishing = false

    var isRecording: Bool { stream != nil }

    func startRecording(
        window: SCWindow,
        applicationName: String,
        completion: @escaping (Result<Void, Error>) -> Void
    ) {
        sessionQueue.async { [weak self] in
            guard let self else { return }

            let filter = SCContentFilter(desktopIndependentWindow: window)
            let info = SCShareableContent.info(for: filter)
            let configuration = SCStreamConfiguration()
            configuration.width = Int(window.frame.width * CGFloat(info.pointPixelScale))
            configuration.height = Int(window.frame.height * CGFloat(info.pointPixelScale))
            configuration.captureResolution = .best
            configuration.ignoreShadowsSingleWindow = false
            configuration.showsCursor = true
            configuration.minimumFrameInterval = CMTime(value: 1, timescale: 30)

            let url = Self.makeOutputURL()
            let writer: AVAssetWriter
            do {
                writer = try AVAssetWriter(outputURL: url, fileType: .mp4)
            } catch {
                DispatchQueue.main.async { completion(.failure(error)) }
                return
            }

            let input = AVAssetWriterInput(
                mediaType: .video,
                outputSettings: [
                    AVVideoCodecKey: AVVideoCodecType.h264,
                    AVVideoWidthKey: configuration.width,
                    AVVideoHeightKey: configuration.height,
                    AVVideoCompressionPropertiesKey: [
                        AVVideoAverageBitRateKey: 6_000_000,
                        AVVideoMaxKeyFrameIntervalKey: 60,
                    ],
                ]
            )
            input.expectsMediaDataInRealTime = true
            writer.add(input)
            guard writer.startWriting() else {
                DispatchQueue.main.async {
                    completion(.failure(writer.error ?? WindowRecordingError.writerSetupFailed))
                }
                return
            }
            // startSession 延迟到第一帧到达时，用真实 PTS，否则视频时间轴错乱

            let stream = SCStream(filter: filter, configuration: configuration, delegate: nil)
            do {
                try stream.addStreamOutput(
                    self,
                    type: .screen,
                    sampleHandlerQueue: self.outputQueue
                )
            } catch {
                DispatchQueue.main.async { completion(.failure(error)) }
                return
            }

            self.assetWriter = writer
            self.writerInput = input
            self.outputURL = url
            self.applicationName = applicationName
            self.startedAt = Date()
            self.outputQueue.sync {
                self.didStartSession = false
                self.isFinishing = false
            }

            stream.startCapture { [weak self] error in
                guard let self else { return }
                if let error {
                    self.cleanupAfterFailure()
                    DispatchQueue.main.async { completion(.failure(error)) }
                } else {
                    self.stream = stream
                    DispatchQueue.main.async { completion(.success(())) }
                }
            }
        }
    }

    func stopRecording(completion: @escaping (Result<RecordingResult, Error>) -> Void) {
        sessionQueue.async { [weak self] in
            guard let self, let stream = self.stream else {
                DispatchQueue.main.async {
                    completion(.failure(WindowRecordingError.captureStartFailed))
                }
                return
            }

            // 先在 outputQueue 上封口：之后到达的帧一律丢弃，避免与 markAsFinished 竞态
            var didStartSession = false
            self.outputQueue.sync {
                self.isFinishing = true
                didStartSession = self.didStartSession
            }
            self.writerInput?.markAsFinished()

            let writer = self.assetWriter
            let url = self.outputURL
            let appName = self.applicationName

            stream.stopCapture { [weak self] _ in
                guard let self else { return }

                guard didStartSession, let writer else {
                    // 一帧都没写进去（例如窗口全程被最小化），别留下坏文件
                    writer?.cancelWriting()
                    if let url {
                        try? FileManager.default.removeItem(at: url)
                    }
                    self.cleanupAfterFailure()
                    DispatchQueue.main.async {
                        completion(.failure(WindowRecordingError.writerSetupFailed))
                    }
                    return
                }

                writer.finishWriting {
                    self.stream = nil
                    self.assetWriter = nil
                    self.writerInput = nil
                    self.outputURL = nil
                    self.startedAt = nil

                    if let url, writer.status == .completed {
                        DispatchQueue.main.async {
                            completion(.success(RecordingResult(url: url, applicationName: appName)))
                        }
                    } else {
                        DispatchQueue.main.async {
                            completion(.failure(writer.error ?? WindowRecordingError.writerSetupFailed))
                        }
                    }
                }
            }
        }
    }

    var elapsed: TimeInterval? {
        guard let startedAt else { return nil }
        return Date().timeIntervalSince(startedAt)
    }

    private func cleanupAfterFailure() {
        // didStartSession / isFinishing 在每次 startRecording 时重置，这里不碰（调用方队列不确定）
        stream = nil
        assetWriter = nil
        writerInput = nil
        outputURL = nil
        startedAt = nil
    }

    private static func makeOutputURL() -> URL {
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd_HH-mm-ss"
        formatter.locale = Locale(identifier: "en_US_POSIX")
        let stamp = formatter.string(from: Date())

        var directory = SaveDirectoryStore().directory
        var isDirectory: ObjCBool = false
        let exists = FileManager.default.fileExists(atPath: directory.path, isDirectory: &isDirectory)
        if !exists || !isDirectory.boolValue {
            // 所选目录被删除/失效时先尝试重建，不行就退回默认目录
            if (try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)) == nil {
                directory = SaveDirectoryStore.defaultDirectory
            }
        }
        return directory.appendingPathComponent("应用快照-\(stamp).mp4")
    }
}

extension WindowRecordingService: SCStreamOutput {
    // 注意方法名必须是 didOutputSampleBuffer:of:，否则协议可选方法不会被回调
    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard !isFinishing else { return }
        guard type == .screen, sampleBuffer.isValid else { return }
        guard let attachments = CMSampleBufferGetSampleAttachmentsArray(sampleBuffer, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let statusRawValue = attachments.first?[.status] as? Int,
              let status = SCFrameStatus(rawValue: statusRawValue),
              status == .complete else {
            return
        }
        guard let assetWriter, let writerInput else { return }

        if !didStartSession {
            assetWriter.startSession(atSourceTime: sampleBuffer.presentationTimeStamp)
            didStartSession = true
        }
        guard writerInput.isReadyForMoreMediaData else { return }
        writerInput.append(sampleBuffer)
    }
}