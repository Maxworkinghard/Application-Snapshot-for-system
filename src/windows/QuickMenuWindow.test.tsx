import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { InvokeArgs } from "@tauri-apps/api/core";
import type { RecordingStatus } from "../types";
import { setupTauriMock } from "../test/tauri";

const windowList = [{ id: 73, appName: "Visual Studio Code", title: "notes.md", iconDataUrl: null }];

describe("quick menu recording picker", () => {
  it("opens the recording picker and starts recording the selected window id", async () => {
    const calls: Array<{ command: string; payload?: InvokeArgs }> = [];
    setupTauriMock((command, payload) => {
      calls.push({ command, payload });
      if (command === "get_recording_status") {
        return { active: false, target: null, startedAt: null } satisfies RecordingStatus;
      }
      if (command === "list_capturable_windows") return windowList;
      if (command === "toggle_recording") {
        return { active: true, target: "Visual Studio Code", startedAt: 123 } satisfies RecordingStatus;
      }
      return undefined;
    }, { currentWindow: "quick-menu", shouldMockEvents: true });
    vi.resetModules();
    const { QuickMenuWindow } = await import("./QuickMenuWindow");
    const user = userEvent.setup();

    render(<QuickMenuWindow />);
    await user.click(await screen.findByRole("button", { name: "录制" }));
    expect(await screen.findByText("选择要录制的窗口")).toBeTruthy();
    await user.click(await screen.findByRole("button", { name: /Visual Studio Code/ }));
    await screen.findByText("已开始录制 Visual Studio Code");

    await waitFor(() => {
      expect(calls.find((call) => call.command === "toggle_recording")?.payload).toEqual({ targetId: 73 });
    });
    expect(calls.some((call) => call.command === "list_capturable_windows")).toBe(true);
  });

  it("stops an active recording directly without opening the picker", async () => {
    const calls: Array<{ command: string; payload?: InvokeArgs }> = [];
    setupTauriMock((command, payload) => {
      calls.push({ command, payload });
      if (command === "get_recording_status") {
        return { active: true, target: "Visual Studio Code", startedAt: 123 } satisfies RecordingStatus;
      }
      if (command === "toggle_recording") {
        return { active: false, target: null, startedAt: null } satisfies RecordingStatus;
      }
      return undefined;
    }, { currentWindow: "quick-menu", shouldMockEvents: true });
    vi.resetModules();
    const { QuickMenuWindow } = await import("./QuickMenuWindow");
    const user = userEvent.setup();

    render(<QuickMenuWindow />);
    await user.click(await screen.findByRole("button", { name: "停止录制" }));
    await screen.findByText("录制已保存");

    expect(calls.find((call) => call.command === "toggle_recording")?.payload).toEqual({ targetId: null });
    expect(calls.some((call) => call.command === "list_capturable_windows")).toBe(false);
    expect(screen.queryByText("选择要录制的窗口")).toBeNull();
  });
});
