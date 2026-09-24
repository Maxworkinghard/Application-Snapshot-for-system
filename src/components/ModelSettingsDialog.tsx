import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { fetchModels, savePromptSettings } from "../lib/backend";
import type { Settings } from "../types";

/** 润色用的 OpenAI 兼容接口。API Key 存进系统钥匙串，不写进配置文件 */
export function ModelSettingsDialog({
  settings,
  onSaved,
  notify,
  onClose,
  leaving = false,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string, kind?: "info" | "error") => void;
  onClose: () => void;
  /** 正在退场（淡出、缩一点），这时不再接收点击 */
  leaving?: boolean;
}) {
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl);
  const [model, setModel] = useState(settings.model);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  async function loadModels() {
    if (!baseUrl.trim()) {
      setError("先填接口地址");
      return;
    }
    setFetchingModels(true);
    setError(null);
    try {
      const models = await fetchModels(baseUrl.trim(), apiKey.trim() || null);
      setAvailableModels(models);
      if (!model.trim() && models[0]) setModel(models[0]);
      notify(`拉到了 ${models.length} 个模型`);
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setFetchingModels(false);
    }
  }

  async function save() {
    setSaving(true);
    setError(null);
    try {
      // 模板归 Prompt 页管，这里原样回传，避免互相覆盖
      const next = await savePromptSettings({
        baseUrl: baseUrl.trim(),
        model: model.trim(),
        apiKey: apiKey.trim() || null,
        activeTemplateId: settings.activeTemplateId,
        templates: settings.templates,
      });
      onSaved(next);
      setApiKey("");
      notify("模型设置已保存");
      onClose();
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setSaving(false);
    }
  }

  return createPortal(
    <div className={`dialog-backdrop ${leaving ? "is-leaving" : ""}`} onClick={onClose}>
      <div
        className="dialog dialog-wide"
        role="dialog"
        aria-modal="true"
        aria-labelledby="model-settings-title"
        onClick={(event) => event.stopPropagation()}
      >
        <h2 className="dialog-title" id="model-settings-title">润色用的模型</h2>

        <div className="form">
          <label className="form-field">
            <span className="form-label">接口地址</span>
            <input
              className="input"
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
              placeholder="https://api.example.com/v1"
              spellCheck={false}
            />
          </label>
          <label className="form-field">
            <span className="form-label">API Key</span>
            <input
              className="input"
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={settings.hasApiKey ? "已保存；留空就不改" : "可选"}
            />
          </label>
          <label className="form-field">
            <span className="form-label">模型</span>
            <span className="form-inline">
              <input
                className="input mono"
                value={model}
                list="available-models"
                onChange={(event) => setModel(event.target.value)}
                spellCheck={false}
              />
              <button type="button" className="btn" onClick={() => void loadModels()} disabled={fetchingModels}>
                {fetchingModels ? "拉取中…" : "从接口拉取"}
              </button>
            </span>
            <datalist id="available-models">
              {availableModels.map((item) => <option key={item} value={item} />)}
            </datalist>
          </label>
        </div>

        {error && <p className="signal small" role="alert">{error}</p>}

        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onClose}>取消</button>
          <button type="button" className="btn btn-primary" onClick={() => void save()} disabled={saving}>
            {saving ? "保存中…" : "保存"}
          </button>
        </div>
      </div>
    </div>,
    document.body,
  );
}
