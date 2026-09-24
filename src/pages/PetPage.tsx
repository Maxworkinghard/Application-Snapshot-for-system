import { useEffect, useMemo, useRef, useState } from "react";
import { CatShape } from "../components/ui/CatShape";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { addPetAssets, deletePetAsset, renamePetAsset, selectPetAppearance } from "../lib/backend";
import { petThumbUrl, petUrl } from "../lib/media";
import { fileNameOf, formatBytes } from "../lib/format";
import { commonPrefix, entryLabel } from "../lib/petNames";
import { errorText, useApp } from "../app/context";
import type { PetAsset } from "../types";

const ACCEPTED = /\.(zip|gif)$/i;

/** 形象架上的一格：静态第一帧站在一条线上，名字在线下 */
function ShelfItem({
  asset,
  selected,
  onChoose,
  onRemove,
}: {
  asset: PetAsset | null;
  selected: boolean;
  onChoose: () => void;
  onRemove?: () => void;
}) {
  const [failed, setFailed] = useState(false);
  const broken = Boolean(asset?.missing);
  const name = asset ? asset.name : "前台应用图标";
  return (
    <div className={`shelf-item ${selected ? "is-selected" : ""} ${broken ? "is-broken" : ""}`}>
      <button
        type="button"
        role="radio"
        aria-checked={selected}
        aria-disabled={broken || undefined}
        className="shelf-button"
        title={asset?.source ? `${name}\n${asset.source}` : name}
        onClick={() => !broken && onChoose()}
      >
        <span className="shelf-figure">
          {!asset ? (
            <span className="app-glyph" aria-hidden="true"><span /></span>
          ) : broken || failed ? (
            <CatShape height={46} outline />
          ) : (
            <img src={petThumbUrl(asset.id)} alt="" loading="lazy" draggable={false} onError={() => setFailed(true)} />
          )}
        </span>
        <span className="shelf-name">{name}</span>
      </button>
      {broken && (
        <span className="shelf-broken small">
          原文件丢了{" "}
          {onRemove && <button type="button" className="link" onClick={onRemove}>移除</button>}
        </span>
      )}
    </div>
  );
}

