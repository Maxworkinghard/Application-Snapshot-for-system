import React, { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { openRecordingsDir, savePreferences, saveShortcuts } from "../lib/backend";
import { middleTruncatePath, normalizeKey, shortcutKeys } from "../lib/format";
import { Keys } from "../components/ui/Keys";
import { Choices } from "../components/ui/Choices";
import { Switch } from "../components/ui/Switch";
import { errorText, useApp } from "../app/context";
import type { Settings, ShortcutAction, ShortcutBinding } from "../types";

export const ACTION_LABELS: Record<ShortcutAction, { name: string }> = {
  snapshot: { name: "窗口快照" },
  fullscreen: { name: "全屏快照" },
  scrolling: { name: "滚动长截图" },
  record: { name: "窗口录制" },
  polish: { name: "润色 Prompt" },
  palette: { name: "呼出输入框" },
};

const MODIFIERS = ["Control", "Shift", "Alt", "Meta"];

function pressedModifiers(event: React.KeyboardEvent): string[] {
  const parts: string[] = [];
  if (event.ctrlKey || event.metaKey) parts.push("CommandOrControl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  return parts;
}

/** 全局快捷键列表 + 保存条。改动先留在本页，按「保存」才交给后端校验、注册、落盘 */
export function ShortcutsPanel() {
  const { settings, onSaved, notify, conflicts, setConflicts, caps } = useApp();
  const [shortcuts, setShortcuts] = useState(settings.shortcuts);
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [held, setHeld] = useState<string[]>([]);
  const [rejected, setRejected] = useState<{ action: ShortcutAction; key: string } | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => setShortcuts(settings.shortcuts), [settings.shortcuts]);

  const changed = useMemo(
    () =>
      shortcuts.filter((item) => {
        const saved = settings.shortcuts.find((binding) => binding.action === item.action);
        return (saved?.accelerator ?? null) !== item.accelerator;
      }).length,
    [shortcuts, settings.shortcuts],
  );

  function stopRecording() {
    setRecording(null);
    setHeld([]);
  }

  function onRowKeyDown(action: ShortcutAction, event: React.KeyboardEvent<HTMLButtonElement>) {
    if (recording !== action) {
      if (event.key === "Enter" || event.key === " ") {
        event.preventDefault();
        setRecording(action);
        setRejected(null);
      }
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      stopRecording();
      return;
    }
    const modifiers = pressedModifiers(event);
    if (MODIFIERS.includes(event.key)) {
      setHeld(modifiers);
      return;
    }
    const key = normalizeKey(event.key);
    if (modifiers.length === 0) {
      // 原先这里是静默 return，用户按了没反应还以为坏了
      setRejected({ action, key });
      return;
    }
    const accelerator = [...modifiers, key].join("+");
    setShortcuts((items) => items.map((item) => (item.action === action ? { ...item, accelerator } : item)));
    setRejected(null);
    stopRecording();
  }

  function onRowKeyUp(action: ShortcutAction, event: React.KeyboardEvent<HTMLButtonElement>) {
    if (recording === action) setHeld(pressedModifiers(event));
  }

  async function save() {
    setSaving(true);
    try {
      // 校验（含重复）、注册、落盘都在后端一步完成；有键注册不上时整组不生效也不保存
      onSaved(await saveShortcuts(shortcuts));
      setConflicts([]);
      notify("快捷键已保存，立即生效");
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setSaving(false);
    }
  }

  const renderRow = (binding: ShortcutBinding) => {
    const label = ACTION_LABELS[binding.action] ?? { name: binding.action };
    const isRecording = recording === binding.action;
    const saved = settings.shortcuts.find((item) => item.action === binding.action)?.accelerator ?? null;
    const conflicted = Boolean(binding.accelerator && binding.accelerator === saved && conflicts.includes(binding.accelerator));
    const unavailable = binding.action === "record" && caps && !caps.recording.available;
    const rejection = rejected?.action === binding.action ? rejected.key : null;
    return (
      <div key={binding.action} className={`key-row ${isRecording ? "is-recording" : ""}`}>
        <button
          type="button"
          className="key-row-main"
          aria-label={`快捷键：${label.name}，当前按键：${binding.accelerator || "未设置"}`}
          onClick={() => {
            // 这一行是「录快捷键」的选中态，不是执行功能：点它开始录，再点一次取消
            setRejected(null);
            if (isRecording) stopRecording();
            else setRecording(binding.action);
          }}
          onKeyDown={(event) => onRowKeyDown(binding.action, event)}
          onKeyUp={(event) => onRowKeyUp(binding.action, event)}
          onBlur={() => isRecording && stopRecording()}
        >
          <span className="key-row-text">
            <span className="key-row-name">
              <span className={isRecording ? "strong" : ""}>{label.name}</span>
            </span>
            {conflicted && <span className="signal small">没注册上，这组键已被其他程序占用</span>}
            {unavailable && !conflicted && <span className="quiet small">不可用</span>}
            {rejection && (
              <span className="signal small">只按了 {rejection}。至少要带上 Ctrl、Alt、Shift 中的一个</span>
            )}
          </span>
          <span className="key-row-value">
            {isRecording ? (
              <span className="keys-pending">
                {held.length > 0 && <Keys value={held.join("+")} />}
                <span className="kbd-slot" aria-hidden="true" />
                <span className="quiet small">Esc 取消</span>
              </span>
            ) : binding.accelerator ? (
              <Keys value={binding.accelerator} struck={conflicted} />
            ) : (
              <span className="quiet small">未设置</span>
            )}
          </span>
        </button>
        {!isRecording && binding.accelerator && (
          <button
            type="button"
            className="key-row-clear"
            aria-label={`清除「${label.name}」的快捷键`}
            onClick={() =>
              setShortcuts((items) => items.map((item) => (item.action === binding.action ? { ...item, accelerator: null } : item)))
            }
          >
            清除
          </button>
        )}
      </div>
    );
  };

  return (
    <section className="panel panel-shortcuts">
      <div className="panel-head">
        <h2 className="panel-title">全局快捷键</h2>
        <button
          type="button"
          className="text-btn quiet small"
          onClick={() => setShortcuts(shortcuts.map((item) => ({ ...item, accelerator: null })))}
        >
          全部清除
        </button>
      </div>
      <div className="rows">{shortcuts.map(renderRow)}</div>
      <div className="save-bar">
        <span className="small ink-2">{changed > 0 ? `改了 ${changed} 处，还没保存` : ""}</span>
        <span className="save-bar-actions">
          {changed > 0 && (
            <button type="button" className="text-btn ink-2" onClick={() => setShortcuts(settings.shortcuts)}>撤销</button>
          )}
          <button
            type="button"
            className="btn btn-primary"
            aria-label="保存快捷键"
            onClick={() => void save()}
            disabled={saving || changed === 0}
          >
            {saving ? "保存中…" : "保存"}
          </button>
        </span>
      </div>
    </section>
  );
}

/** 剪贴板、图片格式、保存位置、录制——这一组和快捷键页放在一起，都是截图的去向 */
export function StoragePanel() {
  const { settings, onSaved, notify, caps } = useApp();

  async function update(patch: Partial<Settings>, message?: string) {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(errorText(error), "error");
    }
  }

  async function pickDir(key: "saveDir" | "recordingDir") {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") await update({ [key]: selected }, key === "saveDir" ? "快照保存位置已更改" : "录像保存位置已更改");
    } catch {
      notify("当前环境不支持选择目录", "error");
    }
  }

  const recordingDirLabel = settings.recordingDir
    ? middleTruncatePath(settings.recordingDir)
    : settings.saveDir
      ? "跟随快照目录"
      : "系统「下载」目录";

  return (
    <section className="panel panel-storage">
      <h2 className="panel-title">剪贴板与保存</h2>
      <div className="rows">
        <div className="row">
          <span className="row-label">自动清空剪贴板</span>
          <Choices
            ariaLabel="自动清空剪贴板"
            value={settings.clipboardAutoClear}
            onChange={(value) => void update({ clipboardAutoClear: value })}
            options={[
              { value: "30s", label: "30 秒" },
              { value: "60s", label: "60 秒" },
              { value: "5m", label: "5 分钟" },
              { value: "never", label: "不清空" },
            ]}
          />
        </div>
        <div className="row">
          <span className="row-label">图像格式</span>
          <Choices
            ariaLabel="图像格式"
            value={settings.snapshotFormat}
            onChange={(value) => void update({ snapshotFormat: value })}
            options={[
              { value: "png", label: "PNG" },
              { value: "jpeg", label: "JPEG" },
              { value: "webp", label: "WebP" },
            ]}
          />
        </div>
        <div className="row row-stack">
          <div className="row-line">
            <span className="row-label">快照存到</span>
            <button type="button" className="link small" onClick={() => void pickDir("saveDir")}>更改</button>
          </div>
          <span className="mono small ink-2 nowrap" title={settings.saveDir || undefined}>
            {settings.saveDir ? middleTruncatePath(settings.saveDir) : "应用数据目录（默认）"}
          </span>
        </div>
      </div>

      <h2 className="panel-title panel-title-gap">录制</h2>
      <div className="rows">
        <div className="row row-stack">
          <div className="row-line">
            <span className="row-label">录像存到</span>
            <span className="row-links small">
              <button type="button" className="link" onClick={() => openRecordingsDir().catch((error) => notify(errorText(error), "error"))}>打开</button>
              <button type="button" className="link" onClick={() => void pickDir("recordingDir")}>更改</button>
              {settings.recordingDir && (
                <button type="button" className="link" onClick={() => void update({ recordingDir: "" }, "已恢复默认录像位置")}>恢复默认</button>
              )}
            </span>
          </div>
          <span className={`small nowrap ${settings.recordingDir ? "mono ink-2" : "quiet"}`} title={settings.recordingDir || undefined}>
            {recordingDirLabel}
          </span>
        </div>
        <div className="row">
          <label htmlFor="switch-system-audio" className="row-label">同时录系统声音</label>
          <Switch
            id="switch-system-audio"
            label="同时录系统声音"
            value={settings.recordSystemAudio}
            disabled={Boolean(caps && !caps.recordingSystemAudio.available && !settings.recordSystemAudio)}
            onChange={(value) => void update({ recordSystemAudio: value })}
          />
        </div>
        <div className="row">
          <label htmlFor="switch-microphone" className="row-label">同时录麦克风</label>
          <Switch
            id="switch-microphone"
            label="同时录麦克风"
            value={settings.recordMicrophone}
            disabled={Boolean(caps && !caps.recordingMicrophone.available && !settings.recordMicrophone)}
            onChange={(value) => void update({ recordMicrophone: value })}
          />
        </div>
      </div>
    </section>
  );
}

export function ShortcutsPage() {
  return (
    <div className="page-grid page-grid-2">
      <ShortcutsPanel />
      <StoragePanel />
    </div>
  );
}

/** 给别处用：某个动作当前绑的键（输入框里要显示） */
export function acceleratorOf(settings: Settings, action: ShortcutAction) {
  return settings.shortcuts.find((item) => item.action === action)?.accelerator ?? null;
}

export { shortcutKeys };
