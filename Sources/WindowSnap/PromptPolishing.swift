import Foundation

// MARK: - 类型模型

/// 润色文本类型：带稳定标识与中文显示名的轻量值类型。
/// 有意不用封闭 enum：未来真实后端可返回新的 id + displayName，UI 无需改造即可直接展示。
struct PromptType: Equatable {
    let id: String
    let displayName: String
}

extension PromptType {
    static let programming = PromptType(id: "programming", displayName: "编程类")
    static let creative = PromptType(id: "creative", displayName: "创作类")
    static let analytical = PromptType(id: "analytical", displayName: "分析类")
    static let general = PromptType(id: "general", displayName: "通用类")
}

// MARK: - 服务协议

struct PromptPolishRequest {
    let text: String
    /// 本地识别层给出的非权威提示：Mock 原样采用，真实后端可采用、细化或覆盖。
    let typeHint: PromptType
}

struct PromptPolishResponse {
    let polishedText: String
    /// 实际生效的润色类型；UI 的最终展示以此为准，
    /// 避免「界面显示 A 类型、服务实际按 B 类型润色」的分裂状态。
    let effectiveType: PromptType
}

/// 可取消的润色请求句柄：用户在处理中再次点击时用于中止，避免过期结果回填。
protocol PromptPolishingTask {
    func cancel()
}

/// 润色服务协议。completion 统一在主线程回调；取消后不应再调用 completion。
protocol PromptPolishingService {
    @discardableResult
    func polish(
        request: PromptPolishRequest,
        completion: @escaping (Result<PromptPolishResponse, Error>) -> Void
    ) -> PromptPolishingTask
}

// MARK: - 本地类型识别

/// 提交时的确定性本地启发式识别，顺序：编程 → 创作 → 分析 → 通用
/// （避免一段包含代码的需求被泛化为普通文本）。识别结果仅作为 typeHint 提示，不承诺权威。
final class PromptTypeDetector {
    func detect(from text: String) -> PromptType {
        if Self.looksLikeCode(text) {
            return .programming
        }
        if Self.containsAny(of: Self.creativeKeywords, in: text) {
            return .creative
        }
        if Self.containsAny(of: Self.analyticalKeywords, in: text) {
            return .analytical
        }
        return .general
    }

    private static let creativeKeywords = [
        "写作", "生成", "创作", "文案", "标题", "故事", "诗"
    ]

    private static let analyticalKeywords = [
        "分析", "比较", "总结", "提取", "解释", "评估"
    ]

    private static let codeTokenPattern = try! NSRegularExpression(
        pattern: #"(?m)^\s*(import|func|class|struct|enum|extension|def|package)\b"#
    )

    private static func looksLikeCode(_ text: String) -> Bool {
        if text.contains("```") {
            return true
        }
        return codeTokenPattern.firstMatch(
            in: text,
            range: NSRange(text.startIndex..., in: text)
        ) != nil
    }

    private static func containsAny(of keywords: [String], in text: String) -> Bool {
        keywords.contains { text.contains($0) }
    }
}

// MARK: - Mock 实现（本地离线）

/// 本地 Mock：按识别类型套用固定的 Prompt 骨架并保留用户原文，不访问网络与数据库。
/// 响应刻意延迟 0.6 秒，让「处理中」状态在真实操作里可见。
final class MockPromptPolishingService: PromptPolishingService {
    private static let mockDelay: TimeInterval = 0.6

    private struct WorkItemTask: PromptPolishingTask {
        let workItem: DispatchWorkItem
        func cancel() { workItem.cancel() }
    }

    @discardableResult
    func polish(
        request: PromptPolishRequest,
        completion: @escaping (Result<PromptPolishResponse, Error>) -> Void
    ) -> PromptPolishingTask {
        let workItem = DispatchWorkItem {
            completion(.success(PromptPolishResponse(
                polishedText: Self.skeleton(for: request.typeHint, originalText: request.text),
                effectiveType: request.typeHint
            )))
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + Self.mockDelay, execute: workItem)
        return WorkItemTask(workItem: workItem)
    }

    private static func skeleton(for type: PromptType, originalText: String) -> String {
        switch type {
        case .programming:
            return """
            # 角色
            你是一位严谨的软件工程师。

            # 任务
            按照下方需求完成实现，并给出关键设计说明。

            # 需求原文
            \(originalText)

            # 约束
            - 优先使用与现有代码风格一致的最小实现，避免过度设计
            - 不引入新依赖；对不确定的平台行为显式标注为假设

            # 输出格式
            先给结论或代码，再附不超过 3 条的实现说明。

            # 验证
            给出可直接执行的验证步骤（命令或测试用例）。
            """
        case .creative:
            return """
            # 目标受众
            （补充：这段内容写给谁看）

            # 语气与风格
            自然、克制、具体，避免堆砌形容词。

            # 内容要求
            \(originalText)

            # 结构与长度
            - 按需分段，总量不超过原文长度的 1.2 倍
            - 禁用空洞套话与营销腔
            """
        case .analytical:
            return """
            # 分析目标
            \(originalText)

            # 范围
            仅基于给定信息作答，不引入未经验证的外部事实。

            # 步骤
            1. 拆解问题要点
            2. 逐点给出证据与推理
            3. 汇总结论

            # 结论格式
            结论先行，随后列出依据清单；置信度低的部分显式标注。
            """
        default:
            return """
            # 任务目标
            \(originalText)

            # 背景
            （补充：任务的上下文与限制）

            # 要求
            - 保留原始意图，不擅自扩大或缩小范围
            - 关键决定给出理由

            # 输出格式
            直接输出结果；如有歧义，先列出需要澄清的问题再作答。
            """
        }
    }
}
