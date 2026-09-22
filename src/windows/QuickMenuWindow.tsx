import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  Camera,
  CheckCircle2,
  ChevronLeft,
  CircleStop,
  Loader2,
  TextCursorInput,
  Video,
  X,
  XCircle,
} from "lucide-react";
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

type QuickStatus = { kind: "info" | "busy" | "ok" | "error"; text: string };

/** 成功提示停留多久后自动收起菜单 */
const OK_HIDE_DELAY_MS = 900;

export function QuickMenuWindow() {
  const [page, setPage] = useState<"menu" | "windows">("menu");
  const [windows, setWindows] = useState<CapturableWindow[]>([]);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<QuickStatus | null>(null);
  const [recording, setRecording] = useState<RecordingStatus>({ active: false, target: null, startedAt: null });
  const hideTimer = useRef<number | null>(null);

  useEffect(() => {
    void getRecordingStatus().then(setRecording);
    const pending = getCurrentWindow().onFocusChanged(({ payload }) => {
      if (payload) {
        setPage("menu");
        setStatus(null);
        void setQuickMenuExpanded(false);
      } else {
        void hideQuickMenu();
      }
    });
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  function showStatus(next: QuickStatus) {
    if (hideTimer.current !== null) {
      window.clearTimeout(hideTimer.current);
      hideTimer.current = null;
    }
    setStatus(next);
  }

  /** 成功类提示短暂展示后自动收起；失败/进行中提示留在原地由用户处理 */
  function scheduleHideAfterOk() {
    hideTimer.current = window.setTimeout(() => void hideQuickMenu(), OK_HIDE_DELAY_MS);
  }

  async function openWindows() {
    setLoading(true);
    showStatus({ kind: "info", text: "" });
    setPage("windows");
    void setQuickMenuExpanded(true);
    try {
      setWindows(await listWindows());
    } catch (error) {
      showStatus({ kind: "error", text: error instanceof Error ? error.message : String(error) });
    } finally {
      setLoading(false);
    }
  }

  function returnToMenu() {
    setPage("menu");
    showStatus({ kind: "info", text: "" });
    void setQuickMenuExpanded(false);
  }

  async function capture(id: number) {
    showStatus({ kind: "busy", text: "正在截取…" });
    try {
      showStatus({ kind: "ok", text: await captureWindow(id) });
      scheduleHideAfterOk();
    } catch (error) {
      showStatus({ kind: "error", text: error instanceof Error ? error.message : String(error) });
    }
  }

  async function record(targetId?: number) {
    const wasActive = recording.active;
    showStatus(wasActive ? { kind: "busy", text: "正在停止并保存录制…" } : { kind: "busy", text: "正在启动录制…" });
    try {
      // 停止时不传目标；开始时传了就录指定窗口
      const next = await toggleRecording(wasActive ? undefined : targetId);
      setRecording(next);
      if (next.active) {
        // 录制进行中菜单不自动收起，方便随时回来点“停止”
        // Linux portal 会带回 message（须重新选窗/屏）；否则用默认文案
        showStatus({
          kind: "ok",
          text: next.message?.trim()
            || ("已开始录制 " + (next.target ?? "应用窗口")),
        });
      } else {
        showStatus({ kind: "ok", text: wasActive ? "录制已保存" : "录制已停止" });
        scheduleHideAfterOk();
      }
    } catch (error) {
      showStatus({ kind: "error", text: error instanceof Error ? error.message : String(error) });
    }
  }

  async function polish() {
    showStatus({ kind: "busy", text: "正在润色剪贴板，最长等待 180 秒…" });
    try {
      showStatus({ kind: "ok", text: await polishClipboard() });
      scheduleHideAfterOk();
    } catch (error) {
      showStatus({ kind: "error", text: error instanceof Error ? error.message : String(error) });
    }
  }

  const statusIcon =
    status?.kind === "busy" ? (
      <Loader2 size={13} className="spin" />
    ) : status?.kind === "ok" ? (
      <CheckCircle2 size={13} />
    ) : status?.kind === "error" ? (
      <XCircle size={13} />
    ) : null;

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
            <div className="window-picker-row" key={item.id}>
              <button className="window-pick-main" onClick={() => void capture(item.id)} title="截取这个窗口">
                <span className="window-icon">{item.iconDataUrl ? <img src={item.iconDataUrl} alt="" /> : <Camera size={17} />}</span>
                <span><strong>{item.appName}</strong><small>{item.title}</small></span>
              </button>
              <button
                className="window-pick-record"
                onClick={() => void record(item.id)}
                disabled={recording.active}
                title={recording.active ? "正在录制，先停止当前录制" : "录制这个窗口"}
              >
                <Video size={15} />
              </button>
            </div>
          ))}
        </div>
      )}

      {status && status.text && (
        <div className={"quick-status " + status.kind} role="status">
          {statusIcon}
          <span>{status.text}</span>
        </div>
      )}
    </div>
  );
}
