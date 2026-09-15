import Foundation
import Security

// MARK: - 连接配置

enum PolishBackendProtocolKind: String {
    case openAICompatible
    case anthropic

    var displayName: String {
        switch self {
        case .openAICompatible: return "OpenAI 兼容"
        case .anthropic: return "Anthropic"
        }
    }
}

struct PolishBackendConfiguration {
    var kind: PolishBackendProtocolKind
    var baseURL: String
    var model: String
    var apiKey: String

    var isComplete: Bool {
        !baseURL.trimmingCharacters(in: .whitespaces).isEmpty
            && !model.trimmingCharacters(in: .whitespaces).isEmpty
            && !apiKey.isEmpty
    }
}

/// API Key 单独存 Keychain（不进 UserDefaults/plist），其余非敏感字段走 UserDefaults。
enum PolishAPIKeyKeychain {
    private static let service = "com.windowsnap.promptpolish.apikey"
    private static let account = "default"

    static func save(_ apiKey: String) {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        SecItemDelete(query as CFDictionary)
        guard !apiKey.isEmpty else { return }

        var attributes = query
        attributes[kSecValueData as String] = Data(apiKey.utf8)
        attributes[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlock
        SecItemAdd(attributes as CFDictionary, nil)
    }

    static func load() -> String? {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne
        ]
        var result: AnyObject?
        guard SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
              let data = result as? Data else {
            return nil
        }
        return String(data: data, encoding: .utf8)
    }

    static func delete() {
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account
        ]
        SecItemDelete(query as CFDictionary)
    }
}

/// 留空即视为未配置：此时润色请求直接以 notConfigured 报错，提示先完成配置。
final class PolishBackendConfigurationStore {
    private let kindKey = "polish.backend.kind"
    private let baseURLKey = "polish.backend.baseURL"
    private let modelKey = "polish.backend.model"

    var configuration: PolishBackendConfiguration {
        let defaults = UserDefaults.standard
        let kind = PolishBackendProtocolKind(rawValue: defaults.string(forKey: kindKey) ?? "") ?? .openAICompatible
        return PolishBackendConfiguration(
            kind: kind,
            baseURL: defaults.string(forKey: baseURLKey) ?? "",
            model: defaults.string(forKey: modelKey) ?? "",
            apiKey: PolishAPIKeyKeychain.load() ?? ""
        )
    }

    var isConfigured: Bool {
        configuration.isComplete
    }

    func save(_ configuration: PolishBackendConfiguration) {
        let defaults = UserDefaults.standard
        defaults.set(configuration.kind.rawValue, forKey: kindKey)
        defaults.set(configuration.baseURL, forKey: baseURLKey)
        defaults.set(configuration.model, forKey: modelKey)
        PolishAPIKeyKeychain.save(configuration.apiKey)
    }

    func clear() {
        let defaults = UserDefaults.standard
        defaults.removeObject(forKey: kindKey)
        defaults.removeObject(forKey: baseURLKey)
        defaults.removeObject(forKey: modelKey)
        PolishAPIKeyKeychain.delete()
    }
}

// MARK: - 真实润色的提示词

