import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  getShortcutConflicts,
  loadSettings,
  onCaptureFeedback,
  onSettingsChanged,
  platformCapabilities,
} from "./lib/backend";
import { previewHintSound } from "./lib/sound";
import { shortcutKeys } from "./lib/format";
import {
  applyTheme,
  broadcastTheme,
  persistThemePreference,
  readThemePreference,
  resolveTheme,
  watchSystemTheme,
  type ThemePreference,
} from "./lib/theme";
import {
  applyMotion,
  broadcastMotion,
  persistLayout,
  persistMotionPreference,
  readLayout,
  readMotionPreference,
  resolveMotion,
  watchSystemMotion,
  type LayoutTheme,
  type MotionPreference,
} from "./lib/prefs";
import { viewTransition, type TransitionDirection } from "./lib/motion";
import { AppContext, errorText, useApp, type AppContextValue, type Notice, type NoticeKind } from "./app/context";
import { LedgerList, NavMark, NoticeLine, SideNav, TitleBar, TopNav } from "./app/Shell";
import { Companion } from "./components/Companion";
import { LoadingState } from "./components/LoadingState";
import { useActivityLog, useClipboardState, useRecordingStatus } from "./hooks/useLive";
import { ShortcutsPage } from "./pages/ShortcutsPage";
import { PromptPage } from "./pages/PromptPage";
import { HistoryPage } from "./pages/HistoryPage";
import { PetPage } from "./pages/PetPage";
import { PreferencesPage } from "./pages/PreferencesPage";
import { ThemesPage } from "./pages/ThemesPage";
import { TimelineHome } from "./pages/TimelineHome";
import { SettingsDoc } from "./pages/SettingsDoc";
import type { NavPage, PlatformCapabilities, Settings } from "./types";

/** 每种布局能到的页面，以及打开时落在哪一页 */
const LAYOUT_PAGES: Record<LayoutTheme, { home: NavPage; pages: NavPage[] }> = {
  companion: { home: "shortcuts", pages: ["shortcuts", "prompt", "history", "pet", "prefs", "themes"] },
  ledger: { home: "shortcuts", pages: ["shortcuts", "prompt", "history", "pet", "prefs", "themes"] },
  topbar: { home: "shortcuts", pages: ["shortcuts", "prompt", "history", "pet", "prefs", "themes"] },
  timeline: { home: "home", pages: ["home", "prompt", "history", "pet", "settings"] },
};

/** 两种布局之间换页时，对应到最接近的那一页 */
function mapPage(page: NavPage, layout: LayoutTheme): NavPage {
  const { home, pages } = LAYOUT_PAGES[layout];
  if (pages.includes(page)) return page;
  if (layout === "timeline") return ["shortcuts", "prefs", "themes"].includes(page) ? "settings" : home;
  if (page === "settings") return "themes";
  return home;
}

/** 导航里的先后顺序：往后走新页从下面（或右边）来，往回走从上面（或左边）来 */
const NAV_ORDER: Record<LayoutTheme, NavPage[]> = {
  companion: ["shortcuts", "prompt", "history", "pet", "prefs", "themes"],
  ledger: ["shortcuts", "prompt", "history", "pet", "prefs", "themes"],
  topbar: ["shortcuts", "prompt", "history", "pet", "prefs", "themes"],
  timeline: ["home", "prompt", "history", "pet", "settings"],
};

function directionFor(layout: LayoutTheme, from: NavPage, to: NavPage): TransitionDirection {
  const order = NAV_ORDER[layout];
  const forward = order.indexOf(to) >= order.indexOf(from);
  // 侧栏是竖排，换页上下走；标题栏里的导航是横排，换页左右走
  const vertical = layout === "companion" || layout === "ledger";
  return vertical ? (forward ? "down" : "up") : forward ? "right" : "left";
}

/** 普通提示自己消失，出错的多留一会儿（伴侣布局里由猫说，别处浮在左下角） */
const NOTICE_MS: Record<NoticeKind, number> = { info: 3200, error: 9000 };

function PageView({ page }: { page: NavPage }) {
  switch (page) {
    case "home":
      return <TimelineHome />;
    case "prompt":
      return <PromptPage />;
    case "history":
      return <HistoryPage />;
    case "pet":
      return <PetPage />;
    case "prefs":
      return <PreferencesPage />;
    case "themes":
      return <ThemesPage />;
    case "settings":
      return <SettingsDoc />;
    default:
      return <ShortcutsPage />;
  }
}

