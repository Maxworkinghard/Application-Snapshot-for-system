import Foundation

// MARK: - 服务协议

struct PromptPolishRequest {
    let text: String
}

struct PromptPolishResponse {
    let polishedText: String
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
