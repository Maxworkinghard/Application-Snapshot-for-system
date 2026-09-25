import { useMemo, useState } from "react";
import { copyText, openRecordingsDir } from "../lib/backend";
import { formatClock, groupByDay } from "../lib/format";
import { stagger } from "../lib/motion";
import { parseSize } from "../lib/activity";
import { SnapshotThumb } from "../components/SnapshotThumb";
import { Companion } from "../components/Companion";
import { useNow } from "../hooks/useLive";
import { errorText, useApp } from "../app/context";
import type { ActivityEntry } from "../types";

type Filter = "all" | "capture" | "record" | "polish";

const FILTERS: Array<{ key: Filter; label: string }> = [
  { key: "all", label: "全部" },
  { key: "capture", label: "截图" },
  { key: "record", label: "录制" },
  { key: "polish", label: "润色" },
];

/** 「时间线」布局的首页：今天做过的事排成一条，顶上一个输入框，粘贴进去就能润色 */
export function TimelineHome() {
  const { activity, clipboard, navigate, notify } = useApp();
  const [filter, setFilter] = useState<Filter>("all");
  const [draft, setDraft] = useState("");
  const now = useNow(Boolean(clipboard?.clearAt));

  const counts = useMemo(() => {
    const result: Record<Filter, number> = { all: 0, capture: 0, record: 0, polish: 0 };
    for (const entry of activity) {
      if (entry.kind === "error") continue;
      result.all += 1;
      result[entry.kind] += 1;
    }
    return result;
  }, [activity]);

  const visible = useMemo(
    () => activity.filter((entry) => (filter === "all" ? true : entry.kind === filter)),
    [activity, filter],
  );
  const groups = useMemo(() => groupByDay(visible, (entry) => entry.at), [visible]);

  function submit() {
    if (!draft.trim()) return;
    navigate("prompt", { draft });
    setDraft("");
  }

  let order = 0;
  const renderEntry = (entry: ActivityEntry) => {
    let body: React.ReactNode;
    if (entry.kind === "capture") {
      const size = parseSize(entry.meta);
      const inClipboard = clipboard?.snapshotId && clipboard.snapshotId === entry.snapshotId && clipboard.clearAt;
      body = (
        <div className="entry-media">
          {entry.snapshotId && size ? (
            <SnapshotThumb
              id={entry.snapshotId}
              width={size.width}
              height={size.height}
              label={entry.title}
              thumbHeight={60}
              maxWidth={107}
              onPress={() => navigate("history", { previewId: entry.snapshotId })}
            />
          ) : (
            <span className="thumb thumb-placeholder" style={{ width: 107, height: 60 }} aria-hidden="true" />
          )}
          <span className="entry-text">
            <span className="entry-title" title={entry.title}>{entry.title}</span>
            <span className="mono small quiet">{entry.meta}{entry.snapshotId ? "" : " · 没存进历史"}</span>
            {inClipboard && clipboard?.clearAt && (
              <span className="small ink-2">在剪贴板里，<span className="mono">{Math.max(0, Math.ceil((clipboard.clearAt - now) / 1000))}</span> 秒后清空</span>
            )}
          </span>
        </div>
      );
    } else if (entry.kind === "record") {
      body = (
        <span className="entry-text">
          <span className="entry-title">{entry.title} · 录制</span>
          <span className="small quiet">
            <span className="mono">{entry.meta}</span>
            {entry.detail && <span className="mono"> · {entry.detail.split(/[\\/]/).pop()}</span>}
            {" "}
            <button type="button" className="link" onClick={() => openRecordingsDir().catch((error) => notify(errorText(error), "error"))}>打开文件夹</button>
          </span>
        </span>
      );
    } else if (entry.kind === "polish") {
      body = (
        <span className="entry-text">
          <span className="small quiet">润色 · {entry.title} · <span className="mono">{entry.meta}</span></span>
          <span className="entry-snippet">{entry.detail}</span>
          {entry.detail && (
            <span className="small">
              <button
                type="button"
                className="link"
                onClick={() => copyText(entry.detail!).then(() => notify("结果已复制")).catch((error) => notify(errorText(error), "error"))}
              >
                复制结果
              </button>
            </span>
          )}
        </span>
      );
    } else {
      body = (
        <span className="entry-text small">
          <span className="signal">{entry.title}</span>
          {entry.detail && <span className="ink-2">{entry.detail}</span>}
        </span>
      );
    }
    return (
      <article className="entry" key={entry.id} style={stagger(order++)}>
        <time className="mono small quiet">{formatClock(entry.at)}</time>
        {body}
      </article>
    );
  };

  return (
    <div className="timeline">
      <div className="timeline-main">
        <div className="composer">
          <textarea
            className="composer-input"
            rows={draft.includes("\n") ? 4 : 1}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            placeholder="粘贴或写下要润色的 Prompt"
            aria-label="要润色的 Prompt"
            spellCheck={false}
          />
          <button type="button" className="btn btn-small" onClick={submit} disabled={!draft.trim()}>
            润色
          </button>
        </div>

        <div className="stream">
          {groups.length === 0 ? (
            <div className="empty">
              <h2 className="empty-title">{filter === "all" ? "这里还空着" : "这一类还没有记录"}</h2>
              <p className="empty-body">截一张图、录一段、润色一次，都会按时间排在这里。</p>
            </div>
          ) : (
            groups.map((group) => (
              <section key={group.key} className="stream-day" aria-label={group.label}>
                <h2 className="stream-day-title"><span>{group.label}</span></h2>
                {group.items.map(renderEntry)}
              </section>
            ))
          )}
        </div>
      </div>

      <aside className="timeline-side">
        <nav className="filter" aria-label="只看某一类">
          <span className="small quiet filter-label">只看</span>
          {FILTERS.map((item) => (
            <button
              key={item.key}
              type="button"
              aria-pressed={filter === item.key}
              className={`filter-item ${filter === item.key ? "is-on" : ""}`}
              onClick={() => setFilter(item.key)}
            >
              <span>{item.label}</span>
              <span className="mono small quiet">{counts[item.key]}</span>
            </button>
          ))}
        </nav>
        <Companion compact />
      </aside>
    </div>
  );
}
