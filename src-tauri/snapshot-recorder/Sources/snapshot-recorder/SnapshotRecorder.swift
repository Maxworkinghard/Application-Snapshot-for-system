// snapshot-recorder —— macOS ScreenCaptureKit 窗口录制 sidecar
//
// 协议：
//   snapshot-recorder --probe
//   snapshot-recorder --window-id <CGWindowID> --output <file.mp4> [--include-cursor]
//
// 成功收到并写入首帧后 stdout 输出一行 `READY <width> <height>`；stdin 收到 `q` 或
// EOF 后停止并封装 MP4。启动失败时 stdout 输出 `ERROR <message>`，同时以非零状态退出。

import AVFoundation
import AppKit
import CoreGraphics
import CoreMedia
import CoreVideo
import Foundation
import ScreenCaptureKit

struct RecorderError: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

func emit(_ line: String) {
    guard let data = (line + "\n").data(using: .utf8) else { return }
    FileHandle.standardOutput.write(data)
    try? FileHandle.standardOutput.synchronize()
}

func emitError(_ message: String) {
    guard let data = (message + "\n").data(using: .utf8) else { return }
    FileHandle.standardError.write(data)
    try? FileHandle.standardError.synchronize()
}

struct Arguments {
    let windowID: CGWindowID
    let outputURL: URL
    let includeCursor: Bool
    let includeSystemAudio: Bool
    let includeMicrophone: Bool

    static func parse(_ values: [String]) throws -> Arguments {
        var windowID: CGWindowID?
        var outputPath: String?
        var includeCursor = false
        var includeSystemAudio = false
        var includeMicrophone = false
        var index = 1
        while index < values.count {
            switch values[index] {
            case "--window-id":
                index += 1
                guard index < values.count, let raw = UInt32(values[index]) else {
                    throw RecorderError(message: "--window-id 缺少有效的窗口编号")
                }
                windowID = raw
            case "--output":
                index += 1
                guard index < values.count, !values[index].isEmpty else {
                    throw RecorderError(message: "--output 缺少输出路径")
                }
                outputPath = values[index]
            case "--include-cursor":
                includeCursor = true
            case "--system-audio":
                includeSystemAudio = true
            case "--microphone":
                includeMicrophone = true
            default:
                throw RecorderError(message: "未知参数：\(values[index])")
            }
            index += 1
        }

        guard let windowID else {
            throw RecorderError(message: "缺少 --window-id")
        }
        guard let outputPath else {
            throw RecorderError(message: "缺少 --output")
        }
        return Arguments(
            windowID: windowID,
            outputURL: URL(fileURLWithPath: outputPath),
            includeCursor: includeCursor,
            includeSystemAudio: includeSystemAudio,
            includeMicrophone: includeMicrophone
        )
    }
}

final class RecordingCoordinator: NSObject, SCStreamOutput, SCStreamDelegate, @unchecked Sendable {
    private let writer: AVAssetWriter
    private let videoInput: AVAssetWriterInput
    private let systemAudioInput: AVAssetWriterInput?
    private let microphoneInput: AVAssetWriterInput?
    private let lock = NSLock()
    private let readySignal = DispatchSemaphore(value: 0)
    private let stopSignal = DispatchSemaphore(value: 0)
    private var sessionStarted = false
    private var readySignaled = false
    private var stopSignaled = false
    private var failure: String?
    private var sessionStartTime = CMTime.invalid
    private var sessionStartUptimeNanoseconds: UInt64?
    private var lastSampleBuffer: CMSampleBuffer?
    private var lastPresentationTime = CMTime.invalid
    private var lastAudioEndTime = CMTime.invalid

