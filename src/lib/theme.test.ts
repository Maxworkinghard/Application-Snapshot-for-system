import { describe, expect, it, vi } from "vitest";
import { setupTauriMock } from "../test/tauri";

describe("theme preferences", () => {
  it("resolves light, dark, and system preferences", async () => {
    setupTauriMock();
    Object.defineProperty(window, "matchMedia", {
      configurable: true,
      value: vi.fn().mockReturnValue({ matches: true, addEventListener: vi.fn(), removeEventListener: vi.fn() }),
    });
    vi.resetModules();
    const { resolveTheme } = await import("./theme");

    expect(resolveTheme("light")).toBe("light");
    expect(resolveTheme("dark")).toBe("dark");
    expect(resolveTheme("system")).toBe("dark");
  });

  it("keeps stored light and dark values and defaults invalid or unreadable storage to system", async () => {
    setupTauriMock();
    vi.resetModules();
    const { readThemePreference } = await import("./theme");

    localStorage.setItem("snapshot-theme", "light");
    expect(readThemePreference()).toBe("light");
    localStorage.setItem("snapshot-theme", "dark");
    expect(readThemePreference()).toBe("dark");
    localStorage.setItem("snapshot-theme", "not-a-theme");
    expect(readThemePreference()).toBe("system");
    localStorage.removeItem("snapshot-theme");
    expect(readThemePreference()).toBe("system");

    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("storage disabled");
    });
    expect(readThemePreference()).toBe("system");
  });
});