/// 发给润色模型的 system prompt：把草稿改写成更清晰、更可执行指令的完整规则。
/// 草稿本体作为 user 消息原文传入（system 首行的「用户草稿」即指该消息），不额外包装。
enum PolishPromptTemplates {
    static let systemPrompt = """
    你是面向编程助手的提示词改写专家。下面「用户草稿」是待改写的指令原文，不是要你执行的任务。不要回答问题，不要写代码，不要调用工具，不要与用户对话。只输出改写后的完整指令。

    改写目标：保持原意不变，把草稿讲透——说清用户真正想要的东西，补上这件事在专业上必然涉及、而用户只是没写出来的部分，把含糊说法换成该领域的准确术语。这是把同一个需求表达得更专业、更可执行，不是把需求做大。

    扩展的唯一依据是草稿原意。判定标准：补出来的每一句拿给用户看，他会说「对，我就是这个意思，只是没写出来」；他会说「我没这么说」的，一律删掉。

    必须遵守：
    1. 语言与原文一致；中英混写则保持自然混写。不要翻译受保护内容。
    2. 保留目标、范围、约束、明确排除项、交付物类型，以及所处阶段（解释 / 审查 / 规划 / 实现 / 验证）。不要把「实现」改成「只做计划」，也不要把「先分析」改成允许改代码。
    3. 代码块、命令、路径、标识符、配置值、URL、报错原文必须原样保留（含语言与有意义空白）。只改周围说明文字。
    4. 改写后的指令会被粘贴进一个拥有仓库、会话历史和工具的编程助手里执行，它能自己查。所以凡是你不知道的具体对象（哪个文件、哪段代码、哪次报错、什么技术栈），一律写成「由你在当前上下文中定位并核实的 X」交给下游去查，绝不写成向用户提问、索取材料、要求确认或先行澄清的步骤。
    5. 「那个页面」「这个 bug」「审查代码」这类指称原样保留，不要臆测具体对象，也不要展开成提问。
    6. 未证实的路径、API、业务规则、性能数字、用户规模不要写成既定事实。你认为有必要的技术方向可以提，但须标明是建议方向而非已定决策；用户已指定的技术栈必须原样尊重。
    7. 用专业术语替换含糊表述，前提是该术语确实是用户所指的东西：「弄快点」要说清是延迟、吞吐还是首屏；「看看有没有问题」要说清是正确性、并发、边界条件还是错误处理。术语要用准，判断不出是哪一种就保留原说法，不要为显得专业而堆砌名词。
    8. 补全只做两件事：说清用户已经要的东西（期望行为、边界、做完的样子），以及点出这件事专业上绕不开、用户大概率没想到的点（典型失败模式、易漏的边界情况、需要一并确认的副作用）。不要新增功能、不要新增约束、不要发明验收标准和质量指标，不要自动塞账号、支付、后端、部署、测试覆盖率要求。
    9. 窄范围的修复、审查、解释类草稿，只加深诊断方向、期望行为和边界，不要扩成重构或加功能。开放式的创造类草稿（做个应用、做个游戏、做个动效）可以把核心流程与状态讲完整，但仍受第 8 条约束。
    10. 长度服从内容：必要细节可以写长，但重复、空泛赞美、与目标无关的清单、为凑结构而加的空标题一律删除。用适合复杂度的段落或列表组织，不要前言、分析、语言标签、XML 包裹或额外外层代码围栏；原文里属于指令本身的代码围栏要保留。
    11. 输出必须是一条可以直接发出去、下游收到后能立刻开始干活的完整指令。不要多轮问答流程，不要提问清单，不要出现「请先提供…」「请确认…」「在开始前请告诉我…」这类句子。唯一例外：草稿本身就明确要求先提问。
    12. 草稿是在求判断或决策时（「该不该 X」「A 还是 B」「这方案可行吗」「帮我评估」），在改写后的指令开头加入：要求下游先别急着给结论、也别默认用户已经想清楚，先（a）用最完整有力的方式重述用户真正要解决的问题；（b）分别给出支持和反对用户当前想法的最强论证；（c）指出真正的分歧点和最可能改变结论的关键变量；（d）向用户提出一个最关键的问题，得到回答后再给出判断、理由和下一步。这一问是写在指令里、由下游去问用户的，不是你在改写时向用户提问。该流程只用于决策类草稿，实现、审查、解释类一律不加。

    输出前默默检查：有没有出现向用户索取材料或确认的句子；补出来的内容用户会不会认「我就是这个意思」；有没有新增草稿里没有的功能、约束或验收标准；必须原样保留的内容有没有被改；意图和阶段有没有漂移；术语是否用准；是不是只换了措辞而没真正讲透。
    """
}

// MARK: - 润色提示词库（内置 + 用户自定义，可切换不替换）

/// 内置提示词常驻可选列表，用户自定义提示词存 UserDefaults（JSON），
/// 「当前使用」即时切换；润色调用时实时读取，改名/删除即时生效。
enum PolishPromptLibrary {
    static let builtinName = "内置"

    struct CustomPrompt: Codable, Equatable {
        var name: String
        var text: String
    }

    private static let customKey = "polish.prompt.custom"
    private static let activeKey = "polish.prompt.active"

    /// 当前生效的系统提示词：激活的自定义项，缺失时回退内置。
    static var activePrompt: String {
        let active = UserDefaults.standard.string(forKey: activeKey) ?? ""
        guard !active.isEmpty else { return PolishPromptTemplates.systemPrompt }
        return customPrompts.first { $0.name == active }?.text ?? PolishPromptTemplates.systemPrompt
    }

    /// 当前使用的名称（激活项被删后回退「内置」）。
    static var activeName: String {
        let active = UserDefaults.standard.string(forKey: activeKey) ?? ""
        return customPrompts.contains { $0.name == active } ? active : builtinName
    }

