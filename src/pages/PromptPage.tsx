import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { copyText, polishText, savePromptSettings } from "../lib/backend";
import { readAutoCopy } from "../lib/prefs";
import { usePresence } from "../lib/motion";
import { Choices } from "../components/ui/Choices";
import { errorText, useApp } from "../app/context";

type PolishResult = { text: string; rule: string };

/**
 * 编辑框只放自己写的原文，润色从不改动它；结果单独弹一个窗口给，复制、重新生成都在窗口里。
 * 顶部是润色规则，点哪条就用哪条——和「偏好设置 → 润色规则」里的「用这条」是同一个设置。
 */
export function PromptPage() {
  const { settings, onSaved, notify, pendingDraft, consumeDraft } = useApp();

  const [text, setText] = useState("");
  const [result, setResult] = useState<PolishResult | null>(null);
  const [showing, setShowing] = useState(false);
  const [polishing, setPolishing] = useState(false);
  const [runningRule, setRunningRule] = useState("");
  const [failure, setFailure] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [startedAt, setStartedAt] = useState(0);
  const [, forceTick] = useState(0);
  const dialog = usePresence(showing && result);

  const active = settings.templates.find((item) => item.id === settings.activeTemplateId) ?? settings.templates[0];
  const serviceReady = Boolean(settings.baseUrl && settings.model);
  const canRun = serviceReady && !polishing && text.trim().length > 0;

  // 从时间线的输入框跳过来：带着草稿，直接开跑
  useEffect(() => {
    if (!pendingDraft) return;
    setText(pendingDraft);
    consumeDraft();
    if (serviceReady) void run(pendingDraft);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingDraft]);

  useEffect(() => {
    if (!polishing) return;
    const timer = window.setInterval(() => forceTick((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [polishing]);

  function markCopied() {
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  }

  async function run(source = text) {
    if (!source.trim() || polishing) return;
    // 规则在开跑时定下来：跑的途中再切规则，只影响下一次
    const rule = active;
    setPolishing(true);
    setRunningRule(rule?.name ?? "");
    setFailure(null);
    setStartedAt(Date.now());
    try {
      const polished = await polishText(source, rule?.id ?? settings.activeTemplateId);
      setResult({ text: polished, rule: rule?.name ?? "" });
      setShowing(true);
      if (readAutoCopy()) {
        await copyText(polished).then(markCopied, () => notify("自动复制失败", "error"));
      }
    } catch (error) {
      setFailure(errorText(error));
    } finally {
      setPolishing(false);
    }
  }

  async function copyResult() {
    if (!result) return;
    try {
      await copyText(result.text);
      markCopied();
    } catch {
      notify("复制失败，请手动选中复制", "error");
    }
  }

  async function switchRule(id: string) {
    if (id === active?.id) return;
    try {
      onSaved(
        await savePromptSettings({
          baseUrl: settings.baseUrl,
          model: settings.model,
          apiKey: null,
          activeTemplateId: id,
          templates: settings.templates,
        }),
      );
    } catch (error) {
      notify(errorText(error), "error");
    }
  }

  return (
    <div className="prompt-page">
      <div className="prompt-tabs">
        <span className="prompt-rules">
          <span className="small quiet">规则</span>
          <Choices
            className="choices-wrap"
            ariaLabel="润色规则"
            value={active?.id ?? ""}
            options={settings.templates.map((item) => ({ value: item.id, label: item.name }))}
            onChange={(id) => void switchRule(id)}
          />
        </span>
        <span className="mono small quiet">{text.length} 字</span>
      </div>

      <div className="prompt-field">
        <textarea
          className="input prompt-textarea"
          value={text}
          readOnly={polishing}
          aria-label="要润色的 Prompt"
          onChange={(event) => setText(event.target.value)}
          placeholder="写下要润色的 Prompt"
          spellCheck={false}
        />
        {polishing && (
          <div className="prompt-busy" role="status">
            <span className="busy-line" aria-hidden="true" />
            正在按「{runningRule}」润色 · <span className="mono">{Math.floor((Date.now() - startedAt) / 1000)}</span> 秒
          </div>
        )}
      </div>

      {failure && !dialog.item && (
        <p className="prompt-failure" role="alert">
          <span className="strong">请求失败</span>　{failure}
          <button type="button" className="link" onClick={() => void run()}>重试</button>
        </p>
      )}

      <div className="prompt-foot">
        <span className="prompt-foot-actions">
          {result && !showing && (
            <button type="button" className="btn" onClick={() => setShowing(true)}>查看结果</button>
          )}
          <button type="button" className="btn btn-primary" onClick={() => void run()} disabled={!canRun}>
            {polishing ? "生成中…" : "生成"}
          </button>
        </span>
      </div>

      {dialog.item && (
        <ResultDialog
          result={dialog.item}
          leaving={dialog.leaving}
          busy={polishing}
          canRegenerate={canRun}
          copied={copied}
          failure={failure}
          onCopy={() => void copyResult()}
          onRegenerate={() => void run()}
          onClose={() => setShowing(false)}
        />
      )}
    </div>
  );
}

/** 润色结果。原文留在底下的编辑框里没动，这里只给新的这一份 */
function ResultDialog({
  result,
  leaving,
  busy,
  canRegenerate,
  copied,
  failure,
  onCopy,
  onRegenerate,
  onClose,
}: {
  result: PolishResult;
  leaving: boolean;
  busy: boolean;
  canRegenerate: boolean;
  copied: boolean;
  failure: string | null;
  onCopy: () => void;
  onRegenerate: () => void;
  onClose: () => void;
}) {
  const copyRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    copyRef.current?.focus();
  }, []);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  return createPortal(
    <div className={`dialog-backdrop ${leaving ? "is-leaving" : ""}`} onClick={onClose}>
      <div
        className="dialog dialog-result"
        role="dialog"
        aria-modal="true"
        aria-labelledby="polish-result-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="dialog-result-head">
          <h2 className="dialog-title" id="polish-result-title">润色结果</h2>
          <span className="mono small quiet">
            {result.rule && `按「${result.rule}」 · `}{result.text.length} 字
          </span>
        </div>
        <textarea
          className={`input dialog-result-text ${busy ? "is-stale" : ""}`}
          value={result.text}
          readOnly
          aria-label="润色后的 Prompt"
          spellCheck={false}
        />
        {failure && <p className="signal small" role="alert">请求失败　{failure}</p>}
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>关闭</button>
          <button type="button" className="btn" onClick={onRegenerate} disabled={!canRegenerate}>
            {busy ? "生成中…" : "重新生成"}
          </button>
          <button ref={copyRef} type="button" className="btn btn-primary" onClick={onCopy} disabled={busy}>
            {copied ? "已复制" : "复制"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
