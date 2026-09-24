import { useEffect, useMemo, useRef, useState } from "react";
import { copyText, polishText } from "../lib/backend";
import { readAutoCopy } from "../lib/prefs";
import { errorText, useApp } from "../app/context";

/**
 * 只有一个编辑框：草稿写在这里，润色结果就地替换，窄窗口下不用左右分屏对着看。
 * 用哪条规则、生成后要不要自动复制，都在「偏好设置 → 润色规则」里定，这页不再放。
 * original 留着润色前的原文，lastResult 用来判断框里的内容有没有被手改过。
 */
export function PromptPage() {
  const { settings, notify, pendingDraft, consumeDraft } = useApp();

  const [text, setText] = useState("");
  const [original, setOriginal] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [peeking, setPeeking] = useState(false);
  const [polishing, setPolishing] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [startedAt, setStartedAt] = useState(0);
  const [, forceTick] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const activeId = settings.activeTemplateId;
  const active = settings.templates.find((item) => item.id === activeId) ?? settings.templates[0];
  const serviceReady = Boolean(settings.baseUrl && settings.model);
  const shown = peeking ? original ?? "" : text;
  const canRun = serviceReady && !polishing && !peeking && shown.trim().length > 0;

  // 从时间线的输入框跳过来：带着草稿，直接开跑
  useEffect(() => {
    if (!pendingDraft) return;
    setText(pendingDraft);
    setOriginal(null);
    setLastResult(null);
    consumeDraft();
    if (serviceReady) void run(pendingDraft);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingDraft]);

  useEffect(() => {
    if (!polishing) return;
    const timer = window.setInterval(() => forceTick((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [polishing]);

  async function run(source?: string) {
    // 框里是上一次的结果且没被手改过，就拿原文重跑——否则一次次润色自己，很快就跑偏了
    const input = source ?? (original !== null && text === lastResult ? original : text);
    if (!input.trim() || polishing) return;
    setPolishing(true);
    setFailure(null);
    setPeeking(false);
    setStartedAt(Date.now());
    try {
      const polished = await polishText(input, activeId);
      setOriginal(input);
      setLastResult(polished);
      setText(polished);
      if (readAutoCopy()) await copyText(polished).catch(() => notify("自动复制失败", "error"));
    } catch (error) {
      setFailure(errorText(error));
    } finally {
      setPolishing(false);
    }
  }

  async function copyResult() {
    try {
      await copyText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1600);
    } catch {
      notify("复制失败，请手动选中复制", "error");
    }
  }

  const counts = useMemo(() => {
    if (original === null) return `${shown.length} 字`;
    return `${text.length} 字 · 原文 ${original.length} 字`;
  }, [original, shown.length, text.length]);

  return (
    <div className="prompt-page">
      <div className="prompt-tabs">
        {original !== null ? (
          <div role="tablist" className="prompt-tablist">
            <button type="button" role="tab" aria-selected={!peeking} className={`prompt-tab ${!peeking ? "is-on" : ""}`} onClick={() => setPeeking(false)}>结果</button>
            <button type="button" role="tab" aria-selected={peeking} className={`prompt-tab ${peeking ? "is-on" : ""}`} onClick={() => setPeeking(true)}>原文</button>
          </div>
        ) : (
          <span className="prompt-tab is-on">草稿</span>
        )}
        <span className="mono small quiet">{counts}</span>
      </div>

      <div className="prompt-field">
        <textarea
          ref={textareaRef}
          className="input prompt-textarea"
          value={shown}
          readOnly={peeking || polishing}
          aria-label={peeking ? "润色前原文" : original !== null ? "润色结果" : "草稿"}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
              event.preventDefault();
              void run();
            }
          }}
          placeholder="写下要润色的 Prompt"
          spellCheck={false}
        />
        {polishing && (
          <div className="prompt-busy" role="status">
            <span className="busy-line" aria-hidden="true" />
            正在按「{active?.name}」润色 · <span className="mono">{Math.floor((Date.now() - startedAt) / 1000)}</span> 秒
          </div>
        )}
      </div>

      {failure && (
        <p className="prompt-failure" role="alert">
          <span className="strong">请求失败</span>　{failure}　
          <button type="button" className="link" onClick={() => void run()}>重试</button>
        </p>
      )}

      <div className="prompt-foot">
        <span className="prompt-foot-actions">
          {original !== null && (
            <button type="button" className="btn" onClick={() => void copyResult()} disabled={peeking}>
              {copied ? "已复制" : "复制"}
            </button>
          )}
          <button type="button" className="btn btn-primary" onClick={() => void run()} disabled={!canRun}>
            {polishing ? "生成中…" : original !== null ? "重新生成" : "生成"}
          </button>
        </span>
      </div>
    </div>
  );
}
