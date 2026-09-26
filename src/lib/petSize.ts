/** 偏好里「桌宠大小」能调的范围（百分比），和 Rust settings.rs 的 PET_SCALE_RANGE 一致 */
export const PET_SCALE = { min: 50, max: 300, step: 10, default: 100 } as const;

/**
 * 桌宠窗口的边长（逻辑像素）。基准是屏幕短边的 6%，限在 58–72 之间，100% 时就是原来的大小；
 * 偏好里的百分比在基准上缩放，再大也不超过屏幕短边的四成。
 */
export function petWindowSize(shortEdge: number, scalePercent: number): number {
  const base = Math.max(58, Math.min(72, shortEdge * 0.06));
  return Math.round(Math.min(base * (scalePercent / 100), shortEdge * 0.4));
}
