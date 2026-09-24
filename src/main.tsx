import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { App } from "./App";
import { PetWindow } from "./windows/PetWindow";
import { QuickMenuWindow } from "./windows/QuickMenuWindow";
import { AnnotateWindow } from "./windows/AnnotateWindow";
import { applyTheme, listenThemeChanges, readTheme } from "./lib/theme";
import "./styles/variables.css";
import "./styles/app.css";
import "./styles.css";
import "./styles/responsive.css";

// 各个 WebView 独立，每个都要在首帧前套上主题，避免浅色闪一下
applyTheme(readTheme());
// 设置页改了主题后，其余窗口靠这条广播当场跟上，不必重开
listenThemeChanges(applyTheme);

function mount() {
  const label = getCurrentWindow().label;
  const usesTransparentSurface = label === "pet" || label === "quick-menu";
  document.documentElement.classList.toggle("transparent-window", usesTransparentSurface);
  document.body.classList.toggle("transparent-window", usesTransparentSurface);

  const CurrentWindow =
    label === "pet" ? PetWindow
    : label === "quick-menu" ? QuickMenuWindow
    : label === "annotate" ? AnnotateWindow
    : App;
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <CurrentWindow />
    </StrictMode>,
  );
}

// 直接在浏览器里打开页面（npm run dev）时没有 Tauri 后端：先装一层假后端再挂载
if ("__TAURI_INTERNALS__" in window) {
  mount();
} else {
  void import("./lib/preview").then(({ installPreviewBackend }) => {
    installPreviewBackend();
    mount();
  });
}
