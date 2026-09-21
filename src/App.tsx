import { useEffect, useMemo, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  AppWindow,
  Check,
  ChevronDown,
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
  Moon,
  Sun,
  Wand2,
  ClipboardCopy,
  Camera,
  Maximize2,
  Layers,
  Cpu,
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
  Volume2,
} from "lucide-react";
import {
  addPetAsset,
  deletePetAsset,
  fetchModels,
  getPetAssetDataUrl,
  loadSettings,
  onSettingsChanged,
  clearSnapshots,
  deleteSnapshot,
  getSnapshotDataUrl,
  listSnapshots,
  ocrCapability,
  ocrClipboard,
  ocrSnapshot,
  openSnapshotsDir,
  polishText,
  savePreferences,
  savePromptSettings,
  saveShortcuts,
  selectPetAppearance,
} from "./lib/backend";
import { applyGlobalShortcuts } from "./lib/shortcuts";
import { renderPetMedia } from "./windows/PetWindow";
import { KbdBadge } from "./components/ui/KbdBadge";
import { applyTheme, persistTheme, readTheme, type ThemeMode } from "./lib/theme";
import type {
  NavPage,
  OcrCapability,
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
      { id: "ocr", label: "文字识别", icon: Cpu },
      { id: "models", label: "模型设置", icon: HardDrive },
    ],
  },
];

