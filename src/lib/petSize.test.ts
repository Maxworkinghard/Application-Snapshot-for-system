import { describe, expect, it } from "vitest";
import { petWindowSize } from "./petSize";

describe("桌宠大小", () => {
  it("100% 是屏幕短边的五分之一，换屏幕跟着变", () => {
    expect(petWindowSize(852, 100)).toBe(170);
    expect(petWindowSize(1080, 100)).toBe(216);
    expect(petWindowSize(600, 100)).toBe(120);
  });

  it("按百分比在默认大小上缩放", () => {
    expect(petWindowSize(1000, 30)).toBe(60);
    expect(petWindowSize(1000, 150)).toBe(300);
  });

  it("再大也不超过屏幕短边的四成", () => {
    expect(petWindowSize(1000, 200)).toBe(400);
    expect(petWindowSize(1000, 250)).toBe(400);
  });
});
