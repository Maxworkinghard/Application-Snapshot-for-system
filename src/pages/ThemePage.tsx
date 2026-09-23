import { Monitor, Moon, Sun } from "lucide-react";
import type { ThemeMode, ThemePreference } from "../lib/theme";

const themeOptions: Array<{
  value: ThemePreference;
  label: string;
  hint: string;
  icon: typeof Sun;
}> = [
  { value: "light", label: "浅色", hint: "始终使用浅色界面", icon: Sun },
  { value: "dark", label: "深色", hint: "始终使用深色界面", icon: Moon },
  { value: "system", label: "跟随系统", hint: "随系统外观自动切换", icon: Monitor },
];

export function ThemePage({
  preference,
  resolved,
  onChange,
}: {
  preference: ThemePreference;
  resolved: ThemeMode;
  onChange: (next: ThemePreference) => void;
}) {
  const resolvedLabel = resolved === "dark" ? "深色" : "浅色";

  return (
    <div className="prompt-lab-workspace-container">
      <div className="prompt-endpoint-drawer" role="region" aria-label="界面主题">
        <div className="drawer-header-row">
          <div className="drawer-title-group">
            <span className="drawer-title">界面主题</span>
            <span className="honest-hint-tag">
              {preference === "system" ? `跟随系统 · 当前为${resolvedLabel}` : `已固定为${resolvedLabel}`}
            </span>
          </div>
        </div>

        <div className="theme-choice-grid" role="radiogroup" aria-label="界面主题">
          {themeOptions.map((option) => {
            const Icon = option.icon;
            const isActive = preference === option.value;
            return (
              <button
                key={option.value}
                type="button"
                role="radio"
                aria-checked={isActive}
                className={`theme-choice-card ${isActive ? "is-active" : ""}`}
                onClick={() => onChange(option.value)}
              >
                <span className={`theme-choice-swatch is-${option.value}`} aria-hidden="true">
                  <Icon size={16} />
                </span>
                <span className="theme-choice-name">{option.label}</span>
                <span className="theme-choice-hint">{option.hint}</span>
              </button>
            );
          })}
        </div>

        <div className="theme-note-list">
          <span className="history-sub">
            主题对主窗口、桌面伴侣与快捷菜单同时生效。
          </span>
          <span className="history-sub">
            该选择存在本机浏览器存储里，不写入配置文件，也不随配置同步到别的设备。
          </span>
          <span className="history-sub">自定义主题导入尚未接入，目前只有以上三项内置主题。</span>
        </div>
      </div>
    </div>
  );
}
