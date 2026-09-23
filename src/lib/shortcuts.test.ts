import { describe, expect, it, vi } from "vitest";
import type { ShortcutBinding } from "../types";
import { invokeArgs, setupTauriMock, userAgents } from "../test/tauri";

const shortcutCommand = "plugin:global-shortcut|register";
const unregisterAllCommand = "plugin:global-shortcut|unregister_all";

function registeredShortcut(payload: Parameters<typeof invokeArgs>[0]): string {
  const shortcuts = invokeArgs(payload).shortcuts;
  return Array.isArray(shortcuts) ? String(shortcuts[0]) : "";
}

describe("global shortcut registration", () => {
  it("serializes concurrent calls that replace the same registration", async () => {
    const calls: string[] = [];
    const registered = new Set<string>();

    setupTauriMock(async (command, payload) => {
      if (command === unregisterAllCommand) {
        calls.push("unregisterAll");
        registered.clear();
        return;
      }
      if (command === shortcutCommand) {
        const key = registeredShortcut(payload);
        calls.push(`register:${key}`);
        await new Promise((resolve) => setTimeout(resolve, 15));
        if (registered.has(key)) throw new Error("already registered");
        registered.add(key);
      }
    });
    vi.resetModules();
    const { applyGlobalShortcuts } = await import("./shortcuts");
    const bindings: ShortcutBinding[] = [{ action: "snapshot", accelerator: "Alt+Shift+2" }];

    const first = applyGlobalShortcuts(bindings);
    const second = applyGlobalShortcuts(bindings);
    await expect(Promise.all([first, second])).resolves.toEqual([undefined, undefined]);

    expect(calls).toEqual([
      "unregisterAll",
      "register:Alt+Shift+2",
      "unregisterAll",
      "register:Alt+Shift+2",
    ]);
    expect(registered).toEqual(new Set(["Alt+Shift+2"]));
  });

  it("keeps successful shortcuts registered when one key fails", async () => {
    const registered = new Set<string>();
    let unregisterAllCalls = 0;

    setupTauriMock((command, payload) => {
      if (command === unregisterAllCommand) {
        unregisterAllCalls += 1;
        registered.clear();
        return;
      }
      if (command === shortcutCommand) {
        const key = registeredShortcut(payload);
        if (key === "Alt+Shift+F") throw new Error("occupied");
        registered.add(key);
      }
    });
    vi.resetModules();
    const { applyGlobalShortcuts } = await import("./shortcuts");
    const bindings: ShortcutBinding[] = [
      { action: "snapshot", accelerator: "Alt+Shift+2" },
      { action: "fullscreen", accelerator: "Alt+Shift+F" },
      { action: "polish", accelerator: "Alt+Shift+P" },
    ];

    await expect(applyGlobalShortcuts(bindings)).rejects.toThrow("Alt+Shift+F");
    expect(registered).toEqual(new Set(["Alt+Shift+2", "Alt+Shift+P"]));
    expect(unregisterAllCalls).toBe(1);
  });
});

describe("platform shortcut support", () => {
  it.each([
    ["macOS", userAgents.macos, false],
    ["Windows", userAgents.windows, false],
    ["Linux", userAgents.linux, true],
  ])("supports scrolling only on %s", async (_platform, userAgent, expected) => {
    setupTauriMock(() => undefined, { userAgent });
    vi.resetModules();
    const { platformSupports } = await import("./shortcuts");
    expect(platformSupports("scrolling")).toBe(expected);
  });
});
