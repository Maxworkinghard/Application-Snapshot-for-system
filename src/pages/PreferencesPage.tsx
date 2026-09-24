import { useEffect, useState } from "react";
import { Play, Upload } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import { platformCapabilities, savePreferences } from "../lib/backend";
import { fileNameOf } from "../lib/format";
import { previewHintSound } from "../lib/sound";
import { PrefToggle } from "../components/ui/PrefToggle";
import { SegGroup } from "../components/ui/SegGroup";
import { ModelSettingsDialog } from "../components/ModelSettingsDialog";
import type { PlatformCapabilities, Settings } from "../types";

export function PreferencesPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
}) {
  const [caps, setCaps] = useState<PlatformCapabilities | null>(null);
  const [showModelSettings, setShowModelSettings] = useState(false);
  useEffect(() => {
    void platformCapabilities()
      .then(setCaps)
      .catch(() => setCaps(null));
  }, []);

  async function updatePrefs(patch: Partial<Settings>, message?: string) {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function importShutterSound() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: "音效文件", extensions: ["mp3", "wav", "ogg", "m4a"] }] });
      if (typeof selected === "string") {
        await updatePrefs({ shutterSound: "custom", customSoundPath: selected }, "自定义音效已导入");
      }
    } catch {
      notify("当前环境不支持选择文件");
    }
  }

  const localCapabilities = caps
    ? [
        { label: "录制", ...caps.recording },
        ...(caps.scrolling ? [{ label: "滚动长截图", ...caps.scrolling }] : []),
      ]
    : [];

  return (
    <div className="hub-preferences-page">
      <header className="hub-top-strip">
        <div className="hub-title-line">
          <h1 className="hub-heading">偏好设置</h1>
        </div>
      </header>

      <div className="hub-preferences-page-body">
        <section className="hub-section-block">
          <div className="section-label-bar">
            <span className="section-name">截屏与行为</span>
          </div>
        <div className="preferences-group-card">
          <div className="pref-item-row">
            <span className="pref-title">截屏音效</span>
            <SegGroup
              ariaLabel="截屏音效"
              value={settings.shutterSound}
              onChange={(value) => {
                if (value === "custom" && !settings.customSoundPath) {
                  void importShutterSound();
                  return;
                }
                void updatePrefs({ shutterSound: value });
              }}
              options={[
                { value: "crisp", label: "清脆" },
                { value: "soft", label: "轻柔" },
                { value: "none", label: "无音效" },
                { value: "custom", label: "自定义" },
              ]}
            />
          </div>
          {settings.shutterSound !== "none" && (
            <div className="pref-item-row no-desc">
              <span className="history-sub">
                {settings.shutterSound === "custom"
                  ? fileNameOf(settings.customSoundPath) || "未选择音效文件"
                  : "内置提示音"}
              </span>
              <div className="folder-picker-box">
                <button
                  type="button"
                  className="folder-action-btn"
                  onClick={() =>
                    previewHintSound(
                      settings.shutterSound === "soft" ? "soft" : "crisp",
                      settings.shutterSound === "custom" ? settings.customSoundPath : null,
                      80,
                    )
                  }
                >
                  <Play size={12} />
                  试听
                </button>
                <button type="button" className="folder-action-btn" onClick={() => void importShutterSound()}>
                  <Upload size={12} />
                  导入
                </button>
              </div>
            </div>
          )}
          <div className="pref-item-row">
            <span className="pref-title">截屏提示闪烁</span>
            <PrefToggle
              label="截屏提示闪烁"
              value={settings.flashOnCapture}
              onChange={(value) => void updatePrefs({ flashOnCapture: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">复制后隐藏主窗口</span>
            <PrefToggle
              label="复制后隐藏主窗口"
              value={settings.hideAfterCopy}
              onChange={(value) => void updatePrefs({ hideAfterCopy: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">自动写入本地文件</span>
            <PrefToggle
              label="自动写入本地文件"
              value={settings.autoSaveLocal}
              onChange={(value) => void updatePrefs({ autoSaveLocal: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">开机静默自启动</span>
            <PrefToggle
              label="开机静默自启动"
              value={settings.launchOnBoot}
              disabled={caps ? !caps.autostart.available : false}
              onChange={(value) => void updatePrefs({ launchOnBoot: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">截屏包含鼠标光标</span>
            <PrefToggle
              label="截屏包含鼠标光标"
              value={settings.includeCursor}
              disabled={caps ? !caps.includeCursor.available : false}
              onChange={(value) => void updatePrefs({ includeCursor: value })}
            />
          </div>
          <div className="pref-item-row">
            <span className="pref-title">截图完成后动作</span>
            <SegGroup
              ariaLabel="截图完成后动作"
              value={settings.afterCapture}
              onChange={(value) => void updatePrefs({ afterCapture: value })}
              options={[
                { value: "clipboard", label: "复制剪贴板" },
                { value: "annotate", label: "打开标注" },
                { value: "saveas", label: "另存为…" },
              ]}
            />
          </div>
        </div>
        </section>

      {caps && (
        <section className="hub-section-block">
          <div className="section-label-bar">
            <span className="section-name">本机能力</span>
          </div>
          <div className="local-capabilities-card">
            <div className="capability-platform-row">
              <span className="capability-platform-name">{caps.os}</span>
              <span className="capability-platform-separator" aria-hidden="true">·</span>
              <span className="capability-platform-name">{caps.displayServer}</span>
            </div>
            <div className="capability-list">
              {localCapabilities.map((capability) => (
                <div className="capability-row" key={capability.label}>
                  <span className="capability-name">{capability.label}</span>
                  <span className={`capability-status ${capability.available ? "is-available" : "is-unavailable"}`}>
                    <span className="capability-status-dot" aria-hidden="true" />
                    {capability.available ? "可用" : "不可用"}
                  </span>
                  <span className="capability-detail">{capability.detail}</span>
                </div>
              ))}
              <div className="capability-model-row">
                <span className="capability-name">模型</span>
                <span className={`capability-model-name ${settings.model ? "" : "is-empty"}`} title={settings.model || "未配置模型"}>
                  {settings.model || "未配置模型"}
                </span>
                <button className="capability-edit-btn" type="button" onClick={() => setShowModelSettings(true)}>
                  编辑
                </button>
              </div>
            </div>
          </div>
        </section>
      )}
      </div>
      {showModelSettings && (
        <ModelSettingsDialog
          settings={settings}
          onSaved={onSaved}
          notify={notify}
          onClose={() => setShowModelSettings(false)}
        />
      )}
    </div>
  );
}
