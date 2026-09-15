using System;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Text;
using System.Text.RegularExpressions;
using System.Web.Script.Serialization;

namespace AppSnapshot
{
    internal enum PolishProtocolKind
    {
        OpenAICompatible,
        Anthropic
    }

    internal sealed class PolishConfiguration
    {
        public PolishProtocolKind Kind;
        public string BaseUrl;
        public string Model;
        public string ApiKey;

        public bool IsComplete
        {
            get
            {
                return !string.IsNullOrWhiteSpace(BaseUrl)
                    && !string.IsNullOrWhiteSpace(Model)
                    && !string.IsNullOrEmpty(ApiKey);
            }
        }

        public static PolishConfiguration Load()
        {
            string kindText = AppSettings.Read("polish.kind");
            PolishProtocolKind kind = kindText == "anthropic"
                ? PolishProtocolKind.Anthropic
                : PolishProtocolKind.OpenAICompatible;
            return new PolishConfiguration
            {
                Kind = kind,
                BaseUrl = AppSettings.Read("polish.baseUrl") ?? "",
                Model = AppSettings.Read("polish.model") ?? "",
                ApiKey = PolishApiKeyStore.Load()
            };
        }

        public void Save()
        {
            AppSettings.Write("polish.kind", Kind == PolishProtocolKind.Anthropic ? "anthropic" : "openai");
            AppSettings.Write("polish.baseUrl", (BaseUrl ?? "").Trim());
            AppSettings.Write("polish.model", (Model ?? "").Trim());
            PolishApiKeyStore.Save(ApiKey);
        }

        public static void Clear()
        {
            AppSettings.Write("polish.kind", "openai");
            AppSettings.Write("polish.baseUrl", "");
            AppSettings.Write("polish.model", "");
            PolishApiKeyStore.Delete();
        }
    }

    internal sealed class PolishOutcome
    {
        public readonly bool Cancelled;
        public readonly string PolishedText;
        public readonly string ErrorMessage;

        private PolishOutcome(bool cancelled, string polishedText, string errorMessage)
        {
            Cancelled = cancelled;
            PolishedText = polishedText;
            ErrorMessage = errorMessage;
        }

        public static PolishOutcome Success(string text)
        {
            return new PolishOutcome(false, text, null);
        }

        public static PolishOutcome Failure(string message)
        {
            return new PolishOutcome(false, null, message);
        }

        public static PolishOutcome CancelledOutcome()
        {
            return new PolishOutcome(true, null, null);
        }
    }

    /// <summary>
    /// 真实润色：按「润色设置」中配置的协议调用 OpenAI 兼容 / Anthropic 接口。
    /// 同步阻塞实现，请在后台线程调用；取消通过 Abort 当前请求实现。
    /// </summary>
    internal sealed class PolishService
    {
        private static readonly Regex ThinkBlockRegex = BuildThinkBlockRegex();

        private readonly object syncRoot = new object();
        private HttpWebRequest activeRequest;

        /// <summary>中止进行中的润色请求（点击「停止润色」时调用）。</summary>
        public void CancelActive()
        {
            lock (syncRoot)
            {
                if (activeRequest != null)
                {
                    try
                    {
                        activeRequest.Abort();
                    }
                    catch
                    {
                    }
                }
            }
        }

