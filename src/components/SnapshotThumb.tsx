import { useState } from "react";
import { thumbUrl } from "../lib/media";

/** 按截图原本的宽高比算缩略图宽度：长截图就是一条窄条，带鱼屏就是一块宽块 */
function thumbWidth(width: number, height: number, thumbHeight: number, max = 200, min = 22) {
  if (!width || !height) return Math.round(thumbHeight * 16 / 9);
  return Math.max(min, Math.min(max, Math.round((thumbHeight * width) / height)));
}

/**
 * 一张快照的缩略图。取不到（文件被挪走）时留一块底色，不显示裂图。
 * 选择模式下左上角出现勾选框。
 */
export function SnapshotThumb({
  id,
  width,
  height,
  label,
  thumbHeight = 76,
  maxWidth = 200,
  selecting = false,
  selected = false,
  onPress,
}: {
  id: string;
  width: number;
  height: number;
  label: string;
  thumbHeight?: number;
  maxWidth?: number;
  selecting?: boolean;
  selected?: boolean;
  onPress: (element: HTMLButtonElement, event: React.MouseEvent) => void;
}) {
  const [failed, setFailed] = useState(false);
  const w = thumbWidth(width, height, thumbHeight, maxWidth);
  return (
    <button
      type="button"
      className={`thumb ${selected ? "is-selected" : ""} ${selecting ? "is-selecting" : ""} ${w < 36 ? "is-narrow" : ""}`}
      style={{ width: w, height: thumbHeight }}
      aria-label={selecting ? `选择：${label}` : `放大：${label}`}
      aria-pressed={selecting ? selected : undefined}
      data-thumb-id={id}
      onClick={(event) => onPress(event.currentTarget, event)}
    >
      {!failed && (
        <img src={thumbUrl(id)} alt="" loading="lazy" decoding="async" draggable={false} onError={() => setFailed(true)} />
      )}
      {selecting && (
        <span className="thumb-check" aria-hidden="true">
          {selected && (
            <svg width="10" height="8" viewBox="0 0 10 8"><path d="M1 4l2.8 2.8L9 1" fill="none" stroke="currentColor" strokeWidth="1.6" /></svg>
          )}
        </span>
      )}
    </button>
  );
}
