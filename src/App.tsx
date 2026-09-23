import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AppWindow,
  Check,
  MonitorSmartphone,
  Keyboard,
  ImagePlus,
  PanelsTopLeft,
  PawPrint,
  Plus,
  RefreshCw,
  Save,
  TextCursorInput,
  Trash2,
  X,
  History,
  ScanText,
  Sparkles,
  SlidersHorizontal,
  Minus,
  Wand2,
  ClipboardCopy,
  Camera,
  Maximize2,
  Layers,
  Cpu,
  Eye,
  EyeOff,
  HardDrive,
  RotateCcw,
  Send,
  Upload,
  Search,
  Clock,
  Info,
  FolderOpen,
  Palette,
  Play,
} from "lucide-react";
import {
  addPetAsset,
  deletePetAsset,
  fetchModels,
  getPetAssetDataUrl,
  loadSettings,
  onSettingsChanged,
  onCaptureFeedback,
  clearSnapshots,
  deleteSnapshot,
  getSnapshotDataUrl,
  listSnapshots,
  platformCapabilities,
  ocrSnapshot,
  openRecordingsDir,
  openSnapshotsDir,
  polishText,
  savePreferences,
  savePromptSettings,
  saveShortcuts,
  selectPetAppearance,
} from "./lib/backend";
import { applyGlobalShortcuts, platformSupports } from "./lib/shortcuts";
import { fileNameOf, formatBytes, formatWhen, normalizeKey } from "./lib/format";
import { previewHintSound } from "./lib/sound";
import { renderPetMedia } from "./windows/PetWindow";
import { KbdBadge } from "./components/ui/KbdBadge";
import { PrefToggle } from "./components/ui/PrefToggle";
import { SegGroup } from "./components/ui/SegGroup";
import { ThemePage } from "./pages/ThemePage";
import { OcrPage } from "./pages/OcrPage";
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
  SnapshotRecord,
  PromptTemplate,
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
  recordings: { name: "打开录制目录", tag: "文件管理器" },
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
    { action: "recordings", accelerator: null },
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

