import { emit, listen } from "@tauri-apps/api/event";

/**
 * 只影响界面的两项偏好：布局主题和动效强度。和颜色主题一样存在本机 localStorage，
 * 不进后端设置文件——换台机器、重装都无所谓，界面自己的事界面自己记。
 */

export type LayoutTheme = "companion" | "timeline" | "ledger" | "topbar";
export type MotionPreference = "system" | "full" | "reduced";
export type MotionMode = "full" | "reduced";

const LAYOUT_KEY = "snapshot-layout";
const MOTION_KEY = "snapshot-motion";
const REDUCED_QUERY = "(prefers-reduced-motion: reduce)";

export const LAYOUTS: LayoutTheme[] = ["companion", "timeline", "ledger", "topbar"];

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    // 存不下就只在本次会话生效
  }
}

/** 默认是「伴侣侧栏」 */
export function readLayout(): LayoutTheme {
  const saved = read(LAYOUT_KEY);
  return LAYOUTS.includes(saved as LayoutTheme) ? (saved as LayoutTheme) : "companion";
}

export function persistLayout(layout: LayoutTheme) {
  write(LAYOUT_KEY, layout);
}

export function readMotionPreference(): MotionPreference {
  const saved = read(MOTION_KEY);
  return saved === "full" || saved === "reduced" ? saved : "system";
}

export function persistMotionPreference(preference: MotionPreference) {
  write(MOTION_KEY, preference);
}

export function resolveMotion(preference: MotionPreference): MotionMode {
  if (preference !== "system") return preference;
  return window.matchMedia?.(REDUCED_QUERY).matches ? "reduced" : "full";
}

/** 所有动效时长都挂在 CSS 变量上；reduced 时变量整体退成 120ms 淡入淡出 */
export function applyMotion(mode: MotionMode) {
  document.documentElement.dataset.motion = mode;
}

export function watchSystemMotion(onChange: (mode: MotionMode) => void): () => void {
  const query = window.matchMedia?.(REDUCED_QUERY);
  if (!query) return () => {};
  const handler = (event: MediaQueryListEvent) => onChange(event.matches ? "reduced" : "full");
  query.addEventListener("change", handler);
  return () => query.removeEventListener("change", handler);
}

const MOTION_EVENT = "motion-changed";
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** 桌宠、输入框是独立 WebView，改了动效要广播给它们 */
export function broadcastMotion(mode: MotionMode) {
  if (!inTauri) return;
  void emit(MOTION_EVENT, mode);
}

export function listenMotionChanges(onChange: (mode: MotionMode) => void): () => void {
  if (!inTauri) return () => {};
  const pending = listen<MotionMode>(MOTION_EVENT, (event) => onChange(event.payload));
  return () => {
    void pending.then((unlisten) => unlisten());
  };
}
