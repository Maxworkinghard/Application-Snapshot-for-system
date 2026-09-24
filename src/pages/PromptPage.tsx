import { useEffect, useMemo, useRef, useState } from "react";
import { copyText, polishText, savePromptSettings } from "../lib/backend";
import { Keys } from "../components/ui/Keys";
import { usePresence, useSlidingMark } from "../lib/motion";
import { ModelSettingsDialog } from "../components/ModelSettingsDialog";
import { errorText, useApp } from "../app/context";
import type { PromptTemplate } from "../types";

const sameTemplates = (a: PromptTemplate[], b: PromptTemplate[]) => JSON.stringify(a) === JSON.stringify(b);

/**
 * 草稿与结果共用一个编辑框：润色后就地替换，窄窗口下不用左右分屏对着看。
 * original 留着润色前的原文，lastResult 用来判断框里的内容有没有被手改过。
 */
export function PromptPage() {
  const { settings, onSaved, notify, pendingDraft, consumeDraft } = useApp();
  const [templates, setTemplates] = useState(settings.templates);
  const [activeId, setActiveId] = useState(settings.activeTemplateId);
  const [editingRule, setEditingRule] = useState(false);
  const [savingRules, setSavingRules] = useState(false);
  const [editingModel, setEditingModel] = useState(false);
  const modelDialog = usePresence(editingModel || null);

  const [text, setText] = useState("");
  const [original, setOriginal] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [peeking, setPeeking] = useState(false);
  const [polishing, setPolishing] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [autoCopy, setAutoCopy] = useState(false);
  const [startedAt, setStartedAt] = useState(0);
  const [, forceTick] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const tabsRef = useSlidingMark<HTMLDivElement>(`${original !== null}-${peeking}`);

  useEffect(() => {
    setTemplates(settings.templates);
    setActiveId(settings.activeTemplateId);
  }, [settings.templates, settings.activeTemplateId]);

  const rulesDirty = !sameTemplates(templates, settings.templates);
  const active = templates.find((item) => item.id === activeId) ?? templates[0];
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

  async function persistRules(nextActive = activeId, nextTemplates = templates) {
    setSavingRules(true);
    try {
      onSaved(
        await savePromptSettings({
          baseUrl: settings.baseUrl,
          model: settings.model,
          apiKey: null,
          activeTemplateId: nextActive,
          templates: nextTemplates,
        }),
      );
      return true;
    } catch (error) {
      notify(errorText(error), "error");
      return false;
    } finally {
      setSavingRules(false);
    }
  }

  function chooseRule(id: string) {
    setActiveId(id);
    // 换规则立即记住；规则正文有没保存的改动时一起存
    void persistRules(id, rulesDirty ? templates : settings.templates);
  }

  function addRule() {
    const id = `custom-${Date.now()}`;
    const next = [...templates, { id, name: "新规则", content: "", builtin: false }];
    setTemplates(next);
    setActiveId(id);
    setEditingRule(true);
  }

  function deleteRule() {
    if (!active || active.builtin) return;
    const next = templates.filter((item) => item.id !== active.id);
    const nextActive = next[0]?.id ?? "builtin-default";
    setTemplates(next);
    setActiveId(nextActive);
    void persistRules(nextActive, next).then((ok) => ok && notify(`删掉了规则「${active.name}」`));
  }

  function updateActive(patch: Partial<PromptTemplate>) {
    if (!active) return;
    setTemplates((items) => items.map((item) => (item.id === active.id ? { ...item, ...patch, builtin: false } : item)));
  }

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
      if (autoCopy) await copyText(polished).catch(() => notify("自动复制失败", "error"));
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
      <div className="prompt-bar">
        <label className="rule-picker">
          <span className="quiet small">规则</span>
          <span className="select-wrap">
            <select value={activeId} onChange={(event) => chooseRule(event.target.value)} aria-label="当前规则">
              {templates.map((item) => (
                <option key={item.id} value={item.id}>{item.name}</option>
              ))}
            </select>
            <svg width="9" height="6" viewBox="0 0 9 6" aria-hidden="true"><path d="M.5.5l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.2" /></svg>
          </span>
          {rulesDirty && <span className="signal small">已改动</span>}
        </label>
        <span className="prompt-bar-links small ink-2">
          <button type="button" className={`text-btn ${editingRule ? "is-on" : ""}`} onClick={() => setEditingRule((value) => !value)}>
            {editingRule ? "收起规则" : "改规则"}
          </button>
          <button type="button" className="text-btn" onClick={addRule}>新建</button>
          <button type="button" className="text-btn" onClick={deleteRule} disabled={!active || active.builtin}>删除</button>
        </span>
        <span className="spacer" />
        {rulesDirty ? (
          <span className="prompt-bar-links small">
            <span className="quiet">生成仍按上次保存的版本</span>
            <button type="button" className="text-btn ink-2" onClick={() => setTemplates(settings.templates)}>放弃改动</button>
            <button type="button" className="btn btn-small" onClick={() => void persistRules().then((ok) => ok && notify("规则已保存"))} disabled={savingRules}>
              {savingRules ? "保存中…" : "保存规则"}
            </button>
          </span>
        ) : (
          <>
            {serviceReady ? (
              <button type="button" className="text-btn mono small ink-2" title="修改模型" onClick={() => setEditingModel(true)}>
                {settings.model}
              </button>
            ) : (
              <span className="small prompt-bar-links">
                <span className="signal">未配置模型</span>
                <button type="button" className="link" onClick={() => setEditingModel(true)}>去填写</button>
              </span>
            )}
            <label className="check small ink-2">
              <input type="checkbox" checked={autoCopy} onChange={(event) => setAutoCopy(event.target.checked)} />
              生成后自动复制
            </label>
          </>
        )}
      </div>

      {editingRule && active && (
        <div className="rule-editor">
          <input
            className="input rule-name"
            value={active.name}
            onChange={(event) => updateActive({ name: event.target.value })}
            aria-label="规则名称"
          />
          <textarea
            className="input rule-body"
            value={active.content}
            onChange={(event) => updateActive({ content: event.target.value })}
            aria-label="规则正文"
            spellCheck={false}
          />
        </div>
      )}

      <div className="prompt-tabs">
        {original !== null ? (
          <div role="tablist" className="tabs has-mark" ref={tabsRef}>
            <button type="button" role="tab" aria-selected={!peeking} className={`tab ${!peeking ? "is-on" : ""}`} onClick={() => setPeeking(false)}>结果</button>
            <button type="button" role="tab" aria-selected={peeking} className={`tab ${peeking ? "is-on" : ""}`} onClick={() => setPeeking(true)}>原文</button>
          </div>
        ) : (
          <span className="tab is-on is-static">草稿</span>
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
          placeholder="把想让编程助手做的事写在这里，不用讲究措辞。"
          spellCheck={false}
        />
        {polishing && (
          <div className="prompt-busy" role="status">
            <span className="busy-line" aria-hidden="true" />
            正在按「{active?.name}」润色 · <span className="mono">{Math.floor((Date.now() - startedAt) / 1000)}</span> 秒，最长等 180 秒
          </div>
        )}
      </div>

      {failure && (
        <p className="prompt-failure" role="alert">
          <span className="strong">请求失败</span>　{failure}　草稿没动。
          <button type="button" className="link" onClick={() => void run()}>重试</button>
        </p>
      )}

      <div className="prompt-foot">
        <span className="small quiet prompt-hint">
          {peeking ? (
            "正在看原文，只读；切回「结果」才能改"
          ) : serviceReady ? (
            <>
              <Keys value="CommandOrControl+Enter" />
              {original !== null ? "拿原文重新生成，不会在结果上再润色" : "生成"}
            </>
          ) : (
            "还没填模型接口，生成用不了"
          )}
        </span>
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

      {modelDialog.item && (
        <ModelSettingsDialog
          settings={settings}
          onSaved={onSaved}
          notify={notify}
          onClose={() => setEditingModel(false)}
          leaving={modelDialog.leaving}
        />
      )}
    </div>
  );
}
