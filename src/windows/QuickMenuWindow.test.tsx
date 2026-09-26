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

describe("输入框：窗口大小", () => {
  /** 只记下在观察谁；什么时候回调由测试决定（jsdom 没有布局，也没有 ResizeObserver） */
  class FakeResizeObserver {
    static live = new Set<FakeResizeObserver>();
    targets = new Set<Element>();
    constructor(private callback: ResizeObserverCallback) {
      FakeResizeObserver.live.add(this);
    }
    observe(target: Element) {
      this.targets.add(target);
    }
    unobserve(target: Element) {
      this.targets.delete(target);
    }
    disconnect() {
      this.targets.clear();
      FakeResizeObserver.live.delete(this);
    }
    /** 相当于浏览器排完版后通知：旧面板被移走、新面板量好，都会走到这里 */
    static notifyAll() {
      for (const observer of [...FakeResizeObserver.live]) {
        if (observer.targets.size) observer.callback([], observer as unknown as ResizeObserver);
      }
    }
  }

  it("每次打开都量新挂上的面板，旧面板移走时不会把窗口缩到最小", async () => {
    FakeResizeObserver.live.clear();
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    let panelHeight = 300;
    // 面板在页面里时有高度，被移出页面后是 0（浏览器里就是这样）
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
      return this.isConnected && this.classList.contains("palette") ? panelHeight : 0;
    });

    const { calls } = await renderPalette(() => undefined);
    // 没有桌宠形象时上方留 24，下方留 40
    const heights = () =>
      calls.filter((call) => call.command === "resize_quick_menu").map((call) => (call.payload as { height: number }).height);
    await waitFor(() => expect(heights()).toContain(300 + 24 + 40));

    const firstPanel = document.querySelector(".palette");
    const { emit } = await import("@tauri-apps/api/event");
    await emit("palette-opened");
    await waitFor(() => expect(document.querySelector(".palette")).not.toBe(firstPanel));

    // 新面板更高（比如开着录制时那一行多了时长）：窗口要跟着变高
    panelHeight = 380;
    FakeResizeObserver.notifyAll();
    await waitFor(() => expect(heights().at(-1)).toBe(380 + 24 + 40));
    expect(heights().every((height) => height >= 300 + 24 + 40)).toBe(true);
    vi.unstubAllGlobals();
  });

  it("菜单窄，点「快照」列窗口时变宽，Esc 回菜单再变窄", async () => {
    // jsdom 没有布局：面板宽度按外层按视图给的宽度算，高度随便给一个
    vi.spyOn(HTMLElement.prototype, "offsetWidth", "get").mockImplementation(function (this: HTMLElement) {
      return this.classList.contains("palette") ? parseFloat((this.parentElement as HTMLElement).style.width) || 0 : 0;
    });
    vi.spyOn(HTMLElement.prototype, "offsetHeight", "get").mockImplementation(function (this: HTMLElement) {
      return this.isConnected && this.classList.contains("palette") ? 250 : 0;
    });
    const { calls, user } = await renderPalette((command) => (command === "list_capturable_windows" ? windowList : undefined));
    const widths = () =>
      calls.filter((call) => call.command === "resize_quick_menu").map((call) => (call.payload as { width: number }).width);
    // 面板宽 + 左右各 40
    await waitFor(() => expect(widths().at(-1)).toBe(320 + 80));

    await user.click(await screen.findByRole("option", { name: "快照" }));
    expect(await screen.findByRole("listbox", { name: "选择要截取的窗口" })).toBeTruthy();
    await waitFor(() => expect(widths().at(-1)).toBe(400 + 80));

    await user.keyboard("{Escape}");
    await waitFor(() => expect(widths().at(-1)).toBe(320 + 80));
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
