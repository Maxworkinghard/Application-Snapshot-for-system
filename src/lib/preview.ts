import { mockConvertFileSrc, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { PlatformCapabilities, Settings } from "../types";

/**
 * 浏览器预览（直接 `npm run dev` 打开页面）用的假后端。
 *
 * 界面代码只认 invoke；这里在最底层把 invoke 接住，业务代码就不必每个函数都判断
 * 「是不是在 Tauri 里」。只在没有 Tauri 时由 main.tsx 动态加载，不进正式包的执行路径。
 */

let settings: Settings = {
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
  recordSystemAudio: false,
  recordMicrophone: false,
  afterCapture: "clipboard",
};

const unavailable = { available: false, detail: "预览模式" };
const capabilities: PlatformCapabilities = {
  os: "preview",
  displayServer: "n/a",
  recording: unavailable,
  recordingSystemAudio: unavailable,
  recordingMicrophone: unavailable,
  ocr: unavailable,
  autostart: unavailable,
  scrolling: unavailable,
  includeCursor: unavailable,
  trayNote: "",
  notes: [],
};

const idleRecording = { active: false, target: null, startedAt: null };
const noFileManager = "预览模式下不会打开文件管理器";

/** 改设置的命令在预览里也要「生效」，页面才能照常来回切换 */
function update(patch: Partial<Settings>): Settings {
  settings = { ...settings, ...patch };
  return structuredClone(settings);
}

async function handle(command: string, payload?: InvokeArgs): Promise<unknown> {
  // 入参形状由 backend.ts 那一侧的类型保证，这里只是假后端，不再重复校验
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const args = (payload ?? {}) as Record<string, any>;
  switch (command) {
    case "load_settings":
      return structuredClone(settings);
    case "save_prompt_settings": {
      const input = { ...args.input };
      delete input.apiKey;
      return update(input);
    }
    case "save_preferences":
      return update(args.prefs);
    case "save_shortcuts":
      return update({ shortcuts: args.shortcuts });
    case "select_pet_appearance":
      return update({ selectedAppearanceId: args.id });
    case "add_pet_asset": {
      const path: string = args.path;
      const asset = { id: `pet-${Date.now()}`, name: path.split(/[\\/]/).pop() ?? "桌宠", path, entry: "", animations: [] };
      return update({ selectedAppearanceId: asset.id, petAssets: [...settings.petAssets, asset] });
    }
    case "delete_pet_asset":
      return update({
        selectedAppearanceId: "app-icon",
        petAssets: settings.petAssets.filter((asset) => asset.id !== args.id),
      });
    case "fetch_models":
      return ["gpt-4.1", "gpt-4.1-mini", "gpt-4o-mini"];
    case "get_previous_app":
      return { id: null, name: "上一个应用", title: "等待切换应用", iconDataUrl: null };
    case "capture_window":
      return "预览模式下不会读取系统窗口";
    case "toggle_recording":
    case "get_recording_status":
      return idleRecording;
    case "polish_clipboard":
      return "预览模式下不会访问剪贴板";
    case "polish_text":
      await new Promise((resolve) => setTimeout(resolve, 400));
      return `【预览模式：未调用模型】\n\n${String(args.text).trim()}`;
    case "open_snapshots_dir":
    case "open_recordings_dir":
      return noFileManager;
    case "list_snapshots":
    case "list_capturable_windows":
    case "delete_snapshot":
    case "clear_snapshots":
      return [];
    case "get_snapshot_data_url":
    case "get_pet_asset_data_url":
    case "ocr_snapshot":
    case "ocr_clipboard":
      return "";
    case "ocr_capability":
      return { available: false, language: null, detail: "预览模式下不调用系统 OCR" };
    case "platform_capabilities":
      return capabilities;
    default:
      // 窗口控制、对话框、事件订阅等插件命令：预览里什么都不做
      return undefined;
  }
}

export function installPreviewBackend() {
  mockWindows("main");
  mockConvertFileSrc("windows");
  mockIPC(handle, { shouldMockEvents: true });
}
