import { useEffect, useState } from "react";
import {
  getClipboardState,
  getRecordingStatus,
  listActivity,
  onActivity,
  onClipboardChanged,
  onRecordingChanged,
} from "../lib/backend";
import type { ActivityEntry, ClipboardState, RecordingStatus } from "../types";
import { askPetAnimation, onPetAnimation, type PetAnimation } from "../lib/petSync";

/** 订阅一个 Tauri 事件；卸载时取消 */
function useTauriListener(subscribe: () => Promise<() => void>) {
  useEffect(() => {
    const pending = subscribe();
    return () => {
      void pending.then((unlisten) => unlisten()).catch(() => {});
    };
    // 只订阅一次：回调里用的都是 setState
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}

export function useActivityLog() {
  const [entries, setEntries] = useState<ActivityEntry[]>([]);
  useEffect(() => {
    listActivity()
      .then((list) => setEntries(Array.isArray(list) ? list : []))
      .catch(() => {});
  }, []);
  useTauriListener(() =>
    onActivity((entry) => setEntries((list) => [entry, ...list.filter((item) => item.id !== entry.id)].slice(0, 300))),
  );
  return entries;
}

export function useClipboardState() {
  const [state, setState] = useState<ClipboardState | null>(null);
  useEffect(() => {
    getClipboardState()
      .then((value) => setState(value ?? null))
      .catch(() => {});
  }, []);
  useTauriListener(() => onClipboardChanged((value) => setState(value ?? null)));
  return state;
}

const idle: RecordingStatus = { active: false, target: null, startedAt: null };

export function useRecordingStatus() {
  const [status, setStatus] = useState<RecordingStatus>(idle);
  useEffect(() => {
    getRecordingStatus()
      .then((value) => setStatus(value ?? idle))
      .catch(() => {});
  }, []);
  useTauriListener(() => onRecordingChanged((value) => setStatus(value ?? idle)));
  return status;
}

/** 需要跟着时间走的显示（倒计时、录制时长）；不需要时不开计时器 */
export function useNow(enabled: boolean, intervalMs = 1000) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!enabled) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(timer);
  }, [enabled, intervalMs]);
  return now;
}

/** 窗口在不在前台：不在前台时猫停在静态帧，省电也不打扰 */
/** 主窗口是否在屏幕上（没最小化、没藏起来）；失去焦点也算在 */
export function useWindowVisible() {
  const [visible, setVisible] = useState(() => (typeof document === "undefined" ? true : !document.hidden));
  useEffect(() => {
    const update = () => setVisible(!document.hidden);
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);
  return visible;
}

/** 桌宠此刻在播的动作（桌宠窗口广播，这里跟着）；还没收到时是 null */
export function usePetAnimation() {
  const [value, setValue] = useState<PetAnimation | null>(null);
  useEffect(() => {
    const stop = onPetAnimation(setValue);
    askPetAnimation();
    return stop;
  }, []);
  return value;
}

