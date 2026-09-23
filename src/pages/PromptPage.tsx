import { useEffect, useMemo, useState } from "react";
import {
  Check,
  ClipboardCopy,
  Eye,
  EyeOff,
  Plus,
  RefreshCw,
  Save,
  Send,
  SlidersHorizontal,
  Trash2,
} from "lucide-react";
import { polishText, savePromptSettings } from "../lib/backend";
import type { Settings } from "../types";

export function PromptPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
}) {
  const [templates, setTemplates] = useState(settings.templates);
  const [activeId, setActiveId] = useState(settings.activeTemplateId);
  const [saving, setSaving] = useState(false);
  const [showRuleEditor, setShowRuleEditor] = useState(false);

  // 草稿与结果共用一个编辑框：润色后直接就地替换，窄窗口下不用左右分屏对着看。
  // original 留着润色前的原文，lastResult 用来判断框里的内容有没有被手改过。
  const [text, setText] = useState("");
  const [original, setOriginal] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<string | null>(null);
  const [peeking, setPeeking] = useState(false);
  const [polishing, setPolishing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [autoCopy, setAutoCopy] = useState(false);

  useEffect(() => {
    setTemplates(settings.templates);
    setActiveId(settings.activeTemplateId);
  }, [settings]);

  const active = templates.find((item) => item.id === activeId) ?? templates[0];
  const serviceReady = Boolean(settings.baseUrl && settings.model);
  // 看原文时编辑框显示原文且只读，切回来仍是润色结果
  const shown = peeking ? original ?? "" : text;

  function updateActive(content: string) {
    if (!active) return;
    setTemplates((items) =>
      items.map((item) => (item.id === active.id ? { ...item, content, builtin: false } : item)),
    );
  }

  function addTemplate() {
    const id = `custom-${Date.now()}`;
    setTemplates((items) => [...items, { id, name: "新提示词", content: "", builtin: false }]);
    setActiveId(id);
    setShowRuleEditor(true);
  }

  function deleteTemplate() {
    if (!active || active.builtin) return;
    const next = templates.filter((item) => item.id !== active.id);
    setTemplates(next);
    setActiveId(next[0]?.id ?? "builtin-default");
  }

  async function save() {
    setSaving(true);
    try {
      // 连接配置归模型设置页管，这里原样回传
      const next = await savePromptSettings({
        baseUrl: settings.baseUrl,
        model: settings.model,
        apiKey: null,
        activeTemplateId: activeId,
        templates,
      });
      onSaved(next);
      notify("提示词已保存");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  async function runPolish() {
    // 框里是上一次的结果且没被手改过，就拿原文重跑——否则一次次润色自己，
    // 每轮都在上一轮的措辞上再加工，很快就跑偏了。
    const source = original !== null && text === lastResult ? original : text;
    if (!source.trim() || polishing || peeking) return;
    setPolishing(true);
    try {
      // 用的是当前已保存的提示词；改了规则要先保存才会生效
      const polished = await polishText(source);
      setOriginal(source);
      setLastResult(polished);
      setText(polished);
      if (autoCopy) {
        await navigator.clipboard.writeText(polished).catch(() => notify("自动复制失败"));
      }
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setPolishing(false);
    }
  }

  async function copyResult() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      notify("复制失败，请手动选中复制");
    }
  }

  async function pasteDraft() {
    try {
      setText(await navigator.clipboard.readText());
      setOriginal(null);
      setLastResult(null);
      setPeeking(false);
    } catch {
      notify("读取剪贴板失败");
    }
  }

  return (
    <div className="prompt-lab-workspace-container">
      <div className="prompt-top-control-bus">
        <div className="control-bus-left">
          <div className="rule-selector-combo">
            <span className="bus-label">当前规则:</span>
            <select
              className="bus-rule-select"
              value={activeId}
              onChange={(event) => setActiveId(event.target.value)}
              title="切换当前生效的提示词规则"
            >
              {templates.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name} {item.builtin ? "(内置)" : "(自定义)"}
                </option>
              ))}
            </select>
          </div>

          <button
            className={`bus-action-btn ${showRuleEditor ? "is-active" : ""}`}
            onClick={() => setShowRuleEditor((prev) => !prev)}
            title="查看或修改当前规则正文"
          >
            <SlidersHorizontal size={13} />
            <span>{showRuleEditor ? "收起" : "编辑"}</span>
          </button>

          <button className="bus-action-btn secondary" onClick={addTemplate} title="新建自定义规则">
            <Plus size={13} />
            <span>新建</span>
          </button>

          <button
            className="bus-action-btn secondary"
            onClick={deleteTemplate}
            disabled={!active || active.builtin}
            title="删除当前自定义规则"
          >
            <Trash2 size={13} />
            <span>删除</span>
          </button>
        </div>

        <div className="control-bus-right">
          <span className={`bus-model-pill ${serviceReady ? "is-active" : ""}`} title="模型连接状态">
            <span className="model-dot" />
            <span className="model-text">{serviceReady ? `${settings.model}` : "未配置模型"}</span>
          </span>

          <label className="bus-auto-copy-toggle" title="生成完毕后自动复制到剪贴板">
            <input type="checkbox" checked={autoCopy} onChange={(event) => setAutoCopy(event.target.checked)} />
            <span className="toggle-label-text">生成后自动复制</span>
          </label>

          <button className="bus-action-btn" onClick={save} disabled={saving}>
            <Save size={13} />
            <span>{saving ? "保存中…" : "保存规则"}</span>
          </button>
        </div>
      </div>

      {showRuleEditor && active && (
        <div className="prompt-rule-editor-drawer" role="region" aria-label="规则定义编辑面板">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">规则正文</span>
              <span className="pane-char-count tabular-nums">{active.content.length} 字符</span>
            </div>
          </div>
          <input
            className="template-name"
            value={active.name}
            disabled={active.builtin}
            onChange={(event) => setTemplates((items) => items.map((item) => item.id === active.id ? { ...item, name: event.target.value } : item))}
            aria-label="规则名称"
          />
          <textarea
            className="draft-input-textarea"
            value={active.content}
            onChange={(event) => updateActive(event.target.value)}
            aria-label="规则正文"
            spellCheck={false}
          />
        </div>
      )}

      <div className="prompt-single-workbench">
        <div className="workbench-pane draft-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">
                {peeking ? "润色前原文" : original !== null ? "润色结果" : "输入草稿"}
              </span>
              <span className="pane-char-count tabular-nums">{shown.length} 字符</span>
            </div>
            <div className="pane-quick-samples">
              {original !== null && (
                <button
                  className={`paste-clip-btn ${peeking ? "is-active" : ""}`}
                  onClick={() => setPeeking((value) => !value)}
                  aria-pressed={peeking}
                  title={peeking ? "回到润色结果" : "查看润色前的原文"}
                >
                  {peeking ? <EyeOff size={13} /> : <Eye size={13} />}
                  <span>{peeking ? "看结果" : "看原文"}</span>
                </button>
              )}
              <button
                className="paste-clip-btn"
                onClick={() => void pasteDraft()}
                disabled={peeking}
                title="从系统剪贴板填入草稿"
              >
                <ClipboardCopy size={13} />
                <span>粘贴剪贴板</span>
              </button>
            </div>
          </div>

          <div className="pane-textarea-wrap">
            {polishing ? (
              <div className="polish-placeholder">
                <span className="inline-spinner" />正在调用模型，最长等待 180 秒…
              </div>
            ) : (
              <textarea
                className="draft-input-textarea"
                value={shown}
                readOnly={peeking}
                onChange={(event) => setText(event.target.value)}
                onKeyDown={(event) => {
                  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
                    event.preventDefault();
                    void runPolish();
                  }
                }}
                placeholder="把想让编程助手做的事写在这里，不用讲究措辞… (按 Ctrl+Enter 立即生成)"
                spellCheck={false}
              />
            )}
          </div>

          <div className="pane-action-bar">
            <span className="action-kbd-hint">
              {peeking ? (
                "正在看原文，只读；切回结果才能编辑"
              ) : serviceReady ? (
                <>按 <kbd>Ctrl</kbd> + <kbd>Enter</kbd> 触发生成</>
              ) : (
                "请先到「偏好设置」编辑模型"
              )}
            </span>
            <div className="pane-action-buttons">
              {original !== null && (
                <button
                  className={`result-tool-btn ${copied ? "copied" : ""}`}
                  onClick={() => void copyResult()}
                  disabled={peeking}
                  title="复制当前内容"
                >
                  {copied ? <Check size={12} /> : <ClipboardCopy size={12} />}
                  <span>{copied ? "已复制" : "复制"}</span>
                </button>
              )}
              <button
                className="main-action-btn generate"
                onClick={() => void runPolish()}
                disabled={polishing || peeking || !shown.trim() || !serviceReady}
                title={original !== null ? "拿原文按当前规则重跑一遍" : "按当前规则改写草稿"}
              >
                {original !== null ? <RefreshCw size={13} /> : <Send size={13} />}
                <span>
                  {polishing ? "生成中…" : original !== null ? "重新生成" : "生成 (Ctrl+↵)"}
                </span>
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