export function App() {
  // 默认值只在后端有一份；拿到之前显示加载态，不在前端再抄一份默认设置
  const [settings, setSettings] = useState<Settings | null>(null);
  const [layout, setLayoutState] = useState<LayoutTheme>(() => readLayout());
  const [page, setPage] = useState<NavPage>(() => LAYOUT_PAGES[readLayout()].home);
  const [themePreference, setThemePreferenceState] = useState<ThemePreference>(() => readThemePreference());
  const [motionPreference, setMotionPreferenceState] = useState<MotionPreference>(() => readMotionPreference());
  const [notice, setNotice] = useState<Notice | null>(null);
  const [conflicts, setConflicts] = useState<string[]>([]);
  const [caps, setCaps] = useState<PlatformCapabilities | null>(null);
  const [flash, setFlash] = useState(false);
  const [pendingPreviewId, setPendingPreviewId] = useState<string | null>(null);
  const [pendingDraft, setPendingDraft] = useState<string | null>(null);
  const noticeTimer = useRef<number | null>(null);
  const mainRef = useRef<HTMLElement>(null);

  const activity = useActivityLog();
  const clipboard = useClipboardState();
  const recording = useRecordingStatus();

  const notify = useCallback((text: string, kind: NoticeKind = "info") => {
    if (noticeTimer.current !== null) window.clearTimeout(noticeTimer.current);
    setNotice({ id: Date.now(), text, kind, at: Date.now() });
    noticeTimer.current = window.setTimeout(() => setNotice(null), NOTICE_MS[kind]);
  }, []);

  const dismissNotice = useCallback(() => setNotice(null), []);

  useEffect(() => {
    loadSettings()
      .then(setSettings)
      .catch((error) => notify(`读取设置失败：${errorText(error)}`, "error"));
    platformCapabilities()
      .then((value) => setCaps(value ?? null))
      .catch(() => setCaps(null));
    const pending = onSettingsChanged(setSettings);
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, [notify]);

  // 快捷键由后端在启动时注册；那一刻网页还没加载，注册不上的键等到这里再提示
  useEffect(() => {
    getShortcutConflicts()
      .then((failed) => {
        if (!Array.isArray(failed) || failed.length === 0) return;
        setConflicts(failed);
        notify(`${failed.map((key) => shortcutKeys(key).join(" ")).join("、")} 没能注册，可能被别的程序占用了`, "error");
      })
      // 只是一条提示，取不到不影响使用
      .catch(() => {});
  }, [notify]);

  useEffect(() => {
    const pending = onCaptureFeedback((payload) => {
      if (payload.flash) {
        setFlash(true);
        window.setTimeout(() => setFlash(false), 220);
      }
      if (payload.shutterSound !== "none") {
        previewHintSound(
          payload.shutterSound === "soft" ? "soft" : "crisp",
          payload.shutterSound === "custom" ? payload.customSoundPath : null,
          80,
        );
      }
    });
    return () => {
      void pending.then((unlisten) => unlisten());
    };
  }, []);

  // 颜色：跟随系统时，系统外观变了要当场跟上
  const setThemePreference = useCallback((next: ThemePreference) => {
    setThemePreferenceState(next);
    persistThemePreference(next);
    const mode = resolveTheme(next);
    // 从点的那个选项铺开新颜色
    if (document.documentElement.dataset.theme !== mode) viewTransition("theme", () => applyTheme(mode));
    broadcastTheme(mode);
  }, []);

  useEffect(() => {
    if (themePreference !== "system") return;
    return watchSystemTheme((mode) => {
      viewTransition("theme", () => applyTheme(mode));
      broadcastTheme(mode);
    });
  }, [themePreference]);

  const setMotionPreference = useCallback((next: MotionPreference) => {
    setMotionPreferenceState(next);
    persistMotionPreference(next);
    const mode = resolveMotion(next);
    applyMotion(mode);
    broadcastMotion(mode);
  }, []);

  useEffect(() => {
    if (motionPreference !== "system") return;
    return watchSystemMotion((mode) => {
      applyMotion(mode);
      broadcastMotion(mode);
    });
  }, [motionPreference]);

  const pageRef = useRef(page);
  const layoutRef = useRef(layout);
  pageRef.current = page;
  layoutRef.current = layout;

  // 换布局：结构变了就当翻一页，新布局整体淡入、轻轻落定，不让零件各自飞
  const setLayout = useCallback((next: LayoutTheme) => {
    if (next === layoutRef.current) return;
    viewTransition("layout", () => {
      setLayoutState(next);
      setPage((current) => mapPage(current, next));
    });
    persistLayout(next);
  }, []);

  // 换页：旧页往反方向退半步淡掉，新页从导航的方向滑进来；导航下的墨线跟着滑过去
  const navigate = useCallback<AppContextValue["navigate"]>((next, options) => {
    if (options?.previewId) setPendingPreviewId(options.previewId);
    if (options?.draft) setPendingDraft(options.draft);
    const from = pageRef.current;
    if (next === from) return;
    viewTransition(
      "page",
      () => {
        setPage(next);
        mainRef.current?.scrollTo?.({ top: 0 });
      },
      { direction: directionFor(layoutRef.current, from, next) },
    );
  }, []);

  const value = useMemo<AppContextValue | null>(
    () =>
      settings && {
        settings,
        onSaved: setSettings,
        notify,
        notice,
        dismissNotice,
        conflicts,
        setConflicts,
        caps,
        activity,
        clipboard,
        recording,
        layout,
        setLayout,
        themePreference,
        setThemePreference,
        motionPreference,
        setMotionPreference,
        page,
        navigate,
        pendingPreviewId,
        consumePreviewId: () => setPendingPreviewId(null),
        pendingDraft,
        consumeDraft: () => setPendingDraft(null),
      },
    [
      settings, notify, notice, dismissNotice, conflicts, caps, activity, clipboard, recording,
      layout, setLayout, themePreference, setThemePreference, motionPreference, setMotionPreference,
      page, navigate, pendingPreviewId, pendingDraft,
    ],
  );

  if (!value) {
    return (
      <div className="window">
        <BareTitleBar />
        <LoadingState />
        {notice && <div className="notice-line is-error" role="alert">{notice.text}</div>}
      </div>
    );
  }

  const content = (
    <main className="main" ref={mainRef}>
      <div key={`${layout}-${page}`} className={`page-view page-${page}`}>
        <PageView page={page} />
      </div>
    </main>
  );

  // 猫在不在眼前：不在的地方，提示才浮到左下角
  const catVisible = (layout === "companion" && page !== "pet") || (layout === "timeline" && page === "home");

  return (
    <AppContext.Provider value={value}>
      <div className={`window layout-${layout}`}>
        {layout === "topbar" ? (
          <TitleBar tall>
            <TopNav />
          </TitleBar>
        ) : layout === "timeline" ? (
          <TitleBar right={<TimelineLinks />} />
        ) : (
          <TitleBar />
        )}

        {layout === "companion" || layout === "ledger" ? (
          <div className="shell">
            <aside className="sidebar">
              <SideNav />
              {layout === "companion" ? <Companion away={page === "pet"} /> : <LedgerList />}
            </aside>
            {content}
          </div>
        ) : (
          <div className="shell shell-full">{content}</div>
        )}

        {!catVisible && <NoticeLine />}
        {flash && <div className="shutter-flash" aria-hidden="true" />}
      </div>
    </AppContext.Provider>
  );
}

/** 设置还没读到时也得能拖动、关闭窗口 */
function BareTitleBar() {
  return (
    <header className="titlebar" data-tauri-drag-region>
      <span className="titlebar-name">应用快照</span>
    </header>
  );
}

/** 「时间线」布局的标题栏右侧：回到今天、全部历史、形象、设置 */
function TimelineLinks() {
  const { page, navigate } = useApp();
  const links: Array<{ page: NavPage; label: string }> = [
    { page: "home", label: "今天" },
    { page: "history", label: "全部历史" },
    { page: "pet", label: "形象" },
    { page: "settings", label: "设置" },
  ];
  return (
    <nav className="top-nav top-nav-right" aria-label="主导航">
      {links.map((link) => (
        <button
          key={link.page}
          type="button"
          className={`top-item small ${page === link.page ? "is-on" : ""}`}
          aria-current={page === link.page ? "page" : undefined}
          onClick={() => navigate(link.page)}
        >
          {link.label}
          {page === link.page && <NavMark />}
        </button>
      ))}
    </nav>
  );
}
