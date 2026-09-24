import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { snapshotUrl } from "../lib/media";
import { formatBytes, formatClock, dayLabel } from "../lib/format";
import { growFrom, shrinkTo } from "../lib/motion";
import type { SnapshotRecord } from "../types";

/** 列表里这张缩略图此刻在哪（它可能已经滚出视野，找不到就淡出） */
function thumbRect(id: string): DOMRect | null {
  const safe = typeof CSS !== "undefined" && CSS.escape ? CSS.escape(id) : id;
  const node = document.querySelector<HTMLElement>(`[data-thumb-id="${safe}"]`);
  if (!node) return null;
  const rect = node.getBoundingClientRect();
  return rect.width ? rect : null;
}

/**
 * 放大查看。图从缩略图原位长出来，关掉时缩回原位——始终知道它在列表哪儿。
 * ← → 切换，Esc 返回，Del 删除（按两次），Ctrl C 复制。
 */
export function Lightbox({
  records,
  startId,
  onClose,
  onCopy,
  onReveal,
  onDelete,
}: {
  records: SnapshotRecord[];
  startId: string;
  onClose: () => void;
  onCopy: (id: string) => void;
  onReveal: () => void;
  onDelete: (id: string) => void;
}) {
  const [currentId, setCurrentId] = useState(startId);
  const [box, setBox] = useState<{ width: number; height: number } | null>(null);
  const [armedDelete, setArmedDelete] = useState(false);
  const stageRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const opened = useRef(false);
  const closing = useRef(false);

  const index = Math.max(0, records.findIndex((item) => item.id === currentId));
  const record = records[index];

  // 按原图比例算出显示尺寸，这样元素的框就是图的框，放大动画才对得上
  useLayoutEffect(() => {
    const stage = stageRef.current;
    if (!stage || !record) return;
    const measure = () => {
      const { width, height } = stage.getBoundingClientRect();
      const scale = Math.min(width / record.width, height / record.height, 1);
      setBox({ width: Math.round(record.width * scale), height: Math.round(record.height * scale) });
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(measure);
    observer.observe(stage);
    return () => observer.disconnect();
  }, [record]);

  useLayoutEffect(() => {
    if (!box || opened.current || !imageRef.current) return;
    opened.current = true;
    const from = thumbRect(startId);
    if (from) growFrom(imageRef.current, from);
  }, [box, startId]);

  const close = useCallback(async () => {
    if (closing.current) return;
    closing.current = true;
    if (imageRef.current && record) await shrinkTo(imageRef.current, thumbRect(record.id));
    onClose();
  }, [onClose, record]);

  const step = useCallback(
    (delta: number) => {
      const next = records[index + delta];
      if (next) {
        setArmedDelete(false);
        setCurrentId(next.id);
      }
    },
    [index, records],
  );

  const remove = useCallback(() => {
    if (!record) return;
    if (!armedDelete) {
      setArmedDelete(true);
      return;
    }
    const next = records[index + 1] ?? records[index - 1];
    onDelete(record.id);
    setArmedDelete(false);
    if (next) setCurrentId(next.id);
    else onClose();
  }, [armedDelete, index, onClose, onDelete, record, records]);

  useEffect(() => {
    if (!armedDelete) return;
    const timer = window.setTimeout(() => setArmedDelete(false), 2400);
    return () => window.clearTimeout(timer);
  }, [armedDelete]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void close();
      } else if (event.key === "ArrowLeft") step(-1);
      else if (event.key === "ArrowRight") step(1);
      else if (event.key === "Delete") remove();
      else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "c" && record) onCopy(record.id);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [close, onCopy, record, remove, step]);

  if (!record) return null;

  // 挂到 body 上：页面切换的位移动画会让祖先变成定位容器，放在里面会被框住
  return createPortal(
    <div className="lightbox" role="dialog" aria-modal="true" aria-label={`放大查看：${record.appName}`}>
      <div className="lightbox-head">
        <button type="button" className="lightbox-back" onClick={() => void close()}>
          <svg width="7" height="12" viewBox="0 0 7 12" aria-hidden="true"><path d="M6 1L1 6l5 5" fill="none" stroke="currentColor" strokeWidth="1.4" /></svg>
          返回
        </button>
        <span className="lightbox-title" title={record.appName}>{record.appName}</span>
        <span className="lightbox-count mono">{index + 1} / {records.length}</span>
      </div>

      <div className="lightbox-body">
        <button type="button" className="lightbox-arrow" aria-label="上一张" disabled={index === 0} onClick={() => step(-1)}>
          <svg width="12" height="22" viewBox="0 0 12 22" aria-hidden="true"><path d="M11 1L1 11l10 10" fill="none" stroke="currentColor" strokeWidth="1.4" /></svg>
        </button>
        <div className="lightbox-stage" ref={stageRef}>
          {box && (
            <img
              key={record.id}
              ref={imageRef}
              className={`lightbox-image ${opened.current ? "is-swapped" : ""}`}
              src={snapshotUrl(record.id)}
              alt={record.appName}
              style={{ width: box.width, height: box.height }}
              draggable={false}
            />
          )}
        </div>
        <button type="button" className="lightbox-arrow" aria-label="下一张" disabled={index >= records.length - 1} onClick={() => step(1)}>
          <svg width="12" height="22" viewBox="0 0 12 22" aria-hidden="true"><path d="M1 1l10 10L1 21" fill="none" stroke="currentColor" strokeWidth="1.4" /></svg>
        </button>
      </div>

      <div className="lightbox-foot">
        <span className="mono lightbox-meta">
          {record.width} × {record.height} · {formatBytes(record.sizeBytes)} · {record.fileName.split(".").pop()?.toUpperCase()} · {dayLabel(record.createdAt)} {formatClock(record.createdAt)}
        </span>
        <span className="lightbox-actions">
          <button type="button" onClick={() => onCopy(record.id)}>复制<span className="mono">Ctrl C</span></button>
          <button type="button" onClick={onReveal}>在文件夹中显示</button>
          <button type="button" className="is-danger" onClick={remove} aria-live="polite">
            {armedDelete ? "再按一次删除" : "删除"}<span className="mono">Del</span>
          </button>
        </span>
      </div>
    </div>,
    document.body,
  );
}
