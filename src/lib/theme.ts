import { emit, listen } from "@tauri-apps/api/event";

/** 实际落到 DOM 上的主题，只有两种 */
export type ThemeMode = "light" | "dark";
/** 用户的选择。system 表示跟随系统，实际是浅是深要解析后才知道 */
export type ThemePreference = ThemeMode | "system";

const STORAGE_KEY = "snapshot-theme";
const DARK_QUERY = "(prefers-color-scheme: dark)";

function systemTheme(): ThemeMode {
  return window.matchMedia?.(DARK_QUERY).matches ? "dark" : "light";
}

/** 读取用户选择。没存过、存了认不得的值、或压根读不到，都按跟随系统 */
export function readThemePreference(): ThemePreference {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved === "light" || saved === "dark" || saved === "system") return saved;
  } catch {
    // 隐私模式或存储被禁用时读不到，跟随系统即可
  }
  return "system";
}

/** 把用户选择解析成真正要应用的主题 */
export function resolveTheme(preference: ThemePreference): ThemeMode {
  return preference === "system" ? systemTheme() : preference;
}

/** 开机即用的快捷方式：读选择并解析成当前该用哪个 */
export function readTheme(): ThemeMode {
  return resolveTheme(readThemePreference());
}

/** 主窗口、桌宠、快捷菜单等 WebView 各自独立，都要调用一次 */
export function applyTheme(mode: ThemeMode): void {
  document.documentElement.dataset.theme = mode;
}

export function persistThemePreference(preference: ThemePreference): void {
  try {
    localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    // 存不下就只在本次会话生效，不影响渲染
  }
}

/**
 * 跟随系统时，系统外观变了要立刻跟上，否则得重开窗口才生效。
 * 返回取消监听的函数；matchMedia 不可用时返回空操作。
 */
export function watchSystemTheme(onChange: (mode: ThemeMode) => void): () => void {
  const query = window.matchMedia?.(DARK_QUERY);
  if (!query) return () => {};
  const handler = (event: MediaQueryListEvent) => onChange(event.matches ? "dark" : "light");
  query.addEventListener("change", handler);
  return () => query.removeEventListener("change", handler);
}

const THEME_EVENT = "theme-changed";
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * 把主题广播给其它窗口。桌宠、快捷菜单等各是独立 WebView，
 * 只在启动时各读一次存储；不广播的话主窗口改完它们要重开才跟上。
 */
export function broadcastTheme(mode: ThemeMode): void {
  if (!inTauri) return;
  void emit(THEME_EVENT, mode);
}

/** 每个窗口都监听一次；返回取消监听的函数 */
export function listenThemeChanges(onChange: (mode: ThemeMode) => void): () => void {
  if (!inTauri) return () => {};
  const pending = listen<ThemeMode>(THEME_EVENT, (event) => onChange(event.payload));
  return () => {
    void pending.then((unlisten) => unlisten());
  };
}
