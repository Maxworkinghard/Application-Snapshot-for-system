import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Keyboard,
  PawPrint,
  TextCursorInput,
  X,
  History,
  Sparkles,
  SlidersHorizontal,
  Minus,
  Maximize2,
  Layers,
  Cpu,
  Info,
  Palette,
} from "lucide-react";
import {
  loadSettings,
  onSettingsChanged,
  onCaptureFeedback,
} from "./lib/backend";
import { applyGlobalShortcuts, platformSupports } from "./lib/shortcuts";
import { previewHintSound } from "./lib/sound";
import { ThemePage } from "./pages/ThemePage";
import { OcrPage } from "./pages/OcrPage";
import { HistoryPage } from "./pages/HistoryPage";
import { LoadingState } from "./components/LoadingState";
import { PromptPage } from "./pages/PromptPage";
import { PetPage } from "./pages/PetPage";
import { PreferencesPage } from "./pages/PreferencesPage";
import { ShortcutsPage } from "./pages/ShortcutsPage";
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
  Settings,
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
