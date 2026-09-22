import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";

type Point = { x: number; y: number };

/**
 * 全屏半透明框选：拖出矩形后把物理像素坐标回传给后端，Esc 取消。
 * 坐标用 screenX/screenY，与 xcap Monitor::from_point / capture_region 对齐。
 */
export function RegionPickerWindow() {
  const [origin, setOrigin] = useState<Point | null>(null);
  const [current, setCurrent] = useState<Point | null>(null);
  const screenOrigin = useRef<Point | null>(null);
  const dragging = useRef(false);

  const cancel = useCallback(() => {
    void invoke("cancel_region_capture");
  }, []);

  const commit = useCallback((a: Point, b: Point) => {
    const x = Math.min(a.x, b.x);
    const y = Math.min(a.y, b.y);
    const width = Math.abs(a.x - b.x);
    const height = Math.abs(a.y - b.y);
    if (width < 4 || height < 4) {
      void invoke("cancel_region_capture");
      return;
    }
    void invoke("complete_region_capture", { x, y, width, height });
  }, []);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        cancel();
      }
    };
    window.addEventListener("keydown", onKey);
    void getCurrentWindow().setFocus();
    return () => window.removeEventListener("keydown", onKey);
  }, [cancel]);

  const rect =
    origin && current
      ? {
          left: Math.min(origin.x, current.x),
          top: Math.min(origin.y, current.y),
          width: Math.abs(origin.x - current.x),
          height: Math.abs(origin.y - current.y),
        }
      : null;

  return (
    <div
      className="region-picker-root"
      onMouseDown={(event) => {
        dragging.current = true;
        screenOrigin.current = { x: event.screenX, y: event.screenY };
        const point = { x: event.clientX, y: event.clientY };
        setOrigin(point);
        setCurrent(point);
      }}
      onMouseMove={(event) => {
        if (!dragging.current) return;
        setCurrent({ x: event.clientX, y: event.clientY });
      }}
      onMouseUp={(event) => {
        if (!dragging.current || !screenOrigin.current) return;
        dragging.current = false;
        setCurrent({ x: event.clientX, y: event.clientY });
        commit(screenOrigin.current, { x: event.screenX, y: event.screenY });
      }}
    >
      <div className="region-picker-hint">拖拽选择区域 · Esc 取消 / Drag to select · Esc cancel</div>
      {rect && (
        <div
          className="region-picker-rect"
          style={{
            left: rect.left,
            top: rect.top,
            width: rect.width,
            height: rect.height,
          }}
        />
      )}
    </div>
  );
}
