import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { Settings } from "../types";
import { invokeArgs, setupTauriMock } from "../test/tauri";

const base: Settings = {
  baseUrl: "https://api.example.com/v1",
  model: "demo",
  hasApiKey: true,
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

/** 渲染主窗口；后端的设置是有状态的，润色结果带上所用规则，方便断言 */
async function renderApp() {
  let settings = structuredClone(base);
  const polished: Array<{ text: string; templateId: string }> = [];
  const copied: string[] = [];
  setupTauriMock(
    (command: string, payload?: InvokeArgs) => {
      const args = invokeArgs(payload);
      if (command === "load_settings") return settings;
      if (command === "polish_text") {
        polished.push({ text: args.text as string, templateId: args.templateId as string });
        return `润色后（${args.templateId}）：${args.text}`;
      }
      if (command === "copy_text") {
        copied.push(args.text as string);
        return undefined;
      }
      if (command === "save_prompt_settings") {
        const input = args.input as Pick<Settings, "templates" | "activeTemplateId">;
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
  return { user: userEvent.setup(), polished, copied };
}

async function openPromptPage() {
  const app = await renderApp();
  await app.user.click(await screen.findByRole("button", { name: /^Prompt$/ }));
  const draft = await screen.findByRole("textbox", { name: "要润色的 Prompt" });
  return { ...app, draft };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 150));

describe("Prompt 页的润色", () => {
  it("生成后原文不动，结果单独弹窗给出", async () => {
    const { user, draft, copied } = await openPromptPage();
    await user.type(draft, "帮我写个脚本");
    await user.click(screen.getByRole("button", { name: "生成" }));

    const dialog = await screen.findByRole("dialog", { name: "润色结果" });
    expect(within(dialog).getByRole("textbox", { name: "润色后的 Prompt" })).toHaveProperty(
      "value",
      "润色后（builtin-default）：帮我写个脚本",
    );
    expect(draft).toHaveProperty("value", "帮我写个脚本");

    await user.click(within(dialog).getByRole("button", { name: "复制" }));
    expect(copied).toEqual(["润色后（builtin-default）：帮我写个脚本"]);

    // 关掉之后结果还在，能再打开
    await user.click(within(dialog).getByRole("button", { name: "关闭" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    await user.click(screen.getByRole("button", { name: "查看结果" }));
    expect(await screen.findByRole("dialog", { name: "润色结果" })).toBeTruthy();
  });

  it("顶部点规则就切过去，下一次生成用它", async () => {
    const { user, draft, polished } = await openPromptPage();
    await user.click(screen.getByRole("radio", { name: "代码修改任务" }));
    await waitFor(() => expect(screen.getByRole("radio", { name: "代码修改任务" }).getAttribute("aria-checked")).toBe("true"));

    await user.type(draft, "改个 bug");
    await user.click(screen.getByRole("button", { name: "生成" }));
    await screen.findByRole("dialog", { name: "润色结果" });
    expect(polished).toEqual([{ text: "改个 bug", templateId: "custom-code" }]);
  });

  it("输入框里没有 Ctrl+Enter：时间线「今天」和 Prompt 页都只能点按钮润色", async () => {
    localStorage.setItem("snapshot-layout", "timeline");
    const { user, polished } = await renderApp();
    const composer = await screen.findByRole("textbox", { name: "要润色的 Prompt" });
    await user.type(composer, "改个 bug");
    await user.keyboard("{Control>}{Enter}{/Control}");
    await settle();
    expect(polished).toEqual([]);
    expect(screen.queryByRole("button", { name: "生成" })).toBeNull();

    // 点「润色」才会带着草稿去 Prompt 页开跑
    await user.click(screen.getByRole("button", { name: "润色" }));
    const dialog = await screen.findByRole("dialog", { name: "润色结果" });
    expect(polished).toHaveLength(1);
    await user.click(within(dialog).getByRole("button", { name: "关闭" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());

    // Prompt 页的编辑框同样不认 Ctrl+Enter
    await user.click(screen.getByRole("textbox", { name: "要润色的 Prompt" }));
    await user.keyboard("{Control>}{Enter}{/Control}");
    await settle();
    expect(polished).toHaveLength(1);
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
