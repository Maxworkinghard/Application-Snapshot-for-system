import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { RecordingStatus, Settings } from "../types";
import { setupTauriMock } from "../test/tauri";

const windowList = [{ id: 73, pid: 4242, appName: "Visual Studio Code", title: "notes.md" }];

type Call = { command: string; payload?: InvokeArgs };

async function renderPalette(handle: (command: string, payload?: InvokeArgs) => unknown) {
  const calls: Call[] = [];
  setupTauriMock((command, payload) => {
    calls.push({ command, payload });
    return handle(command, payload);
  }, { currentWindow: "quick-menu", shouldMockEvents: true });
  vi.resetModules();
  const { QuickMenuWindow } = await import("./QuickMenuWindow");
  render(<QuickMenuWindow />);
  return { calls, user: userEvent.setup() };
}

describe("输入框：录制", () => {
  it("选「录一个窗口…」先列窗口，点哪个就录哪个", async () => {
    const { calls, user } = await renderPalette((command) => {
      if (command === "get_recording_status") return { active: false, target: null, startedAt: null } satisfies RecordingStatus;
      if (command === "list_capturable_windows") return windowList;
      if (command === "toggle_recording") return { active: true, target: "Visual Studio Code", startedAt: 123 } satisfies RecordingStatus;
      return undefined;
    });

    await user.click(await screen.findByRole("option", { name: "录一个窗口…" }));
    expect(await screen.findByRole("listbox", { name: "选择要录制的窗口" })).toBeTruthy();
    await user.click(await screen.findByRole("option", { name: /Visual Studio Code/ }));
    expect(await screen.findByText("开始录制 Visual Studio Code")).toBeTruthy();

    await waitFor(() => {
      expect(calls.find((call) => call.command === "toggle_recording")?.payload).toEqual({ targetId: 73 });
    });
  });

  it("正在录时那一行变成停止，点了直接停，不再列窗口", async () => {
    const { calls, user } = await renderPalette((command) => {
      if (command === "get_recording_status") return { active: true, target: "Visual Studio Code", startedAt: Date.now() } satisfies RecordingStatus;
      if (command === "toggle_recording") return { active: false, target: null, startedAt: null } satisfies RecordingStatus;
      return undefined;
    });

    await user.click(await screen.findByRole("option", { name: /停止录制 · Visual Studio Code/ }));
    expect(await screen.findByText("录制已保存")).toBeTruthy();

    expect(calls.find((call) => call.command === "toggle_recording")?.payload).toEqual({ targetId: null });
    expect(calls.some((call) => call.command === "list_capturable_windows")).toBe(false);
    expect(screen.queryByRole("listbox", { name: "选择要录制的窗口" })).toBeNull();
  });
});

describe("输入框：润色", () => {
  const settings = {
    baseUrl: "https://api.example.com",
    model: "demo-model",
    hasApiKey: true,
    templates: [
      { id: "a", name: "默认规则", content: "", builtin: true },
      { id: "b", name: "代码修改任务", content: "", builtin: false },
    ],
    activeTemplateId: "a",
    selectedAppearanceId: "app-icon",
    petAssets: [],
    shortcuts: [],
  } as unknown as Settings;

  it("打一句话回车就润色；Tab 换规则重来；回车复制并收起", async () => {
    const { calls, user } = await renderPalette((command, payload) => {
      if (command === "load_settings") return settings;
      if (command === "get_recording_status") return { active: false, target: null, startedAt: null };
      if (command === "polish_text") {
        const args = payload as { text: string; templateId: string };
        return `【${args.templateId}】${args.text}`;
      }
      return undefined;
    });

    const input = await screen.findByRole("textbox", { name: "命令，或要润色的文字" });
    // 等设置读到（润色要用到当前规则）
    await waitFor(() => expect(calls.some((call) => call.command === "load_settings")).toBe(true));
    await user.type(input, "帮我把历史页按日期分组{Enter}");

    expect(await screen.findByText("【a】帮我把历史页按日期分组")).toBeTruthy();
    await user.keyboard("{Tab}");
    expect(await screen.findByText("【b】帮我把历史页按日期分组")).toBeTruthy();

    await user.keyboard("{Enter}");
    await waitFor(() => {
      expect(calls.find((call) => call.command === "copy_text")?.payload).toEqual({ text: "【b】帮我把历史页按日期分组" });
    });
    expect(calls.some((call) => call.command === "hide_quick_menu")).toBe(true);
  });

  it("没配模型时不发请求，直接说去哪里填", async () => {
    const { calls, user } = await renderPalette((command) => {
      if (command === "load_settings") return { ...settings, baseUrl: "", model: "" };
      return undefined;
    });

    const input = await screen.findByRole("textbox", { name: "命令，或要润色的文字" });
    await waitFor(() => expect(calls.some((call) => call.command === "load_settings")).toBe(true));
    await user.type(input, "帮我把历史页按日期分组{Enter}");

    expect(await screen.findByText(/还没填模型接口/)).toBeTruthy();
    expect(calls.some((call) => call.command === "polish_text")).toBe(false);
  });
});
