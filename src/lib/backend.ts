import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  CapturableWindow,
  OcrCapability,
  PlatformCapabilities,
  PreviousApp,
  RecordingStatus,
  Settings,
  ShortcutBinding,
  SnapshotRecord,
} from "../types";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const demoSettings: Settings = {
  baseUrl: "",
  model: "",
  hasApiKey: false,
  templates: [
    {
      id: "builtin-default",
      name: "清晰、可执行",
      builtin: true,
      content:
        "你是提示词改写专家。保持原意不变，将用户草稿整理为清晰、完整、可执行的指令。只输出改写后的提示词。",
    },
  ],
  activeTemplateId: "builtin-default",
  selectedAppearanceId: "app-icon",
  petAssets: [],
  shortcuts: [
    { action: "snapshot", accelerator: null },
    { action: "fullscreen", accelerator: null },
    { action: "scrolling", accelerator: null },
    { action: "record", accelerator: null },
    { action: "recordings", accelerator: null },
    { action: "polish", accelerator: null },
    { action: "ocr", accelerator: null },
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

export async function loadSettings(): Promise<Settings> {
  if (!inTauri) return structuredClone(demoSettings);
  return invoke<Settings>("load_settings");
}

export async function savePromptSettings(input: {
  baseUrl: string;
  model: string;
  apiKey: string | null;
  activeTemplateId: string;
  templates: Settings["templates"];
}): Promise<Settings> {
  if (!inTauri) return { ...structuredClone(demoSettings), ...input };
  return invoke<Settings>("save_prompt_settings", { input });
}

export async function fetchModels(baseUrl: string, apiKey: string | null): Promise<string[]> {
  if (!inTauri) return ["gpt-4.1", "gpt-4.1-mini", "gpt-4o-mini"];
  return invoke<string[]>("fetch_models", { baseUrl, apiKey });
}

export async function selectPetAppearance(id: string): Promise<Settings> {
  if (!inTauri) return { ...structuredClone(demoSettings), selectedAppearanceId: id };
  return invoke<Settings>("select_pet_appearance", { id });
}

export async function addPetAsset(path: string): Promise<Settings> {
  if (!inTauri) {
    const asset = { id: `pet-${Date.now()}`, name: path.split(/[\\/]/).pop() ?? "桌宠", path, entry: "", animations: [] };
    return { ...structuredClone(demoSettings), selectedAppearanceId: asset.id, petAssets: [asset] };
  }
  return invoke<Settings>("add_pet_asset", { path });
}

export async function deletePetAsset(id: string): Promise<Settings> {
  if (!inTauri) {
    return { ...structuredClone(demoSettings), selectedAppearanceId: "app-icon", petAssets: [] };
  }
  return invoke<Settings>("delete_pet_asset", { id });
}

export async function getPetAssetDataUrl(id: string, entry: string | null = null): Promise<string> {
  if (!inTauri) return "";
  return invoke<string>("get_pet_asset_data_url", { id, entry });
}

export async function saveShortcuts(shortcuts: ShortcutBinding[]): Promise<Settings> {
  if (!inTauri) return { ...structuredClone(demoSettings), shortcuts };
  return invoke<Settings>("save_shortcuts", { shortcuts });
}

export async function savePreferences(prefs: Partial<Settings>): Promise<Settings> {
  if (!inTauri) return { ...structuredClone(demoSettings), ...prefs };
  return invoke<Settings>("save_preferences", { prefs });
}

export async function getPreviousApp(): Promise<PreviousApp> {
  if (!inTauri) {
    return { id: null, name: "上一个应用", title: "等待切换应用", iconDataUrl: null };
  }
  return invoke<PreviousApp>("get_previous_app");
}

export async function listWindows(): Promise<CapturableWindow[]> {
  if (!inTauri) return [];
  return invoke<CapturableWindow[]>("list_capturable_windows");
}

export async function captureWindow(id?: number): Promise<string> {
  if (!inTauri) return "预览模式下不会读取系统窗口";
  return invoke<string>("capture_window", { id: id ?? null });
}

/** `targetId` 指定录哪个窗口；不给就沿用「上一个应用」。停止录制时不需要它。 */
export async function toggleRecording(targetId?: number): Promise<RecordingStatus> {
  if (!inTauri) return { active: false, target: null, startedAt: null };
  return invoke<RecordingStatus>("toggle_recording", { targetId: targetId ?? null });
}

export async function getRecordingStatus(): Promise<RecordingStatus> {
  if (!inTauri) return { active: false, target: null, startedAt: null };
  return invoke<RecordingStatus>("get_recording_status");
}

export async function polishClipboard(): Promise<string> {
  if (!inTauri) return "预览模式下不会访问剪贴板";
  return invoke<string>("polish_clipboard");
}

export async function polishText(text: string): Promise<string> {
  if (!inTauri) {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return `【预览模式：未调用模型】\n\n${text.trim()}`;
  }
  return invoke<string>("polish_text", { text });
}

export async function showQuickMenu(x: number, y: number): Promise<void> {
  if (!inTauri) return;
  await invoke("show_quick_menu", { x, y });
}

export async function hideQuickMenu(): Promise<void> {
  if (!inTauri) return;
  await invoke("hide_quick_menu");
}

export async function setQuickMenuExpanded(expanded: boolean): Promise<void> {
  if (!inTauri) return;
  await invoke("set_quick_menu_expanded", { expanded });
}

export function onSettingsChanged(callback: (settings: Settings) => void) {
  if (!inTauri) return Promise.resolve(() => undefined);
  return listen<Settings>("settings-changed", ({ payload }) => callback(payload));
}

export function onPreviousAppChanged(callback: (app: PreviousApp) => void) {
  if (!inTauri) return Promise.resolve(() => undefined);
  return listen<PreviousApp>("previous-app-changed", ({ payload }) => callback(payload));
}

export type CaptureFeedback = {
  flash: boolean;
  shutterSound: Settings["shutterSound"];
  customSoundPath: string | null;
};

export function onCaptureFeedback(callback: (payload: CaptureFeedback) => void) {
  if (!inTauri) return Promise.resolve(() => undefined);
  return listen<CaptureFeedback>("capture-feedback", ({ payload }) => callback(payload));
}

export async function listSnapshots(): Promise<SnapshotRecord[]> {
  if (!inTauri) return [];
  return invoke<SnapshotRecord[]>("list_snapshots");
}

export async function openSnapshotsDir(): Promise<string> {
  if (!inTauri) return "预览模式下不会打开文件管理器";
  return invoke<string>("open_snapshots_dir");
}

export async function openRecordingsDir(): Promise<string> {
  if (!inTauri) return "预览模式下不会打开文件管理器";
  return invoke<string>("open_recordings_dir");
}

export async function getSnapshotDataUrl(id: string): Promise<string> {
  if (!inTauri) return "";
  return invoke<string>("get_snapshot_data_url", { id });
}

export async function deleteSnapshot(id: string): Promise<SnapshotRecord[]> {
  if (!inTauri) return [];
  return invoke<SnapshotRecord[]>("delete_snapshot", { id });
}

export async function clearSnapshots(): Promise<SnapshotRecord[]> {
  if (!inTauri) return [];
  return invoke<SnapshotRecord[]>("clear_snapshots");
}

export async function ocrCapability(): Promise<OcrCapability> {
  if (!inTauri) return { available: false, language: null, detail: "预览模式下不调用系统 OCR" };
  return invoke<OcrCapability>("ocr_capability");
}

export async function ocrSnapshot(id: string): Promise<string> {
  if (!inTauri) return "";
  return invoke<string>("ocr_snapshot", { id });
}

export async function ocrClipboard(): Promise<string> {
  if (!inTauri) return "";
  return invoke<string>("ocr_clipboard");
}

export async function platformCapabilities(): Promise<PlatformCapabilities> {
  if (!inTauri) {
    return {
      os: "preview",
      displayServer: "n/a",
      recording: { available: false, detail: "预览模式" },
      ocr: { available: false, detail: "预览模式" },
      autostart: { available: false, detail: "预览模式" },
      scrolling: { available: false, detail: "预览模式" },
      includeCursor: { available: false, detail: "预览模式" },
      trayNote: "",
      notes: [],
    };
  }
  return invoke<PlatformCapabilities>("platform_capabilities");
}
