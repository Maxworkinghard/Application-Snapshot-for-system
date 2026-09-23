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

// 前端调用后端的全部入口。浏览器预览时的假数据不在这里，见 preview.ts。

export const loadSettings = () => invoke<Settings>("load_settings");

export const savePromptSettings = (input: {
  baseUrl: string;
  model: string;
  apiKey: string | null;
  activeTemplateId: string;
  templates: Settings["templates"];
}) => invoke<Settings>("save_prompt_settings", { input });

export const fetchModels = (baseUrl: string, apiKey: string | null) =>
  invoke<string[]>("fetch_models", { baseUrl, apiKey });

export const selectPetAppearance = (id: string) => invoke<Settings>("select_pet_appearance", { id });

export const addPetAsset = (path: string) => invoke<Settings>("add_pet_asset", { path });

export const deletePetAsset = (id: string) => invoke<Settings>("delete_pet_asset", { id });

export const getPetAssetDataUrl = (id: string, entry: string | null = null) =>
  invoke<string>("get_pet_asset_data_url", { id, entry });

export const saveShortcuts = (shortcuts: ShortcutBinding[]) =>
  invoke<Settings>("save_shortcuts", { shortcuts });

export const savePreferences = (prefs: Partial<Settings>) =>
  invoke<Settings>("save_preferences", { prefs });

export const getPreviousApp = () => invoke<PreviousApp>("get_previous_app");

export const listWindows = () => invoke<CapturableWindow[]>("list_capturable_windows");

export const captureWindow = (id?: number) => invoke<string>("capture_window", { id: id ?? null });

/** `targetId` 指定录哪个窗口；不给就沿用「上一个应用」。停止录制时不需要它。 */
export const toggleRecording = (targetId?: number) =>
  invoke<RecordingStatus>("toggle_recording", { targetId: targetId ?? null });

export const getRecordingStatus = () => invoke<RecordingStatus>("get_recording_status");

export const polishClipboard = () => invoke<string>("polish_clipboard");

export const polishText = (text: string) => invoke<string>("polish_text", { text });

export const showQuickMenu = (x: number, y: number) => invoke<void>("show_quick_menu", { x, y });

export const hideQuickMenu = () => invoke<void>("hide_quick_menu");

export const setQuickMenuExpanded = (expanded: boolean) =>
  invoke<void>("set_quick_menu_expanded", { expanded });

export const onSettingsChanged = (callback: (settings: Settings) => void) =>
  listen<Settings>("settings-changed", ({ payload }) => callback(payload));

export const onPreviousAppChanged = (callback: (app: PreviousApp) => void) =>
  listen<PreviousApp>("previous-app-changed", ({ payload }) => callback(payload));

export type CaptureFeedback = {
  flash: boolean;
  shutterSound: Settings["shutterSound"];
  customSoundPath: string | null;
};

export const onCaptureFeedback = (callback: (payload: CaptureFeedback) => void) =>
  listen<CaptureFeedback>("capture-feedback", ({ payload }) => callback(payload));

export const listSnapshots = () => invoke<SnapshotRecord[]>("list_snapshots");

export const openSnapshotsDir = () => invoke<string>("open_snapshots_dir");

export const openRecordingsDir = () => invoke<string>("open_recordings_dir");

export const getSnapshotDataUrl = (id: string) => invoke<string>("get_snapshot_data_url", { id });

export const deleteSnapshot = (id: string) => invoke<SnapshotRecord[]>("delete_snapshot", { id });

export const clearSnapshots = () => invoke<SnapshotRecord[]>("clear_snapshots");

export const ocrCapability = () => invoke<OcrCapability>("ocr_capability");

export const ocrSnapshot = (id: string) => invoke<string>("ocr_snapshot", { id });

export const ocrClipboard = () => invoke<string>("ocr_clipboard");

export const platformCapabilities = () => invoke<PlatformCapabilities>("platform_capabilities");
