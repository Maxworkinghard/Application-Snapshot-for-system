import { useEffect, useRef, useState } from "react";
import { CatShape } from "./ui/CatShape";
import { clearClipboardNow, keepClipboard, toggleRecording } from "../lib/backend";
import { petThumbUrl, petUrl } from "../lib/media";
import { formatDuration } from "../lib/format";
import { narrate } from "../lib/activity";
import { motionReduced } from "../lib/motion";
import { errorText, useApp } from "../app/context";
import { useNow, useWindowActive } from "../hooks/useLive";

/** 播报只说最近发生的事，太久以前的不提 */
const FRESH_MS = 15 * 60_000;
const NOTICE_MS = 8_000;

/**
 * 住在侧栏底部的猫，和它说的一句话。
 * 取代了右下角的弹出提示：刚发生了什么、剪贴板多久后清、正在录什么，都由它来说。
 * 在「桌面伴侣」页它会离开侧栏——那一页的大舞台上已经有它了。
 */
export function Companion({ away = false, compact = false }: { away?: boolean; compact?: boolean }) {
  const { settings, notice, activity, clipboard, recording, conflicts, navigate, notify, dismissNotice } = useApp();
  const windowActive = useWindowActive();
  const [hop, setHop] = useState(false);
  const lastSeen = useRef(activity[0]?.id);

  const asset = settings.petAssets.find((item) => item.id === settings.selectedAppearanceId && !item.missing);

  // 有新活动时跳一下——全应用只有猫能弹
  useEffect(() => {
    const newest = activity[0]?.id;
    if (!newest || newest === lastSeen.current) return;
    lastSeen.current = newest;
    if (motionReduced()) return;
    setHop(true);
    const timer = window.setTimeout(() => setHop(false), 420);
    return () => window.clearTimeout(timer);
  }, [activity]);

  const ticking = recording.active || Boolean(clipboard?.clearAt) || Boolean(notice);
  const now = useNow(ticking);

  const say = (() => {
    if (notice && now - notice.at < NOTICE_MS) {
      return { key: `n${notice.id}`, tone: notice.kind, body: <>{notice.text}</>, action: null };
    }
    if (recording.active) {
      return {
        key: "rec",
        tone: "rec" as const,
        body: (
          <>
            正在录 {recording.target ?? "窗口"}
            <span className="mono"> {formatDuration(now - (recording.startedAt ?? now))}</span>
          </>
        ),
        action: (
          <button
            type="button"
            className="link"
            onClick={() => toggleRecording().catch((error) => notify(errorText(error), "error"))}
          >
            停止录制
          </button>
        ),
      };
    }
    if (clipboard?.clearAt) {
      const left = Math.max(0, Math.ceil((clipboard.clearAt - now) / 1000));
      const total = Math.max(1, clipboard.clearAt - clipboard.armedAt);
      const ratio = Math.max(0, Math.min(1, (clipboard.clearAt - now) / total));
      return {
        key: `clip${clipboard.armedAt}`,
        tone: "info" as const,
        body: (
          <>
            {clipboard.label}在剪贴板里，<span className="mono">{left}</span> 秒后清空。
            <span className="drain" aria-hidden="true"><span style={{ width: `${ratio * 100}%` }} /></span>
          </>
        ),
        action: (
          <span className="companion-actions">
            <button type="button" className="link" onClick={() => void clearClipboardNow()}>现在清空</button>
            <button type="button" className="link" onClick={() => void keepClipboard()}>这次别清</button>
          </span>
        ),
      };
    }
    const latest = activity[0];
    if (latest && now - latest.at < FRESH_MS) {
      return {
        key: latest.id,
        tone: latest.kind === "error" ? ("error" as const) : ("info" as const),
        body: <>{narrate(latest)}</>,
        action: null,
      };
    }
    if (conflicts.length > 0) {
      return {
        key: "conflicts",
        tone: "error" as const,
        body: <>有 {conflicts.length} 组快捷键没注册上，按了不会有反应。</>,
        action: (
          <button type="button" className="link" onClick={() => navigate("shortcuts")}>去看看</button>
        ),
      };
    }
    return null;
  })();

  const animate = windowActive && !motionReduced();
  const catSrc = asset ? (animate ? petUrl(asset.id, asset.entry || null) : petThumbUrl(asset.id)) : null;

  return (
    <div className={`companion ${away ? "is-away" : ""} ${compact ? "is-compact" : ""}`}>
      {say && (
        <div key={say.key} className={`companion-say tone-${say.tone}`} role="status" aria-live="polite">
          <span>{say.body}</span>
          {say.action}
          {notice && say.key === `n${notice.id}` && notice.kind === "error" && (
            <button type="button" className="companion-dismiss" aria-label="知道了" onClick={dismissNotice}>
              <svg width="8" height="8" viewBox="0 0 8 8" aria-hidden="true"><path d="M.5.5l7 7M7.5.5l-7 7" stroke="currentColor" strokeWidth="1.2" /></svg>
            </button>
          )}
        </div>
      )}
      <button
        type="button"
        className={`companion-cat ${hop ? "is-hop" : ""}`}
        aria-label={asset ? `${asset.name}，去桌面伴侣页` : "还没有伴侣，去导入一只"}
        onClick={() => navigate("pet")}
      >
        {catSrc ? (
          <img key={catSrc} src={catSrc} alt="" draggable={false} />
        ) : (
          <span className="companion-empty">
            <CatShape height={64} outline />
            <span>还没有伴侣</span>
          </span>
        )}
      </button>
      <div className="companion-floor" aria-hidden="true" />
    </div>
  );
}
