import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Camera, ChevronLeft, CircleStop, TextCursorInput, Video, X } from "lucide-react";
import {
  captureWindow,
  getRecordingStatus,
  hideQuickMenu,
  listWindows,
  polishClipboard,
  setQuickMenuExpanded,
  toggleRecording,
} from "../lib/backend";
import type { CapturableWindow, RecordingStatus } from "../types";

export function QuickMenuWindow() {
  const [page, setPage] = useState<"menu" | "windows">("menu");
  const [windows, setWindows] = useState<CapturableWindow[]>([]);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState("");
  const [recording, setRecording] = useState<RecordingStatus>({ active: false, target: null, startedAt: null });

  useEffect(() => {
    void getRecordingStatus().then(setRecording);
    const pending = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) {
        setPage("menu");
        setStatus("");
        void setQuickMenuExpanded(false);
      } else {
        void hideQuickMenu();
      }
    });
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  async function openWindows() {
    setLoading(true);
    setStatus("");
    setPage("windows");
    void setQuickMenuExpanded(true);
    try {
      setWindows(await listWindows());
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  function returnToMenu() {
    setPage("menu");
    setStatus("");
    void setQuickMenuExpanded(false);
  }

  async function capture(id: number) {
    setStatus("正在截取…");
    try {
      setStatus(await captureWindow(id));
      window.setTimeout(() => void hideQuickMenu(), 700);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function record() {
    try {
      const next = await toggleRecording();
      setRecording(next);
      setStatus(next.active ? `正在录制 ${next.target ?? "应用窗口"}` : "录制已保存");
      if (!next.active) window.setTimeout(() => void hideQuickMenu(), 700);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  async function polish() {
    setStatus("正在润色剪贴板…");
    try {
      setStatus(await polishClipboard());
      window.setTimeout(() => void hideQuickMenu(), 900);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
    }
  }

  return (
    <div className="quick-menu">
      {page === "windows" && (
        <header className="quick-header">
          <button onClick={returnToMenu}><ChevronLeft size={17} /></button>
          <strong>选择应用窗口</strong>
          <button onClick={() => void hideQuickMenu()}><X size={16} /></button>
        </header>
      )}

      {page === "menu" ? (
        <div className="quick-actions menu-only">
          <button onClick={() => void openWindows()}>
            <span className="quick-action-icon violet"><Camera size={16} /></span>
            <strong>应用快照</strong>
          </button>
          <button onClick={() => void record()}>
            <span className="quick-action-icon red">{recording.active ? <CircleStop size={16} /> : <Video size={16} />}</span>
            <strong>{recording.active ? "停止录制" : "录制"}</strong>
          </button>
          <button onClick={() => void polish()}>
            <span className="quick-action-icon amber"><TextCursorInput size={16} /></span>
            <strong>润色 Prompt</strong>
          </button>
        </div>
      ) : (
        <div className="window-picker">
          {loading && <p className="quick-empty">正在读取窗口…</p>}
          {!loading && windows.length === 0 && <p className="quick-empty">没有找到可截取的窗口</p>}
          {windows.map((item) => (
            <button key={item.id} onClick={() => void capture(item.id)}>
              <span className="window-icon">{item.iconDataUrl ? <img src={item.iconDataUrl} alt="" /> : <Camera size={17} />}</span>
              <span><strong>{item.appName}</strong><small>{item.title}</small></span>
            </button>
          ))}
        </div>
      )}

      {status && page === "windows" && <div className="quick-status">{status}</div>}
    </div>
  );
}
