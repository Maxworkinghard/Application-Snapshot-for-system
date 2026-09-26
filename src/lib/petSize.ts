/** 偏好里「桌宠大小」能调的范围（百分比），和 Rust settings.rs 的 PET_SCALE_RANGE 一致 */
export const PET_SCALE = { min: 30, max: 200, step: 10, default: 100 } as const;

/**
 * 桌宠窗口的边长（逻辑像素）。100% 是屏幕短边的五分之一，换屏幕跟着变；
 * 偏好里的百分比在它上面缩放，再大也不超过屏幕短边的四成（正好是 200%）。
 */
export function petWindowSize(shortEdge: number, scalePercent: number): number {
  const base = shortEdge * 0.2;
  return Math.round(Math.min(base * (scalePercent / 100), shortEdge * 0.4));
}
