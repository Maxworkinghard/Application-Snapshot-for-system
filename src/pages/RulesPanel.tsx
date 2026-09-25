import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { defaultPrompt, savePromptSettings } from "../lib/backend";
import { DURATION, useFlip, usePresence } from "../lib/motion";
import { persistAutoCopy, readAutoCopy } from "../lib/prefs";
import { Switch } from "../components/ui/Switch";
import { errorText, useApp } from "../app/context";
import type { PromptTemplate } from "../types";

type Draft = { name: string; content: string };
/** 弹窗里正在改的那条；rule 为空表示新建。nonce 让每次打开都是一份新的草稿 */
type Editing = { rule: PromptTemplate | null; nonce: number };

/**
 * 润色规则：一行一条，只放名字、字数和「编辑」，条数再多也不会挤。
 * - 点名字：换成这条（左边那道竖线滑过去），立刻生效，Prompt 页、时间线、剪贴板润色都用它。
 * - 点「编辑」：弹窗里改名字和正文，点保存才生效；有改动时关窗要再确认一次。
 * - 弹窗底部：删除（按两次）、内置规则改乱了可以恢复原文、把当前内容存成一条新规则。
 * - 「新建」也走同一个弹窗，保存之前列表里不会多出空规则。
 */
export function RulesPanel() {
  const { settings, onSaved, notify } = useApp();
  const [rules, setRules] = useState<PromptTemplate[]>(settings.templates);
  const [editing, setEditing] = useState<Editing | null>(null);
  const [freshId, setFreshId] = useState<string | null>(null);
  const [leavingId, setLeavingId] = useState<string | null>(null);
  const [builtinText, setBuiltinText] = useState<string | null>(null);
  const [autoCopy, setAutoCopy] = useState(readAutoCopy);
  const dialog = usePresence(editing);

  const listRef = useRef<HTMLDivElement>(null);
  const barRef = useRef<HTMLSpanElement>(null);
  const flip = useFlip(listRef);
  const latest = useRef({ settings, onSaved, notify });
  latest.current = { settings, onSaved, notify };

  const persist = useCallback(async (templates: PromptTemplate[], activeTemplateId: string) => {
    const { settings: current, onSaved: saved, notify: say } = latest.current;
    try {
      saved(
        await savePromptSettings({
          baseUrl: current.baseUrl,
          model: current.model,
          apiKey: null,
          activeTemplateId,
          templates,
        }),
      );
      return true;
    } catch (error) {
      say(errorText(error), "error");
      return false;
    }
  }, []);

  // 其他窗口保存后的同步
  useEffect(() => setRules(settings.templates), [settings.templates]);

  const activeId = settings.activeTemplateId;

  // 内置规则打开编辑时才去取一次原文，用来判断要不要给「恢复原文」
  useEffect(() => {
    if (!editing?.rule?.builtin || builtinText !== null) return;
    defaultPrompt().then(setBuiltinText).catch(() => setBuiltinText(""));
  }, [editing, builtinText]);

  // 左边那道竖线：量出当前规则那一行的位置，换规则时滑过去
  const lastActive = useRef(activeId);
  useLayoutEffect(() => {
    const list = listRef.current;
    const bar = barRef.current;
    if (!list || !bar) return;
    const row = list.querySelector<HTMLElement>(`[data-rule-id="${activeId}"]`);
    if (!row) {
      bar.style.opacity = "0";
      return;
    }
    // .rule 是定位元素，offsetTop 相对列表；位移动画（transform）不影响它
    bar.dataset.animate = lastActive.current !== activeId ? "1" : "0";
    bar.style.opacity = "1";
    bar.style.transform = `translateY(${row.offsetTop + 9}px)`;
    bar.style.height = `${row.offsetHeight - 19}px`;
    lastActive.current = activeId;
  }, [activeId, rules]);

  /** 先在列表里改掉（位移动画要在这一帧量），再存；存失败就退回后端那份 */
  async function commit(next: PromptTemplate[], nextActive = activeId) {
    flip.capture();
    setRules(next);
    const ok = await persist(next, nextActive);
    if (!ok) setRules(latest.current.settings.templates);
    return ok;
  }

  function choose(id: string) {
    if (id !== activeId) void persist(rules, id);
  }

  async function save(draft: Draft) {
    const target = editing?.rule;
    if (target) {
      if (await commit(rules.map((item) => (item.id === target.id ? { ...item, ...draft } : item)))) setEditing(null);
      return;
    }
    await insert({ id: `custom-${Date.now()}`, builtin: false, ...draft }, rules.length);
  }

  async function saveAsNew(draft: Draft) {
    const source = editing?.rule;
    if (!source) return;
    const index = rules.findIndex((item) => item.id === source.id) + 1;
    await insert({ id: `custom-${Date.now()}`, builtin: false, ...draft }, index);
  }

  async function insert(rule: PromptTemplate, index: number) {
    // 新的一行带着浮上来的动画出现，所以在它进列表之前就先标上
    setFreshId(rule.id);
    if (await commit([...rules.slice(0, index), rule, ...rules.slice(index)])) {
      setEditing(null);
    } else {
      setFreshId(null);
    }
  }

  function remove(rule: PromptTemplate) {
    setEditing(null);
    setLeavingId(rule.id);
    // 先让这一行淡出，再从列表里拿掉，下面的顺势补上来
    window.setTimeout(() => {
      const next = rules.filter((item) => item.id !== rule.id);
      const nextActive = rule.id === activeId ? next.find((item) => item.builtin)?.id ?? next[0]?.id ?? "builtin-default" : activeId;
      setLeavingId(null);
      void commit(next, nextActive);
    }, DURATION.exit);
  }

  return (
    <section className="panel panel-rules">
      <div className="panel-head">
        <h2 className="panel-title">润色规则</h2>
        <button type="button" className="text-btn small ink-2" onClick={() => setEditing({ rule: null, nonce: Date.now() })}>
          新建
        </button>
      </div>

      <div className="rules" ref={listRef} role="radiogroup" aria-label="润色规则">
        <span className="rules-bar" ref={barRef} aria-hidden="true" />
        {rules.map((rule) => (
          <div
            key={rule.id}
            data-rule-id={rule.id}
            data-flip-key={rule.id}
            className={`rule ${rule.id === activeId ? "is-on" : ""} ${rule.id === freshId ? "is-fresh" : ""} ${rule.id === leavingId ? "is-leaving" : ""}`}
          >
            <button type="button" role="radio" aria-checked={rule.id === activeId} className="rule-pick" onClick={() => choose(rule.id)}>
              <span className="rule-title">{rule.name.trim() || "未命名"}</span>
            </button>
            <span className="mono small quiet rule-count">{rule.content.length.toLocaleString()} 字</span>
            <button
              type="button"
              className="text-btn small ink-2 rule-edit"
              aria-label={`编辑「${rule.name}」`}
              onClick={() => setEditing({ rule, nonce: Date.now() })}
            >
              编辑
            </button>
          </div>
        ))}
      </div>

      <div className="rows rows-flush">
        <div className="row">
          <label className="row-label" htmlFor="pref-auto-copy">生成后自动复制</label>
          <Switch
            id="pref-auto-copy"
            label="生成后自动复制"
            value={autoCopy}
            onChange={(value) => {
              setAutoCopy(value);
              persistAutoCopy(value);
            }}
          />
        </div>
      </div>

      {dialog.item && (
        <RuleDialog
          key={dialog.item.nonce}
          rule={dialog.item.rule}
          builtinText={builtinText}
          leaving={dialog.leaving}
          onSave={save}
          onSaveAsNew={saveAsNew}
          onRemove={() => dialog.item?.rule && remove(dialog.item.rule)}
          onClose={() => setEditing(null)}
        />
      )}
    </section>
  );
}