function ModelSettingsDialog({
  settings,
  onSaved,
  notify,
  onClose,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
  onClose: () => void;
}) {
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl);
  const [model, setModel] = useState(settings.model);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);

  useEffect(() => {
    setBaseUrl(settings.baseUrl);
    setModel(settings.model);
  }, [settings]);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  const serviceReady = Boolean(settings.baseUrl && settings.model);

  async function loadModels() {
    if (!baseUrl.trim()) {
      notify("请先填写 Base URL");
      return;
    }
    setFetchingModels(true);
    try {
      const models = await fetchModels(baseUrl.trim(), apiKey.trim() || null);
      setAvailableModels(models);
      if (!model.trim() && models[0]) setModel(models[0]);
      notify(`已拉取 ${models.length} 个模型`);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setFetchingModels(false);
    }
  }

  async function save() {
    setSaving(true);
    try {
      // 模板归 Prompt 页管，这里原样回传，避免互相覆盖
      const next = await savePromptSettings({
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        apiKey: apiKey.trim() || null,
        activeTemplateId: settings.activeTemplateId,
        templates: settings.templates,
      });
      onSaved(next);
      setApiKey("");
      notify("模型设置已保存");
      onClose();
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  return createPortal(
    <div
      className="snapshot-lightbox-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="model-settings-title"
      onClick={onClose}
    >
      <div className="model-settings-dialog-panel" onClick={(event) => event.stopPropagation()}>
        <div className="model-settings-dialog-header">
          <div className="model-settings-dialog-heading">
            <div className="model-settings-dialog-title-row">
              <h2 className="model-settings-dialog-title" id="model-settings-title">编辑模型</h2>
              <span className="honest-hint-tag">{serviceReady ? "已配置" : "未配置"}</span>
            </div>
            <p className="model-settings-dialog-subtitle">配置模型端点与凭证，API Key 将保存到系统凭据管理器。</p>
          </div>
          <button className="lightbox-close-btn" onClick={onClose} title="关闭 (Esc)" aria-label="关闭模型编辑弹窗">
            <X size={16} />
          </button>
        </div>

        <div className="model-settings-fields">
          <label className="model-settings-field is-wide">
            <span className="drawer-input-label">接口端点 (Base URL)</span>
            <input
              type="text"
              className="drawer-text-input"
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
              placeholder="https://api.example.com/v1"
            />
          </label>

          <label className="model-settings-field">
            <span className="drawer-input-label">访问密钥 (API Key)</span>
            <input
              type="password"
              className="drawer-text-input"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={settings.hasApiKey ? "已保存，留空保持不变" : "可选"}
            />
          </label>

          <label className="model-settings-field">
            <span className="drawer-input-label">目标模型</span>
            <input
              type="text"
              className="drawer-text-input"
              value={model}
              list="available-models"
              onChange={(event) => setModel(event.target.value)}
              placeholder="输入或拉取模型"
            />
            <datalist id="available-models">
              {availableModels.map((item) => <option key={item} value={item} />)}
            </datalist>
          </label>
        </div>

        <div className="model-settings-dialog-footer">
          <span className="drawer-footer-tip">
            {serviceReady ? `当前生效：${settings.model}` : "填好端点与模型后，润色功能才可用"}
          </span>
          <div className="pane-action-buttons">
            <button className="drawer-test-btn" onClick={() => void loadModels()} disabled={fetchingModels}>
              <RefreshCw size={12} className={fetchingModels ? "spinning" : ""} />
              <span>{fetchingModels ? "拉取中…" : "拉取模型"}</span>
            </button>
            <button className="main-action-btn generate" onClick={() => void save()} disabled={saving}>
              <Save size={13} />
              <span>{saving ? "保存中…" : "保存设置"}</span>
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

function PromptPage({
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

function PetPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
}) {
  const [previews, setPreviews] = useState<Record<string, string>>({});
  const [stageMedia, setStageMedia] = useState<string>("");
  const [activeEntry, setActiveEntry] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  const selectedId = settings.selectedAppearanceId;
  const selectedAsset = settings.petAssets.find((item) => item.id === selectedId) ?? null;

  // 形态列表缩略图
  useEffect(() => {
    let active = true;
    void Promise.all(
      settings.petAssets.map(async (asset) => {
        try {
          return [asset.id, await getPetAssetDataUrl(asset.id)] as const;
        } catch {
          return [asset.id, ""] as const;
        }
      }),
    ).then((entries) => {
      if (active) setPreviews(Object.fromEntries(entries));
    });
    return () => { active = false; };
  }, [settings.petAssets]);

  // 切换形态时重置到该素材包的首个动作
  useEffect(() => {
    setActiveEntry(selectedAsset?.entry || selectedAsset?.animations[0] || null);
  }, [selectedId, selectedAsset?.entry]);

  // 展台预览：按当前选中的动作单独取一次
  useEffect(() => {
    let active = true;
    if (!selectedAsset) {
      setStageMedia("");
      return () => { active = false; };
    }
    void getPetAssetDataUrl(selectedAsset.id, activeEntry)
      .then((value) => { if (active) setStageMedia(value); })
      .catch(() => { if (active) setStageMedia(""); });
    return () => { active = false; };
  }, [selectedAsset?.id, activeEntry]);

  async function choose(id: string) {
    try {
      onSaved(await selectPetAppearance(id));
      notify("形象已切换");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function addAsset() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "桌宠形象", extensions: ["zip", "gif"] }],
    });
    if (typeof selected !== "string") return;
    setImporting(true);
    try {
      onSaved(await addPetAsset(selected));
      notify("素材包已导入");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setImporting(false);
    }
  }

  async function removeAsset(id: string) {
    try {
      onSaved(await deletePetAsset(id));
      notify("形象已删除");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function updatePrefs(patch: Partial<Settings>, message?: string) {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  /** 常见动作名的中文对照；素材包命名五花八门，命中不了就退回原名 */
  const ACTION_NAMES: Record<string, string> = {
    idle: "待机",
    sleep: "打盹",
    sleeping: "打盹",
    dance: "跳舞",
    dancing: "跳舞",
    think: "思考",
    thinking: "思考",
    type: "敲键盘",
    typing: "敲键盘",
    "type-keyboard": "敲键盘",
    keyboard: "敲键盘",
    build: "搭建",
    carry: "搬运",
    cheer: "欢呼",
    crabwalk: "横着走",
    error: "报错",
    jump: "跳跃",
    "jump-happy": "开心跳",
    point: "指路",
    sweep: "打扫",
    listening: "聆听",
    listen: "聆听",
    "xmas-idle": "圣诞待机",
    xmas: "圣诞",
    greeting: "挥爪问候",
    greet: "挥爪问候",
    wave: "招手",
    snuggle: "撒娇",
    walk: "走动",
    run: "跑动",
    eat: "进食",
    work: "工作",
    happy: "开心",
    sad: "难过",
    angry: "生气",
    surprised: "惊讶",
    love: "卖萌",
    hello: "打招呼",
    bye: "告别",
    "type-laptop": "敲笔记本",
    laptop: "笔记本",
    read: "阅读",
    write: "书写",
    code: "写代码",
    coding: "写代码",
    search: "搜索",
    wait: "等待",
    waiting: "等待",
    alert: "警告",
    warning: "警告",
    success: "成功",
    done: "完成",
    loading: "加载中",
    drag: "拖拽",
    click: "点击",
    sit: "坐下",
    stand: "站立",
  };

  /** 整包共有的前缀（如 clawd-）对用户没有信息量，逐条截掉 */
  const entryPrefix = useMemo(() => {
    const stems = (selectedAsset?.animations ?? []).map(
      (entry) => (entry.split("/").pop() ?? entry).replace(/\.[^.]+$/, ""),
    );
    if (stems.length < 2) return "";
    let prefix = stems[0];
    for (const stem of stems.slice(1)) {
      while (prefix && !stem.startsWith(prefix)) prefix = prefix.slice(0, -1);
      if (!prefix) break;
    }
    // 只在分隔符处截断，免得把 "sleep" 砍成 "sle"
    const cut = Math.max(prefix.lastIndexOf("-"), prefix.lastIndexOf("_"));
    return cut > 0 ? prefix.slice(0, cut + 1) : "";
  }, [selectedAsset?.id, selectedAsset?.animations]);

  /** cat/clawd-type-keyboard.gif -> 敲键盘 */
  function entryLabel(entry: string): string {
    const stem = (entry.split("/").pop() ?? entry).replace(/\.[^.]+$/, "");
    const key = (entryPrefix && stem.startsWith(entryPrefix) ? stem.slice(entryPrefix.length) : stem).toLowerCase();
    if (ACTION_NAMES[key]) return ACTION_NAMES[key];
    // 整体没命中就按分隔符逐段翻，但要求每段都能译——
    // 只译出一半会得到「敲键盘laptop」这种中英混搭，不如保留原名
    const parts = key.split(/[-_]/).filter(Boolean);
    if (parts.length > 1 && parts.every((part) => ACTION_NAMES[part])) {
      return parts.map((part) => ACTION_NAMES[part]).join("");
    }
    return key || stem;
  }

  const rows = [
    { id: "app-icon", name: "当前应用图标", desc: "不显示动画，仅展示前台应用的程序图标", builtin: true },
    ...settings.petAssets.map((asset) => ({
      id: asset.id,
      name: asset.name,
      desc: `${asset.animations.length} 个动作 · ${asset.path}`,
      builtin: false,
    })),
  ];

  return (
    <div className="pet-atelier-workspace">
      <section className="stage-canvas-panel">
        <div className="panel-header-strip">
          <span className="panel-title-text">伴侣悬浮演示</span>
        </div>

        <div className="viewport-desktop-canvas">
          <div className="stage-live-badge">
            <span className="status-led-dot" />
            <span>{selectedAsset ? "动态预览" : "外观预览"}</span>
          </div>

          <div className="viewport-avatar-center">
            <div className="cat-stage-orb">
              {selectedAsset ? (
                stageMedia ? renderPetMedia(stageMedia, "stage-image-element")
                            : <div className="polish-placeholder"><span className="inline-spinner" />读取素材…</div>
              ) : (
                <div className="pixel-stage-orb">
                  <AppWindow size={40} />
                </div>
              )}
            </div>
          </div>
        </div>

        {selectedAsset && selectedAsset.animations.length > 1 && (
          <div className="stage-actions-segmented-row">
            <div className="segmented-track">
              {selectedAsset.animations.map((entry) => (
                <button
                  key={entry}
                  className={`segmented-item-btn ${activeEntry === entry ? "is-active" : ""}`}
                  onClick={() => setActiveEntry(entry)}
                  title={entry}
                >
                  {entryLabel(entry)}
                </button>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="config-wardrobe-panel">
        <div className="panel-header-strip">
          <span className="panel-title-text">形态与特性</span>
          <button
            className="import-zip-ghost-btn"
            onClick={() => void addAsset()}
            disabled={importing}
            title="导入 .zip 素材包"
          >
            <Upload size={14} />
            <span>{importing ? "导入中" : "导入素材包 (.zip)"}</span>
          </button>
        </div>

        <div className="appearance-cards-col" role="radiogroup" aria-label="伴侣形态选择">
          {rows.map((row) => {
            const isSelected = selectedId === row.id;
            return (
              <div
                key={row.id}
                role="radio"
                aria-checked={isSelected}
                tabIndex={0}
                className={`appearance-row-item ${isSelected ? "is-selected" : ""}`}
                onClick={() => void choose(row.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    void choose(row.id);
                  }
                }}
              >
                <div className="avatar-thumbnail-wrap">
                  {row.builtin ? (
                    <div className="thumbnail-app-icon"><AppWindow size={16} /></div>
                  ) : previews[row.id] ? (
                    renderPetMedia(previews[row.id], "thumbnail-img")
                  ) : (
                    <div className="thumbnail-app-icon"><PawPrint size={16} /></div>
                  )}
                </div>

                <div className="avatar-meta-col">
                  <span className="avatar-title">{row.name}</span>
                  <span className="avatar-subtitle">{row.desc}</span>
                </div>

                {!row.builtin && (
                  <button
                    className="clear-keycap-ghost-btn"
                    title="删除这个形象"
                    onClick={(event) => { event.stopPropagation(); void removeAsset(row.id); }}
                  ><Trash2 size={13} /></button>
                )}

                <div className={`radio-dot-indicator ${isSelected ? "is-active" : ""}`}>
                  {isSelected && <Check size={12} />}
                </div>
              </div>
            );
          })}
        </div>

        <div className="companion-behavior-settings">
          <div className="behavior-header">
            <SlidersHorizontal size={14} />
            <span>形象素材要求</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">导入方式</span>
            <span className="history-sub">装着多个 GIF 的 ZIP 压缩包，或直接选一个 GIF</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">支持格式</span>
            <span className="history-sub">仅 GIF，包内其它格式会被跳过</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">默认动作</span>
            <span className="history-sub">文件名含 idle 的会排在最前</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">体积上限</span>
            <span className="history-sub">整包 100MB，单个 GIF 50MB</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">注意</span>
            <span className="history-sub">素材按需读取、不会复制，移走原文件形象会失效</span>
          </div>
        </div>
      </section>
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
              : binding.action === "recordings" ? <FolderOpen size={15} />
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

function LoadingState() {
  return <div className="loading-state"><span /><p>正在读取本机设置…</p></div>;
}

function HistoryPage({ notify }: { notify: (message: string) => void }) {
  const [records, setRecords] = useState<SnapshotRecord[]>([]);
  const [thumbs, setThumbs] = useState<Record<string, string>>({});
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [preview, setPreview] = useState<SnapshotRecord | null>(null);
  const [ocrText, setOcrText] = useState("");
  const [ocrBusy, setOcrBusy] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [clearing, setClearing] = useState(false);

  async function openFolder() {
    try {
      notify(`已打开：${await openSnapshotsDir()}`);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function refresh() {
    setLoading(true);
    try {
      setRecords(await listSnapshots());
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void refresh(); }, []);

  // 缩略图按需拉取，已取过的不重复请求
  useEffect(() => {
    let active = true;
    const missing = records.filter((item) => !thumbs[item.id]);
    if (missing.length === 0) return;
    void Promise.all(
      missing.map(async (item) => {
        try {
          return [item.id, await getSnapshotDataUrl(item.id)] as const;
        } catch {
          return [item.id, ""] as const;
        }
      }),
    ).then((entries) => {
      if (active) setThumbs((prev) => ({ ...prev, ...Object.fromEntries(entries) }));
    });
    return () => { active = false; };
  }, [records]);

  useEffect(() => {
    if (!preview && !confirmClear) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      // 确认框叠在放大浮层之上时，Esc 先关确认框
      if (confirmClear) setConfirmClear(false);
      else setPreview(null);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [preview, confirmClear]);

  const filtered = useMemo(
    () => records.filter((item) => item.appName.toLowerCase().includes(query.trim().toLowerCase())),
    [records, query],
  );

  async function removeOne(id: string) {
    try {
      setRecords(await deleteSnapshot(id));
      if (preview?.id === id) setPreview(null);
      notify("已删除该快照");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function removeAll() {
    setClearing(true);
    try {
      setRecords(await clearSnapshots());
      setThumbs({});
      setPreview(null);
      setConfirmClear(false);
      notify("已清空快照历史");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setClearing(false);
    }
  }

  async function extractText(id: string) {
    setOcrBusy(id);
    setOcrText("");
    try {
      setOcrText(await ocrSnapshot(id));
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setOcrBusy(null);
    }
  }

  return (
    <div className="snapshot-history-container">
      <div className="history-header-bar">
        <div className="history-title-group">
          <div className="history-heading-line">
            <h2 className="history-main-title">快照历史库</h2>
            <button
              className="history-folder-btn"
              onClick={() => void openFolder()}
              title="打开快照保存位置"
              aria-label="打开快照保存位置"
            >
              <FolderOpen size={14} />
            </button>
          </div>
          <p className="history-desc">窗口快照会自动存到本机并登记在这里，最多保留 200 条。</p>
        </div>

        <div className="history-actions-group">
          <button className="history-ghost-btn" onClick={() => void refresh()} title="重新读取">
            <RefreshCw size={13} />
            <span>刷新</span>
          </button>
          {records.length > 0 && (
            <button className="history-ghost-btn" onClick={() => setConfirmClear(true)} title="清空全部快照">
              <Trash2 size={13} />
              <span>清空</span>
            </button>
          )}
        </div>
      </div>

      <div className="history-filter-strip">
        <div className="history-search-box">
          <Search size={14} className="search-icon" />
          <input
            type="text"
            className="history-search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="按来源应用名称搜索…"
          />
          {query && <button className="clear-search-btn" onClick={() => setQuery("")}>清空</button>}
        </div>
        <div className="history-type-pills">
          <span className="history-pill-btn is-active">
            <span>全部</span>
            <span className="pill-count tabular-nums">{filtered.length}</span>
          </span>
        </div>
      </div>

      {loading ? (
        <LoadingState />
      ) : filtered.length > 0 ? (
        <div className="history-cards-grid">
          {filtered.map((item) => (
            <div key={item.id} className="history-card-item">
              <button
                type="button"
                className="card-preview-stage"
                onClick={() => { setPreview(item); setOcrText(""); }}
                title="放大预览"
              >
                {thumbs[item.id] && <img className="card-preview-art" src={thumbs[item.id]} alt="" />}
                <span className="stage-zoom-hint">
                  <Maximize2 size={14} />
                  <span>放大预览</span>
                </span>
                <div className="stage-app-chip">
                  <Camera size={12} />
                  <span>{item.appName}</span>
                </div>
                <span className="stage-dim-tag tabular-nums">{item.width} × {item.height}</span>
              </button>

              <div className="card-info-box">
                <div className="card-title-line" title={item.appName}>
                  <span className="card-title-text">{item.appName}</span>
                </div>
                <div className="card-meta-line">
                  <span className="meta-time"><Clock size={11} /><span>{formatWhen(item.createdAt)}</span></span>
                  <span className="meta-dot">·</span>
                  <span className="meta-size tabular-nums">{formatBytes(item.sizeBytes)}</span>
                  {/* 窄窗口下缩略图被隐藏，尺寸信息要在这里补上 */}
                  <span className="meta-dot compact-only">·</span>
                  <span className="meta-size tabular-nums compact-only">{item.width} × {item.height}</span>
                </div>
                <div className="card-bottom-actions">
                  {/* 同理，缩略图隐藏后需要另一个放大入口 */}
                  <button
                    className="card-action-btn preview compact-only"
                    onClick={() => { setPreview(item); setOcrText(""); }}
                    title="放大预览"
                  >
                    <Maximize2 size={12} />
                    <span>预览</span>
                  </button>
                  <button
                    className="card-action-btn copy"
                    onClick={() => void extractText(item.id)}
                    disabled={ocrBusy !== null}
                    title="用系统 OCR 提取这张图里的文字"
                  >
                    <ScanText size={12} />
                    <span>{ocrBusy === item.id ? "识别中…" : "提取文字"}</span>
                  </button>
                  <button className="card-action-btn delete" onClick={() => void removeOne(item.id)} title="删除此快照">
                    <Trash2 size={12} />
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="history-empty-view">
          <div className="empty-icon-circle"><Camera size={28} /></div>
          <h3 className="empty-title">{query ? "未找到相关快照" : "还没有快照"}</h3>
          <p className="empty-desc">
            {query ? "没有符合当前关键词的记录，试试换个应用名。" : "用快捷键或便携坞截一张窗口快照，这里就会出现记录。"}
          </p>
          {query && <button className="empty-reset-btn" onClick={() => setQuery("")}>清空搜索</button>}
        </div>
      )}

      {ocrText && (
        <div className="prompt-rule-editor-drawer">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">识别结果</span>
              <span className="pane-char-count tabular-nums">{ocrText.length} 字符</span>
            </div>
            <div className="pane-result-actions">
              <button className="result-tool-btn" onClick={() => void navigator.clipboard.writeText(ocrText)}>
                <ClipboardCopy size={12} />
                <span>复制</span>
              </button>
              <button className="result-tool-btn" onClick={() => setOcrText("")}>
                <X size={12} />
                <span>关闭</span>
              </button>
            </div>
          </div>
          <pre className="polish-output">{ocrText}</pre>
        </div>
      )}

      {confirmClear && (
        <div
          className="snapshot-lightbox-backdrop"
          role="dialog"
          aria-modal="true"
          aria-labelledby="confirm-clear-title"
          onClick={() => setConfirmClear(false)}
        >
          <div className="confirm-dialog-panel" onClick={(event) => event.stopPropagation()}>
            <div className="confirm-dialog-head">
              <span className="confirm-dialog-icon"><Trash2 size={16} /></span>
              <div>
                <h3 className="confirm-dialog-title" id="confirm-clear-title">清空快照历史</h3>
                <p className="confirm-dialog-desc">
                  将删除全部 {records.length} 条记录，本机上对应的图片文件也会一并删除，无法恢复。
                </p>
              </div>
            </div>

            <div className="confirm-dialog-actions">
              <button className="confirm-cancel-btn" onClick={() => setConfirmClear(false)} disabled={clearing}>
                取消
              </button>
              <button className="confirm-danger-btn" onClick={() => void removeAll()} disabled={clearing} autoFocus>
                <Trash2 size={13} />
                <span>{clearing ? "清空中…" : "确认清空"}</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {preview && (
        <div
          className="snapshot-lightbox-backdrop"
          role="dialog"
          aria-modal="true"
          aria-label={`${preview.appName} 放大预览`}
          onClick={() => setPreview(null)}
        >
          <div className="snapshot-lightbox-panel" onClick={(event) => event.stopPropagation()}>
            <div className="lightbox-header">
              <div className="lightbox-title-group">
                <span className="lightbox-type-tag">窗口快照</span>
                <span className="lightbox-title-text" title={preview.appName}>{preview.appName}</span>
              </div>
              <button className="lightbox-close-btn" onClick={() => setPreview(null)} title="关闭预览 (Esc)">
                <X size={16} />
              </button>
            </div>

            <div className="lightbox-stage">
              {thumbs[preview.id]
                ? <img className="lightbox-art" src={thumbs[preview.id]} alt="" style={{ objectFit: "contain" }} />
                : <div className="polish-placeholder"><span className="inline-spinner" />图片加载中…</div>}
            </div>

            <div className="lightbox-meta-row">
              <span className="lightbox-meta-item"><Camera size={12} /><span>{preview.appName}</span></span>
              <span className="lightbox-meta-item tabular-nums"><Maximize2 size={12} /><span>{preview.width} × {preview.height}</span></span>
              <span className="lightbox-meta-item tabular-nums"><HardDrive size={12} /><span>{formatBytes(preview.sizeBytes)}</span></span>
              <span className="lightbox-meta-item"><Clock size={12} /><span>{formatWhen(preview.createdAt)}</span></span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

function PreferencesPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
}) {
  const [caps, setCaps] = useState<PlatformCapabilities | null>(null);
  const [showModelSettings, setShowModelSettings] = useState(false);
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

  async function importShutterSound() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: "音效文件", extensions: ["mp3", "wav", "ogg", "m4a"] }] });
      if (typeof selected === "string") {
        await updatePrefs({ shutterSound: "custom", customSoundPath: selected }, "自定义音效已导入");
      }
    } catch {
      notify("当前环境不支持选择文件");
    }
  }

  const localCapabilities = caps
    ? [
        { label: "录制", ...caps.recording },
        { label: "OCR", ...caps.ocr },
        ...(caps.scrolling ? [{ label: "滚动长截图", ...caps.scrolling }] : []),
      ]
    : [];

  return (
    <div className="hub-preferences-page">
      <header className="hub-top-strip">
        <div className="hub-title-line">
          <h1 className="hub-heading">偏好设置</h1>
        </div>
      </header>

      <div className="hub-preferences-page-body">
        <section className="hub-section-block">
          <div className="section-label-bar">
            <span className="section-name">截屏与行为</span>
          </div>
        <div className="preferences-group-card">
          <div className="pref-item-row">
            <span className="pref-title">截屏音效</span>
            <SegGroup
              ariaLabel="截屏音效"
              value={settings.shutterSound}
              onChange={(value) => {
                if (value === "custom" && !settings.customSoundPath) {
                  void importShutterSound();
                  return;
                }
                void updatePrefs({ shutterSound: value });
              }}
              options={[
                { value: "crisp", label: "清脆" },
                { value: "soft", label: "轻柔" },
                { value: "none", label: "无音效" },
                { value: "custom", label: "自定义" },
              ]}
            />
          </div>
          {settings.shutterSound !== "none" && (
            <div className="pref-item-row no-desc">
              <span className="history-sub">
                {settings.shutterSound === "custom"
                  ? fileNameOf(settings.customSoundPath) || "未选择音效文件"
                  : "内置提示音"}
              </span>
              <div className="folder-picker-box">
                <button
                  type="button"
                  className="folder-action-btn"
                  onClick={() =>
                    previewHintSound(
                      settings.shutterSound === "soft" ? "soft" : "crisp",
                      settings.shutterSound === "custom" ? settings.customSoundPath : null,
                      80,
                    )
                  }
                >
                  <Play size={12} />
                  试听
                </button>
                <button type="button" className="folder-action-btn" onClick={() => void importShutterSound()}>
                  <Upload size={12} />
                  导入
                </button>
              </div>
            </div>
          )}
          <div className="pref-item-row">
            <span className="pref-title">截屏提示闪烁</span>
            <PrefToggle
              label="截屏提示闪烁"
              value={settings.flashOnCapture}
              onChange={(value) => void updatePrefs({ flashOnCapture: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">复制后隐藏主窗口</span>
            <PrefToggle
              label="复制后隐藏主窗口"
              value={settings.hideAfterCopy}
              onChange={(value) => void updatePrefs({ hideAfterCopy: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">自动写入本地文件</span>
            <PrefToggle
              label="自动写入本地文件"
              value={settings.autoSaveLocal}
              onChange={(value) => void updatePrefs({ autoSaveLocal: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">开机静默自启动</span>
            <PrefToggle
              label="开机静默自启动"
              value={settings.launchOnBoot}
              disabled={caps ? !caps.autostart.available : false}
              onChange={(value) => void updatePrefs({ launchOnBoot: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">截屏包含鼠标光标</span>
            <PrefToggle
              label="截屏包含鼠标光标"
              value={settings.includeCursor}
              disabled={caps ? !caps.includeCursor.available : false}
              onChange={(value) => void updatePrefs({ includeCursor: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">截图完成后动作</span>
            <SegGroup
              ariaLabel="截图完成后动作"
              value={settings.afterCapture}
              onChange={(value) => void updatePrefs({ afterCapture: value })}
              options={[
                { value: "clipboard", label: "复制剪贴板" },
                { value: "annotate", label: "打开标注" },
                { value: "saveas", label: "另存为…" },
              ]}
            />
          </div>
        </div>
        </section>

      {caps && (
        <section className="hub-section-block">
          <div className="section-label-bar">
            <span className="section-name">本机能力</span>
          </div>
          <div className="local-capabilities-card">
            <div className="capability-platform-row">
              <span className="capability-platform-name">{caps.os}</span>
              <span className="capability-platform-separator" aria-hidden="true">·</span>
              <span className="capability-platform-name">{caps.displayServer}</span>
            </div>
            <div className="capability-list">
              {localCapabilities.map((capability) => (
                <div className="capability-row" key={capability.label}>
                  <span className="capability-name">{capability.label}</span>
                  <span className={`capability-status ${capability.available ? "is-available" : "is-unavailable"}`}>
                    <span className="capability-status-dot" aria-hidden="true" />
                    {capability.available ? "可用" : "不可用"}
                  </span>
                  <span className="capability-detail">{capability.detail}</span>
                </div>
              ))}
              <div className="capability-model-row">
                <span className="capability-name">模型</span>
                <span className={`capability-model-name ${settings.model ? "" : "is-empty"}`} title={settings.model || "未配置模型"}>
                  {settings.model || "未配置模型"}
                </span>
                <button className="capability-edit-btn" type="button" onClick={() => setShowModelSettings(true)}>
                  编辑
                </button>
              </div>
            </div>
          </div>
        </section>
      )}
      </div>
      {showModelSettings && (
        <ModelSettingsDialog
          settings={settings}
          onSaved={onSaved}
          notify={notify}
          onClose={() => setShowModelSettings(false)}
        />
      )}
    </div>
  );
}
