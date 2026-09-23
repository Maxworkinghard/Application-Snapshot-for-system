import { useEffect, useState } from "react";
import { Check, ClipboardCopy, ScanText } from "lucide-react";
import { ocrCapability, ocrClipboard } from "../lib/backend";
import type { OcrCapability } from "../types";

export function OcrPage({ notify }: { notify: (message: string) => void }) {
  const [capability, setCapability] = useState<OcrCapability | null>(null);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    void ocrCapability().then(setCapability).catch(() => setCapability(null));
  }, []);

  async function runOcr() {
    setBusy(true);
    setText("");
    try {
      setText(await ocrClipboard());
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setBusy(false);
    }
  }

  async function copyText() {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1800);
    } catch {
      notify("复制失败，请手动选中复制");
    }
  }

  return (
    <div className="prompt-lab-workspace-container">
      <div className="prompt-top-control-bus">
        <div className="control-bus-left">
          <span className="bus-label">文字识别</span>
          <span className={`bus-model-pill ${capability?.available ? "is-active" : ""}`}>
            <span className="model-dot" />
            <span className="model-text">{capability?.available ? "引擎可用" : "引擎不可用"}</span>
          </span>
        </div>
        <div className="control-bus-right">
          <button className="bus-action-btn" onClick={() => void runOcr()} disabled={busy || !capability?.available}>
            <ScanText size={13} />
            <span>{busy ? "识别中…" : "识别剪贴板图片"}</span>
          </button>
        </div>
      </div>

      <div className="prompt-main-split-workbench">
        <div className="workbench-pane draft-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">引擎状态</span>
            </div>
          </div>
          <div className="companion-behavior-settings">
            <div className="behavior-row no-desc">
              <span className="b-title">识别引擎</span>
              <span className="history-sub">{capability?.detail ?? "正在检测…"}</span>
            </div>
            <div className="behavior-row no-desc">
              <span className="b-title">隐私</span>
              <span className="history-sub">调用系统本地引擎，图片不出本机</span>
            </div>
            <div className="behavior-row no-desc">
              <span className="b-title">用法</span>
              <span className="history-sub">先截图或复制一张图片，再点右上角按钮</span>
            </div>
            <div className="behavior-row no-desc">
              <span className="b-title">历史库</span>
              <span className="history-sub">「快照历史」里每张快照也可以单独提取文字</span>
            </div>
          </div>
        </div>

        <div className="workbench-pane result-pane">
          <div className="pane-header-strip">
            <div className="pane-title-group">
              <span className="pane-main-title">识别结果</span>
              {text && <span className="pane-char-count tabular-nums">{text.length} 字符</span>}
            </div>
            <div className="pane-result-actions">
              {text && (
                <div className="result-action-button-group">
                  <button className={`result-tool-btn ${copied ? "copied" : ""}`} onClick={() => void copyText()}>
                    {copied ? <Check size={12} /> : <ClipboardCopy size={12} />}
                    <span>{copied ? "已复制" : "复制结果"}</span>
                  </button>
                </div>
              )}
            </div>
          </div>

          <div className="pane-textarea-wrap">
            {busy ? (
              <div className="polish-placeholder"><span className="inline-spinner" />正在识别…</div>
            ) : text ? (
              <pre className="polish-output">{text}</pre>
            ) : (
              <div className="polish-placeholder">识别结果会显示在这里。</div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
