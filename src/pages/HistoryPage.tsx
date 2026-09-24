import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  deleteSnapshots,
  listSnapshots,
  onSnapshotsChanged,
  openSnapshotsDir,
  savePreferences,
} from "../lib/backend";
import { formatBytes, formatClock, groupByDay } from "../lib/format";
import { useFlip, wait, DURATION, stagger, usePresence } from "../lib/motion";
import { SnapshotThumb } from "../components/SnapshotThumb";
import { Lightbox } from "../components/Lightbox";
import { ConfirmDialog } from "../components/ConfirmDialog";
import { LoadingState } from "../components/LoadingState";
import { Keys } from "../components/ui/Keys";
import { acceleratorOf } from "./ShortcutsPage";
import { errorText, useApp } from "../app/context";
import type { SnapshotRecord } from "../types";

const LIMIT = 200;
const NEAR_LIMIT = 180;

export function HistoryPage() {
  const { settings, onSaved, notify, pendingPreviewId, consumePreviewId } = useApp();
  const [records, setRecords] = useState<SnapshotRecord[] | null>(null);
  const [query, setQuery] = useState("");
  const [selecting, setSelecting] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchor, setAnchor] = useState<string | null>(null);
  const [previewId, setPreviewId] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [leaving, setLeaving] = useState<Set<string>>(new Set());
  const [fresh, setFresh] = useState<Set<string>>(new Set());
  const known = useRef<Set<string> | null>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const flip = useFlip(listRef);

  const refresh = useCallback(async () => {
    try {
      const list = await listSnapshots();
      const next = Array.isArray(list) ? list : [];
      // 第一次加载不算「新来的」；之后多出来的那几张要从顶上挤进来
      if (known.current) {
        const added = next.filter((item) => !known.current!.has(item.id)).map((item) => item.id);
        if (added.length) setFresh(new Set(added));
      }
      known.current = new Set(next.map((item) => item.id));
      flip.capture();
      setRecords(next);
    } catch (error) {
      setRecords([]);
      notify(errorText(error), "error");
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    void refresh();
    const pending = onSnapshotsChanged(() => void refresh());
    return () => {
      void pending.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [refresh]);

  // 从时间线点进来的那张，直接放大
  useEffect(() => {
    if (!pendingPreviewId || !records) return;
    if (records.some((item) => item.id === pendingPreviewId)) setPreviewId(pendingPreviewId);
    consumePreviewId();
  }, [pendingPreviewId, records, consumePreviewId]);

  useEffect(() => {
    if (!fresh.size) return;
    const timer = window.setTimeout(() => setFresh(new Set()), 1400);
    return () => window.clearTimeout(timer);
  }, [fresh]);

  const filtered = useMemo(() => {
    const needle = query.trim().toLowerCase();
    return (records ?? []).filter((item) => !needle || item.appName.toLowerCase().includes(needle));
  }, [records, query]);
  const groups = useMemo(() => groupByDay(filtered, (item) => item.createdAt), [filtered]);

  const exitSelecting = useCallback(() => {
    setSelecting(false);
    setSelected(new Set());
    setAnchor(null);
  }, []);

  useEffect(() => {
    if (!selecting || previewId || confirming) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") exitSelecting();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [selecting, previewId, confirming, exitSelecting]);

  function toggle(id: string, range: boolean) {
    setSelected((current) => {
      const next = new Set(current);
      if (range && anchor) {
        const from = filtered.findIndex((item) => item.id === anchor);
        const to = filtered.findIndex((item) => item.id === id);
        if (from >= 0 && to >= 0) {
          const [start, end] = from < to ? [from, to] : [to, from];
          filtered.slice(start, end + 1).forEach((item) => next.add(item.id));
          return next;
        }
      }
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
    setAnchor(id);
  }

  function toggleGroup(ids: string[]) {
    setSelected((current) => {
      const next = new Set(current);
      const all = ids.every((id) => next.has(id));
      ids.forEach((id) => (all ? next.delete(id) : next.add(id)));
      return next;
    });
  }

  async function remove(ids: string[]) {
    if (!ids.length) return;
    setDeleting(true);
    try {
      // 先消失再合拢：顺序反了会像是别的条目被删了
      setLeaving(new Set(ids));
      await wait(DURATION.fade);
      const next = await deleteSnapshots(ids);
      known.current = new Set(next.map((item) => item.id));
      flip.capture();
      setRecords(next);
      setLeaving(new Set());
      setSelected((current) => new Set([...current].filter((id) => !ids.includes(id))));
      notify(ids.length === 1 ? "删掉了 1 张快照" : `删掉了 ${ids.length} 张快照`);
    } catch (error) {
      setLeaving(new Set());
      notify(errorText(error), "error");
    } finally {
      setDeleting(false);
      setConfirming(false);
    }
  }

  async function revealFolder() {
    try {
      await openSnapshotsDir();
    } catch (error) {
      notify(errorText(error), "error");
    }
  }

  const selectedBytes = useMemo(
    () => (records ?? []).filter((item) => selected.has(item.id)).reduce((sum, item) => sum + item.sizeBytes, 0),
    [records, selected],
  );

  const total = records?.length ?? 0;
  const allSelected = total > 0 && selected.size === total;
  // 退场时文案别跟着变成「删除 0 张」：打开那一刻的文案一直留到对话框淡出
  const confirmDialog = usePresence(
    confirming
      ? {
          title: allSelected ? `删除全部 ${total} 张快照？` : `删除选中的 ${selected.size} 张快照？`,
          label: `删除 ${selected.size} 张`,
        }
      : null,
  );

  if (records === null) return <LoadingState text="正在读取快照历史…" />;

  const header = selecting ? (
    <div className="history-bar">
      <span className="strong">已选 {selected.size} 张</span>
      {selected.size > 0 && <span className="mono small quiet">{formatBytes(selectedBytes)}</span>}
      <button
        type="button"
        className="link small"
        onClick={() => setSelected(allSelected ? new Set() : new Set(filtered.map((item) => item.id)))}
      >
        {allSelected ? "全不选" : query ? `全选 ${filtered.length} 张` : `全选 ${total} 张`}
      </button>
      <span className="spacer" />
      <button type="button" className="text-btn ink-2" onClick={exitSelecting}>取消</button>
      <button
        type="button"
        className="btn btn-danger btn-small"
        disabled={selected.size === 0}
        onClick={() => setConfirming(true)}
      >
        删除 {selected.size || ""} 张
      </button>
    </div>
  ) : (
    <div className="history-bar">
      <label className="search">
        <svg width="12" height="12" viewBox="0 0 12 12" aria-hidden="true"><circle cx="5" cy="5" r="4" fill="none" stroke="currentColor" strokeWidth="1.2" /><path d="M8 8l3.5 3.5" stroke="currentColor" strokeWidth="1.2" /></svg>
        <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="按应用名筛选" aria-label="按应用名筛选" />
        {query && (
          <button type="button" className="search-clear" aria-label="清除筛选" onClick={() => setQuery("")}>
            <svg width="8" height="8" viewBox="0 0 8 8" aria-hidden="true"><path d="M.5.5l7 7M7.5.5l-7 7" stroke="currentColor" strokeWidth="1.2" /></svg>
          </button>
        )}
      </label>
      {/* 快到上限时数字变红：再截就会挤掉最旧的，不用另写一句解释 */}
      <span className={`mono small ${!query && total >= NEAR_LIMIT ? "signal" : "quiet"}`}>
        {query ? `${filtered.length} / ${total}` : `${total} / ${LIMIT}`}
      </span>
      <span className="spacer" />
      <button type="button" className="text-btn small ink-2" onClick={() => void revealFolder()}>打开文件夹</button>
      <button type="button" className="btn btn-small" onClick={() => setSelecting(true)} disabled={total === 0}>选择</button>
    </div>
  );

  let body;
  if (total === 0) {
    const shortcut = acceleratorOf(settings, "snapshot");
    body = settings.autoSaveLocal ? (
      <div className="empty">
        <h2 className="empty-title">还没有快照</h2>
        <p className="empty-body">
          {shortcut ? (
            <>按 <Keys value={shortcut} /> 截取当前窗口，或右键桌宠从输入框里截。截到的都会留在这里，最多 200 张。</>
          ) : (
            <>右键桌宠，从输入框里截一个窗口；截到的都会留在这里，最多 200 张。也可以在「快捷操作」里绑一个截图快捷键。</>
          )}
        </p>
      </div>
    ) : (
      <div className="empty">
        <h2 className="empty-title">还没有快照</h2>
        <p className="empty-body">「截图同时存进历史」现在是关的，截图只进剪贴板，不会记到这里。</p>
        <div className="empty-actions">
          <button
            type="button"
            className="btn btn-primary"
            onClick={() =>
              savePreferences({ autoSaveLocal: true })
                .then((next) => {
                  onSaved(next);
                  notify("打开了：之后的截图会留在这里");
                })
                .catch((error) => notify(errorText(error), "error"))
            }
          >
            打开它
          </button>
        </div>
      </div>
    );
  } else if (filtered.length === 0) {
    body = (
      <p className="empty-line">
        没有应用名含「{query.trim()}」的快照。
        <button type="button" className="link" onClick={() => setQuery("")}>清除筛选</button>
      </p>
    );
  } else {
    // 逐个入场只数前十几张：再往后已经在屏幕外，等也白等
    let order = 0;
    body = (
      <div className="history-groups" ref={listRef}>
        {groups.map((group) => {
          const ids = group.items.map((item) => item.id);
          const picked = ids.filter((id) => selected.has(id)).length;
          return (
            <section className="day" key={group.key} aria-label={group.label}>
              <div className="day-label">
                <h3 className="day-title">{group.label}</h3>
                <span className="small quiet">{selecting && picked ? `选了 ${picked} / ${ids.length}` : `${ids.length} 张`}</span>
                {selecting && (
                  <button type="button" className="link small" onClick={() => toggleGroup(ids)}>
                    {picked === ids.length ? "取消整组" : "选整组"}
                  </button>
                )}
              </div>
              <div className="day-items">
                {group.items.map((item) => (
                  <figure
                    key={item.id}
                    data-flip-key={item.id}
                    style={stagger(order++)}
                    className={`shot ${leaving.has(item.id) ? "is-leaving" : ""} ${fresh.has(item.id) ? "is-fresh" : ""}`}
                  >
                    <SnapshotThumb
                      id={item.id}
                      width={item.width}
                      height={item.height}
                      label={item.appName}
                      selecting={selecting}
                      selected={selected.has(item.id)}
                      onPress={(_element, event) => {
                        if (selecting) toggle(item.id, event.shiftKey);
                        else setPreviewId(item.id);
                      }}
                    />
                    <figcaption className="shot-caption">
                      <span className="shot-name" title={item.appName}>{item.appName}</span>
                      <span className="mono small quiet">{formatClock(item.createdAt)} · {formatBytes(item.sizeBytes)}</span>
                    </figcaption>
                  </figure>
                ))}
              </div>
            </section>
          );
        })}
      </div>
    );
  }

  return (
    <div className="history-page">
      {header}
      <div className="history-scroll">{body}</div>

      {previewId && (
        <Lightbox
          records={filtered.length ? filtered : records}
          startId={previewId}
          onClose={() => setPreviewId(null)}
        />
      )}
      {confirmDialog.item && (
        <ConfirmDialog
          title={confirmDialog.item.title}
          body="图片文件会从本机一起删掉，没法恢复。"
          confirmLabel={confirmDialog.item.label}
          busy={deleting}
          leaving={confirmDialog.leaving}
          onCancel={() => setConfirming(false)}
          onConfirm={() => void remove([...selected]).then(() => allSelected && exitSelecting())}
        />
      )}
    </div>
  );
}