    init(outputURL: URL, width: Int, height: Int, includeSystemAudio: Bool, includeMicrophone: Bool) throws {
        try? FileManager.default.removeItem(at: outputURL)
        writer = try AVAssetWriter(outputURL: outputURL, fileType: .mp4)

        let pixels = max(width * height, 1)
        let bitRate = min(max(pixels * 5, 2_000_000), 24_000_000)
        let settings: [String: Any] = [
            AVVideoCodecKey: AVVideoCodecType.h264,
            AVVideoWidthKey: width,
            AVVideoHeightKey: height,
            AVVideoCompressionPropertiesKey: [
                AVVideoAverageBitRateKey: bitRate,
                AVVideoExpectedSourceFrameRateKey: 30,
                AVVideoMaxKeyFrameIntervalKey: 60,
                AVVideoProfileLevelKey: AVVideoProfileLevelH264HighAutoLevel,
            ],
        ]
        videoInput = AVAssetWriterInput(mediaType: .video, outputSettings: settings)
        videoInput.expectsMediaDataInRealTime = true
        guard writer.canAdd(videoInput) else {
            throw RecorderError(message: "系统编码器拒绝当前窗口的视频参数")
        }
        writer.add(videoInput)

        func makeAudioInput(_ writer: AVAssetWriter) throws -> AVAssetWriterInput {
            let input = AVAssetWriterInput(
                mediaType: .audio,
                outputSettings: [
                    AVFormatIDKey: kAudioFormatMPEG4AAC,
                    AVSampleRateKey: 48_000,
                    AVNumberOfChannelsKey: 2,
                    AVEncoderBitRateKey: 160_000,
                ]
            )
            input.expectsMediaDataInRealTime = true
            guard writer.canAdd(input) else {
                throw RecorderError(message: "系统编码器拒绝当前 AAC 音频参数")
            }
            writer.add(input)
            return input
        }
        systemAudioInput = includeSystemAudio ? try makeAudioInput(writer) : nil
        microphoneInput = includeMicrophone ? try makeAudioInput(writer) : nil
        super.init()
    }

    func stream(
        _ stream: SCStream,
        didOutputSampleBuffer sampleBuffer: CMSampleBuffer,
        of outputType: SCStreamOutputType
    ) {
        guard sampleBuffer.isValid, sampleBuffer.dataReadiness == .ready else {
            return
        }

        lock.lock()
        defer { lock.unlock() }
        guard failure == nil else { return }

        let isMicrophoneOutput: Bool
        if #available(macOS 15.0, *) {
            isMicrophoneOutput = outputType == .microphone
        } else {
            isMicrophoneOutput = false
        }
        if outputType == .audio || isMicrophoneOutput {
            guard sessionStarted else { return }
            let input = outputType == .audio ? systemAudioInput : microphoneInput
            guard let input, input.isReadyForMoreMediaData else { return }
            guard input.append(sampleBuffer) else {
                recordFailureLocked("写入音频失败：\(writer.error?.localizedDescription ?? "未知错误")")
                return
            }
            let presentationTime = sampleBuffer.presentationTimeStamp
            let duration = sampleBuffer.duration
            if presentationTime.isValid, duration.isValid {
                let endTime = CMTimeAdd(presentationTime, duration)
                if !lastAudioEndTime.isValid || CMTimeCompare(endTime, lastAudioEndTime) > 0 {
                    lastAudioEndTime = endTime
                }
            }
            return
        }

        guard outputType == .screen, CMSampleBufferGetImageBuffer(sampleBuffer) != nil else {
            return
        }

        if !sessionStarted {
            guard writer.startWriting() else {
                recordFailureLocked("无法启动 H.264 写入：\(writer.error?.localizedDescription ?? "未知错误")")
                return
            }
            sessionStartTime = sampleBuffer.presentationTimeStamp
            sessionStartUptimeNanoseconds = DispatchTime.now().uptimeNanoseconds
            writer.startSession(atSourceTime: sessionStartTime)
            sessionStarted = true
        }

        guard videoInput.isReadyForMoreMediaData else { return }
        guard videoInput.append(sampleBuffer) else {
            recordFailureLocked("写入视频帧失败：\(writer.error?.localizedDescription ?? "未知错误")")
            return
        }
        lastSampleBuffer = sampleBuffer
        lastPresentationTime = sampleBuffer.presentationTimeStamp
        if !readySignaled {
            readySignaled = true
            readySignal.signal()
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        recordFailure("ScreenCaptureKit 已停止：\(error.localizedDescription)")
    }

