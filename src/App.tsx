import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  MonitorSmartphone,
  Keyboard,
  PawPrint,
  Save,
  TextCursorInput,
  X,
  History,
  ScanText,
  Sparkles,
  SlidersHorizontal,
  Minus,
  Camera,
  Maximize2,
  Layers,
  Cpu,
  RotateCcw,
  Info,
  FolderOpen,
  Palette,
} from "lucide-react";
import {
  loadSettings,
  onSettingsChanged,
  onCaptureFeedback,
  platformCapabilities,
  openRecordingsDir,
  savePreferences,
  saveShortcuts,
} from "./lib/backend";
import { applyGlobalShortcuts, platformSupports } from "./lib/shortcuts";
import { normalizeKey } from "./lib/format";
import { previewHintSound } from "./lib/sound";
import { KbdBadge } from "./components/ui/KbdBadge";
import { SegGroup } from "./components/ui/SegGroup";
import { ThemePage } from "./pages/ThemePage";
import { OcrPage } from "./pages/OcrPage";
import { HistoryPage } from "./pages/HistoryPage";
import { LoadingState } from "./components/LoadingState";
import { PromptPage } from "./pages/PromptPage";
import { PetPage } from "./pages/PetPage";
import { PreferencesPage } from "./pages/PreferencesPage";
import {
  applyTheme,
  broadcastTheme,
  persistThemePreference,
  readThemePreference,
  resolveTheme,
  watchSystemTheme,
  type ThemeMode,
  type ThemePreference,
} from "./lib/theme";
import type {
  NavPage,
  PlatformCapabilities,
  Settings,
  ShortcutAction,
  ShortcutBinding,
} from "./types";

type NavEntry = { id: NavPage; label: string; icon: typeof TextCursorInput };

const navGroups: Array<{ title: string; items: NavEntry[] }> = [
  {
    title: "工作台",
    items: [
      { id: "shortcuts", label: "快捷操作", icon: Keyboard },
      { id: "prompt", label: "Prompt 编辑", icon: Sparkles },
      { id: "history", label: "快照历史", icon: History },
      { id: "pet", label: "桌面伴侣", icon: PawPrint },
    ],
  },
  {
    title: "设置",
    items: [
      { id: "prefs", label: "偏好设置", icon: SlidersHorizontal },
      { id: "theme", label: "界面主题", icon: Palette },
      { id: "ocr", label: "文字识别", icon: Cpu },
    ],
  },
];

const actionLabels: Record<ShortcutAction, { name: string; tag?: string }> = {
  snapshot: { name: "窗口快照", tag: "前台窗口" },
  fullscreen: { name: "全屏快照", tag: "主显示器" },
  scrolling: { name: "滚动长截图", tag: "窗口连拍" },
  record: { name: "窗口录制", tag: "MP4" },
  polish: { name: "润色 Prompt", tag: "剪贴板" },
  ocr: { name: "提取文字 (OCR)", tag: "离线识别" },
};

// 滚动长截图仅在 Linux/X11 实现；Win/mac 不展示、不注册、不迁移。
// 标签表保留该项，真正的开关是下面默认绑定里的这一行。
const SCROLLING_SUPPORTED = platformSupports("scrolling");

const initialSettings: Settings = {
  baseUrl: "",
  model: "",
  hasApiKey: false,
  templates: [],
  activeTemplateId: "builtin-default",
  selectedAppearanceId: "app-icon",
  petAssets: [],
  shortcuts: [
    { action: "snapshot", accelerator: "Alt+Shift+2" },
    { action: "fullscreen", accelerator: "Alt+Shift+F" },
    ...(SCROLLING_SUPPORTED ? [{ action: "scrolling" as const, accelerator: null }] : []),
    { action: "record", accelerator: null },
    { action: "polish", accelerator: "Alt+Shift+P" },
    { action: "ocr", accelerator: "Alt+Shift+O" },
  ],
  clipboardAutoClear: "60s",
  snapshotFormat: "png",
  saveDir: "",
  recordingDir: "",
  customTheme: null,
  shutterSound: "crisp",
  customSoundPath: null,
  flashOnCapture: true,
  hideAfterCopy: false,
  autoSaveLocal: true,
  launchOnBoot: false,
  includeCursor: false,
  recordSystemAudio: false,
  recordMicrophone: false,
  afterCapture: "clipboard",
};

