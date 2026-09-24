import { useEffect, useRef } from "react";
import { createPortal } from "react-dom";

/** 不可恢复的操作前问一句。只有一句标题、一句后果、两个按钮 */
export function ConfirmDialog({
  title,
  body,
  confirmLabel,
  busy = false,
  onConfirm,
  onCancel,
}: {
  title: string;
  body: string;
  confirmLabel: string;
  busy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}) {
  const confirmRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    confirmRef.current?.focus();
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        onCancel();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [onCancel]);

  return createPortal(
    <div className="dialog-backdrop" onClick={onCancel}>
      <div
        className="dialog"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        aria-describedby="confirm-body"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 id="confirm-title" className="dialog-title">{title}</h2>
        <p id="confirm-body" className="dialog-body">{body}</p>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel} disabled={busy}>取消</button>
          <button ref={confirmRef} type="button" className="btn btn-danger" onClick={onConfirm} disabled={busy}>
            {busy ? "正在删除…" : confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