    func waitUntilReady(timeout: TimeInterval) -> Bool {
        readySignal.wait(timeout: .now() + timeout) == .success
    }

    func waitUntilStopRequested() {
        stopSignal.wait()
    }

    func requestStop() {
        lock.lock()
        signalStopLocked()
        lock.unlock()
    }

    func cancel() {
        writer.cancelWriting()
    }

    func failureMessage() -> String? {
        lock.lock()
        defer { lock.unlock() }
        return failure
    }

    func finish() throws {
        lock.lock()
        let didStart = sessionStarted
        let existingFailure = failure
        let startedAt = sessionStartTime
        let startedAtUptime = sessionStartUptimeNanoseconds
        let finalSourceBuffer = lastSampleBuffer
        let finalSourceTime = lastPresentationTime
        let audioEndTime = lastAudioEndTime
        lock.unlock()

        if let existingFailure {
            writer.cancelWriting()
            throw RecorderError(message: existingFailure)
        }
        guard didStart else {
            writer.cancelWriting()
            throw RecorderError(message: "没有收到可写入的视频帧")
        }

        // ScreenCaptureKit 在窗口画面静止时会给出 idle，不会持续产生新图像。
        // 如果直接 finishWriting，AVAssetWriter 会以「最后一帧时间」作为文件结束，
        // 用户录了十几秒的静止窗口也可能只得到 1 秒。在真实停止时刻复制一帧，
        // 再显式结束 session，让 MP4 时长与实际录制时长一致。
        let frameDuration = CMTime(value: 1, timescale: 30)
        let elapsedSeconds: Double
        if let startedAtUptime {
            let stoppedAt = DispatchTime.now().uptimeNanoseconds
            elapsedSeconds = Double(stoppedAt - startedAtUptime) / 1_000_000_000
        } else {
            elapsedSeconds = 0
        }
        let wallClockEnd = CMTimeAdd(
            startedAt,
            CMTime(seconds: max(elapsedSeconds, 1.0 / 30.0), preferredTimescale: 600)
        )
        let afterLastFrame = finalSourceTime.isValid
            ? CMTimeAdd(finalSourceTime, frameDuration)
            : wallClockEnd
        let finalPresentationTime = CMTimeCompare(wallClockEnd, afterLastFrame) >= 0
            ? wallClockEnd
            : afterLastFrame
        var sessionEndTime = finalPresentationTime

        if let finalSourceBuffer, videoInput.isReadyForMoreMediaData {
            var timing = CMSampleTimingInfo(
                duration: frameDuration,
                presentationTimeStamp: finalPresentationTime,
                decodeTimeStamp: .invalid
            )
            var finalSampleBuffer: CMSampleBuffer?
            let status = CMSampleBufferCreateCopyWithNewTiming(
                allocator: kCFAllocatorDefault,
                sampleBuffer: finalSourceBuffer,
                sampleTimingEntryCount: 1,
                sampleTimingArray: &timing,
                sampleBufferOut: &finalSampleBuffer
            )
            if status == noErr, let finalSampleBuffer {
                guard videoInput.append(finalSampleBuffer) else {
                    writer.cancelWriting()
                    throw RecorderError(message: "写入录制结束帧失败：\(writer.error?.localizedDescription ?? "未知错误")")
                }
                sessionEndTime = CMTimeAdd(finalPresentationTime, frameDuration)
            }
        }

        if audioEndTime.isValid, CMTimeCompare(audioEndTime, sessionEndTime) > 0 {
            sessionEndTime = audioEndTime
        }

        writer.endSession(atSourceTime: sessionEndTime)
        videoInput.markAsFinished()
        systemAudioInput?.markAsFinished()
        microphoneInput?.markAsFinished()
        let finished = DispatchSemaphore(value: 0)
        writer.finishWriting {
            finished.signal()
        }
        guard finished.wait(timeout: .now() + 15) == .success else {
            writer.cancelWriting()
            throw RecorderError(message: "等待 MP4 封装完成超时")
        }
        guard writer.status == .completed else {
            throw RecorderError(message: "MP4 封装失败：\(writer.error?.localizedDescription ?? "未知错误")")
        }
    }