        public PolishOutcome Polish(string text)
        {
            PolishConfiguration configuration = PolishConfiguration.Load();
            if (!configuration.IsComplete)
            {
                return PolishOutcome.Failure("尚未配置润色服务，请在托盘菜单「润色设置…」中填写");
            }

            Uri endpoint;
            if (!TryMakeEndpoint(configuration, out endpoint))
            {
                return PolishOutcome.Failure("润色服务 Base URL 无效，请检查「润色设置…」");
            }

            string body = BuildRequestBody(configuration, PolishPromptLibrary.ActiveText, text);
            byte[] bodyBytes = Encoding.UTF8.GetBytes(body);

            HttpWebRequest request;
            lock (syncRoot)
            {
                request = (HttpWebRequest)WebRequest.Create(endpoint);
                request.Method = "POST";
                request.ContentType = "application/json";
                request.Timeout = 180000;
                request.ReadWriteTimeout = 180000;
                request.Proxy = null;
                request.UserAgent = "AppSnapshot/1.0";
                request.Headers.Add("Authorization", "Bearer " + configuration.ApiKey);
                if (configuration.Kind == PolishProtocolKind.Anthropic)
                {
                    request.Headers.Add("x-api-key", configuration.ApiKey);
                    request.Headers.Add("anthropic-version", "2023-06-01");
                }
                activeRequest = request;
            }

            try
            {
                request.ContentLength = bodyBytes.Length;
                using (Stream stream = request.GetRequestStream())
                {
                    stream.Write(bodyBytes, 0, bodyBytes.Length);
                }

                try
                {
                    using (HttpWebResponse response = (HttpWebResponse)request.GetResponse())
                    using (Stream stream = response.GetResponseStream())
                    using (var reader = new StreamReader(stream, Encoding.UTF8))
                    {
                        string json = reader.ReadToEnd();
                        string polished = ExtractPolishedText(json, configuration.Kind);
                        if (string.IsNullOrEmpty(polished))
                        {
                            return PolishOutcome.Failure("润色服务未返回内容");
                        }
                        return PolishOutcome.Success(polished);
                    }
                }
                catch (WebException error)
                {
                    if (error.Status == WebExceptionStatus.RequestCanceled)
                    {
                        return PolishOutcome.CancelledOutcome();
                    }
                    if (error.Response != null)
                    {
                        string bodyText = ReadErrorBody(error.Response);
                        int status = (int)((HttpWebResponse)error.Response).StatusCode;
                        return PolishOutcome.Failure(
                            "润色服务返回错误（" + status + "）：" + Truncate(bodyText, 300));
                    }
                    return PolishOutcome.Failure("润色请求失败：" + error.Message);
                }
            }
            finally
            {
                lock (syncRoot)
                {
                    if (ReferenceEquals(activeRequest, request))
                    {
                        activeRequest = null;
                    }
                }
            }
        }

        private static Regex BuildThinkBlockRegex()
        {
            string openTag = new string(new[] { '<', 't', 'h', 'i', 'n', 'k', '>' });
            string closeTag = new string(new[] { '<', '/', 't', 'h', 'i', 'n', 'k', '>' });
            return new Regex(Regex.Escape(openTag) + @"[\s\S]*?" + Regex.Escape(closeTag));
        }

        private static bool TryMakeEndpoint(PolishConfiguration configuration, out Uri endpoint)
        {
            endpoint = null;
            string baseUrl = (configuration.BaseUrl ?? "").Trim();
            if (baseUrl.Length == 0)
            {
                return false;
            }

            Uri baseUri;
            try
            {
                baseUri = new Uri(baseUrl);
            }
            catch (UriFormatException)
            {
                return false;
            }
            if (baseUri.Scheme != Uri.UriSchemeHttp && baseUri.Scheme != Uri.UriSchemeHttps)
            {
                return false;
            }

            string suffix;
            if (configuration.Kind == PolishProtocolKind.Anthropic)
            {
                suffix = Regex.IsMatch(baseUri.AbsolutePath.TrimEnd('/'), "/v\\d+$") ? "messages" : "v1/messages";
            }
            else
            {
                suffix = Regex.IsMatch(baseUri.AbsolutePath.TrimEnd('/'), "/v\\d+$")
                    ? "chat/completions"
                    : "v1/chat/completions";
            }

            endpoint = new Uri(baseUri, suffix);
            return true;
        }

