import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { RefreshCw, Save, X } from "lucide-react";
import { fetchModels, savePromptSettings } from "../lib/backend";
import type { Settings } from "../types";

export function ModelSettingsDialog({
  settings,
  onSaved,
  notify,
  onClose,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
  onClose: () => void;
}) {
  const [baseUrl, setBaseUrl] = useState(settings.baseUrl);
  const [model, setModel] = useState(settings.model);
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [apiKey, setApiKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [fetchingModels, setFetchingModels] = useState(false);

  useEffect(() => {
    setBaseUrl(settings.baseUrl);
    setModel(settings.model);
  }, [settings]);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [onClose]);

  const serviceReady = Boolean(settings.baseUrl && settings.model);

  async function loadModels() {
    if (!baseUrl.trim()) {
      notify("请先填写 Base URL");
      return;
    }
    setFetchingModels(true);
    try {
      const models = await fetchModels(baseUrl.trim(), apiKey.trim() || null);
      setAvailableModels(models);
      if (!model.trim() && models[0]) setModel(models[0]);
      notify(`已拉取 ${models.length} 个模型`);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setFetchingModels(false);
    }
  }

  async function save() {
    setSaving(true);
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
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setSaving(false);
    }
  }

  return createPortal(
    <div
      className="snapshot-lightbox-backdrop"
      role="dialog"
      aria-modal="true"
      aria-labelledby="model-settings-title"
      onClick={onClose}
    >
      <div className="model-settings-dialog-panel" onClick={(event) => event.stopPropagation()}>
        <div className="model-settings-dialog-header">
          <div className="model-settings-dialog-heading">
            <div className="model-settings-dialog-title-row">
              <h2 className="model-settings-dialog-title" id="model-settings-title">编辑模型</h2>
              <span className="honest-hint-tag">{serviceReady ? "已配置" : "未配置"}</span>
            </div>
            <p className="model-settings-dialog-subtitle">配置模型端点与凭证，API Key 将保存到系统凭据管理器。</p>
          </div>
          <button className="lightbox-close-btn" onClick={onClose} title="关闭 (Esc)" aria-label="关闭模型编辑弹窗">
            <X size={16} />
          </button>
        </div>

        <div className="model-settings-fields">
          <label className="model-settings-field is-wide">
            <span className="drawer-input-label">接口端点 (Base URL)</span>
            <input
              type="text"
              className="drawer-text-input"
              value={baseUrl}
              onChange={(event) => setBaseUrl(event.target.value)}
              placeholder="https://api.example.com/v1"
            />
          </label>

          <label className="model-settings-field">
            <span className="drawer-input-label">访问密钥 (API Key)</span>
            <input
              type="password"
              className="drawer-text-input"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={settings.hasApiKey ? "已保存，留空保持不变" : "可选"}
            />
          </label>

          <label className="model-settings-field">
            <span className="drawer-input-label">目标模型</span>
            <input
              type="text"
              className="drawer-text-input"
              value={model}
              list="available-models"
              onChange={(event) => setModel(event.target.value)}
              placeholder="输入或拉取模型"
            />
            <datalist id="available-models">
              {availableModels.map((item) => <option key={item} value={item} />)}
            </datalist>
          </label>
        </div>

        <div className="model-settings-dialog-footer">
          <span className="drawer-footer-tip">
            {serviceReady ? `当前生效：${settings.model}` : "填好端点与模型后，润色功能才可用"}
          </span>
          <div className="pane-action-buttons">
            <button className="drawer-test-btn" onClick={() => void loadModels()} disabled={fetchingModels}>
              <RefreshCw size={12} className={fetchingModels ? "spinning" : ""} />
              <span>{fetchingModels ? "拉取中…" : "拉取模型"}</span>
            </button>
            <button className="main-action-btn generate" onClick={() => void save()} disabled={saving}>
              <Save size={13} />
              <span>{saving ? "保存中…" : "保存设置"}</span>
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}
