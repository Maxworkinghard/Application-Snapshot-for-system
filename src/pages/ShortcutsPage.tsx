import React, { useEffect, useState } from "react";
import {
  Camera,
  FolderOpen,
  Layers,
  MonitorSmartphone,
  RotateCcw,
  Save,
  ScanText,
  TextCursorInput,
  X,
} from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { openRecordingsDir, platformCapabilities, savePreferences, saveShortcuts } from "../lib/backend";
import { applyGlobalShortcuts, platformSupports } from "../lib/shortcuts";
import { normalizeKey } from "../lib/format";
import { KbdBadge } from "../components/ui/KbdBadge";
import { SegGroup } from "../components/ui/SegGroup";
import { PrefToggle } from "../components/ui/PrefToggle";
import type { PlatformCapabilities, Settings, ShortcutAction, ShortcutBinding } from "../types";

const actionLabels: Record<ShortcutAction, { name: string; tag?: string }> = {
  snapshot: { name: "窗口快照", tag: "前台窗口" },
  fullscreen: { name: "全屏快照", tag: "主显示器" },
  scrolling: { name: "滚动长截图", tag: "窗口连拍" },
  record: { name: "窗口录制", tag: "MP4" },
  polish: { name: "润色 Prompt", tag: "剪贴板" },
  ocr: { name: "提取文字 (OCR)", tag: "离线识别" },
};

