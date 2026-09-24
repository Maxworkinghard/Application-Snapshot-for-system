import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { Settings } from "../types";
import { setupTauriMock } from "../test/tauri";

const base: Settings = {
  baseUrl: "",
  model: "",
  hasApiKey: false,
  templates: [
    { id: "builtin-default", name: "清晰、可执行", content: "内置原文", builtin: true },
    { id: "custom-code", name: "代码修改任务", content: "改成修改任务", builtin: false },
  ],
  activeTemplateId: "builtin-default",
  selectedAppearanceId: "app-icon",
  petAssets: [],
  shortcuts: [],
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

type Saved = { templates: Settings["templates"]; activeTemplateId: string };

/** 打开偏好设置页；后端的设置是有状态的，存什么下次就读到什么 */
async function openPreferences() {
  let settings = structuredClone(base);
  const saves: Saved[] = [];
  setupTauriMock(
    (command: string, payload?: InvokeArgs) => {
      if (command === "load_settings") return settings;
      if (command === "default_prompt") return "内置原文";
      if (command === "save_prompt_settings") {
        const input = (payload as { input: Saved }).input;
        saves.push(structuredClone({ templates: input.templates, activeTemplateId: input.activeTemplateId }));
        settings = { ...settings, templates: input.templates, activeTemplateId: input.activeTemplateId };
        return settings;
      }
      return undefined;
    },
    { currentWindow: "main", shouldMockEvents: true },
  );
  vi.resetModules();
  const { App } = await import("../App");
  render(<App />);
  const user = userEvent.setup();
  await user.click(await screen.findByRole("button", { name: /^偏好设置/ }));
  await screen.findByText("润色规则");
  return { user, saves };
}

describe("偏好设置里的润色规则", () => {
  it("点名字就换成这条规则", async () => {
    const { user, saves } = await openPreferences();
    await user.click(screen.getByRole("radio", { name: "代码修改任务" }));
    await waitFor(() => expect(saves.at(-1)?.activeTemplateId).toBe("custom-code"));
  });

  it("新建一条：名字原地可改，停手后自动存", async () => {
    const { user, saves } = await openPreferences();
    await user.click(screen.getByRole("button", { name: "新建" }));
    const name = await screen.findByRole("textbox", { name: "规则名称" });
    await waitFor(() => expect(document.activeElement).toBe(name));
    await user.clear(name);
    await user.type(name, "翻成英文");
    await waitFor(() => expect(saves.at(-1)?.templates.map((item) => item.name)).toContain("翻成英文"), { timeout: 2500 });
    expect(saves.at(-1)?.templates).toHaveLength(3);
  });

  it("删除要点两次；内置规则没有删除", async () => {
    const { user, saves } = await openPreferences();
    const row = (id: string) => within(document.querySelector<HTMLElement>(`[data-rule-id="${id}"]`)!);
    await user.click(screen.getByRole("button", { name: "编辑「清晰、可执行」" }));
    expect(row("builtin-default").queryByRole("button", { name: "删除" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "收起「清晰、可执行」" }));

    await user.click(screen.getByRole("button", { name: "编辑「代码修改任务」" }));
    await user.click(row("custom-code").getByRole("button", { name: "删除" }));
    expect(saves.some((item) => item.templates.length === 1)).toBe(false);
    await user.click(row("custom-code").getByRole("button", { name: "再点一次删除" }));
    await waitFor(() => expect(saves.at(-1)?.templates.map((item) => item.id)).toEqual(["builtin-default"]));
  });
});

// jsdom 未实现 Element.prototype.scrollTo；App 换页后会调用它回到顶部。
if (!Element.prototype.scrollTo) {
  Element.prototype.scrollTo = () => {};
}