export function PetPage() {
  const { settings, onSaved, notify } = useApp();
  const [activeEntry, setActiveEntry] = useState<string | null>(null);
  const [filter, setFilter] = useState("");
  const [importing, setImporting] = useState(false);
  const [failures, setFailures] = useState<Array<{ path: string; reason: string }>>([]);
  const [dropping, setDropping] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [armedRemove, setArmedRemove] = useState(false);
  const importRef = useRef<(paths: string[]) => Promise<void>>(async () => {});

  const selectedId = settings.selectedAppearanceId;
  const selected = settings.petAssets.find((item) => item.id === selectedId) ?? null;
  const prefix = useMemo(() => commonPrefix(selected?.animations ?? []), [selected?.animations]);

  useEffect(() => {
    setActiveEntry(selected?.entry || selected?.animations[0] || null);
    setRenaming(null);
    setArmedRemove(false);
  }, [selectedId, selected?.entry, selected?.animations]);

  // 最近用过的在前；从没用过的按导入先后
  const shelf = useMemo(() => {
    const needle = filter.trim().toLowerCase();
    return [...settings.petAssets]
      .sort((a, b) => (b.lastUsedAt ?? 0) - (a.lastUsedAt ?? 0) || (b.importedAt ?? 0) - (a.importedAt ?? 0))
      .filter((item) => !needle || item.name.toLowerCase().includes(needle));
  }, [settings.petAssets, filter]);

  async function choose(id: string) {
    try {
      onSaved(await selectPetAppearance(id));
    } catch (error) {
      notify(errorText(error), "error");
    }
  }

  async function importPaths(paths: string[]) {
    const accepted = paths.filter((path) => ACCEPTED.test(path));
    const skipped = paths.filter((path) => !ACCEPTED.test(path));
    if (!accepted.length) {
      if (skipped.length) notify("只认 ZIP 或 GIF", "error");
      return;
    }
    setImporting(true);
    try {
      const result = await addPetAssets(accepted);
      onSaved(result.settings);
      setFailures([...result.failed, ...skipped.map((path) => ({ path, reason: "不是 ZIP 或 GIF" }))]);
      if (result.imported > 0) notify(result.imported === 1 ? "导入了 1 只，已经换上" : `导入了 ${result.imported} 只`);
    } catch (error) {
      notify(errorText(error), "error");
    } finally {
      setImporting(false);
    }
  }
  importRef.current = importPaths;

  async function pickFiles() {
    try {
      const picked = await open({ multiple: true, directory: false, filters: [{ name: "伴侣素材", extensions: ["zip", "gif"] }] });
      const paths = Array.isArray(picked) ? picked : typeof picked === "string" ? [picked] : [];
      if (paths.length) await importPaths(paths);
    } catch {
      notify("当前环境不支持选择文件", "error");
    }
  }

  // 直接把 ZIP / GIF 拖进窗口
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    try {
      void getCurrentWebview()
        .onDragDropEvent((event) => {
          const payload = event.payload;
          if (payload.type === "over" || payload.type === "enter") setDropping(true);
          else if (payload.type === "leave") setDropping(false);
          else if (payload.type === "drop") {
            setDropping(false);
            void importRef.current(payload.paths);
          }
        })
        .then((fn) => {
          unlisten = fn;
        })
        .catch(() => {});
    } catch {
      // 浏览器预览与测试环境里没有拖放事件
    }
    return () => unlisten?.();
  }, []);

  async function commitRename(id: string, value: string) {
    setRenaming(null);
    const current = settings.petAssets.find((item) => item.id === id);
    if (!current || !value.trim() || value.trim() === current.name) return;
    try {
      onSaved(await renamePetAsset(id, value));
    } catch (error) {
      notify(errorText(error), "error");
    }
  }

  async function remove(id: string) {
    const name = settings.petAssets.find((item) => item.id === id)?.name ?? "";
    try {
      onSaved(await deletePetAsset(id));
      notify(`移除了「${name}」`);
    } catch (error) {
      notify(errorText(error), "error");
    }
  }

  const stageSrc = selected && !selected.missing ? petUrl(selected.id, activeEntry) : null;

  return (
    <div className={`pet-page ${dropping ? "is-dropping" : ""}`}>
      <section className="pet-current" aria-label="当前伴侣">
        <div className="pet-stage">
          {stageSrc ? (
            <img key={stageSrc} className="pet-stage-image" src={stageSrc} alt={selected?.name ?? ""} draggable={false} />
          ) : selected ? (
            <CatShape height={150} outline />
          ) : (
            <span className="app-glyph app-glyph-large" aria-hidden="true"><span /></span>
          )}
        </div>
        <div className="pet-floor" aria-hidden="true" />

        {selected ? (
          <>
            <div className="pet-name-line">
              {renaming === selected.id ? (
                <input
                  className="input pet-rename"
                  autoFocus
                  defaultValue={selected.name}
                  aria-label="新名字"
                  maxLength={40}
                  onBlur={(event) => void commitRename(selected.id, event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void commitRename(selected.id, event.currentTarget.value);
                    if (event.key === "Escape") setRenaming(null);
                  }}
                />
              ) : (
                <>
                  <h2 className="pet-name">{selected.name}</h2>
                  <button type="button" className="link small quiet" onClick={() => setRenaming(selected.id)}>改名</button>
                </>
              )}
            </div>
            <span className="mono small quiet">
              {selected.animations.length} 个动作{selected.sizeBytes ? ` · ${formatBytes(selected.sizeBytes)}` : ""}
            </span>
            {selected.animations.length > 1 && (
              <div className="choices choices-wrap pet-actions" role="radiogroup" aria-label="预览动作">
                {selected.animations.map((entry) => (
                  <button
                    key={entry}
                    type="button"
                    role="radio"
                    aria-checked={activeEntry === entry}
                    className={`choice ${activeEntry === entry ? "is-on" : ""}`}
                    title={entry}
                    onClick={() => setActiveEntry(entry)}
                  >
                    {entryLabel(entry, prefix)}
                  </button>
                ))}
              </div>
            )}
            <span className="spacer-v" />
            <button
              type="button"
              className="link small signal pet-remove"
              onClick={() => {
                if (armedRemove) void remove(selected.id);
                else {
                  setArmedRemove(true);
                  window.setTimeout(() => setArmedRemove(false), 2400);
                }
              }}
            >
              {armedRemove ? "再点一次就移除" : "移除这只"}
            </button>
          </>
        ) : (
          <>
            <h2 className="pet-name">前台应用图标</h2>
            <p className="small quiet pet-desc">桌宠显示你正在用的那个应用的图标，不带动画。导入一只 GIF 伴侣，它就会住进来。</p>
          </>
        )}
      </section>

      <section className="pet-shelf-area" aria-label="全部形象">
        <div className="panel-head">
          <span className="panel-head-left">
            <h2 className="panel-title">全部形象</h2>
            <span className="mono small quiet">{settings.petAssets.length}</span>
          </span>
          <span className="panel-head-right">
            {settings.petAssets.length > 5 && (
              <label className="search search-small">
                <svg width="11" height="11" viewBox="0 0 12 12" aria-hidden="true"><circle cx="5" cy="5" r="4" fill="none" stroke="currentColor" strokeWidth="1.2" /><path d="M8 8l3.5 3.5" stroke="currentColor" strokeWidth="1.2" /></svg>
                <input value={filter} onChange={(event) => setFilter(event.target.value)} placeholder="按名称筛选" aria-label="按名称筛选形象" />
              </label>
            )}
            <button type="button" className="btn btn-small" onClick={() => void pickFiles()} disabled={importing}>
              {importing ? "导入中…" : "导入…"}
            </button>
          </span>
        </div>
        <p className="panel-hint">最近用过的在前。点一下就换上；可以一次选多个 ZIP，或直接拖进窗口。</p>

        {failures.length > 0 && (
          <div className="import-failures" role="alert">
            <span className="small">
              <span className="signal">这 {failures.length} 个没导进来：</span>
              {failures.map((item) => `${fileNameOf(item.path)}（${item.reason}）`).join("、")}
            </span>
            <button type="button" className="link small" onClick={() => setFailures([])}>知道了</button>
          </div>
        )}

        <div className="shelf" role="radiogroup" aria-label="选择伴侣">
          {!filter && <ShelfItem asset={null} selected={selectedId === "app-icon"} onChoose={() => void choose("app-icon")} />}
          {shelf.map((asset) => (
            <ShelfItem
              key={asset.id}
              asset={asset}
              selected={asset.id === selectedId}
              onChoose={() => void choose(asset.id)}
              onRemove={() => void remove(asset.id)}
            />
          ))}
          {dropping && <div className="shelf-drop small">松手导入</div>}
        </div>
        {filter && shelf.length === 0 && (
          <p className="empty-line">
            没有名字含「{filter.trim()}」的形象。
            <button type="button" className="link" onClick={() => setFilter("")}>清除筛选</button>
          </p>
        )}

        <p className="footnote">只认 GIF，ZIP 里其它格式会跳过；文件名含 idle 的当默认动作。整包不超过 100 MB，单个 GIF 不超过 50 MB。导入时会复制一份，原文件可以删。</p>
      </section>
    </div>
  );
}
