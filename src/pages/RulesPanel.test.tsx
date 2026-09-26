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
  petScale: 100,
  shortcuts: [],
  clipboardAutoClear: "60s",
  snapshotFormat: "png",
  saveDir: "",
  recordingDir: "",
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

  it("新建：弹窗里填好名字和正文，点保存才进列表；取消不留空规则", async () => {
    const { user, saves } = await openPreferences();
    await user.click(screen.getByRole("button", { name: "新建" }));
    let dialog = await screen.findByRole("dialog", { name: "新建规则" });
    await user.type(within(dialog).getByRole("textbox", { name: "规则名称" }), "草稿");
    // 有改动时第一次「取消」只是确认，再点「放弃修改」才关
    await user.click(within(dialog).getByRole("button", { name: "取消" }));
    await user.click(within(dialog).getByRole("button", { name: "放弃修改" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saves).toHaveLength(0);

    await user.click(screen.getByRole("button", { name: "新建" }));
    dialog = await screen.findByRole("dialog", { name: "新建规则" });
    const save = within(dialog).getByRole("button", { name: "保存" });
    await user.type(within(dialog).getByRole("textbox", { name: "规则名称" }), "翻成英文");
    expect(save).toHaveProperty("disabled", true);
    await user.type(within(dialog).getByRole("textbox", { name: "规则正文" }), "把草稿翻成英文");
    await user.click(save);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saves.at(-1)?.templates.map((item) => item.name)).toEqual(["清晰、可执行", "代码修改任务", "翻成英文"]);
    expect(screen.getByRole("radio", { name: "翻成英文" })).toBeTruthy();
  });

  it("编辑：改动点保存才生效；按 Esc 关窗时要再确认一次", async () => {
    const { user, saves } = await openPreferences();
    await user.click(screen.getByRole("button", { name: "编辑「代码修改任务」" }));
    let dialog = await screen.findByRole("dialog", { name: "编辑规则" });
    await user.type(within(dialog).getByRole("textbox", { name: "规则正文" }), "，只改相关文件");
    await user.keyboard("{Escape}");
    expect(within(dialog).getByRole("button", { name: "放弃修改" })).toBeTruthy();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saves).toHaveLength(0);

    // 重新打开是上次存下的内容，刚才没保存的改动不在了
    await user.click(screen.getByRole("button", { name: "编辑「代码修改任务」" }));
    dialog = await screen.findByRole("dialog", { name: "编辑规则" });
    const body = within(dialog).getByRole("textbox", { name: "规则正文" });
    expect(body).toHaveProperty("value", "改成修改任务");
    await user.type(body, "，只改相关文件");
    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(saves.at(-1)?.templates.find((item) => item.id === "custom-code")?.content).toBe("改成修改任务，只改相关文件");
  });

  it("存为新规则：内置规则改一版另存，原来那条不动", async () => {
    const { user, saves } = await openPreferences();
    await user.click(screen.getByRole("button", { name: "编辑「清晰、可执行」" }));
    const dialog = await screen.findByRole("dialog", { name: "编辑规则" });
    const body = within(dialog).getByRole("textbox", { name: "规则正文" });
    await user.clear(body);
    await user.type(body, "只改错别字");
    // 内置规则改过了才给「恢复原文」
    expect(await within(dialog).findByRole("button", { name: "恢复原文" })).toBeTruthy();
    await user.click(within(dialog).getByRole("button", { name: "存为新规则" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    const templates = saves.at(-1)?.templates ?? [];
    expect(templates.map((item) => item.name)).toEqual(["清晰、可执行", "清晰、可执行 副本", "代码修改任务"]);
    expect(templates[0].content).toBe("内置原文");
    expect(templates[1]).toMatchObject({ content: "只改错别字", builtin: false });
  });

  it("删除要点两次；内置规则没有删除", async () => {
    const { user, saves } = await openPreferences();
    await user.click(screen.getByRole("button", { name: "编辑「清晰、可执行」" }));
    let dialog = await screen.findByRole("dialog", { name: "编辑规则" });
    expect(within(dialog).queryByRole("button", { name: "删除" })).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "取消" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    await user.click(screen.getByRole("button", { name: "编辑「代码修改任务」" }));
    dialog = await screen.findByRole("dialog", { name: "编辑规则" });
    await user.click(within(dialog).getByRole("button", { name: "删除" }));
    expect(saves).toHaveLength(0);
    await user.click(within(dialog).getByRole("button", { name: "再点一次删除" }));
    await waitFor(() => expect(saves.at(-1)?.templates.map((item) => item.id)).toEqual(["builtin-default"]));
  });
});

// jsdom 未实现 Element.prototype.scrollTo；App 换页后会调用它回到顶部。
if (!Element.prototype.scrollTo) {
  Element.prototype.scrollTo = () => {};
}