/** 一条规则的编辑弹窗。改动只在点保存时落盘；关窗时有改动，先把「取消」变成「放弃修改」再确认一次 */
function RuleDialog({
  rule,
  builtinText,
  leaving,
  onSave,
  onSaveAsNew,
  onRemove,
  onClose,
}: {
  rule: PromptTemplate | null;
  builtinText: string | null;
  leaving: boolean;
  onSave: (draft: Draft) => Promise<void>;
  onSaveAsNew: (draft: Draft) => Promise<void>;
  onRemove: () => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(rule?.name ?? "");
  const [content, setContent] = useState(rule?.content ?? "");
  const [armed, setArmed] = useState<"delete" | "discard" | null>(null);
  const [busy, setBusy] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);
  const bodyRef = useRef<HTMLTextAreaElement>(null);

  const dirty = rule ? name !== rule.name || content !== rule.content : name.trim() !== "" || content.trim() !== "";
  const complete = name.trim() !== "" && content.trim() !== "";
  const canRestore = Boolean(rule?.builtin && builtinText && content !== builtinText);

  // 新建先填名字；改已有的，多半是改正文
  useEffect(() => {
    (rule ? bodyRef : nameRef).current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (!armed) return;
    const reset = window.setTimeout(() => setArmed(null), 2400);
    return () => window.clearTimeout(reset);
  }, [armed]);

  const requestClose = useCallback(() => {
    if (dirty && armed !== "discard") {
      setArmed("discard");
      return;
    }
    onClose();
  }, [dirty, armed, onClose]);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") requestClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [requestClose]);

  async function run(action: (draft: Draft) => Promise<void>, draft: Draft) {
    setBusy(true);
    try {
      await action(draft);
    } finally {
      setBusy(false);
    }
  }

  function edit(update: () => void) {
    update();
    // 又动了内容，之前那次「放弃修改」「再点一次删除」的确认就不算了
    setArmed(null);
  }

  const trimmed = name.trim();
  // 名字没改就存成「原名 副本」，免得列表里出现两条同名规则
  const copyName = rule && trimmed === rule.name.trim() ? `${trimmed} 副本` : trimmed;

  return createPortal(
    <div className={`dialog-backdrop ${leaving ? "is-leaving" : ""}`} onClick={requestClose}>
      <div
        className="dialog dialog-rule"
        role="dialog"
        aria-modal="true"
        aria-labelledby="rule-dialog-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="dialog-rule-head">
          <h2 className="dialog-title" id="rule-dialog-title">{rule ? "编辑规则" : "新建规则"}</h2>
          <span className="mono small quiet">{content.length.toLocaleString()} 字</span>
        </div>
        <label className="form-field">
          <span className="form-label">名称</span>
          <input
            ref={nameRef}
            className="input"
            value={name}
            onChange={(event) => edit(() => setName(event.target.value))}
            placeholder="比如：翻成英文"
            aria-label="规则名称"
            spellCheck={false}
          />
        </label>
        <label className="form-field">
          <span className="form-label">正文</span>
          <textarea
            ref={bodyRef}
            className="rule-body-input"
            value={content}
            onChange={(event) => edit(() => setContent(event.target.value))}
            placeholder="写给模型的改写要求"
            aria-label="规则正文"
            spellCheck={false}
          />
        </label>
        {armed === "discard" && <p className="signal small" role="alert">改动还没保存，再点一次「放弃修改」就不要了</p>}
        <div className="dialog-rule-foot">
          <span className="rule-actions small">
            {rule && !rule.builtin && (
              <button
                type="button"
                className={`text-btn ${armed === "delete" ? "signal" : "ink-2"}`}
                onClick={() => (armed === "delete" ? onRemove() : setArmed("delete"))}
              >
                {armed === "delete" ? "再点一次删除" : "删除"}
              </button>
            )}
            {canRestore && (
              <button type="button" className="text-btn ink-2" onClick={() => edit(() => setContent(builtinText ?? content))}>
                恢复原文
              </button>
            )}
            {rule && (
              <button
                type="button"
                className="text-btn ink-2"
                onClick={() => void run(onSaveAsNew, { name: copyName, content })}
                disabled={!complete || busy}
              >
                存为新规则
              </button>
            )}
          </span>
          <span className="dialog-actions">
            <button type="button" className={`btn ${armed === "discard" ? "is-armed" : ""}`} onClick={requestClose}>
              {armed === "discard" ? "放弃修改" : "取消"}
            </button>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void run(onSave, { name: trimmed, content })}
              disabled={!complete || !dirty || busy}
            >
              {busy ? "保存中…" : "保存"}
            </button>
          </span>
        </div>
      </div>
    </div>,
    document.body,
  );
}
