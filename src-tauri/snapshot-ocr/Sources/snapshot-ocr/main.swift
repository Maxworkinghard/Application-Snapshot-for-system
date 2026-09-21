// snapshot-ocr —— macOS 文字识别桥接程序
//
// Vision 没有系统自带的命令行入口，Rust 侧（src-tauri/src/ocr.rs）按以下约定调用本程序：
//
//     snapshot-ocr            # stdin 收 PNG，逐行识别结果写 stdout，成功退出码 0
//     snapshot-ocr --probe    # 不读 stdin，探测 Vision 可用性，stdout 输出可读的识别语言
//
// 任一失败路径都把原因写 stderr、退出码非 0，由 Rust 侧的 stderr_or() 取用。

import Foundation
import ImageIO
import Vision

/// 识别用的语言集合：与 tesseract adapter（src-tauri/src/ocr.rs 的 tesseract_languages）
/// 保持一致的默认偏好——中英混排。
let recognitionLanguages = ["zh-Hans", "en-US"]

func fail(_ message: String) -> Never {
    FileHandle.standardError.write((message + "\n").data(using: .utf8)!)
    exit(1)
}

/// 把支持的语言列表压成一行人类可读的描述，风格上比照 Windows 的 DisplayName
/// 和 Linux 的 "chi_sim+eng"。
func describeLanguages(_ supported: [String]) -> String {
    let hasChinese = supported.contains(recognitionLanguages[0])
    let hasEnglish = supported.contains(recognitionLanguages[1])
    switch (hasChinese, hasEnglish) {
    case (true, true):
        return "简体中文+English"
    case (true, false):
        return "简体中文"
    case (false, true):
        return "English"
    case (false, false):
        return supported.first ?? "未知"
    }
}

func makeRequest() -> VNRecognizeTextRequest {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = true
    request.recognitionLanguages = recognitionLanguages
    return request
}

func runProbe() -> Never {
    let request = makeRequest()
    do {
        let supported = try request.supportedRecognitionLanguages()
        if supported.isEmpty {
            fail("Vision 未报告可用的识别语言")
        }
        print(describeLanguages(supported))
        exit(0)
    } catch {
        fail("Vision 引擎不可用：\(error.localizedDescription)")
    }
}

/// 按阅读顺序排序：Vision 的 boundingBox 是归一化坐标、原点在左下角，
/// 先按顶边从高到低（越靠屏幕上方越先），同一行内再按左边从左到右。
func readingOrder(_ lhs: VNRecognizedTextObservation, _ rhs: VNRecognizedTextObservation) -> Bool {
    let lhsTop = lhs.boundingBox.origin.y + lhs.boundingBox.height
    let rhsTop = rhs.boundingBox.origin.y + rhs.boundingBox.height
    if abs(lhsTop - rhsTop) > 0.01 {
        return lhsTop > rhsTop
    }
    return lhs.boundingBox.origin.x < rhs.boundingBox.origin.x
}

func runRecognize() -> Never {
    let inputData = FileHandle.standardInput.readDataToEndOfFile()
    guard !inputData.isEmpty else {
        fail("stdin 没有读到图像数据")
    }
    guard let source = CGImageSourceCreateWithData(inputData as CFData, nil),
          let cgImage = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
        fail("图像解码失败")
    }

    let request = makeRequest()
    let handler = VNImageRequestHandler(cgImage: cgImage, options: [:])
    do {
        try handler.perform([request])
    } catch {
        fail("识别失败：\(error.localizedDescription)")
    }

    let observations = request.results ?? []
    for observation in observations.sorted(by: readingOrder) {
        if let candidate = observation.topCandidates(1).first {
            print(candidate.string)
        }
    }
    exit(0)
}

let arguments = CommandLine.arguments
if arguments.count > 1 && arguments[1] == "--probe" {
    runProbe()
} else {
    runRecognize()
}
