import { useEffect, useMemo, useState } from "react";
import { AppWindow, Check, PawPrint, SlidersHorizontal, Trash2, Upload } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  addPetAsset,
  deletePetAsset,
  savePreferences,
  selectPetAppearance,
} from "../lib/backend";
import { petUrl } from "../lib/media";
import { MediaImage } from "../components/MediaImage";
import type { Settings } from "../types";

export function PetPage({
  settings,
  onSaved,
  notify,
}: {
  settings: Settings;
  onSaved: (value: Settings) => void;
  notify: (message: string) => void;
}) {
  const [activeEntry, setActiveEntry] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);

  const selectedId = settings.selectedAppearanceId;
  const selectedAsset = settings.petAssets.find((item) => item.id === selectedId) ?? null;

  // 切换形态时重置到该素材包的首个动作
  useEffect(() => {
    setActiveEntry(selectedAsset?.entry || selectedAsset?.animations[0] || null);
  }, [selectedId, selectedAsset?.entry]);

  async function choose(id: string) {
    try {
      onSaved(await selectPetAppearance(id));
      notify("形象已切换");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function addAsset() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "桌宠形象", extensions: ["zip", "gif"] }],
    });
    if (typeof selected !== "string") return;
    setImporting(true);
    try {
      onSaved(await addPetAsset(selected));
      notify("素材包已导入");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    } finally {
      setImporting(false);
    }
  }

  async function removeAsset(id: string) {
    try {
      onSaved(await deletePetAsset(id));
      notify("形象已删除");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  async function updatePrefs(patch: Partial<Settings>, message?: string) {
    try {
      onSaved(await savePreferences(patch));
      if (message) notify(message);
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error));
    }
  }

  /** 常见动作名的中文对照；素材包命名五花八门，命中不了就退回原名 */
  const ACTION_NAMES: Record<string, string> = {
    idle: "待机",
    sleep: "打盹",
    sleeping: "打盹",
    dance: "跳舞",
    dancing: "跳舞",
    think: "思考",
    thinking: "思考",
    type: "敲键盘",
    typing: "敲键盘",
    "type-keyboard": "敲键盘",
    keyboard: "敲键盘",
    build: "搭建",
    carry: "搬运",
    cheer: "欢呼",
    crabwalk: "横着走",
    error: "报错",
    jump: "跳跃",
    "jump-happy": "开心跳",
    point: "指路",
    sweep: "打扫",
    listening: "聆听",
    listen: "聆听",
    "xmas-idle": "圣诞待机",
    xmas: "圣诞",
    greeting: "挥爪问候",
    greet: "挥爪问候",
    wave: "招手",
    snuggle: "撒娇",
    walk: "走动",
    run: "跑动",
    eat: "进食",
    work: "工作",
    happy: "开心",
    sad: "难过",
    angry: "生气",
    surprised: "惊讶",
    love: "卖萌",
    hello: "打招呼",
    bye: "告别",
    "type-laptop": "敲笔记本",
    laptop: "笔记本",
    read: "阅读",
    write: "书写",
    code: "写代码",
    coding: "写代码",
    search: "搜索",
    wait: "等待",
    waiting: "等待",
    alert: "警告",
    warning: "警告",
    success: "成功",
    done: "完成",
    loading: "加载中",
    drag: "拖拽",
    click: "点击",
    sit: "坐下",
    stand: "站立",
  };

  /** 整包共有的前缀（如 clawd-）对用户没有信息量，逐条截掉 */
  const entryPrefix = useMemo(() => {
    const stems = (selectedAsset?.animations ?? []).map(
      (entry) => (entry.split("/").pop() ?? entry).replace(/\.[^.]+$/, ""),
    );
    if (stems.length < 2) return "";
    let prefix = stems[0];
    for (const stem of stems.slice(1)) {
      while (prefix && !stem.startsWith(prefix)) prefix = prefix.slice(0, -1);
      if (!prefix) break;
    }
    // 只在分隔符处截断，免得把 "sleep" 砍成 "sle"
    const cut = Math.max(prefix.lastIndexOf("-"), prefix.lastIndexOf("_"));
    return cut > 0 ? prefix.slice(0, cut + 1) : "";
  }, [selectedAsset?.id, selectedAsset?.animations]);

  /** cat/clawd-type-keyboard.gif -> 敲键盘 */
  function entryLabel(entry: string): string {
    const stem = (entry.split("/").pop() ?? entry).replace(/\.[^.]+$/, "");
    const key = (entryPrefix && stem.startsWith(entryPrefix) ? stem.slice(entryPrefix.length) : stem).toLowerCase();
    if (ACTION_NAMES[key]) return ACTION_NAMES[key];
    // 整体没命中就按分隔符逐段翻，但要求每段都能译——
    // 只译出一半会得到「敲键盘laptop」这种中英混搭，不如保留原名
    const parts = key.split(/[-_]/).filter(Boolean);
    if (parts.length > 1 && parts.every((part) => ACTION_NAMES[part])) {
      return parts.map((part) => ACTION_NAMES[part]).join("");
    }
    return key || stem;
  }

  const rows = [
    { id: "app-icon", name: "当前应用图标", desc: "不显示动画，仅展示前台应用的程序图标", builtin: true },
    ...settings.petAssets.map((asset) => ({
      id: asset.id,
      name: asset.name,
      desc: `${asset.animations.length} 个动作 · ${asset.path}`,
      builtin: false,
    })),
  ];

  return (
    <div className="pet-atelier-workspace">
      <section className="stage-canvas-panel">
        <div className="panel-header-strip">
          <span className="panel-title-text">伴侣悬浮演示</span>
        </div>

        <div className="viewport-desktop-canvas">
          <div className="stage-live-badge">
            <span className="status-led-dot" />
            <span>{selectedAsset ? "动态预览" : "外观预览"}</span>
          </div>

          <div className="viewport-avatar-center">
            <div className="cat-stage-orb">
              {selectedAsset ? (
                <MediaImage
                  key={`${selectedAsset.id}/${activeEntry ?? ""}`}
                  className="stage-image-element"
                  src={petUrl(selectedAsset.id, activeEntry)}
                  fallback={<div className="polish-placeholder">素材读取失败，原文件可能已被移走</div>}
                />
              ) : (
                <div className="pixel-stage-orb">
                  <AppWindow size={40} />
                </div>
              )}
            </div>
          </div>
        </div>

        {selectedAsset && selectedAsset.animations.length > 1 && (
          <div className="stage-actions-segmented-row">
            <div className="segmented-track">
              {selectedAsset.animations.map((entry) => (
                <button
                  key={entry}
                  className={`segmented-item-btn ${activeEntry === entry ? "is-active" : ""}`}
                  onClick={() => setActiveEntry(entry)}
                  title={entry}
                >
                  {entryLabel(entry)}
                </button>
              ))}
            </div>
          </div>
        )}
      </section>

      <section className="config-wardrobe-panel">
        <div className="panel-header-strip">
          <span className="panel-title-text">形态与特性</span>
          <button
            className="import-zip-ghost-btn"
            onClick={() => void addAsset()}
            disabled={importing}
            title="导入 .zip 素材包"
          >
            <Upload size={14} />
            <span>{importing ? "导入中" : "导入素材包 (.zip)"}</span>
          </button>
        </div>

        <div className="appearance-cards-col" role="radiogroup" aria-label="伴侣形态选择">
          {rows.map((row) => {
            const isSelected = selectedId === row.id;
            return (
              <div
                key={row.id}
                role="radio"
                aria-checked={isSelected}
                tabIndex={0}
                className={`appearance-row-item ${isSelected ? "is-selected" : ""}`}
                onClick={() => void choose(row.id)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    void choose(row.id);
                  }
                }}
              >
                <div className="avatar-thumbnail-wrap">
                  {row.builtin ? (
                    <div className="thumbnail-app-icon"><AppWindow size={16} /></div>
                  ) : (
                    <MediaImage
                      className="thumbnail-img"
                      src={petUrl(row.id)}
                      fallback={<div className="thumbnail-app-icon"><PawPrint size={16} /></div>}
                    />
                  )}
                </div>

                <div className="avatar-meta-col">
                  <span className="avatar-title">{row.name}</span>
                  <span className="avatar-subtitle">{row.desc}</span>
                </div>

                {!row.builtin && (
                  <button
                    className="clear-keycap-ghost-btn"
                    title="删除这个形象"
                    onClick={(event) => { event.stopPropagation(); void removeAsset(row.id); }}
                  ><Trash2 size={13} /></button>
                )}

                <div className={`radio-dot-indicator ${isSelected ? "is-active" : ""}`}>
                  {isSelected && <Check size={12} />}
                </div>
              </div>
            );
          })}
        </div>

        <div className="companion-behavior-settings">
          <div className="behavior-header">
            <SlidersHorizontal size={14} />
            <span>形象素材要求</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">导入方式</span>
            <span className="history-sub">装着多个 GIF 的 ZIP 压缩包，或直接选一个 GIF</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">支持格式</span>
            <span className="history-sub">仅 GIF，包内其它格式会被跳过</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">默认动作</span>
            <span className="history-sub">文件名含 idle 的会排在最前</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">体积上限</span>
            <span className="history-sub">整包 100MB，单个 GIF 50MB</span>
          </div>
          <div className="behavior-row no-desc">
            <span className="b-title">注意</span>
            <span className="history-sub">素材按需读取、不会复制，移走原文件形象会失效</span>
          </div>
        </div>
      </section>
    </div>
  );
}
