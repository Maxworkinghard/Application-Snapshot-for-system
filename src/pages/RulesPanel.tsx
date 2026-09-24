import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { defaultPrompt, savePromptSettings } from "../lib/backend";
import { DURATION, useFlip } from "../lib/motion";
import { persistAutoCopy, readAutoCopy } from "../lib/prefs";
import { Switch } from "../components/ui/Switch";
import { errorText, useApp } from "../app/context";
import type { PromptTemplate } from "../types";

const SAVE_DELAY_MS = 700;
const EDITOR_MAX_HEIGHT = 420;

/**
 * 润色规则：一行一条。
 * - 点名字：换成这条（左边那道竖线滑过去），立刻生效，Prompt 页、时间线、剪贴板润色都用它。
 * - 点右边的折角：这一行就地展开。名字原地变成可改的，下面长出正文；停手一会儿自动存，没有保存按钮。
 * - 展开后底下：用这条、复制一份（在它下面多出一条，直接展开）、删除（按两次）、内置规则改乱了可以恢复原文。
 */
export function RulesPanel() {
  const { settings, onSaved, notify } = useApp();
  const [rules, setRules] = useState<PromptTemplate[]>(settings.templates);
  const [openId, setOpenId] = useState<string | null>(null);
  const [freshId, setFreshId] = useState<string | null>(null);
  const [leavingId, setLeavingId] = useState<string | null>(null);
  const [armedId, setArmedId] = useState<string | null>(null);
  const [savedId, setSavedId] = useState<string | null>(null);
  const [builtinText, setBuiltinText] = useState<string | null>(null);
  const [autoCopy, setAutoCopy] = useState(readAutoCopy);

  const listRef = useRef<HTMLDivElement>(null);
  const focusedFresh = useRef<string | null>(null);
  const barRef = useRef<HTMLSpanElement>(null);
  const flip = useFlip(listRef);

  // 打字时不立刻存：停手 SAVE_DELAY_MS 再存。等待中的那份放 ref，卸载、切换时先落盘
  const pending = useRef<PromptTemplate[] | null>(null);
  const timer = useRef<number | null>(null);
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

  const flush = useCallback(() => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    const templates = pending.current;
    if (!templates) return Promise.resolve(true);
    pending.current = null;
    return persist(templates, latest.current.settings.activeTemplateId);
  }, [persist]);

  useEffect(() => () => void flush(), [flush]);

  // 别处改了规则（输入框里 Tab 不改这里；这里是其他窗口保存后的同步）且自己没有待存的改动时，跟上
  useEffect(() => {
    if (!pending.current) setRules(settings.templates);
  }, [settings.templates]);

  const activeId = settings.activeTemplateId;
  const openRule = rules.find((item) => item.id === openId) ?? null;

  // 内置规则展开时才去取一次原文，用来判断要不要给「恢复原文」
  useEffect(() => {
    if (!openRule?.builtin || builtinText !== null) return;
    defaultPrompt().then(setBuiltinText).catch(() => setBuiltinText(""));
  }, [openRule?.builtin, builtinText]);

  // 左边那道竖线：量出当前规则那一行的位置。换规则时滑过去；展开收起引起的位移直接跟上，不拖泥带水
  const placeBar = useCallback((animate: boolean) => {
    const list = listRef.current;
    const bar = barRef.current;
    if (!list || !bar) return;
    const head = list.querySelector<HTMLElement>(`[data-rule-id="${activeId}"] .rule-head`);
    if (!head) {
      bar.style.opacity = "0";
      return;
    }
    // .rule 是定位元素：head.offsetTop 相对这一行，row.offsetTop 相对列表；位移动画（transform）不影响这两个值
    const row = head.parentElement as HTMLElement;
    bar.dataset.animate = animate ? "1" : "0";
    bar.style.opacity = "1";
    bar.style.transform = `translateY(${row.offsetTop + head.offsetTop + 9}px)`;
    bar.style.height = `${head.offsetHeight - 18}px`;
  }, [activeId]);

  const lastActive = useRef(activeId);
  useLayoutEffect(() => {
    placeBar(lastActive.current !== activeId);
    lastActive.current = activeId;
  }, [activeId, rules, placeBar]);

  useEffect(() => {
    const list = listRef.current;
    if (!list || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => placeBar(false));
    observer.observe(list);
    return () => observer.disconnect();
  }, [placeBar]);

  useEffect(() => {
    if (!armedId) return;
    const reset = window.setTimeout(() => setArmedId(null), 2400);
    return () => window.clearTimeout(reset);
  }, [armedId]);

  useEffect(() => {
    if (!savedId) return;
    const reset = window.setTimeout(() => setSavedId(null), 1400);
    return () => window.clearTimeout(reset);
  }, [savedId]);

  function choose(id: string) {
    if (id === activeId) return;
    const templates = pending.current ?? rules;
    pending.current = null;
    if (timer.current !== null) window.clearTimeout(timer.current);
    void persist(templates, id);
  }

  function toggle(id: string) {
    void flush();
    setArmedId(null);
    setFreshId(null);
    setOpenId((current) => (current === id ? null : id));
  }

  function edit(id: string, patch: Partial<PromptTemplate>) {
    const next = rules.map((item) => (item.id === id ? { ...item, ...patch } : item));
    setRules(next);
    pending.current = next;
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void flush().then((ok) => ok && setSavedId(id));
    }, SAVE_DELAY_MS);
  }

  function insert(rule: PromptTemplate, after?: string) {
    void flush();
    flip.capture();
    const index = after ? rules.findIndex((item) => item.id === after) + 1 : rules.length;
    const next = [...rules.slice(0, index), rule, ...rules.slice(index)];
    setRules(next);
    setFreshId(rule.id);
    setOpenId(rule.id);
    void persist(next, activeId);
  }

  function create() {
    insert({ id: `custom-${Date.now()}`, name: "新规则", content: "", builtin: false });
  }

  function duplicate(rule: PromptTemplate) {
    insert({ id: `custom-${Date.now()}`, name: `${rule.name} 副本`, content: rule.content, builtin: false }, rule.id);
  }

  function remove(rule: PromptTemplate) {
    if (rule.builtin) return;
    if (armedId !== rule.id) {
      setArmedId(rule.id);
      return;
    }
    setArmedId(null);
    setOpenId(null);
    setLeavingId(rule.id);
    // 先让这一行淡出，再从列表里拿掉，下面的顺势补上来
    window.setTimeout(() => {
      flip.capture();
      const base = pending.current ?? rules;
      pending.current = null;
      const next = base.filter((item) => item.id !== rule.id);
      const nextActive = rule.id === activeId ? next.find((item) => item.builtin)?.id ?? next[0]?.id ?? "builtin-default" : activeId;
      setRules(next);
      setLeavingId(null);
      void persist(next, nextActive);
    }, DURATION.exit);
  }

  async function restore(rule: PromptTemplate) {
    const text = builtinText ?? (await defaultPrompt().catch(() => null));
    if (text) edit(rule.id, { content: text });
  }

  return (
    <section className="panel panel-rules">
      <div className="panel-head">
        <h2 className="panel-title">润色规则</h2>
        <button type="button" className="text-btn small ink-2" onClick={create}>新建</button>
      </div>

      <div className="rules" ref={listRef} role="radiogroup" aria-label="润色规则">
        <span className="rules-bar" ref={barRef} aria-hidden="true" />
        {rules.map((rule) => {
          const isActive = rule.id === activeId;
          const isOpen = rule.id === openId;
          const canRestore = rule.builtin && builtinText !== null && builtinText !== "" && rule.content !== builtinText;
          return (
            <div
              key={rule.id}
              data-rule-id={rule.id}
              data-flip-key={rule.id}
              className={`rule ${isActive ? "is-on" : ""} ${isOpen ? "is-open" : ""} ${rule.id === freshId ? "is-fresh" : ""} ${rule.id === leavingId ? "is-leaving" : ""}`}
            >
              <div className="rule-head">
                {isOpen ? (
                  // 展开时名字就地可改：同一个位置、同样的字，不在下面再放一个名字框
                  <input
                    ref={(element) => {
                      if (element && rule.id === freshId && focusedFresh.current !== rule.id) {
                        focusedFresh.current = rule.id;
                        element.focus();
                        element.select();
                      }
                    }}
                    className={`rule-title-input ${isActive ? "is-on" : ""}`}
                    value={rule.name}
                    placeholder="未命名"
                    onChange={(event) => edit(rule.id, { name: event.target.value })}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") {
                        event.preventDefault();
                        listRef.current?.querySelector<HTMLTextAreaElement>(`[data-rule-id="${rule.id}"] .rule-body-input`)?.focus();
                      } else if (event.key === "Escape") {
                        event.preventDefault();
                        toggle(rule.id);
                      }
                    }}
                    aria-label="规则名称"
                    spellCheck={false}
                  />
                ) : (
                  <button type="button" role="radio" aria-checked={isActive} className="rule-pick" onClick={() => choose(rule.id)}>
                    <span className="rule-title">{rule.name.trim() || "未命名"}</span>
                  </button>
                )}
                <span className="mono small quiet rule-count">{rule.content.length.toLocaleString()} 字</span>
                <button
                  type="button"
                  className="rule-toggle"
                  aria-expanded={isOpen}
                  aria-label={`${isOpen ? "收起" : "编辑"}「${rule.name}」`}
                  onClick={() => toggle(rule.id)}
                >
                  <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M3 1.5L6.5 5 3 8.5" fill="none" stroke="currentColor" strokeWidth="1.3" /></svg>
                </button>
              </div>

              <div className="rule-drawer" inert={!isOpen || undefined}>
                <div className="rule-drawer-inner">
                  <RuleEditor
                    rule={rule}
                    open={isOpen}
                    active={isActive}
                    saved={savedId === rule.id}
                    armed={armedId === rule.id}
                    canRestore={canRestore}
                    onChange={(patch) => edit(rule.id, patch)}
                    onClose={() => toggle(rule.id)}
                    onUse={() => choose(rule.id)}
                    onDuplicate={() => duplicate(rule)}
                    onRemove={() => remove(rule)}
                    onRestore={() => void restore(rule)}
                  />
                </div>
              </div>
            </div>
          );
        })}
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
    </section>
  );
}