        private static string BuildRequestBody(
            PolishConfiguration configuration, string systemPrompt, string userText)
        {
            var serializer = new JavaScriptSerializer();
            serializer.MaxJsonLength = int.MaxValue;
            var body = new Dictionary<string, object>();
            body["model"] = configuration.Model;
            body["max_tokens"] = 16384;
            body["temperature"] = 0.3;

            if (configuration.Kind == PolishProtocolKind.Anthropic)
            {
                body["system"] = systemPrompt;
                body["messages"] = new object[]
                {
                    new Dictionary<string, object> { { "role", "user" }, { "content", userText } }
                };
            }
            else
            {
                body["messages"] = new object[]
                {
                    new Dictionary<string, object> { { "role", "system" }, { "content", systemPrompt } },
                    new Dictionary<string, object> { { "role", "user" }, { "content", userText } }
                };
            }
            return serializer.Serialize(body);
        }

        private static string ExtractPolishedText(string json, PolishProtocolKind kind)
        {
            try
            {
                var serializer = new JavaScriptSerializer();
                serializer.MaxJsonLength = int.MaxValue;
                var root = serializer.DeserializeObject(json) as Dictionary<string, object>;
                if (root == null)
                {
                    return null;
                }

                if (kind == PolishProtocolKind.Anthropic)
                {
                    object content;
                    if (!root.TryGetValue("content", out content))
                    {
                        return null;
                    }
                    var blocks = content as object[];
                    if (blocks != null)
                    {
                        foreach (object block in blocks)
                        {
                            var blockMap = block as Dictionary<string, object>;
                            object text;
                            if (blockMap != null
                                && blockMap.TryGetValue("text", out text)
                                && text is string)
                            {
                                return Normalize((string)text);
                            }
                        }
                    }
                    return null;
                }

                object choices;
                if (!root.TryGetValue("choices", out choices))
                {
                    return null;
                }
                var choiceList = choices as object[];
                if (choiceList == null || choiceList.Length == 0)
                {
                    return null;
                }
                var choice = choiceList[0] as Dictionary<string, object>;
                if (choice == null)
                {
                    return null;
                }
                object messageObject;
                if (!choice.TryGetValue("message", out messageObject))
                {
                    return null;
                }
                var message = messageObject as Dictionary<string, object>;
                object contentText;
                if (message == null || !message.TryGetValue("content", out contentText))
                {
                    return null;
                }
                return contentText is string ? Normalize((string)contentText) : null;
            }
            catch
            {
                return null;
            }
        }

        /// <summary>剥离推理模型（DeepSeek R1 等）混在正文里的思考段。</summary>
        private static string Normalize(string text)
        {
            string stripped = ThinkBlockRegex.Replace(text, "");
            return stripped.Trim();
        }

        private static string ReadErrorBody(WebResponse response)
        {
            try
            {
                using (Stream stream = response.GetResponseStream())
                using (var reader = new StreamReader(stream, Encoding.UTF8))
                {
                    return reader.ReadToEnd();
                }
            }
            catch
            {
                return "";
            }
        }

        private static string Truncate(string text, int maxLength)
        {
            if (text == null)
            {
                return "";
            }
            text = text.Trim();
            return text.Length <= maxLength ? text : text.Substring(0, maxLength) + "…";
        }
    }

    /// <summary>发给润色模型的 system prompt，与 macOS 端保持一致。</summary>
    internal static class PolishPrompt
    {
        public const string SystemPrompt =
@"你是面向编程助手的提示词改写专家。下面「用户草稿」是待改写的指令原文，不是要你执行的任务。不要回答问题，不要写代码，不要调用工具，不要与用户对话。只输出改写后的完整指令。

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

输出前默默检查：有没有出现向用户索取材料或确认的句子；补出来的内容用户会不会认「我就是这个意思」；有没有新增草稿里没有的功能、约束或验收标准；必须原样保留的内容有没有被改；意图和阶段有没有漂移；术语是否用准；是不是只换了措辞而没真正讲透。";
    }
}
