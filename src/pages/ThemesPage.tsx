import { CatShape } from "../components/ui/CatShape";
import { Choices } from "../components/ui/Choices";
import { useApp } from "../app/context";
import type { LayoutTheme } from "../lib/prefs";

export const LAYOUT_INFO: Record<LayoutTheme, { name: string; description: string }> = {
  companion: { name: "伴侣侧栏", description: "猫住在侧栏底，替你播报刚发生的事" },
  timeline: { name: "时间线", description: "首页就是今天做过的事，设置收进一页" },
  ledger: { name: "今日流水", description: "侧栏下半截列出今天的每一步" },
  topbar: { name: "不要侧栏", description: "入口放进标题栏，内容区最宽" },
};

const bar = (width: string, tone = "var(--rule)") => <span className="mini-bar" style={{ width, background: tone }} />;

/** 四种布局的示意缩略图：只画结构，不画内容 */
function LayoutSketch({ layout }: { layout: LayoutTheme }) {
  const cat = <CatShape height={16} className="mini-cat" />;
  const side = (extra?: React.ReactNode) => (
    <span className="mini-side">
      {bar("70%", "var(--ink)")}
      {bar("70%", "var(--rule-strong)")}
      {bar("70%", "var(--rule-strong)")}
      {bar("70%", "var(--rule-strong)")}
      {extra}
    </span>
  );
  const rows = (
    <span className="mini-main mini-cols">
      {Array.from({ length: 8 }, (_, index) => (
        <span key={index} className="mini-bar" style={{ background: index < 2 ? "var(--rule-strong)" : "var(--rule)" }} />
      ))}
    </span>
  );
  if (layout === "companion") {
    return (
      <span className="mini mini-row">
        {side(<><span className="mini-grow" />{cat}<span className="mini-floor" /></>)}
        {rows}
      </span>
    );
  }
  if (layout === "ledger") {
    return (
      <span className="mini mini-row">
        {side(
          <>
            <span className="mini-rule" />
            {bar("80%")}
            {bar("60%")}
            {bar("90%")}
            {bar("50%", "var(--signal)")}
            {bar("75%")}
          </>,
        )}
        {rows}
      </span>
    );
  }
  if (layout === "topbar") {
    return (
      <span className="mini mini-col">
        <span className="mini-top">
          {bar("14px", "var(--ink)")}
          {bar("14px", "var(--rule-strong)")}
          {bar("14px", "var(--rule-strong)")}
          {bar("14px", "var(--rule-strong)")}
        </span>
        <span className="mini-main mini-cols mini-cols-3">
          {Array.from({ length: 9 }, (_, index) => (
            <span key={index} className="mini-bar" style={{ background: index < 3 ? "var(--rule-strong)" : "var(--rule)" }} />
          ))}
        </span>
      </span>
    );
  }
  return (
    <span className="mini mini-row">
      <span className="mini-main mini-stream">
        <span className="mini-input" />
        {[["var(--ink-2)", "14px"], ["transparent", "14px"], ["var(--rule)", "14px"], ["var(--rule)", "4px"]].map(([tone, width], index) => (
          <span key={index} className="mini-line">
            <span className="mini-thumb" style={{ background: tone, width }} />
            {bar("100%")}
          </span>
        ))}
      </span>
      <span className="mini-side mini-side-right">
        {bar("100%", "var(--ink)")}
        {bar("100%", "var(--rule-strong)")}
        <span className="mini-grow" />
        {cat}
        <span className="mini-floor" />
      </span>
    </span>
  );
}

export function ThemesPanel() {
  const { layout, setLayout, themePreference, setThemePreference, motionPreference, setMotionPreference } = useApp();
  return (
    <section className="panel panel-themes">
      <div className="panel-head">
        <h2 className="panel-title">布局</h2>
        <span className="small quiet">点一下立即生效，随时换回来</span>
      </div>
      <div className="layouts" role="radiogroup" aria-label="布局主题">
        {(Object.keys(LAYOUT_INFO) as LayoutTheme[]).map((key) => (
          <button
            key={key}
            type="button"
            role="radio"
            aria-checked={layout === key}
            className={`layout-option ${layout === key ? "is-on" : ""}`}
            onClick={() => setLayout(key)}
          >
            <LayoutSketch layout={key} />
            <span className="layout-name">
              <span className="strong">{LAYOUT_INFO[key].name}</span>
              {layout === key && <span className="small quiet">使用中{key === "companion" ? " · 默认" : ""}</span>}
              {layout !== key && key === "companion" && <span className="small quiet">默认</span>}
            </span>
            <span className="small quiet layout-desc">{LAYOUT_INFO[key].description}</span>
          </button>
        ))}
      </div>

      <div className="rows rows-gap">
        <div className="row">
          <span className="row-label">颜色</span>
          <Choices
            ariaLabel="颜色"
            value={themePreference}
            onChange={setThemePreference}
            options={[
              { value: "system", label: "跟随系统" },
              { value: "light", label: "浅色" },
              { value: "dark", label: "深色" },
            ]}
          />
        </div>
        <div className="row">
          <span className="row-label-stack">
            <span className="row-label">动效</span>
            <span className="small quiet">减弱：只留淡入淡出，猫停在静态帧</span>
          </span>
          <Choices
            ariaLabel="动效"
            value={motionPreference}
            onChange={setMotionPreference}
            options={[
              { value: "system", label: "跟随系统" },
              { value: "full", label: "完整" },
              { value: "reduced", label: "减弱" },
            ]}
          />
        </div>
      </div>
      <p className="footnote">布局、颜色、动效对主窗口、桌宠和输入框同时生效，存在这台电脑上。</p>
    </section>
  );
}

export function ThemesPage() {
  return (
    <div className="page-single">
      <ThemesPanel />
    </div>
  );
}