function RuleEditor({
  rule,
  open,
  active,
  saved,
  armed,
  canRestore,
  onChange,
  onClose,
  onUse,
  onDuplicate,
  onRemove,
  onRestore,
}: {
  rule: PromptTemplate;
  open: boolean;
  active: boolean;
  saved: boolean;
  armed: boolean;
  canRestore: boolean;
  onChange: (patch: Partial<PromptTemplate>) => void;
  onClose: () => void;
  onUse: () => void;
  onDuplicate: () => void;
  onRemove: () => void;
  onRestore: () => void;
}) {
  const bodyRef = useRef<HTMLTextAreaElement>(null);

  // 正文框跟着内容长高，长到一定程度再出滚动条
  useLayoutEffect(() => {
    const body = bodyRef.current;
    if (!body || !open) return;
    body.style.height = "auto";
    body.style.height = `${Math.min(body.scrollHeight + 2, EDITOR_MAX_HEIGHT)}px`;
  }, [rule.content, open]);

  return (
    <div
      className="rule-editor"
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          onClose();
        }
      }}
    >
      <textarea
        ref={bodyRef}
        className="rule-body-input"
        value={rule.content}
        onChange={(event) => onChange({ content: event.target.value })}
        aria-label="规则正文"
        spellCheck={false}
      />
      <div className="rule-foot small">
        <span className={`rule-saved quiet ${saved ? "is-shown" : ""}`} aria-live="polite">{saved ? "已保存" : ""}</span>
        <span className="rule-actions">
          {!active && <button type="button" className="text-btn strong" onClick={onUse}>用这条</button>}
          {canRestore && <button type="button" className="text-btn ink-2" onClick={onRestore}>恢复原文</button>}
          <button type="button" className="text-btn ink-2" onClick={onDuplicate}>复制一份</button>
          {!rule.builtin && (
            <button type="button" className={`text-btn ${armed ? "signal" : "ink-2"}`} onClick={onRemove}>
              {armed ? "再点一次删除" : "删除"}
            </button>
          )}
        </span>
      </div>
    </div>
  );
}
