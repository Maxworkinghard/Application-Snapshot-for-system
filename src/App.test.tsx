import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { PlatformCapabilities, Settings } from "./types";
import { setupTauriMock } from "./test/tauri";

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
  afterCapture: "clipboard",
};

const capabilities: PlatformCapabilities = {
  os: "windows",
  displayServer: "n/a",
  recording: { available: true, detail: "" },
  ocr: { available: true, detail: "" },
  autostart: { available: true, detail: "" },
  includeCursor: { available: true, detail: "" },
  trayNote: "",
  notes: [],
};

describe("shortcut page save validation", () => {
  it("blocks duplicate accelerators before registering or saving", async () => {
    const calls: Array<{ command: string; payload?: InvokeArgs }> = [];
    setupTauriMock((command, payload) => {
      calls.push({ command, payload });
      if (command === "load_settings") return settings;
      if (command === "platform_capabilities") return capabilities;
      if (command === "plugin:global-shortcut|register") return undefined;
      return undefined;
    }, { currentWindow: "main", shouldMockEvents: true });
    vi.resetModules();
    const { App } = await import("./App");
    const user = userEvent.setup();

    render(<App />);
    const recordRow = await screen.findByRole("button", { name: "快捷键：窗口录制，当前按键：未设置" });
    await waitFor(() => {
      expect(calls.filter((call) => call.command === "plugin:global-shortcut|register")).toHaveLength(1);
    });
    const registerCountBeforeEdit = calls.filter((call) => call.command === "plugin:global-shortcut|register").length;

    await user.click(recordRow);
    recordRow.focus();
    await user.keyboard("{Alt>}{Shift>}2{/Shift}{/Alt}");
    expect(await screen.findByRole("button", { name: "快捷键：窗口录制，当前按键：Alt+Shift+2" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /保存快捷键/ }));

    expect(await screen.findByText("快捷键不能重复")).toBeTruthy();
    expect(calls.filter((call) => call.command === "plugin:global-shortcut|register")).toHaveLength(registerCountBeforeEdit);
    expect(calls.some((call) => call.command === "save_shortcuts")).toBe(false);
  });
});
