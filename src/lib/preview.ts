import { mockConvertFileSrc, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { ActivityEntry, PlatformCapabilities, Settings, SnapshotRecord } from "../types";

/**
 * 浏览器预览（直接 `npm run dev` 打开页面）用的假后端。
 *
 * 界面代码只认 invoke；这里在最底层把 invoke 接住，业务代码就不必每个函数都判断
 * 「是不是在 Tauri 里」。只在没有 Tauri 时由 main.tsx 动态加载，不进正式包的执行路径。
 *
 * 地址里带 `?demo` 时装一份演示数据（历史、活动、伴侣），方便看界面在「用了一阵子」之后的样子；
 * `?window=quick-menu` 可以单独看输入框。
 */

const params = new URLSearchParams(window.location.search);
const demo = params.has("demo");
const windowLabel = params.get("window") ?? "main";

const minutesAgo = (minutes: number) => Date.now() - minutes * 60_000;

const BUILTIN_PROMPT = "你是提示词改写专家。保持原意不变，将用户草稿整理为清晰、完整、可执行的指令。只输出改写后的提示词。";

let settings: Settings = {
  baseUrl: demo ? "https://api.deepseek.com/v1" : "",
  model: demo ? "deepseek-chat" : "",
  hasApiKey: demo,
  templates: [
    {
      id: "builtin-default",
      name: "清晰、可执行",
      builtin: true,
      content: BUILTIN_PROMPT,
    },
    ...(demo
      ? [
          { id: "custom-code", name: "代码修改任务", builtin: false, content: "把草稿改写成给编程助手的修改任务……" },
          { id: "custom-en", name: "翻成英文", builtin: false, content: "Translate the draft into natural English." },
        ]
      : []),
  ],
  activeTemplateId: demo ? "custom-code" : "builtin-default",
  selectedAppearanceId: demo ? "pet-calico" : "app-icon",
  petAssets: demo
    ? [
        { id: "pet-calico", name: "三花猫", path: "pets/pet-calico.zip", entry: "idle.gif", animations: ["idle.gif", "打招呼.gif", "敲键盘.gif", "撒娇.gif", "撒娇2.gif", "睡觉.gif"], source: "D:\\素材\\伴侣\\三花猫-v3-最终.zip", sizeBytes: 24_530_000, importedAt: minutesAgo(9000), lastUsedAt: minutesAgo(10) },
        { id: "pet-typing", name: "键盘三花", path: "pets/pet-typing.gif", entry: "typing.gif", animations: ["typing.gif"], sizeBytes: 3_200_000, importedAt: minutesAgo(8000), lastUsedAt: minutesAgo(600) },
        { id: "pet-idea", name: "灵感三花", path: "pets/pet-idea.gif", entry: "idea.gif", animations: ["idea.gif"], importedAt: minutesAgo(7000), lastUsedAt: minutesAgo(1200) },
        { id: "pet-cheer", name: "三花猫-欢呼-2026春节限定版", path: "pets/pet-cheer.gif", entry: "cheer.gif", animations: ["cheer.gif"], importedAt: minutesAgo(6000), lastUsedAt: minutesAgo(3000) },
        { id: "pet-sit", name: "坐姿三花", path: "pets/pet-sit.gif", entry: "sit.gif", animations: ["sit.gif"], importedAt: minutesAgo(5000) },
        { id: "pet-question", name: "问号三花", path: "pets/pet-question.gif", entry: "q.gif", animations: ["q.gif"], importedAt: minutesAgo(4000) },
        { id: "pet-chubby", name: "胖三花", path: "pets/pet-chubby.gif", entry: "c.gif", animations: ["c.gif"], importedAt: minutesAgo(3000) },
        { id: "pet-clawd", name: "clawd", path: "C:\\Users\\max\\Downloads\\clawd.zip", entry: "idle.gif", animations: ["idle.gif"], importedAt: minutesAgo(2000), missing: true },
      ]
    : [],
  shortcuts: [
    { action: "snapshot", accelerator: demo ? "CommandOrControl+Shift+A" : null },
    { action: "fullscreen", accelerator: demo ? "CommandOrControl+Shift+F" : null },
    { action: "record", accelerator: demo ? "CommandOrControl+Alt+R" : null },
    { action: "polish", accelerator: null },
    { action: "palette", accelerator: demo ? "Alt+Space" : null },
  ],
  clipboardAutoClear: "60s",
  snapshotFormat: "png",
  saveDir: demo ? "D:\\Dropbox\\工作\\2026\\截图归档\\应用快照\\原始文件" : "",
  recordingDir: "",
  customTheme: null,
  shutterSound: "crisp",
  customSoundPath: null,
  flashOnCapture: true,
  hideAfterCopy: false,
  autoSaveLocal: true,
  launchOnBoot: false,
  includeCursor: false,
  recordSystemAudio: demo,
  recordMicrophone: false,
  afterCapture: "clipboard",
};

const demoApps: Array<[string, number, number, number]> = [
  ["Visual Studio Code — HistoryPage.tsx — 应用快照", 1920, 1080, 412_000],
  ["微信", 1100, 780, 96_000],
  ["Google Chrome", 1280, 6400, 2_800_000],
  ["Windows Terminal", 1200, 700, 64_000],
  ["Q3 预算-终版-改2.xlsx - Excel", 2560, 1440, 1_100_000],
  ["记事本", 640, 480, 18_000],
  ["飞书", 1440, 900, 388_000],
  ["Microsoft Edge", 3840, 2160, 5_600_000],
  ["资源管理器", 800, 600, 120_000],
  ["PowerShell", 1100, 620, 80_000],
  ["画图", 1024, 768, 210_000],
];

let snapshots: SnapshotRecord[] = demo
  ? Array.from({ length: 186 }, (_, index) => {
      const [appName, width, height, sizeBytes] = demoApps[index % demoApps.length];
      const minutes = index < 4 ? [6, 33, 165, 254][index] : index < 8 ? 60 * 24 + index * 70 : 60 * 24 * 3 + index * 40;
      return { id: `snap-${index}`, fileName: `snap-${index}.png`, appName, width, height, sizeBytes, createdAt: minutesAgo(minutes) };
    })
  : [];

const activity: ActivityEntry[] = demo
  ? [
      { id: "a1", at: minutesAgo(6), kind: "capture", title: demoApps[0][0], meta: "1920 × 1080", snapshotId: "snap-0" },
      {
        id: "a2",
        at: minutesAgo(33),
        kind: "polish",
        title: "代码修改任务",
        meta: "58 → 427 字",
        detail:
          "你在修改一个 Tauri 2 + React 的桌面应用「应用快照」。这次只动 src/pages/HistoryPage.tsx，不要改 Rust 侧的命令，也不要改 listSnapshots() 的返回结构。要做的事：1. 历史记录按日期分组……",
      },
      { id: "a3", at: minutesAgo(48), kind: "record", title: "Google Chrome", meta: "02:13", detail: "C:\\Users\\max\\Downloads\\应用快照-2026-09-24_13-50-02.mp4" },
      { id: "a4", at: minutesAgo(165), kind: "capture", title: "Google Chrome", meta: "1280 × 6400", snapshotId: "snap-2" },
      { id: "a5", at: minutesAgo(210), kind: "error", title: "润色失败", detail: "请求超时（180 秒）" },
      { id: "a6", at: minutesAgo(254), kind: "capture", title: "Windows Terminal", meta: "1200 × 700", snapshotId: "snap-3" },
      { id: "a7", at: minutesAgo(300), kind: "error", title: "快捷键没注册上", detail: "CommandOrControl+Alt+R" },
    ]
  : [];

const demoClipboard = demo
  ? { label: "Visual Studio Code 截图", snapshotId: "snap-0", armedAt: Date.now() - 18_000, clearAt: Date.now() + 42_000 }
  : null;

const unavailable = { available: false, detail: "预览模式" };
const capabilities: PlatformCapabilities = demo
  ? {
      os: "windows",
      displayServer: "desktop",
      recording: { available: true, detail: "Windows.Graphics.Capture + Media Foundation" },
      recordingSystemAudio: { available: true, detail: "" },
      recordingMicrophone: { available: true, detail: "" },
      autostart: { available: true, detail: "" },
      includeCursor: { available: true, detail: "" },
      trayNote: "",
      notes: [],
    }
  : {
      os: "preview",
      displayServer: "n/a",
      recording: unavailable,
      recordingSystemAudio: unavailable,
      recordingMicrophone: unavailable,
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
      return update({
        selectedAppearanceId: args.id,
        petAssets: settings.petAssets.map((asset) => (asset.id === args.id ? { ...asset, lastUsedAt: Date.now() } : asset)),
      });
    case "add_pet_assets": {
      const added = (args.paths as string[]).map((path, index) => ({
        id: `pet-${Date.now()}-${index}`,
        name: (path.split(/[\\/]/).pop() ?? "伴侣").replace(/\.[^.]+$/, ""),
        path,
        entry: "",
        animations: [],
        importedAt: Date.now(),
        lastUsedAt: Date.now(),
      }));
      const next = update({ selectedAppearanceId: added[added.length - 1]?.id ?? settings.selectedAppearanceId, petAssets: [...settings.petAssets, ...added] });
      return { settings: next, imported: added.length, failed: [] };
    }
    case "rename_pet_asset":
      return update({ petAssets: settings.petAssets.map((asset) => (asset.id === args.id ? { ...asset, name: args.name } : asset)) });
    case "delete_pet_asset":
      return update({
        selectedAppearanceId: settings.selectedAppearanceId === args.id ? "app-icon" : settings.selectedAppearanceId,
        petAssets: settings.petAssets.filter((asset) => asset.id !== args.id),
      });
    case "default_prompt":
      return BUILTIN_PROMPT;
    case "fetch_models":
      return ["gpt-4.1", "gpt-4.1-mini", "gpt-4o-mini"];
    case "get_previous_app":
      return { id: null, pid: null, name: "上一个应用", title: "等待切换应用" };
    case "capture_window":
    case "run_action":
      return "预览模式下不会读取系统窗口";
    case "toggle_recording":
    case "get_recording_status":
      return idleRecording;
    case "polish_clipboard":
      return "预览模式下不会访问剪贴板";
    case "read_clipboard_text":
      return "帮我把历史页按日期分一下组，长截图别裁成方的，名字太长就截断";
    case "polish_text":
      await new Promise((resolve) => setTimeout(resolve, 600));
      return `【预览模式：未调用模型】\n\n${String(args.text).trim()}`;
    case "open_snapshots_dir":
    case "open_recordings_dir":
      return noFileManager;
    case "list_snapshots":
      return structuredClone(snapshots);
    case "delete_snapshots":
      snapshots = snapshots.filter((item) => !(args.ids as string[]).includes(item.id));
      return structuredClone(snapshots);
    case "copy_snapshot":
      return "预览模式下不会写剪贴板";
    case "list_activity":
      return structuredClone(activity);
    case "get_clipboard_state":
      return demoClipboard;
    case "list_capturable_windows":
      return demo
        ? [
            { id: 1, pid: 1, appName: "Visual Studio Code", title: "HistoryPage.tsx — 应用快照" },
            { id: 2, pid: 2, appName: "Google Chrome", title: "Tauri 2 · Window Customization" },
            { id: 3, pid: 3, appName: "微信", title: "微信" },
          ]
        : [];
    case "get_shortcut_conflicts":
      return demo ? ["CommandOrControl+Alt+R"] : [];
    case "platform_capabilities":
      return capabilities;
    default:
      // 窗口控制、对话框、事件订阅、剪贴板写入等：预览里什么都不做
      return undefined;
  }
}

export function installPreviewBackend() {
  mockWindows(windowLabel);
  mockConvertFileSrc("windows");
  mockIPC(handle, { shouldMockEvents: true });
}