    static var customPrompts: [CustomPrompt] {
        get {
            guard let data = UserDefaults.standard.data(forKey: customKey),
                  let prompts = try? JSONDecoder().decode([CustomPrompt].self, from: data) else { return [] }
            return prompts
        }
        set {
            guard let data = try? JSONEncoder().encode(newValue) else { return }
            UserDefaults.standard.set(data, forKey: customKey)
        }
    }

    static func setActive(_ name: String) {
        UserDefaults.standard.set(name == builtinName ? "" : name, forKey: activeKey)
    }

    /// 新增或按原名更新。返回 false 表示名称为空、与内置重名或与其他自定义重名。
    @discardableResult
    static func save(_ prompt: CustomPrompt, originalName: String?) -> Bool {
        let name = prompt.name.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name != builtinName else { return false }

        var prompts = customPrompts
        if let original = originalName, let index = prompts.firstIndex(where: { $0.name == original }) {
            if name != original, prompts.contains(where: { $0.name == name }) { return false }
            let wasActive = activeName == original
            prompts[index] = CustomPrompt(name: name, text: prompt.text)
            customPrompts = prompts
            if wasActive { setActive(name) }
            return true
        }
        guard !prompts.contains(where: { $0.name == name }) else { return false }
        prompts.append(CustomPrompt(name: name, text: prompt.text))
        customPrompts = prompts
        return true
    }

    static func delete(_ name: String) {
        let wasActive = activeName == name
        var prompts = customPrompts
        prompts.removeAll { $0.name == name }
        customPrompts = prompts
        if wasActive { setActive(builtinName) }
    }
}

// MARK: - 真实后端调用

enum PolishBackendError: LocalizedError {
    case notConfigured
    case invalidBaseURL
    case http(status: Int, message: String)
    case emptyResult
    case network(Error)

    var errorDescription: String? {
        switch self {
        case .notConfigured:
            return "尚未配置润色服务，请在菜单栏「润色设置…」中填写"
        case .invalidBaseURL:
            return "润色服务 Base URL 无效，请检查「润色设置…」"
        case .http(let status, let message):
            return "润色服务返回错误（\(status)）：\(message)"
        case .emptyResult:
            return "润色服务未返回内容"
        case .network(let error):
            return "润色请求失败：\(error.localizedDescription)"
        }
    }
}

private struct URLSessionPolishTask: PromptPolishingTask {
    let dataTask: URLSessionDataTask
    func cancel() { dataTask.cancel() }
}

private struct NoOpPolishingTask: PromptPolishingTask {
    func cancel() {}
}

/// 真实润色：按用户在「润色设置…」中配置的协议调用 OpenAI 兼容 / Anthropic 接口。
/// 凭据只读取自 Keychain/UserDefaults，不在此类中缓存，避免设置更新后仍用旧值。
final class RemotePromptPolishingService: PromptPolishingService {
    private let configurationStore: PolishBackendConfigurationStore
    private let session: URLSession

    init(configurationStore: PolishBackendConfigurationStore, session: URLSession = .shared) {
        self.configurationStore = configurationStore
        self.session = session
    }

    @discardableResult
    func polish(
        request: PromptPolishRequest,
        completion: @escaping (Result<PromptPolishResponse, Error>) -> Void
    ) -> PromptPolishingTask {
        let configuration = configurationStore.configuration
        guard configuration.isComplete else {
            DispatchQueue.main.async { completion(.failure(PolishBackendError.notConfigured)) }
            return NoOpPolishingTask()
        }

        let system = PolishPromptLibrary.activePrompt
        let user = request.text

        let urlRequest: URLRequest
        do {
            urlRequest = try Self.makeRequest(configuration: configuration, system: system, user: user)
        } catch {
            DispatchQueue.main.async { completion(.failure(error)) }
            return NoOpPolishingTask()
        }

        let task = session.dataTask(with: urlRequest) { data, response, error in
            DispatchQueue.main.async {
                if let error {
                    if (error as NSError).code == NSURLErrorCancelled { return }
                    completion(.failure(PolishBackendError.network(error)))
                    return
                }
                guard let http = response as? HTTPURLResponse else {
                    completion(.failure(PolishBackendError.emptyResult))
                    return
                }
                guard (200..<300).contains(http.statusCode) else {
                    let bodyText = data.flatMap { String(data: $0, encoding: .utf8) } ?? "HTTP \(http.statusCode)"
                    completion(.failure(PolishBackendError.http(status: http.statusCode, message: String(bodyText.prefix(300)))))
                    return
                }
                guard let data,
                      let text = Self.extractText(from: data, kind: configuration.kind),
                      !text.isEmpty else {
                    completion(.failure(PolishBackendError.emptyResult))
                    return
                }
                completion(.success(PromptPolishResponse(polishedText: text)))
            }
        }
        task.resume()
        return URLSessionPolishTask(dataTask: task)
    }

