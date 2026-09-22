export type NavPage = "shortcuts" | "prompt" | "history" | "pet" | "prefs" | "theme" | "ocr";
export type ShortcutAction =
  | "snapshot"
  | "fullscreen"
  | "scrolling"
  | "record"
  | "recordings"
  | "polish"
  | "ocr";

export interface PromptTemplate {
  id: string;
  name: string;
  content: string;
  builtin: boolean;
}

export interface ShortcutBinding {
  action: ShortcutAction;
  accelerator: string | null;
}

export interface PetAsset {
  id: string;
  name: string;
  path: string;
  entry: string;
  animations: string[];
}

export interface Settings {
  baseUrl: string;
  model: string;
  hasApiKey: boolean;
  templates: PromptTemplate[];
  activeTemplateId: string;
  selectedAppearanceId: string;
  petAssets: PetAsset[];
  shortcuts: ShortcutBinding[];
  clipboardAutoClear: "30s" | "60s" | "5m" | "never";
  snapshotFormat: "png" | "jpeg" | "webp";
  saveDir: string;
  recordingDir: string;
  customTheme: string | null;
  shutterSound: "crisp" | "soft" | "none" | "custom";
  customSoundPath: string | null;
  flashOnCapture: boolean;
  hideAfterCopy: boolean;
  autoSaveLocal: boolean;
  launchOnBoot: boolean;
  includeCursor: boolean;
  afterCapture: "clipboard" | "annotate" | "saveas";
}

export interface PreviousApp {
  id: number | null;
  name: string;
  title: string;
  iconDataUrl: string | null;
}

export interface CapturableWindow {
  id: number;
  appName: string;
  title: string;
  iconDataUrl: string | null;
}

export interface RecordingStatus {
  active: boolean;
  target: string | null;
  startedAt: number | null;
  /** 启动时的提示（如 Linux portal 需重新选窗）；停止时通常缺省 */
  message?: string | null;
}

export interface SnapshotRecord {
  id: string;
  fileName: string;
  appName: string;
  width: number;
  height: number;
  sizeBytes: number;
  createdAt: number;
}

export interface OcrCapability {
  available: boolean;
  language: string | null;
  detail: string;
}

export interface CapabilityStatus {
  available: boolean;
  detail: string;
}

export interface PlatformCapabilities {
  os: string;
  displayServer: string;
  recording: CapabilityStatus;
  ocr: CapabilityStatus;
  autostart: CapabilityStatus;
  /** 仅 Linux 下发：其余平台没有滚动长截图 */
  scrolling?: CapabilityStatus;
  includeCursor: CapabilityStatus;
  trayNote: string;
  notes: string[];
}
