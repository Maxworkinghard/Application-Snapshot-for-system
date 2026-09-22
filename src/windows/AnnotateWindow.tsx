import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

type Tool = "pen" | "rect";

/**
 * 截后标注：展示捕获图，支持画笔/矩形，完成后复制、另存或关闭。
 */
export function AnnotateWindow() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const drawing = useRef(false);
  const start = useRef<{ x: number; y: number } | null>(null);
  const snapshot = useRef<ImageData | null>(null);
  const [tool, setTool] = useState<Tool>("pen");
  const [title, setTitle] = useState("标注截图");
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  const loadImage = useCallback(async () => {
    try {
      const [dataUrl, nextTitle] = await Promise.all([
        invoke<string | null>("get_annotate_image"),
        invoke<string>("annotate_get_title"),
      ]);
      if (nextTitle) setTitle(nextTitle);
      if (!dataUrl) {
        setStatus("没有待标注的截图");
        return;
      }
      const img = new Image();
      img.onload = () => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;
        ctx.drawImage(img, 0, 0);
        setStatus("");
      };
      img.onerror = () => setStatus("图片加载失败");
      img.src = dataUrl;
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }, []);

  useEffect(() => {
    void loadImage();
    void getCurrentWindow().setFocus();
    let unlisten: (() => void) | undefined;
    void listen("annotate-ready", () => {
      void loadImage();
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, [loadImage]);

  function canvasPoint(event: React.MouseEvent<HTMLCanvasElement>) {
    const canvas = canvasRef.current!;
    const rect = canvas.getBoundingClientRect();
    const scaleX = canvas.width / rect.width;
    const scaleY = canvas.height / rect.height;
    return {
      x: (event.clientX - rect.left) * scaleX,
      y: (event.clientY - rect.top) * scaleY,
    };
  }

  function onPointerDown(event: React.MouseEvent<HTMLCanvasElement>) {
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    drawing.current = true;
    const point = canvasPoint(event);
    start.current = point;
    snapshot.current = ctx.getImageData(0, 0, canvas.width, canvas.height);
    if (tool === "pen") {
      ctx.strokeStyle = "#ff3b30";
      ctx.lineWidth = Math.max(2, canvas.width / 400);
      ctx.lineCap = "round";
      ctx.lineJoin = "round";
      ctx.beginPath();
      ctx.moveTo(point.x, point.y);
    }
  }

  function onPointerMove(event: React.MouseEvent<HTMLCanvasElement>) {
    if (!drawing.current || !start.current) return;
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    const point = canvasPoint(event);
    if (tool === "pen") {
      ctx.lineTo(point.x, point.y);
      ctx.stroke();
      return;
    }
    if (snapshot.current) ctx.putImageData(snapshot.current, 0, 0);
    const x = Math.min(start.current.x, point.x);
    const y = Math.min(start.current.y, point.y);
    const w = Math.abs(point.x - start.current.x);
    const h = Math.abs(point.y - start.current.y);
    ctx.strokeStyle = "#007aff";
    ctx.lineWidth = Math.max(2, canvas.width / 400);
    ctx.strokeRect(x, y, w, h);
  }

  function onPointerUp() {
    drawing.current = false;
    start.current = null;
    snapshot.current = null;
  }

  async function exportPng(): Promise<string> {
    const canvas = canvasRef.current;
    if (!canvas) throw new Error("画布不可用");
    return canvas.toDataURL("image/png");
  }

  async function copy() {
    setBusy(true);
    setStatus("");
    try {
      const imageData = await exportPng();
      const message = await invoke<string>("annotate_copy", { imageData });
      setStatus(message);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function save() {
    setBusy(true);
    setStatus("");
    try {
      const imageData = await exportPng();
      const message = await invoke<string>("annotate_save", { imageData });
      setStatus(message);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function close() {
    await invoke("annotate_close");
  }

  return (
    <div className="annotate-root">
      <header className="annotate-toolbar">
        <strong className="annotate-title">{title || "标注截图"}</strong>
        <div className="annotate-tools" role="group" aria-label="标注工具">
          <button
            type="button"
            className={tool === "pen" ? "is-active" : ""}
            onClick={() => setTool("pen")}
          >
            画笔
          </button>
          <button
            type="button"
            className={tool === "rect" ? "is-active" : ""}
            onClick={() => setTool("rect")}
          >
            矩形
          </button>
        </div>
        <div className="annotate-actions">
          <button type="button" className="primary-button" disabled={busy} onClick={() => void copy()}>
            复制
          </button>
          <button type="button" disabled={busy} onClick={() => void save()}>
            另存为…
          </button>
          <button type="button" disabled={busy} onClick={() => void close()}>
            关闭
          </button>
        </div>
      </header>
      <div className="annotate-stage">
        <canvas
          ref={canvasRef}
          className="annotate-canvas"
          onMouseDown={onPointerDown}
          onMouseMove={onPointerMove}
          onMouseUp={onPointerUp}
          onMouseLeave={onPointerUp}
        />
      </div>
      {status ? <div className="annotate-status">{status}</div> : null}
    </div>
  );
}
