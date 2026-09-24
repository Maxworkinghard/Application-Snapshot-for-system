import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { savePreferences } from "../lib/backend";
import { fileNameOf } from "../lib/format";
import { previewHintSound } from "../lib/sound";
import { Switch } from "../components/ui/Switch";
import { Choices } from "../components/ui/Choices";
import { usePresence } from "../lib/motion";
import { ModelSettingsDialog } from "../components/ModelSettingsDialog";
import { errorText, useApp } from "../app/context";
import type { Settings } from "../types";

function usePrefs() {
  const { onSaved, notify } = useApp();
  return async (patch: Partial<Settings>, message?: string) => {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(errorText(error), "error");
    }
  };
}

/** 截屏时的声音、闪烁、截完做什么 */
export function BehaviorPanel() {
  const { settings, caps, notify } = useApp();
  const update = usePrefs();

  async function importShutterSound() {
    try {
      const selected = await open({ multiple: false, filters: [{ name: "音效文件", extensions: ["mp3", "wav", "ogg", "m4a"] }] });
      if (typeof selected === "string") {
        await update({ shutterSound: "custom", customSoundPath: selected }, "自定义音效已导入");
      }
    } catch {
      notify("当前环境不支持选择文件", "error");
    }
  }

  const toggles: Array<{ key: keyof Settings; label: string; disabled?: boolean }> = [
    { key: "flashOnCapture", label: "截屏时闪一下" },
    { key: "autoSaveLocal", label: "截图同时存进历史" },
    { key: "hideAfterCopy", label: "复制后隐藏主窗口" },
    { key: "includeCursor", label: "截图带上鼠标指针", disabled: caps ? !caps.includeCursor.available : false },
    { key: "launchOnBoot", label: "开机时静默启动", disabled: caps ? !caps.autostart.available : false },
  ];

  return (
    <section className="panel">
      <h2 className="panel-title">截屏与行为</h2>
      <div className="rows">
        <div className="row">
          <span className="row-label">截完之后</span>
          <Choices
            ariaLabel="截完之后"
            value={settings.afterCapture}
            onChange={(value) => void update({ afterCapture: value })}
            options={[
              { value: "clipboard", label: "放进剪贴板" },
              { value: "annotate", label: "打开标注" },
              { value: "saveas", label: "另存为…" },
            ]}
          />
        </div>
        <div className="row row-stack">
          <div className="row-line">
            <span className="row-label">快门声</span>
            <Choices
              ariaLabel="快门声"
              value={settings.shutterSound}
              onChange={(value) => {
                if (value === "custom" && !settings.customSoundPath) {
                  void importShutterSound();
                  return;
                }
                void update({ shutterSound: value });
              }}
              options={[
                { value: "crisp", label: "清脆" },
                { value: "soft", label: "轻柔" },
                { value: "custom", label: "自定义" },
                { value: "none", label: "不响" },
              ]}
            />
          </div>
          {settings.shutterSound !== "none" && (
            <span className="row-links small">
              <span className="quiet">
                {settings.shutterSound === "custom" ? fileNameOf(settings.customSoundPath) || "还没选音效文件" : "内置提示音"}
              </span>
              <button
                type="button"
                className="link"
                onClick={() =>
                  previewHintSound(
                    settings.shutterSound === "soft" ? "soft" : "crisp",
                    settings.shutterSound === "custom" ? settings.customSoundPath : null,
                    80,
                  )
                }
              >
                试听
              </button>
              <button type="button" className="link" onClick={() => void importShutterSound()}>换一个文件</button>
            </span>
          )}
        </div>
        {toggles.map((toggle) => (
          <div className="row" key={toggle.key}>
            <label className="row-label" htmlFor={`pref-${toggle.key}`}>{toggle.label}</label>
            <Switch
              id={`pref-${toggle.key}`}
              label={toggle.label}
              value={Boolean(settings[toggle.key])}
              disabled={toggle.disabled}
              onChange={(value) => void update({ [toggle.key]: value } as Partial<Settings>)}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

/** 这台机器能做什么（各端 adapter 实时报告）+ 模型 */
export function MachinePanel() {
  const { settings, caps } = useApp();
  const [editingModel, setEditingModel] = useState(false);
  const modelDialog = usePresence(editingModel || null);
  const { onSaved, notify } = useApp();

  const capabilities = caps
    ? [
        { label: "窗口录制", ...caps.recording },
        ...(caps.scrolling ? [{ label: "滚动长截图", ...caps.scrolling }] : []),
      ]
    : [];

  return (
    <section className="panel">
      <h2 className="panel-title">本机与模型</h2>
      <div className="rows">
        <div className="row">
          <span className="row-label">润色用的模型</span>
          <span className="row-links small">
            <span className={settings.model ? "mono ink-2" : "signal"}>{settings.model || "还没填"}</span>
            <button type="button" className="link" onClick={() => setEditingModel(true)}>
              {settings.model ? "修改" : "去填写"}
            </button>
          </span>
        </div>
        {caps && (
          <div className="row">
            <span className="row-label">系统</span>
            <span className="mono small ink-2">{caps.os} · {caps.displayServer}</span>
          </div>
        )}
        {capabilities.map((capability) => (
          <div className="row" key={capability.label}>
            <span className="row-label">{capability.label}</span>
            <span className={`small ${capability.available ? "ink-2" : "signal"}`}>{capability.available ? "可用" : "不可用"}</span>
          </div>
        ))}
      </div>
      {modelDialog.item && (
        <ModelSettingsDialog
          settings={settings}
          onSaved={onSaved}
          notify={notify}
          onClose={() => setEditingModel(false)}
          leaving={modelDialog.leaving}
        />
      )}
    </section>
  );
}

export function PreferencesPage() {
  return (
    <div className="page-grid page-grid-2">
      <BehaviorPanel />
      <MachinePanel />
    </div>
  );
}