export function ShortcutsPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
}) {
  const [shortcuts, setShortcuts] = useState(settings.shortcuts);
  const [recording, setRecording] = useState<ShortcutAction | null>(null);
  const [saving, setSaving] = useState(false);
  useEffect(() => setShortcuts(settings.shortcuts), [settings.shortcuts]);

  function captureShortcut(action: ShortcutAction, event: React.KeyboardEvent<HTMLDivElement>) {
    event.preventDefault();
    event.stopPropagation();
    if (["Control", "Shift", "Alt", "Meta"].includes(event.key)) return;
    const parts: string[] = [];
    if (event.ctrlKey || event.metaKey) parts.push("CommandOrControl");
    if (event.altKey) parts.push("Alt");
    if (event.shiftKey) parts.push("Shift");
    if (parts.length === 0) return;
    const accelerator = [...parts, normalizeKey(event.key)].join("+");
    setShortcuts((items) => items.map((item) => item.action === action ? { ...item, accelerator } : item));
    setRecording(null);
  }

  async function save() {
    const active = shortcuts.map((item) => item.accelerator).filter(Boolean);
    if (new Set(active).size !== active.length) {
      notify("快捷键不能重复");
      return;
    }
    setSaving(true);
    try {
      await applyGlobalShortcuts(shortcuts);
      const next = await saveShortcuts(shortcuts);
      onSaved(next);
      notify("快捷键已保存并立即生效");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  // 这里只剩快捷键行上的「录制不可用」角标需要 caps
  const [caps, setCaps] = useState<PlatformCapabilities | null>(null);
  useEffect(() => {
    void platformCapabilities()
      .then(setCaps)
      .catch(() => setCaps(null));
  }, []);

  async function updatePrefs(patch: Partial<Settings>, message?: string) {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function chooseSaveDir() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") await updatePrefs({ saveDir: selected }, "保存目录已更新");
    } catch {
      notify("当前环境不支持选择目录");
    }
  }

  async function chooseRecordingDir() {
    try {
      const selected = await open({ directory: true, multiple: false });
      if (typeof selected === "string") {
        await updatePrefs({ recordingDir: selected }, "录制目录已更新");
      }
    } catch {
      notify("当前环境不支持选择目录");
    }
  }

  async function revealRecordings() {
    try {
      // 目录可能还没建过（一次都没录过），后端会先建再打开
      await openRecordingsDir();
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  const renderRow = (binding: ShortcutBinding) => {
    const isRecording = recording === binding.action;
    return (
      <div
        key={binding.action}
        role="button"
        tabIndex={0}
        aria-label={`快捷键：${actionLabels[binding.action].name}，当前按键：${binding.accelerator || "未设置"}`}
        className={`shortcut-interactive-row compact-row ${isRecording ? "is-recording-mode" : ""}`}
        onClick={() => {
          // 这一行是"录制快捷键"的选中态，不是执行功能：点它=选中开始录制，再点一次=取消选中
          setRecording(isRecording ? null : binding.action);
        }}
        onKeyDown={(event) => {
          if (isRecording) {
            captureShortcut(binding.action, event);
          } else if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setRecording(binding.action);
          }
        }}
      >
        <div className="action-identity-col">
          <span className="action-leading-icon">
            {binding.action === "snapshot" ? <MonitorSmartphone size={15} />
              : binding.action === "fullscreen" ? <Camera size={15} />
              : binding.action === "scrolling" ? <Layers size={15} />
              : binding.action === "record" ? <span className="record-symbol" />
              : binding.action === "ocr" ? <ScanText size={15} />
              : <TextCursorInput size={15} />}
          </span>
          <span className="action-name">{actionLabels[binding.action].name}</span>
          {actionLabels[binding.action].tag && (
            <span className="action-tag">{actionLabels[binding.action].tag}</span>
          )}
          {binding.action === "record" && caps && !caps.recording.available && (
            <span className="action-tag" title={caps.recording.detail}>录制不可用</span>
          )}
        </div>

        <div className="action-keycap-col">
          {isRecording ? (
            <div className="recording-active-capsule">
              <span className="pulse-dot-recording" />
              <span className="recording-prompt-text">按下新组合键…</span>
            </div>
          ) : (
            <>
              <div className="kbd-badge-anchor">
                {/* 整行本身就是录制触发区，这里只呈现当前绑定状态，不再放重复的按钮 */}
                <KbdBadge shortcut={binding.accelerator} size="sm" />
              </div>
              <button
                className="clear-keycap-ghost-btn"
                title="清除绑定"
                disabled={!binding.accelerator}
                onClick={(event) => {
                  event.stopPropagation();
                  setShortcuts((items) => items.map((item) => item.action === binding.action ? { ...item, accelerator: null } : item));
                }}
              ><X size={14} /></button>
            </>
          )}
        </div>
      </div>
    );
  };

  // 老配置里可能残留本平台不支持的动作（后端迁移只补不删），别渲染成点了没反应的死行
  const visibleShortcuts = shortcuts.filter((item) => platformSupports(item.action));

  // 录制态下点击页面空白处（非快捷键行）也算取消选中
  function onPageClick(event: React.MouseEvent<HTMLDivElement>) {
    if (!recording) return;
    if ((event.target as HTMLElement).closest(".shortcut-interactive-row")) return;
    setRecording(null);
  }

  return (
    <div className="shortcut-hub-unified-layout" onClick={onPageClick}>
      <header className="hub-top-strip">
        <div className="hub-title-line">
          <h1 className="hub-heading">快捷操作</h1>
        </div>
        <div className="hub-top-actions">
          <button
            className="hub-reset-btn"
            onClick={() => setShortcuts(shortcuts.map((item) => ({ ...item, accelerator: null })))}
            title="清除全部绑定"
          >
            <RotateCcw size={14} />
            <span>全部清除</span>
          </button>
        </div>
      </header>

      <div className="hub-two-col-grid">
        <div className="hub-shortcuts-col">
          <section className="hub-section-block">
            <div className="section-label-bar">
              <span className="section-name">全局快捷键</span>
              <span className="section-count tabular-nums">{visibleShortcuts.length} 项</span>
            </div>
            <div className="shortcuts-list-table">
              {visibleShortcuts.map(renderRow)}
            </div>
          </section>
        </div>

        <div className="hub-preferences-col">
          <section className="hub-section-block">
            <div className="section-label-bar">
              <span className="section-name">剪贴板与保存</span>
            </div>
            <div className="preferences-group-card">
              <div className="pref-item-row">
                <span className="pref-title">自动清空剪贴板</span>
                <SegGroup
                  ariaLabel="自动清空剪贴板"
                  value={settings.clipboardAutoClear}
                  onChange={(value) => void updatePrefs({ clipboardAutoClear: value })}
                  options={[
                    { value: "30s", label: "30秒" },
                    { value: "60s", label: "60秒" },
                    { value: "5m", label: "5分钟" },
                    { value: "never", label: "不清除" },
                  ]}
                />
              </div>
              <div className="pref-item-row">
                <span className="pref-title">快照图像格式</span>
                <SegGroup
                  ariaLabel="快照图像格式"
                  value={settings.snapshotFormat}
                  onChange={(value) => void updatePrefs({ snapshotFormat: value })}
                  options={[
                    { value: "png", label: "PNG 无损" },
                    { value: "jpeg", label: "JPEG" },
                    { value: "webp", label: "WebP" },
                  ]}
                />
              </div>
              <div className="pref-item-row folder-row">
                <span className="pref-title">快照保存目录</span>
                <div className="folder-picker-box">
                  <span className="folder-path-text" title={settings.saveDir || undefined}>
                    {settings.saveDir || "默认快照目录"}
                  </span>
                  <button type="button" className="folder-action-btn" onClick={() => void chooseSaveDir()}>
                    <FolderOpen size={12} />
                    更改
                  </button>
                </div>
              </div>
            </div>
          </section>

          <section className="hub-section-block">
            <div className="section-label-bar">
              <span className="section-name">录制设置</span>
            </div>
            <div className="preferences-group-card">
              <div className="pref-item-row folder-row">
                <span className="pref-title">录制保存目录</span>
                <div className="folder-picker-box">
                  <span className="folder-path-text" title={settings.recordingDir || undefined}>
                    {settings.recordingDir || (settings.saveDir ? "跟随上面的本地保存目录" : "默认下载目录")}
                  </span>
                  <button type="button" className="folder-action-btn" onClick={() => void chooseRecordingDir()}>
                    <FolderOpen size={12} />
                    更改
                  </button>
                  <button type="button" className="folder-action-btn" onClick={() => void revealRecordings()}>
                    打开
                  </button>
                  {settings.recordingDir && (
                    <button
                      type="button"
                      className="folder-action-btn"
                      onClick={() => void updatePrefs({ recordingDir: "" }, "已恢复默认录制目录")}
                      title="清除后跟随上面的本地保存目录，没设过则落到下载目录"
                    >
                      恢复默认
                    </button>
                  )}
                </div>
              </div>
              <div className="pref-item-row recording-audio-row">
                <div className="recording-audio-copy">
                  <span className="pref-title">系统音频</span>
                  <small>{caps?.recordingSystemAudio.detail ?? "录制电脑正在播放的声音"}</small>
                </div>
                <PrefToggle
                  value={settings.recordSystemAudio}
                  label="录制系统音频"
                  disabled={!caps?.recordingSystemAudio.available && !settings.recordSystemAudio}
                  onChange={(value) => void updatePrefs({ recordSystemAudio: value })}
                />
              </div>
              <div className="pref-item-row recording-audio-row">
                <div className="recording-audio-copy">
                  <span className="pref-title">麦克风</span>
                  <small>{caps?.recordingMicrophone.detail ?? "录制麦克风输入"}</small>
                </div>
                <PrefToggle
                  value={settings.recordMicrophone}
                  label="录制麦克风"
                  disabled={!caps?.recordingMicrophone.available && !settings.recordMicrophone}
                  onChange={(value) => void updatePrefs({ recordMicrophone: value })}
                />
              </div>
            </div>
          </section>
        </div>
      </div>

      <div className="hub-bottom-status-strip">
        <button className="primary-button" onClick={save} disabled={saving}>
          <Save size={16} /> {saving ? "保存中…" : "保存快捷键"}
        </button>
      </div>
    </div>
  );
}