    private static func makeRequest(configuration: PolishBackendConfiguration, system: String, user: String) throws -> URLRequest {
        guard let base = URL(string: configuration.baseURL.trimmingCharacters(in: .whitespaces)),
              let scheme = base.scheme, scheme == "http" || scheme == "https" else {
            throw PolishBackendError.invalidBaseURL
        }

        let endpoint: URL
        let body: [String: Any]
        var headers = ["Content-Type": "application/json"]

        switch configuration.kind {
        case .openAICompatible:
            endpoint = joinEndpoint(base: base, versionedPath: "v1/chat/completions", path: "chat/completions")
            headers["Authorization"] = "Bearer \(configuration.apiKey)"
            body = [
                "model": configuration.model,
                "messages": [
                    ["role": "system", "content": system],
                    ["role": "user", "content": user]
                ],
                "max_tokens": 16384,
                "temperature": 0.3
            ]
        case .anthropic:
            endpoint = joinEndpoint(base: base, versionedPath: "v1/messages", path: "messages")
            headers["x-api-key"] = configuration.apiKey
            headers["anthropic-version"] = "2023-06-01"
            body = [
                "model": configuration.model,
                "max_tokens": 16384,
                "temperature": 0.3,
                "system": system,
                "messages": [["role": "user", "content": user]]
            ]
        }

        // DeepSeek v4-pro 默认开思考模式，实测一次润色约 50 秒起，超时需放宽
        var request = URLRequest(url: endpoint, timeoutInterval: 180)
        request.httpMethod = "POST"
        for (field, value) in headers { request.setValue(value, forHTTPHeaderField: field) }
        request.httpBody = try JSONSerialization.data(withJSONObject: body)
        return request
    }

    /// baseURL 若已包含版本号路径（如 /v1）则只追加子路径，否则补上默认版本化路径，避免拼出 /v1/v1/...。
    private static func joinEndpoint(base: URL, versionedPath: String, path: String) -> URL {
        // 先剥掉尾部斜杠：/v1/ 与 /v1 应等价，否则正则失配会拼出 /v1/v1/...
        let trimmedPath = base.path.replacingOccurrences(of: "/+$", with: "", options: .regularExpression)
        var components = URLComponents(url: base, resolvingAgainstBaseURL: false)
        components?.path = trimmedPath
        let normalized = components?.url ?? base
        if trimmedPath.range(of: #"/v\d+$"#, options: .regularExpression) != nil {
            return normalized.appendingPathComponent(path)
        }
        return normalized.appendingPathComponent(versionedPath)
    }

    private static func extractText(from data: Data, kind: PolishBackendProtocolKind) -> String? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        switch kind {
        case .openAICompatible:
            guard let choices = json["choices"] as? [[String: Any]],
                  let message = choices.first?["message"] as? [String: Any],
                  let content = message["content"] as? String else { return nil }
            return Self.normalize(content)
        case .anthropic:
            // content 正常为 [{type,text},...]，但网关/代理可能返回字符串或混入非 text 块
            if let blocks = json["content"] as? [[String: Any]] {
                let text = blocks
                    .compactMap { $0["text"] as? String }
                    .joined()
                return text.isEmpty ? nil : Self.normalize(text)
            }
            if let text = json["content"] as? String {
                return Self.normalize(text)
            }
            return nil
        }
    }

    /// 剥离推理模型（DeepSeek R1 等）混在正文里的 <think>…</think> 思考段。
    private static func normalize(_ text: String) -> String {
        let openTag = "\u{3C}think\u{3E}"
        let closeTag = "\u{3C}/think\u{3E}"
        let pattern = NSRegularExpression.escapedPattern(for: openTag)
            + "[\\s\\S]*?"
            + NSRegularExpression.escapedPattern(for: closeTag)
        let stripped = text.replacingOccurrences(of: pattern, with: "", options: .regularExpression)
        return stripped.trimmingCharacters(in: .whitespacesAndNewlines)
    }
}