const actionLabels: Record<ShortcutAction, { name: string; tag?: string }> = {
  snapshot: { name: "窗口快照", tag: "前台窗口" },
  region: { name: "区域截图", tag: "矩形框选" },
  fullscreen: { name: "全屏快照", tag: "主显示器" },
  scrolling: { name: "滚动长截图", tag: "整页截取" },
  record: { name: "窗口录制", tag: "MP4" },
  polish: { name: "润色 Prompt", tag: "剪贴板" },
  ocr: { name: "提取文字 (OCR)", tag: "离线识别" },
};

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
    { action: "region", accelerator: "Alt+Shift+A" },
    { action: "fullscreen", accelerator: "Alt+Shift+F" },
    { action: "scrolling", accelerator: null },
    { action: "record", accelerator: null },
    { action: "polish", accelerator: "Alt+Shift+P" },
    { action: "ocr", accelerator: "Alt+Shift+O" },
  ],
  clipboardAutoClear: "60s",
  snapshotFormat: "png",
  saveDir: "",
  customTheme: null,
  shutterSound: "crisp",
  customSoundPath: null,
  flashOnCapture: true,
  hideAfterCopy: false,
  autoSaveLocal: true,
  launchOnBoot: false,
  includeCursor: false,
  trayDoubleClick: "workbench",
  afterCapture: "clipboard",
  petSoundEnabled: true,
  petSoundVolume: 65,
  petCustomSoundPath: null,
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
  const [theme, setTheme] = useState<ThemeMode>(() => readTheme());

  function toggleTheme() {
    const next: ThemeMode = theme === "dark" ? "light" : "dark";
    setTheme(next);
    applyTheme(next);
    persistTheme(next);
  }

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
    if (loading) return;
    void applyGlobalShortcuts(settings.shortcuts).catch(() => {
      notify("有快捷键已被其他应用占用，请重新设置");
    });
  }, [loading, settings.shortcuts]);

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
    if (shownPage === "models") {
      return <ModelsPage settings={settings} onSaved={setSettings} notify={notify} />;
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
    return <ShortcutsPage settings={settings} onSaved={setSettings} notify={notify} />;
  }, [shownPage, settings]);

  const appWindow = "__TAURI_INTERNALS__" in window ? getCurrentWindow() : null;

  return (
    <div className="settings-window-frame">
      <div
        className="window-titlebar"
        data-tauri-drag-region
        onDoubleClick={() => void appWindow?.toggleMaximize()}
      >
        <div className="titlebar-left">
          <div className="window-traffic-lights">
            <button className="traffic-dot" onClick={() => void appWindow?.close()} title="关闭" />
            <button className="traffic-dot" onClick={() => void appWindow?.minimize()} title="最小化" />
            <button className="traffic-dot" onClick={() => void appWindow?.toggleMaximize()} title="最大化 / 还原" />
          </div>
          <div className="window-title-chip">
            <span className="title-name">应用快照</span>
          </div>
        </div>

        <div className="titlebar-right">
          <button className="theme-switch-btn" onClick={toggleTheme} title="切换主题">
            {theme === "dark" ? <Sun size={13} /> : <Moon size={13} />}
            <span>{theme === "dark" ? "深色" : "浅色"}</span>
          </button>
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

function ModelsPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
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
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="prompt-lab-workspace-container">
      <div className="prompt-endpoint-drawer" role="region" aria-label="模型与端点设置">
        <div className="drawer-header-row">
          <div className="drawer-title-group">
            <span className="drawer-title">模型端点与凭证</span>
            <span className="honest-hint-tag">
              {serviceReady ? "已配置" : "未配置"} · API Key 存于系统凭据管理器
            </span>
          </div>
        </div>

        <div className="drawer-inputs-grid">
          <div className="drawer-input-item">
            <span className="drawer-input-label">接口端点 (Base URL)</span>
            <input
              type="text"
              className="drawer-text-input"
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
              placeholder="https://api.example.com/v1"
            />
          </div>

          <div className="drawer-input-item">
            <span className="drawer-input-label">访问密钥 (API Key)</span>
            <input
              type="password"
              className="drawer-text-input"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={settings.hasApiKey ? "已保存，留空保持不变" : "可选"}
            />
          </div>

          <div className="drawer-input-item">
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
          </div>
        </div>

        <div className="drawer-footer-row">
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
    </div>
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

  const [draft, setDraft] = useState("");
  const [result, setResult] = useState("");
  const [polishing, setPolishing] = useState(false);
  const [copied, setCopied] = useState(false);
  const [autoCopy, setAutoCopy] = useState(false);

  useEffect(() => {
    setTemplates(settings.templates);
    setActiveId(settings.activeTemplateId);
  }, [settings]);

  const active = templates.find((item) => item.id === activeId) ?? templates[0];
  const serviceReady = Boolean(settings.baseUrl && settings.model);

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
    if (!draft.trim() || polishing) return;
    setPolishing(true);
    setResult("");
    try {
      // 用的是当前已保存的提示词；改了规则要先保存才会生效
      const text = await polishText(draft);
      setResult(text);
      if (autoCopy) {
        await navigator.clipboard.writeText(text).catch(() => notify("自动复制失败"));
      }
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setPolishing(false);
    }
  }

  async function copyResult() {
    try {
      await navigator.clipboard.writeText(result);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      notify("复制失败，请手动选中复制");
    }
  }

  async function pasteDraft() {
    try {
      setDraft(await navigator.clipboard.readText());
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

      <div className="prompt-main-split-workbench">
        <div className="workbench-pane draft-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">输入草稿</span>
              <span className="pane-char-count tabular-nums">{draft.length} 字符</span>
            </div>
            <div className="pane-quick-samples">
              <button className="paste-clip-btn" onClick={() => void pasteDraft()} title="从系统剪贴板填入草稿">
                <ClipboardCopy size={13} />
                <span>粘贴剪贴板</span>
              </button>
            </div>
          </div>

          <div className="pane-textarea-wrap">
            <textarea
              className="draft-input-textarea"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
                  event.preventDefault();
                  void runPolish();
                }
              }}
              placeholder="把想让编程助手做的事写在这里，不用讲究措辞… (按 Ctrl+Enter 立即生成)"
              spellCheck={false}
            />
          </div>

          <div className="pane-action-bar">
            <span className="action-kbd-hint">
              {serviceReady ? <>按 <kbd>Ctrl</kbd> + <kbd>Enter</kbd> 触发生成</> : "请先到「模型设置」填写 Base URL 与模型"}
            </span>
            <div className="pane-action-buttons">
              <button
                className="main-action-btn generate"
                onClick={() => void runPolish()}
                disabled={polishing || !draft.trim() || !serviceReady}
                title="按当前规则改写草稿"
              >
                <Send size={13} />
                <span>{polishing ? "生成中…" : "生成 (Ctrl+↵)"}</span>
              </button>
            </div>
          </div>
        </div>

        <div className="workbench-pane result-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">改写结果</span>
              {result && <span className="pane-char-count tabular-nums">{result.length} 字符</span>}
            </div>
            <div className="pane-result-actions" role="toolbar" aria-label="生成结果操作">
              {result && (
                <div className="result-action-button-group">
                  <button className={`result-tool-btn ${copied ? "copied" : ""}`} onClick={() => void copyResult()} title="复制生成结果">
                    {copied ? <Check size={12} /> : <ClipboardCopy size={12} />}
                    <span>{copied ? "已复制" : "复制结果"}</span>
                  </button>
                  <button className="result-tool-btn" onClick={() => void runPolish()} disabled={polishing} title="重新生成">
                    <RefreshCw size={12} />
                    <span>重新生成</span>
                  </button>
                </div>
              )}
            </div>
          </div>

          <div className="pane-textarea-wrap">
            {polishing ? (
              <div className="polish-placeholder"><span className="inline-spinner" />正在调用模型，最长等待 180 秒…</div>
            ) : result ? (
              <pre className="polish-output">{result}</pre>
            ) : (
              <div className="polish-placeholder">结果会显示在这里，也可以直接复制走。</div>
            )}
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
      filters: [{ name: "桌宠压缩包", extensions: ["zip"] }],
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

  // 音量拖动过程中只动本地 state，松手/失焦时才写盘，避免一次拖动刷几十次设置文件
  const [petVolume, setPetVolume] = useState(settings.petSoundVolume);
  useEffect(() => setPetVolume(settings.petSoundVolume), [settings.petSoundVolume]);

  async function updatePrefs(patch: Partial<Settings>, message?: string) {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  function commitPetVolume(value: number) {
    setPetVolume(value);
    void updatePrefs({ petSoundVolume: value });
  }

  async function importPetSound() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: "音效文件", extensions: ["mp3", "wav", "ogg", "m4a"] }] });
      if (typeof selected === "string") {
        await updatePrefs({ petCustomSoundPath: selected }, "交互提示音已导入");
      }
    } catch {
      notify("当前环境不支持选择文件");
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
            <Volume2 size={14} />
            <span>桌面行为特性</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">交互提示音</span>
            <PrefToggle
              label="交互提示音"
              value={settings.petSoundEnabled}
              onChange={(value) => void updatePrefs({ petSoundEnabled: value })}
            />
          </div>
          {settings.petSoundEnabled && (
            <div className="volume-slider-subrow">
              <span className="volume-label">提示音量</span>
              <input
                type="range"
                className="pet-volume-slider"
                min={0}
                max={100}
                step={1}
                value={petVolume}
                aria-label="提示音量"
                onChange={(event) => setPetVolume(Number(event.target.value))}
                onPointerUp={() => commitPetVolume(petVolume)}
                onKeyUp={() => commitPetVolume(petVolume)}
                onBlur={() => commitPetVolume(petVolume)}
              />
              <span className="volume-label tabular-nums">{petVolume}%</span>
            </div>
          )}
          <div className="behavior-row no-desc">
            <span className="b-title">提示音效</span>
            <SegGroup
              ariaLabel="提示音效"
              value={settings.petCustomSoundPath ? "custom" : "default"}
              onChange={(value) => {
                if (value === "custom") {
                  void importPetSound();
                } else {
                  void updatePrefs({ petCustomSoundPath: null }, "已恢复默认提示音");
                }
              }}
              options={[
                { value: "default", label: "默认" },
                { value: "custom", label: "自定义" },
              ]}
            />
          </div>
          {settings.petCustomSoundPath && (
            <div className="behavior-row no-desc">
              <span className="history-sub" title={settings.petCustomSoundPath}>
                {fileNameOf(settings.petCustomSoundPath)}
              </span>
              <div className="folder-picker-box">
                <button
                  type="button"
                  className="folder-action-btn"
                  onClick={() => previewHintSound("pet", settings.petCustomSoundPath, petVolume)}
                >
                  <Play size={12} />
                  试听
                </button>
                <button type="button" className="folder-action-btn" onClick={() => void importPetSound()}>
                  <Upload size={12} />
                  重新导入
                </button>
              </div>
            </div>
          )}
        </div>

        <div className="companion-behavior-settings">
          <div className="behavior-header">
            <SlidersHorizontal size={14} />
            <span>素材包要求</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">支持格式</span>
            <span className="history-sub">GIF / WebP / APNG / PNG，MP4 / WebM</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">视频编码</span>
            <span className="history-sub">须为 H.264 / HEVC / AV1 / VP9，mp4v 会被拒绝</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">默认动作</span>
            <span className="history-sub">文件名含 idle 的会排在最前</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">体积上限</span>
            <span className="history-sub">整包 100MB，单个文件 50MB</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">注意</span>
            <span className="history-sub">压缩包不会解压，移走原文件形象会失效</span>
          </div>
        </div>
      </section>
    </div>
  );
}