/** 旧页面淡出的时长，必须和 styles 里 page-leave 的 animation-duration 对齐 */
const PAGE_EXIT_MS = 120;

export function App() {
  const [page, setPage] = useState<NavPage>("shortcuts");
  // 实际渲染的页面滞后于 page：先让旧页面演完淡出，再换内容
  const [shownPage, setShownPage] = useState<NavPage>("shortcuts");
  const [isLeaving, setIsLeaving] = useState(false);
  const canvasRef = useRef<HTMLElement | null>(null);
  const [settings, setSettings] = useState<Settings>(initialSettings);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<string | null>(null);
  const [themePreference, setThemePreference] = useState<ThemePreference>(() => readThemePreference());
  const [theme, setTheme] = useState<ThemeMode>(() => resolveTheme(readThemePreference()));

  function changeTheme(next: ThemePreference) {
    setThemePreference(next);
    persistThemePreference(next);
    const mode = resolveTheme(next);
    setTheme(mode);
    applyTheme(mode);
    broadcastTheme(mode);
  }

  // 跟随系统时，系统外观变了要当场跟上，否则得重开窗口才生效
  useEffect(() => {
    if (themePreference !== "system") return;
    return watchSystemTheme((mode) => {
      setTheme(mode);
      applyTheme(mode);
      broadcastTheme(mode);
    });
  }, [themePreference]);

  const [flashVisible, setFlashVisible] = useState(false);

  useEffect(() => {
    loadSettings()
      .then(setSettings)
      .finally(() => setLoading(false));
    const pending = onSettingsChanged(setSettings);
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const pending = onCaptureFeedback((payload) => {
      if (payload.flash) {
        setFlashVisible(true);
        window.setTimeout(() => setFlashVisible(false), 220);
      }
      if (payload.shutterSound !== "none") {
        previewHintSound(
          payload.shutterSound === "soft" ? "soft" : "crisp",
          payload.shutterSound === "custom" ? payload.customSoundPath : null,
          80,
        );
      }
    });
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  // 按值比较而不是数组引用：改任意一项偏好都会换来一份新的 settings，
  // 里面的 shortcuts 数组是新对象但内容没变，按引用依赖会白白重注册一轮
  const shortcutFingerprint = settings.shortcuts
    .map((item) => `${item.action}:${item.accelerator ?? ""}`)
    .join("|");
  useEffect(() => {
    if (loading) return;
    void applyGlobalShortcuts(settings.shortcuts).catch((error) => {
      const conflicted = error instanceof Error ? error.message : "";
      notify(
        conflicted
          ? `这些快捷键没能注册，可能已被其他程序占用：${conflicted}`
          : "快捷键注册失败，请换一组试试",
      );
    });
    // settings.shortcuts 的内容由 shortcutFingerprint 代表
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, shortcutFingerprint]);

  useEffect(() => {
    if (page === shownPage) {
      // 淡出途中又点回原页：直接复位，否则会停在半透明状态
      setIsLeaving(false);
      return;
    }
    setIsLeaving(true);
    const timer = window.setTimeout(() => {
      setShownPage(page);
      setIsLeaving(false);
      // 换页后回到顶部，否则新页面会停在上一页的滚动位置
      canvasRef.current?.scrollTo({ top: 0 });
    }, PAGE_EXIT_MS);
    return () => window.clearTimeout(timer);
  }, [page, shownPage]);

  function notify(message: string) {
    setToast(message);
    window.setTimeout(() => setToast(null), 2600);
  }

  const content = useMemo(() => {
    if (shownPage === "prompt") {
      return <PromptPage settings={settings} onSaved={setSettings} notify={notify} />;
    }
    if (shownPage === "pet") {
      return <PetPage settings={settings} onSaved={setSettings} notify={notify} />;
    }
    if (shownPage === "history") {
      return <HistoryPage notify={notify} />;
    }
    if (shownPage === "ocr") {
      return <OcrPage notify={notify} />;
    }
    if (shownPage === "prefs") {
      return <PreferencesPage settings={settings} onSaved={setSettings} notify={notify} />;
    }
    if (shownPage === "theme") {
      return <ThemePage preference={themePreference} resolved={theme} onChange={changeTheme} />;
    }
    return <ShortcutsPage settings={settings} onSaved={setSettings} notify={notify} />;
  }, [shownPage, settings, themePreference, theme]);

  const appWindow = "__TAURI_INTERNALS__" in window ? getCurrentWindow() : null;

  // 窗口操作要 capability 里显式放行，缺权限时 promise 会被拒。
  // 原先一律 void 掉，表现就是「点了没反应」且不留痕迹——必须让它说话。
  function runWindowAction(action: (() => Promise<unknown>) | undefined, label: string) {
    if (!action) return;
    void action().catch((error) => {
      notify(`${label}失败：${error instanceof Error ? error.message : String(error)}`);
    });
  }

  return (
    <div className="settings-window-frame">
      <div
        className="window-titlebar"
        data-tauri-drag-region
        onDoubleClick={() => runWindowAction(appWindow?.toggleMaximize.bind(appWindow), "最大化")}
      >
        <div className="titlebar-left">
          <div className="window-title-chip">
            <span className="title-name">应用快照</span>
          </div>
        </div>

        <div className="titlebar-right">
          <div className="window-controls">
            <button
              className="window-control-btn"
              onClick={() => runWindowAction(appWindow?.minimize.bind(appWindow), "最小化")}
              title="最小化"
              aria-label="最小化"
            >
              <Minus size={14} />
            </button>
            <button
              className="window-control-btn"
              onClick={() => runWindowAction(appWindow?.toggleMaximize.bind(appWindow), "最大化")}
              title="最大化 / 还原"
              aria-label="最大化或还原"
            >
              <Maximize2 size={12} />
            </button>
            <button
              className="window-control-btn is-close"
              onClick={() => runWindowAction(appWindow?.close.bind(appWindow), "关闭")}
              title="关闭"
              aria-label="关闭"
            >
              <X size={14} />
            </button>
          </div>
        </div>
      </div>

      <div className="window-content-grid">
        <aside className="window-sidebar">
          <div className="sidebar-workspace-header">
            <div className="workspace-avatar-box"><Layers size={15} /></div>
            <div className="workspace-text-group">
              <div className="workspace-title-row">
                <span className="workspace-name-text">个人工作区</span>
              </div>
              <span className="workspace-meta-text">捕捉与整理</span>
            </div>
          </div>

          {navGroups.map((group) => (
            <div className="sidebar-group-block" key={group.title}>
              <div className="sidebar-group-title">{group.title}</div>
              <div className="sidebar-nav-list">
                {group.items.map((item) => {
                  const Icon = item.icon;
                  const isActive = page === item.id;
                  return (
                    <button
                      key={item.id}
                      className={`sidebar-nav-item ${isActive ? "is-active" : ""}`}
                      aria-current={isActive ? "page" : undefined}
                      onClick={() => setPage(item.id)}
                    >
                      <span className="nav-item-icon"><Icon size={15} /></span>
                      <span className="nav-item-title">{item.label}</span>
                    </button>
                  );
                })}
              </div>
            </div>
          ))}

          <div className="sidebar-footer" />
        </aside>

        <main className="window-main-canvas" ref={canvasRef}>
          <div
            key={shownPage}
            className={`page-transition-layer ${isLeaving ? "is-leaving" : "is-entering"} ${shownPage === "prompt" || shownPage === "ocr" ? "is-fullheight-page" : ""}`}
          >
            {loading ? <LoadingState /> : content}
          </div>
        </main>
      </div>

      {flashVisible && <div className="shutter-flash-overlay" aria-hidden />}
      {toast && (
        <div className="toast-portal">
          <div className="toast-card toast-info" role="status">
            <div className="toast-lead"><Info size={16} className="toast-icon info" /></div>
            <div className="toast-body">
              <div className="toast-title">{toast}</div>
            </div>
            <button className="toast-close-btn" onClick={() => setToast(null)} title="关闭提示">
              <X size={12} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function ShortcutsPage({
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