    private func recordFailure(_ message: String) {
        lock.lock()
        recordFailureLocked(message)
        lock.unlock()
    }

    private func recordFailureLocked(_ message: String) {
        if failure == nil {
            failure = message
        }
        if !readySignaled {
            readySignaled = true
            readySignal.signal()
        }
        signalStopLocked()
    }

    private func signalStopLocked() {
        if !stopSignaled {
            stopSignaled = true
            stopSignal.signal()
        }
    }
}

func evenPixelSize(_ value: CGFloat, scale: CGFloat) -> Int {
    let pixels = max(Int((value * scale).rounded()), 2)
    return pixels.isMultiple(of: 2) ? pixels : pixels + 1
}

func runRecording(arguments: Arguments) async throws {
    guard CGPreflightScreenCaptureAccess() || CGRequestScreenCaptureAccess() else {
        throw RecorderError(
            message: "未获得屏幕录制权限。请在系统设置 → 隐私与安全性 → 屏幕与系统音频录制中允许 snapshot；开发模式可能显示 snapshot-recorder 或当前终端。授权后请重新启动应用。"
        )
    }

    if arguments.includeMicrophone {
        guard #available(macOS 15.0, *) else {
            throw RecorderError(message: "麦克风录制需要 macOS 15 或更高版本")
        }
        let status = AVCaptureDevice.authorizationStatus(for: .audio)
        let authorized: Bool
        if status == .notDetermined {
            authorized = await AVCaptureDevice.requestAccess(for: .audio)
        } else {
            authorized = status == .authorized
        }
        guard authorized else {
            throw RecorderError(message: "未获得麦克风权限。请在系统设置 → 隐私与安全性 → 麦克风中允许应用快照")
        }
    }

    let content: SCShareableContent
    do {
        content = try await SCShareableContent.excludingDesktopWindows(
            false,
            onScreenWindowsOnly: false
        )
    } catch {
        throw RecorderError(message: "读取可录制窗口失败：\(error.localizedDescription)")
    }
    guard let window = content.windows.first(where: { $0.windowID == arguments.windowID }) else {
        throw RecorderError(message: "目标窗口已关闭或系统不再允许录制")
    }
    guard window.isOnScreen,
          window.frame.width >= 80,
          window.frame.height >= 80 else {
        throw RecorderError(message: "目标是菜单栏图标、辅助小窗或已离开屏幕，请重新选择应用内容窗口")
    }

    let filter = SCContentFilter(desktopIndependentWindow: window)
    let scale = max(CGFloat(filter.pointPixelScale), 1)
    let width = evenPixelSize(window.frame.width, scale: scale)
    let height = evenPixelSize(window.frame.height, scale: scale)

    let configuration = SCStreamConfiguration()
    configuration.width = width
    configuration.height = height
    configuration.minimumFrameInterval = CMTime(value: 1, timescale: 30)
    configuration.pixelFormat = kCVPixelFormatType_32BGRA
    configuration.queueDepth = 5
    configuration.showsCursor = arguments.includeCursor
    configuration.scalesToFit = true
    configuration.captureResolution = .best
    configuration.ignoreShadowsSingleWindow = true
    configuration.shouldBeOpaque = true
    configuration.capturesAudio = false

    try FileManager.default.createDirectory(
        at: arguments.outputURL.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )
    let coordinator = try RecordingCoordinator(
        outputURL: arguments.outputURL,
        width: width,
        height: height,
        includeSystemAudio: arguments.includeSystemAudio,
        includeMicrophone: arguments.includeMicrophone
    )
    let stream = SCStream(filter: filter, configuration: configuration, delegate: coordinator)
    let captureQueue = DispatchQueue(label: "com.appsnapshot.snapshot-recorder.frames")
    try stream.addStreamOutput(coordinator, type: .screen, sampleHandlerQueue: captureQueue)

    do {
        try await stream.startCapture()
    } catch {
        throw RecorderError(message: "启动 ScreenCaptureKit 失败：\(error.localizedDescription)")
    }

    guard coordinator.waitUntilReady(timeout: 10) else {
        try? await stream.stopCapture()
        throw RecorderError(message: "启动后 10 秒内没有收到视频帧，请检查屏幕录制权限")
    }
    if let failure = coordinator.failureMessage() {
        try? await stream.stopCapture()
        throw RecorderError(message: failure)
    }

    var audioStream: SCStream?
    if arguments.includeSystemAudio || arguments.includeMicrophone {
        guard let display = content.displays.first else {
            try? await stream.stopCapture()
            coordinator.cancel()
            try? FileManager.default.removeItem(at: arguments.outputURL)
            throw RecorderError(message: "找不到可用于录制音频的显示器")
        }
        let audioFilter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
        let audioConfiguration = SCStreamConfiguration()
        audioConfiguration.width = 2
        audioConfiguration.height = 2
        audioConfiguration.capturesAudio = arguments.includeSystemAudio
        if arguments.includeMicrophone {
            guard #available(macOS 15.0, *) else {
                try? await stream.stopCapture()
                coordinator.cancel()
                try? FileManager.default.removeItem(at: arguments.outputURL)
                throw RecorderError(message: "麦克风录制需要 macOS 15 或更高版本")
            }
            audioConfiguration.captureMicrophone = true
        }
        let captureQueue = DispatchQueue(label: "com.appsnapshot.snapshot-recorder.audio")
        let configuredAudioStream = SCStream(filter: audioFilter, configuration: audioConfiguration, delegate: coordinator)
        do {
            if arguments.includeSystemAudio {
                try configuredAudioStream.addStreamOutput(coordinator, type: .audio, sampleHandlerQueue: captureQueue)
            }
            if arguments.includeMicrophone {
                if #available(macOS 15.0, *) {
                    try configuredAudioStream.addStreamOutput(coordinator, type: .microphone, sampleHandlerQueue: captureQueue)
                }
            }
            try await configuredAudioStream.startCapture()
            audioStream = configuredAudioStream
        } catch {
            try? await stream.stopCapture()
            coordinator.cancel()
            try? FileManager.default.removeItem(at: arguments.outputURL)
            throw RecorderError(message: "启动音频录制失败：\(error.localizedDescription)")
        }
    }

    emit("READY \(width) \(height)")
    DispatchQueue.global(qos: .userInitiated).async {
        while let line = readLine() {
            if line.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() == "q" {
                break
            }
        }
        coordinator.requestStop()
    }

    coordinator.waitUntilStopRequested()
    let streamFailure = coordinator.failureMessage()
    if let audioStream {
        do {
            try await audioStream.stopCapture()
        } catch where streamFailure == nil {
            throw RecorderError(message: "停止音频录制失败：\(error.localizedDescription)")
        } catch {
            // 流已因错误停止时，保留原始采集错误。
        }
    }
    do {
        try await stream.stopCapture()
    } catch where streamFailure == nil {
        throw RecorderError(message: "停止 ScreenCaptureKit 失败：\(error.localizedDescription)")
    } catch {
        // 流已经因错误停止时，保留原始错误。
    }
    try coordinator.finish()
    emit("FINISHED")
}

@main
struct SnapshotRecorder {
    @MainActor
    static func main() async {
        if CommandLine.arguments.dropFirst().first == "--probe" {
            emit("ScreenCaptureKit")
            return
        }

        do {
            // ScreenCaptureKit 最终会进入 WindowServer/CGS。普通命令行进程不会像 .app
            // 那样自动初始化 AppKit；未初始化就枚举窗口会触发 CGS_REQUIRE_INIT。
            let application = NSApplication.shared
            application.setActivationPolicy(.prohibited)
            application.finishLaunching()

            let arguments = try Arguments.parse(CommandLine.arguments)
            try await runRecording(arguments: arguments)
        } catch {
            let message = (error as? LocalizedError)?.errorDescription ?? error.localizedDescription
            emit("ERROR \(message)")
            emitError(message)
            exit(1)
        }
    }
}
