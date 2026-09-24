import type { ReactNode } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { formatClock } from "../lib/format";
import { shortLabel } from "../lib/activity";
import { usePresence } from "../lib/motion";
import { errorText, useApp } from "./context";
import type { NavPage } from "../types";

/** 窗口操作要 capability 里显式放行，缺权限时 promise 会被拒——必须让它说出来，不能「点了没反应」 */
function useWindowAction() {
  const { notify } = useApp();
  return (action: () => Promise<unknown>, label: string) => {
    void action().catch((error) => notify(`${label}失败：${errorText(error)}`, "error"));
  };
}

export function TitleBar({ children, right, tall = false }: { children?: ReactNode; right?: ReactNode; tall?: boolean }) {
  const run = useWindowAction();
  const appWindow = getCurrentWindow();
  return (
    <header
      className={`titlebar ${tall ? "is-tall" : ""}`}
      data-tauri-drag-region
      onDoubleClick={(event) => {
        if ((event.target as HTMLElement).closest("button, a, input")) return;
        run(() => appWindow.toggleMaximize(), "最大化");
      }}
    >
      {children ?? <span className="titlebar-name" data-tauri-drag-region>应用快照</span>}
      <span className="titlebar-fill" data-tauri-drag-region />
      {right}
      <span className="window-controls">
        <button type="button" aria-label="最小化" onClick={() => run(() => appWindow.minimize(), "最小化")}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M0 5.5h10" stroke="currentColor" /></svg>
        </button>
        <button type="button" aria-label="最大化或还原" onClick={() => run(() => appWindow.toggleMaximize(), "最大化")}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><rect x=".5" y=".5" width="9" height="9" fill="none" stroke="currentColor" /></svg>
        </button>
        <button type="button" className="is-close" aria-label="关闭" onClick={() => run(() => appWindow.close(), "关闭")}>
          <svg width="10" height="10" viewBox="0 0 10 10" aria-hidden="true"><path d="M.5.5l9 9M9.5.5l-9 9" stroke="currentColor" /></svg>
        </button>
      </span>
    </header>
  );
}

type NavItem = { page: NavPage; label: string; trailing?: ReactNode };

export function useNavItems(): { work: NavItem[]; settings: NavItem[] } {
  const { conflicts, settings, snapshotCount } = useApp();
  return {
    work: [
      {
        page: "shortcuts",
        label: "快捷操作",
        trailing: conflicts.length ? <span className="signal small" aria-label={`${conflicts.length} 个冲突`}>{conflicts.length}</span> : null,
      },
      {
        page: "prompt",
        label: "Prompt 编辑",
        trailing: settings.baseUrl && settings.model ? null : <span className="small quiet">未配置</span>,
      },
      {
        page: "history",
        label: "快照历史",
        trailing: snapshotCount ? <span className="mono small quiet">{snapshotCount}</span> : null,
      },
      { page: "pet", label: "桌面伴侣" },
    ],
    settings: [
      { page: "prefs", label: "偏好设置" },
      { page: "themes", label: "主题库" },
    ],
  };
}

/** 当前页字下那道墨线。换页时它会从旧的一项滑到新的一项（view-transition-name: nav-mark） */
export const NavMark = () => <span className="nav-mark" aria-hidden="true" />;

/** 侧栏导航：只有字。当前页墨色加粗、字下一道线，其余灰字——不加底色 */
export function SideNav() {
  const { page, navigate } = useApp();
  const { work, settings } = useNavItems();
  const item = (entry: NavItem) => (
    <button
      key={entry.page}
      type="button"
      className={`nav-item ${page === entry.page ? "is-on" : ""}`}
      aria-current={page === entry.page ? "page" : undefined}
      onClick={() => navigate(entry.page)}
    >
      <span className="nav-label">
        {entry.label}
        {page === entry.page && <NavMark />}
      </span>
      {entry.trailing}
    </button>
  );
  return (
    <nav className="side-nav" aria-label="主导航">
      <div className="nav-group">{work.map(item)}</div>
      <div className="nav-group">{settings.map(item)}</div>
    </nav>
  );
}

/** 「不要侧栏」：入口放进标题栏 */
export function TopNav() {
  const { page, navigate } = useApp();
  const { work, settings } = useNavItems();
  return (
    <>
      <span className="titlebar-name strong" data-tauri-drag-region>应用快照</span>
      <nav className="top-nav" aria-label="主导航">
        {work.map((entry) => (
          <button
            key={entry.page}
            type="button"
            className={`top-item ${page === entry.page ? "is-on" : ""}`}
            aria-current={page === entry.page ? "page" : undefined}
            onClick={() => navigate(entry.page)}
          >
            {entry.label}
            {entry.trailing}
            {page === entry.page && <NavMark />}
          </button>
        ))}
      </nav>
      <span className="titlebar-fill" data-tauri-drag-region />
      <nav className="top-nav top-nav-right" aria-label="设置">
        {settings.map((entry) => (
          <button
            key={entry.page}
            type="button"
            className={`top-item small ${page === entry.page ? "is-on" : ""}`}
            aria-current={page === entry.page ? "page" : undefined}
            onClick={() => navigate(entry.page)}
          >
            {entry.label}
            {page === entry.page && <NavMark />}
          </button>
        ))}
      </nav>
    </>
  );
}

/** 「今日流水」：侧栏下半截列出今天的每一步，点截图直接放大 */
export function LedgerList() {
  const { activity, navigate } = useApp();
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const entries = activity.filter((entry) => entry.at >= today.getTime());
  return (
    <section className="ledger" aria-label="今天做过的事">
      <h2 className="ledger-title">今天</h2>
      {entries.length === 0 ? (
        <p className="small quiet ledger-empty">今天还没截过、录过、润色过。</p>
      ) : (
        <ol className="ledger-list">
          {entries.map((entry) => (
            <li key={entry.id}>
              <button
                type="button"
                className={`ledger-item ${entry.kind === "error" ? "is-error" : ""}`}
                onClick={() => {
                  if (entry.kind === "capture" && entry.snapshotId) navigate("history", { previewId: entry.snapshotId });
                  else if (entry.kind === "polish") navigate("prompt");
                  else if (entry.kind === "error" && entry.title.includes("快捷键")) navigate("shortcuts");
                }}
                title={entry.detail ?? entry.title}
              >
                <span className="mono quiet">{formatClock(entry.at)}</span>
                <span className="ledger-text">{shortLabel(entry)}</span>
              </button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

/** 没有猫的地方，一句话提示浮在左下角：普通的自己消失，出错的留着等你关 */
export function NoticeLine() {
  const { notice: live, dismissNotice } = useApp();
  // 到时间或点了关闭：往下沉一点淡出，而不是啪一下没了
  const { item: notice, leaving } = usePresence(live);
  if (!notice) return null;
  return (
    <div
      key={notice.id}
      className={`notice-line ${notice.kind === "error" ? "is-error" : ""} ${leaving ? "is-leaving" : ""}`}
      role={notice.kind === "error" ? "alert" : "status"}
    >
      <span>{notice.text}</span>
      {notice.kind === "error" && (
        <button type="button" aria-label="关闭提示" onClick={dismissNotice}>
          <svg width="8" height="8" viewBox="0 0 8 8" aria-hidden="true"><path d="M.5.5l7 7M7.5.5l-7 7" stroke="currentColor" strokeWidth="1.2" /></svg>
        </button>
      )}
    </div>
  );
}
