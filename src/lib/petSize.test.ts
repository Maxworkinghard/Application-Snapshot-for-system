import { describe, expect, it } from "vitest";
import { petWindowSize } from "./petSize";

describe("桌宠大小", () => {
  it("100% 就是原来的大小：屏幕短边的 6%，限在 58–72", () => {
    expect(petWindowSize(1040, 100)).toBe(62);
    expect(petWindowSize(600, 100)).toBe(58);
    expect(petWindowSize(2000, 100)).toBe(72);
  });

  it("按百分比在基准上缩放", () => {
    expect(petWindowSize(1040, 50)).toBe(31);
    expect(petWindowSize(1040, 200)).toBe(125);
  });

  it("再大也不超过屏幕短边的四成", () => {
    expect(petWindowSize(600, 300)).toBe(174);
    expect(petWindowSize(400, 300)).toBe(160);
  });
});
