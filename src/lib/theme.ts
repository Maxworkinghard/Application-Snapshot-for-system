export type ThemeMode = "light" | "dark";

const STORAGE_KEY = "snapshot-theme";

/** 读取已保存的主题；没存过就跟随系统 */
export function readTheme(): ThemeMode {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "light" || saved === "dark") return saved;
  } catch {
    // 隐私模式或存储被禁用时读不到，按系统偏好走即可
  }
  return window.matchMedia?.("(prefers-color-scheme: dark)").matches ? "dark" : "light";
}

/** 主窗口、桌宠、便携坞三个 WebView 各自独立，都要调用一次 */
export function applyTheme(mode: ThemeMode): void {
  document.documentElement.dataset.theme = mode;
}

export function persistTheme(mode: ThemeMode): void {
  try {
    localStorage.setItem(STORAGE_KEY, mode);
  } catch {
    // 存不下就只在本次会话生效，不影响渲染
  }
}
