import { useEffect, useMemo, useState } from "react";
import {
  Camera,
  ClipboardCopy,
  Clock,
  FolderOpen,
  HardDrive,
  Maximize2,
  RefreshCw,
  ScanText,
  Search,
  Trash2,
  X,
} from "lucide-react";
import {
  clearSnapshots,
  deleteSnapshot,
  listSnapshots,
  ocrSnapshot,
  openSnapshotsDir,
} from "../lib/backend";
import { formatBytes, formatWhen } from "../lib/format";
import { snapshotUrl } from "../lib/media";
import { LoadingState } from "../components/LoadingState";
import type { SnapshotRecord } from "../types";

export function HistoryPage({ notify }: { notify: (message: string) => void }) {
  const [records, setRecords] = useState<SnapshotRecord[]>([]);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState("");
  const [preview, setPreview] = useState<SnapshotRecord | null>(null);
  const [ocrText, setOcrText] = useState("");
  const [ocrBusy, setOcrBusy] = useState<string | null>(null);
  const [confirmClear, setConfirmClear] = useState(false);
  const [clearing, setClearing] = useState(false);

  async function openFolder() {
    try {
      notify(`已打开：${await openSnapshotsDir()}`);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function refresh() {
    setLoading(true);
    try {
      setRecords(await listSnapshots());
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => { void refresh(); }, []);

  useEffect(() => {
    if (!preview && !confirmClear) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      // 确认框叠在放大浮层之上时，Esc 先关确认框
      if (confirmClear) setConfirmClear(false);
      else setPreview(null);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [preview, confirmClear]);

  const filtered = useMemo(
    () => records.filter((item) => item.appName.toLowerCase().includes(query.trim().toLowerCase())),
    [records, query],
  );

  async function removeOne(id: string) {
    try {
      setRecords(await deleteSnapshot(id));
      if (preview?.id === id) setPreview(null);
      notify("已删除该快照");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function removeAll() {
    setClearing(true);
    try {
      setRecords(await clearSnapshots());
      setPreview(null);
      setConfirmClear(false);
      notify("已清空快照历史");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setClearing(false);
    }
  }

  async function extractText(id: string) {
    setOcrBusy(id);
    setOcrText("");
    try {
      setOcrText(await ocrSnapshot(id));
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setOcrBusy(null);
    }
  }

  return (
    <div className="snapshot-history-container">
      <div className="history-header-bar">
        <div className="history-title-group">
          <div className="history-heading-line">
            <h2 className="history-main-title">快照历史库</h2>
            <button
              className="history-folder-btn"
              onClick={() => void openFolder()}
              title="打开快照保存位置"
              aria-label="打开快照保存位置"
            >
              <FolderOpen size={14} />
            </button>
          </div>
          <p className="history-desc">窗口快照会自动存到本机并登记在这里，最多保留 200 条。</p>
        </div>

        <div className="history-actions-group">
          <button className="history-ghost-btn" onClick={() => void refresh()} title="重新读取">
            <RefreshCw size={13} />
            <span>刷新</span>
          </button>
          {records.length > 0 && (
            <button className="history-ghost-btn" onClick={() => setConfirmClear(true)} title="清空全部快照">
              <Trash2 size={13} />
              <span>清空</span>
            </button>
          )}
        </div>
      </div>

      <div className="history-filter-strip">
        <div className="history-search-box">
          <Search size={14} className="search-icon" />
          <input
            type="text"
            className="history-search-input"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="按来源应用名称搜索…"
          />
          {query && <button className="clear-search-btn" onClick={() => setQuery("")}>清空</button>}
        </div>
        <div className="history-type-pills">
          <span className="history-pill-btn is-active">
            <span>全部</span>
            <span className="pill-count tabular-nums">{filtered.length}</span>
          </span>
        </div>
      </div>

      {loading ? (
        <LoadingState />
      ) : filtered.length > 0 ? (
        <div className="history-cards-grid">
          {filtered.map((item) => (
            <div key={item.id} className="history-card-item">
              <button
                type="button"
                className="card-preview-stage"
                onClick={() => { setPreview(item); setOcrText(""); }}
                title="放大预览"
              >
                {/* 滚到附近才加载、解码不占主线程：200 张历史也只读眼前那几张 */}
                <img className="card-preview-art" src={snapshotUrl(item.id)} alt="" loading="lazy" decoding="async" />
                <span className="stage-zoom-hint">
                  <Maximize2 size={14} />
                  <span>放大预览</span>
                </span>
                <div className="stage-app-chip">
                  <Camera size={12} />
                  <span>{item.appName}</span>
                </div>
                <span className="stage-dim-tag tabular-nums">{item.width} × {item.height}</span>
              </button>

              <div className="card-info-box">
                <div className="card-title-line" title={item.appName}>
                  <span className="card-title-text">{item.appName}</span>
                </div>
                <div className="card-meta-line">
                  <span className="meta-time"><Clock size={11} /><span>{formatWhen(item.createdAt)}</span></span>
                  <span className="meta-dot">·</span>
                  <span className="meta-size tabular-nums">{formatBytes(item.sizeBytes)}</span>
                  {/* 窄窗口下缩略图被隐藏，尺寸信息要在这里补上 */}
                  <span className="meta-dot compact-only">·</span>
                  <span className="meta-size tabular-nums compact-only">{item.width} × {item.height}</span>
                </div>
                <div className="card-bottom-actions">
                  {/* 同理，缩略图隐藏后需要另一个放大入口 */}
                  <button
                    className="card-action-btn preview compact-only"
                    onClick={() => { setPreview(item); setOcrText(""); }}
                    title="放大预览"
                  >
                    <Maximize2 size={12} />
                    <span>预览</span>
                  </button>
                  <button
                    className="card-action-btn copy"
                    onClick={() => void extractText(item.id)}
                    disabled={ocrBusy !== null}
                    title="用系统 OCR 提取这张图里的文字"
                  >
                    <ScanText size={12} />
                    <span>{ocrBusy === item.id ? "识别中…" : "提取文字"}</span>
                  </button>
                  <button className="card-action-btn delete" onClick={() => void removeOne(item.id)} title="删除此快照">
                    <Trash2 size={12} />
                  </button>
                </div>
              </div>
            </div>
          ))}
        </div>
      ) : (
        <div className="history-empty-view">
          <div className="empty-icon-circle"><Camera size={28} /></div>
          <h3 className="empty-title">{query ? "未找到相关快照" : "还没有快照"}</h3>
          <p className="empty-desc">
            {query ? "没有符合当前关键词的记录，试试换个应用名。" : "用快捷键或便携坞截一张窗口快照，这里就会出现记录。"}
          </p>
          {query && <button className="empty-reset-btn" onClick={() => setQuery("")}>清空搜索</button>}
        </div>
      )}

      {ocrText && (
        <div className="prompt-rule-editor-drawer">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">识别结果</span>
              <span className="pane-char-count tabular-nums">{ocrText.length} 字符</span>
            </div>
            <div className="pane-result-actions">
              <button className="result-tool-btn" onClick={() => void navigator.clipboard.writeText(ocrText)}>
                <ClipboardCopy size={12} />
                <span>复制</span>
              </button>
              <button className="result-tool-btn" onClick={() => setOcrText("")}>
                <X size={12} />
                <span>关闭</span>
              </button>
            </div>
          </div>
          <pre className="polish-output">{ocrText}</pre>
        </div>
      )}

      {confirmClear && (
        <div
          className="snapshot-lightbox-backdrop"
          role="dialog"
          aria-modal="true"
          aria-labelledby="confirm-clear-title"
          onClick={() => setConfirmClear(false)}
        >
          <div className="confirm-dialog-panel" onClick={(event) => event.stopPropagation()}>
            <div className="confirm-dialog-head">
              <span className="confirm-dialog-icon"><Trash2 size={16} /></span>
              <div>
                <h3 className="confirm-dialog-title" id="confirm-clear-title">清空快照历史</h3>
                <p className="confirm-dialog-desc">
                  将删除全部 {records.length} 条记录，本机上对应的图片文件也会一并删除，无法恢复。
                </p>
              </div>
            </div>

            <div className="confirm-dialog-actions">
              <button className="confirm-cancel-btn" onClick={() => setConfirmClear(false)} disabled={clearing}>
                取消
              </button>
              <button className="confirm-danger-btn" onClick={() => void removeAll()} disabled={clearing} autoFocus>
                <Trash2 size={13} />
                <span>{clearing ? "清空中…" : "确认清空"}</span>
              </button>
            </div>
          </div>
        </div>
      )}

      {preview && (
        <div
          className="snapshot-lightbox-backdrop"
          role="dialog"
          aria-modal="true"
          aria-label={`${preview.appName} 放大预览`}
          onClick={() => setPreview(null)}
        >
          <div className="snapshot-lightbox-panel" onClick={(event) => event.stopPropagation()}>
            <div className="lightbox-header">
              <div className="lightbox-title-group">
                <span className="lightbox-type-tag">窗口快照</span>
                <span className="lightbox-title-text" title={preview.appName}>{preview.appName}</span>
              </div>
              <button className="lightbox-close-btn" onClick={() => setPreview(null)} title="关闭预览 (Esc)">
                <X size={16} />
              </button>
            </div>

            <div className="lightbox-stage">
              <img className="lightbox-art" src={snapshotUrl(preview.id)} alt="" style={{ objectFit: "contain" }} />
            </div>

            <div className="lightbox-meta-row">
              <span className="lightbox-meta-item"><Camera size={12} /><span>{preview.appName}</span></span>
              <span className="lightbox-meta-item tabular-nums"><Maximize2 size={12} /><span>{preview.width} × {preview.height}</span></span>
              <span className="lightbox-meta-item tabular-nums"><HardDrive size={12} /><span>{formatBytes(preview.sizeBytes)}</span></span>
              <span className="lightbox-meta-item"><Clock size={12} /><span>{formatWhen(preview.createdAt)}</span></span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
