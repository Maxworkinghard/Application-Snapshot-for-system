import type { ActivityEntry } from "../types";

/** 猫播报用的一句完整的话 */
export function narrate(entry: ActivityEntry): string {
  switch (entry.kind) {
    case "capture":
      return `${entry.title} 截好了。`;
    case "record":
      return `录好了：${entry.title}${entry.meta ? `，${entry.meta}` : ""}。`;
    case "polish":
      return `润色好了，${entry.meta ?? "结果已生成"}。`;
    case "error":
      return `${entry.title}：${entry.detail ?? "原因未知"}`;
    default:
      return entry.title;
  }
}

/** 流水、时间线里的一行短标签 */
export function shortLabel(entry: ActivityEntry): string {
  switch (entry.kind) {
    case "capture":
      return `截图 ${entry.title}`;
    case "record":
      return `录制 ${entry.meta ?? ""} ${entry.title}`.replace(/\s+/g, " ").trim();
    case "polish":
      return `润色 ${entry.meta ?? ""}`.trim();
    case "error":
      return entry.title;
    default:
      return entry.title;
  }
}

/** 「1920 × 1080」→ 宽高；解析不了返回 null */
export function parseSize(meta?: string): { width: number; height: number } | null {
  const match = meta?.match(/(\d+)\s*[×x]\s*(\d+)/);
  if (!match) return null;
  return { width: Number(match[1]), height: Number(match[2]) };
}
