import { describe, expect, it, vi } from "vitest";
import { fileNameOf, formatBytes, formatWhen, normalizeKey, shortcutKeys } from "./format";

describe("format helpers", () => {
  it("keeps the current zero-byte and large-file labels", () => {
    expect(formatBytes(0)).toBe("1 KB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1024.0 MB");
  });

  it("handles an empty path and both path separator styles", () => {
    expect(fileNameOf(null)).toBe("");
    expect(fileNameOf("")).toBe("");
    expect(fileNameOf("C:\\Users\\Max\\capture.mp4")).toBe("capture.mp4");
    expect(fileNameOf("/Users/max/capture.mp4")).toBe("capture.mp4");
  });

  it("normalizes space, arrows, and single-character keys", () => {
    expect(normalizeKey(" ")).toBe("Space");
    expect(normalizeKey("ArrowLeft")).toBe("Left");
    expect(normalizeKey("a")).toBe("A");
    expect(normalizeKey("Enter")).toBe("Enter");
  });

  it("splits shortcut labels into keys and formats relative times", () => {
    expect(shortcutKeys(null)).toEqual([]);
    expect(shortcutKeys("CommandOrControl+Shift+K")).toEqual(["Ctrl", "Shift", "K"]);

    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-09-23T00:00:00.000Z"));
    expect(formatWhen(Date.now() - 59_999)).toBe("刚刚");
    expect(formatWhen(Date.now() - 60_000)).toBe("1 分钟前");
    expect(formatWhen(Date.now() - 3_600_000)).toBe("1 小时前");
  });
});