/** 偏好设置共用的开关控件，样式来自 prototype-port 的 toggle-switch-btn */
function PrefToggle({ value, onChange, label }: { value: boolean; onChange: (next: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={value}
      aria-label={label}
      className={`toggle-switch-btn ${value ? "on" : ""}`}
      onClick={() => onChange(!value)}
    >
      <span className="toggle-thumb" />
    </button>
  );
}

/** 偏好设置共用的分段选择器，样式来自 prototype-port 的 segmented-track */
function SegGroup<T extends string>({
  value,
  options,
  onChange,
  ariaLabel,
}: {
  value: T;
  options: Array<{ value: T; label: string }>;
  onChange: (next: T) => void;
  ariaLabel: string;
}) {
  return (
    <div className="segmented-track" role="radiogroup" aria-label={ariaLabel}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={value === option.value}
          className={`segmented-item-btn ${value === option.value ? "is-active" : ""}`}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

/** 试听提示音：有自定义文件就播文件，否则用 WebAudio 合成一声短促提示 */
function previewHintSound(kind: "crisp" | "soft" | "pet", customPath: string | null, volume: number) {
  const gain = Math.min(1, Math.max(0, volume / 100));
  try {
    if (customPath) {
      const inTauri = "__TAURI_INTERNALS__" in window;
      const audio = new Audio(inTauri ? convertFileSrc(customPath) : customPath);
      audio.volume = gain;
      void audio.play();
      return;
    }
    const AudioContextCtor =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!AudioContextCtor) return;
    const ctx = new AudioContextCtor();
    const osc = ctx.createOscillator();
    const amp = ctx.createGain();
    const now = ctx.currentTime;
    osc.frequency.value = kind === "soft" ? 520 : kind === "pet" ? 660 : 880;
    osc.type = kind === "soft" ? "sine" : "triangle";
    amp.gain.setValueAtTime(0.0001, now);
    amp.gain.exponentialRampToValueAtTime(Math.max(0.0001, gain * 0.35), now + 0.015);
    amp.gain.exponentialRampToValueAtTime(0.0001, now + (kind === "soft" ? 0.35 : 0.18));
    osc.connect(amp).connect(ctx.destination);
    osc.start(now);
    osc.stop(now + 0.4);
  } catch {
    // 试听失败不打断设置流程
  }
}

function fileNameOf(path: string | null): string {
  if (!path) return "";
  return path.split(/[\\/]/).pop() ?? path;
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
    if (event.key === "Escape") {
      setRecording(null);
      return;
    }
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

  const [showMorePrefs, setShowMorePrefs] = useState(true);

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

  async function importTheme() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: "主题文件", extensions: ["json", "css"] }] });
      if (typeof selected === "string") await updatePrefs({ customTheme: selected }, "主题已导入");
    } catch {
      notify("当前环境不支持选择文件");
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

  const renderRow = (binding: ShortcutBinding) => {
    const isRecording = recording === binding.action;
    return (
      <div
        key={binding.action}
        role="button"
        tabIndex={0}
        aria-label={`快捷键：${actionLabels[binding.action].name}，当前按键：${binding.accelerator || "未设置"}`}
        className={`shortcut-interactive-row compact-row ${isRecording ? "is-recording-mode" : ""}`}
        onClick={() => setRecording(binding.action)}
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
              : binding.action === "region" ? <Maximize2 size={15} />
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
        </div>

        <div className="action-keycap-col">
          {isRecording ? (
            <div className="recording-active-capsule">
              <span className="pulse-dot-recording" />
              <span className="recording-prompt-text">按下新组合键…</span>
              <button
                className="cancel-record-pill-btn"
                onClick={(event) => { event.stopPropagation(); setRecording(null); }}
                title="取消录制"
              >
                Esc 取消
              </button>
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

  return (
    <div className="shortcut-hub-unified-layout">
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
              <span className="section-count tabular-nums">{shortcuts.length} 项</span>
            </div>
            <div className="shortcuts-list-table">
              {shortcuts.map(renderRow)}
            </div>
          </section>
        </div>

        <div className="hub-preferences-col">
          <div className="preferences-group-card">
            <div className="section-label-bar">
              <span className="section-name">剪贴板与保存</span>
            </div>
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
              <span className="pref-title">本地保存目录</span>
              <div className="folder-picker-box">
                <span className="folder-path-text" title={settings.saveDir || undefined}>
                  {settings.saveDir || "默认图片目录"}
                </span>
                <button type="button" className="folder-action-btn" onClick={() => void chooseSaveDir()}>
                  <FolderOpen size={12} />
                  更改
                </button>
              </div>
            </div>
            <div className="pref-item-row folder-row">
              <span className="pref-title">界面主题</span>
              <div className="folder-picker-box">
                <span className="folder-path-text" title={settings.customTheme ?? undefined}>
                  {settings.customTheme ? fileNameOf(settings.customTheme) : "默认主题"}
                </span>
                <button type="button" className="folder-action-btn" onClick={() => void importTheme()}>
                  <Palette size={12} />
                  导入主题
                </button>
                {settings.customTheme && (
                  <button
                    type="button"
                    className="folder-action-btn"
                    onClick={() => void updatePrefs({ customTheme: null }, "已恢复默认主题")}
                  >
                    恢复默认
                  </button>
                )}
              </div>
            </div>
          </div>

          <div className="preferences-group-card">
            <div className="section-label-bar">
              <span className="section-name">更多偏好设置</span>
              <button
                type="button"
                className="section-toggle-icon-btn"
                aria-label={showMorePrefs ? "收起更多偏好" : "展开更多偏好"}
                aria-expanded={showMorePrefs}
                onClick={() => setShowMorePrefs((value) => !value)}
              >
                <ChevronDown
                  size={14}
                  style={{ transform: showMorePrefs ? "rotate(180deg)" : undefined, transition: "transform 120ms" }}
                />
              </button>
            </div>
            {showMorePrefs && (
              <>
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
                    onChange={(value) => void updatePrefs({ launchOnBoot: value })}
                  />
                </div>
                <div className="pref-item-row">
                  <span className="pref-title">截屏包含鼠标光标</span>
                  <PrefToggle
                    label="截屏包含鼠标光标"
                    value={settings.includeCursor}
                    onChange={(value) => void updatePrefs({ includeCursor: value })}
                  />
                </div>
                <div className="pref-item-row">
                  <span className="pref-title">双击托盘图标</span>
                  <SegGroup
                    ariaLabel="双击托盘图标"
                    value={settings.trayDoubleClick}
                    onChange={(value) => void updatePrefs({ trayDoubleClick: value })}
                    options={[
                      { value: "workbench", label: "打开工作台" },
                      { value: "snapshot", label: "立即快照" },
                      { value: "dock", label: "灵动坞" },
                    ]}
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
              </>
            )}
          </div>
        </div>
      </div>

      <div className="hub-bottom-status-strip">
        <span className="history-sub">修改后需要保存才会写入系统</span>
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

function normalizeKey(key: string) {
  if (key === " ") return "Space";
  if (key === "ArrowUp") return "Up";
  if (key === "ArrowDown") return "Down";
  if (key === "ArrowLeft") return "Left";
  if (key === "ArrowRight") return "Right";
  return key.length === 1 ? key.toUpperCase() : key;
}

function formatShortcut(value: string | null) {
  if (!value) return "未设置";
  return value
    .replace("CommandOrControl", navigator.platform.includes("Mac") ? "⌘" : "Ctrl")
    .replaceAll("+", "  +  ");
}


function formatBytes(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1024))} KB`;
}

function formatWhen(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return "刚刚";
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)} 小时前`;
  return new Date(timestamp).toLocaleString();
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

function OcrPage({ notify }: { notify: (message: string) => void }) {
  const [capability, setCapability] = useState<OcrCapability | null>(null);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    void ocrCapability().then(setCapability).catch(() => setCapability(null));
  }, []);

  async function runOcr() {
    setBusy(true);
    setText("");
    try {
      setText(await ocrClipboard());
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function copyText() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      notify("复制失败，请手动选中复制");
    }
  }

  return (
    <div className="prompt-lab-workspace-container">
      <div className="prompt-top-control-bus">
        <div className="control-bus-left">
          <span className="bus-label">文字识别</span>
          <span className={`bus-model-pill ${capability?.available ? "is-active" : ""}`}>
            <span className="model-dot" />
            <span className="model-text">{capability?.available ? "引擎可用" : "引擎不可用"}</span>
          </span>
        </div>
        <div className="control-bus-right">
          <button className="bus-action-btn" onClick={() => void runOcr()} disabled={busy || !capability?.available}>
            <ScanText size={13} />
            <span>{busy ? "识别中…" : "识别剪贴板图片"}</span>
          </button>
        </div>
      </div>

      <div className="prompt-main-split-workbench">
        <div className="workbench-pane draft-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">引擎状态</span>
            </div>
          </div>
          <div className="companion-behavior-settings">
            <div className="behavior-row no-desc">
              <span className="b-title">识别引擎</span>
              <span className="history-sub">{capability?.detail ?? "正在检测…"}</span>
            </div>
            <div className="behavior-row no-desc">
              <span className="b-title">隐私</span>
              <span className="history-sub">调用系统本地引擎，图片不出本机</span>
            </div>
            <div className="behavior-row no-desc">
              <span className="b-title">用法</span>
              <span className="history-sub">先截图或复制一张图片，再点右上角按钮</span>
            </div>
            <div className="behavior-row no-desc">
              <span className="b-title">历史库</span>
              <span className="history-sub">「快照历史」里每张快照也可以单独提取文字</span>
            </div>
          </div>
        </div>

        <div className="workbench-pane result-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">识别结果</span>
              {text && <span className="pane-char-count tabular-nums">{text.length} 字符</span>}
            </div>
            <div className="pane-result-actions">
              {text && (
                <div className="result-action-button-group">
                  <button className={`result-tool-btn ${copied ? "copied" : ""}`} onClick={() => void copyText()}>
                    {copied ? <Check size={12} /> : <ClipboardCopy size={12} />}
                    <span>{copied ? "已复制" : "复制结果"}</span>
                  </button>
                </div>
              )}
            </div>
          </div>

          <div className="pane-textarea-wrap">
            {busy ? (
              <div className="polish-placeholder"><span className="inline-spinner" />正在识别…</div>
            ) : text ? (
              <pre className="polish-output">{text}</pre>
            ) : (
              <div className="polish-placeholder">识别结果会显示在这里。</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
