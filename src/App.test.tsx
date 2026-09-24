import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { PlatformCapabilities, Settings } from "./types";
import { invokeArgs, setupTauriMock } from "./test/tauri";

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
  recording: { available: true, detail: "" },
  recordingSystemAudio: { available: true, detail: "" },
  recordingMicrophone: { available: true, detail: "" },
  autostart: { available: true, detail: "" },
  includeCursor: { available: true, detail: "" },
  trayNote: "",
  notes: [],
};

async function renderApp(handle: (command: string, payload?: InvokeArgs) => unknown) {
  setupTauriMock(handle, { currentWindow: "main", shouldMockEvents: true });
  vi.resetModules();
  const { App } = await import("./App");
  render(<App />);
}

describe("global shortcuts", () => {
  it("hands the edited bindings to the backend and shows its verdict", async () => {
    const calls: Array<{ command: string; payload?: InvokeArgs }> = [];
    await renderApp((command, payload) => {
      calls.push({ command, payload });
      if (command === "load_settings") return settings;
      if (command === "platform_capabilities") return capabilities;
      if (command === "get_shortcut_conflicts") return [];
      // 后端才是校验方：重复、认不出、被占用都由它拒绝，页面只负责转述
      if (command === "save_shortcuts") return Promise.reject("快捷键不能重复");
      return undefined;
    });
    const user = userEvent.setup();

    const recordRow = await screen.findByRole("button", { name: "快捷键：窗口录制，当前按键：未设置" });
    await user.click(recordRow);
    recordRow.focus();
    await user.keyboard("{Alt>}{Shift>}2{/Shift}{/Alt}");
    expect(await screen.findByRole("button", { name: "快捷键：窗口录制，当前按键：Alt+Shift+2" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /保存快捷键/ }));

    expect(await screen.findByText("快捷键不能重复")).toBeTruthy();
    const saved = calls.find((call) => call.command === "save_shortcuts");
    expect(invokeArgs(saved?.payload).shortcuts).toEqual([
      { action: "snapshot", accelerator: "Alt+Shift+2" },
      { action: "record", accelerator: "Alt+Shift+2" },
    ]);
    // 网页不再自己注册快捷键
    expect(calls.some((call) => call.command.startsWith("plugin:global-shortcut"))).toBe(false);
  });

  it("tells the user which keys could not be registered at startup", async () => {
    await renderApp((command) => {
      if (command === "load_settings") return settings;
      if (command === "platform_capabilities") return capabilities;
      if (command === "get_shortcut_conflicts") return ["Alt+Shift+2"];
      return undefined;
    });

    expect(await screen.findByText("这些快捷键没能注册，可能已被其他程序占用：Alt+Shift+2")).toBeTruthy();
  });
});
