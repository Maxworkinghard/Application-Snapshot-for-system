import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PlatformCapabilities, Settings } from "../types";
import { setupTauriMock } from "../test/tauri";

/**
 * B2 拆页后的回归网：逐个导航到从 App.tsx 搬出的页面，确认它们在新位置
 * 仍然真的渲染出来（编译通过不等于渲染通过）。找出每个页面独有的文案做断言。
 */

const settings: Settings = {
  baseUrl: "",
  model: "",
  hasApiKey: false,
  templates: [],
  activeTemplateId: "builtin-default",
  selectedAppearanceId: "app-icon",
  petAssets: [],
  shortcuts: [
    { action: "snapshot", accelerator: "Alt+Shift+2" },
    { action: "record", accelerator: null },
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

const capabilities: PlatformCapabilities = {
  os: "windows",
  displayServer: "n/a",
  recording: { available: true, detail: "录制可用" },
  recordingSystemAudio: { available: true, detail: "系统音频可用" },
  recordingMicrophone: { available: true, detail: "麦克风可用" },
  autostart: { available: true, detail: "自启可用" },
  includeCursor: { available: true, detail: "光标可用" },
  trayNote: "",
  notes: [],
};

function handler(command: string) {
  switch (command) {
    case "load_settings":
      return settings;
    case "platform_capabilities":
      return capabilities;
    case "list_snapshots":
      return [];
    case "get_recording_status":
      return { active: false, target: null, startedAt: null };
    default:
      return undefined;
  }
}

async function renderApp() {
  setupTauriMock(handler, { currentWindow: "main", shouldMockEvents: true });
  vi.resetModules();
  const { App } = await import("../App");
  render(<App />);
  // 等设置加载完（加载态消失，出现侧边栏导航）
  await screen.findByRole("button", { name: "快捷操作" });
}

describe("B2 拆页后逐页渲染", () => {
  it("默认落在快捷操作页，能看到快捷键行", async () => {
    await renderApp();
    expect(
      await screen.findByRole("button", { name: "快捷键：窗口快照，当前按键：Alt+Shift+2" }),
    ).toBeTruthy();
  });

  it.each([
    ["Prompt 编辑", "当前规则:"],
    ["快照历史", "快照历史库"],
    ["桌面伴侣", "伴侣悬浮演示"],
    ["偏好设置", "截屏与行为"],
    ["界面主题", "主题对主窗口、桌面伴侣与快捷菜单同时生效。"],
  ])("导航到「%s」能渲染出该页独有内容", async (navLabel, marker) => {
    await renderApp();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: navLabel }));
    // 页面切换有 PAGE_EXIT_MS 的淡出延时，等新页面内容出现
    expect(await screen.findByText(marker, {}, { timeout: 2000 })).toBeTruthy();
  });
});
// jsdom 未实现 Element.prototype.scrollTo；App 换页后会调用它回到顶部。
if (!Element.prototype.scrollTo) {
  Element.prototype.scrollTo = () => {};
}
