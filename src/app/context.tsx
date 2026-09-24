import { createContext, useContext } from "react";
import type { LayoutTheme, MotionPreference } from "../lib/prefs";
import type { ThemePreference } from "../lib/theme";
import type {
  ActivityEntry,
  ClipboardState,
  NavPage,
  PlatformCapabilities,
  RecordingStatus,
  Settings,
} from "../types";

export type NoticeKind = "info" | "error";
export type Notice = { id: number; text: string; kind: NoticeKind; at: number };

/** 主窗口里各页面共用的东西。页面只管自己那一块，跨页的状态都从这里取 */
export type AppContextValue = {
  settings: Settings;
  onSaved: (settings: Settings) => void;
  /** 一句话提示。伴侣侧栏布局里由猫说出来，其它布局在左下角浮一行 */
  notify: (text: string, kind?: NoticeKind) => void;
  notice: Notice | null;
  dismissNotice: () => void;
  /** 启动时没注册上的快捷键 */
  conflicts: string[];
  setConflicts: (keys: string[]) => void;
  caps: PlatformCapabilities | null;
  activity: ActivityEntry[];
  clipboard: ClipboardState | null;
  recording: RecordingStatus;
  snapshotCount: number | null;
  layout: LayoutTheme;
  setLayout: (layout: LayoutTheme) => void;
  themePreference: ThemePreference;
  setThemePreference: (preference: ThemePreference) => void;
  motionPreference: MotionPreference;
  setMotionPreference: (preference: MotionPreference) => void;
  page: NavPage;
  navigate: (page: NavPage, options?: { previewId?: string; draft?: string }) => void;
  /** navigate 顺带的参数：从时间线点一张截图，进历史页直接放大它 */
  pendingPreviewId: string | null;
  consumePreviewId: () => void;
  pendingDraft: string | null;
  consumeDraft: () => void;
};

export const AppContext = createContext<AppContextValue | null>(null);

export function useApp(): AppContextValue {
  const value = useContext(AppContext);
  if (!value) throw new Error("useApp 只能在 App 里用");
  return value;
}

export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
