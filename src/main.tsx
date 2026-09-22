import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { App } from "./App";
import { PetWindow } from "./windows/PetWindow";
import { QuickMenuWindow } from "./windows/QuickMenuWindow";
import { RegionPickerWindow } from "./windows/RegionPickerWindow";
import { AnnotateWindow } from "./windows/AnnotateWindow";
import { applyTheme, listenThemeChanges, readTheme } from "./lib/theme";
import "./styles/variables.css";
import "./styles/prototype-port.css";
import "./styles.css";
import "./styles/responsive.css";

// 各个 WebView 独立，每个都要在首帧前套上主题，避免浅色闪一下
applyTheme(readTheme());
// 设置页改了主题后，其余窗口靠这条广播当场跟上，不必重开
listenThemeChanges(applyTheme);

const label = "__TAURI_INTERNALS__" in window ? getCurrentWindow().label : "main";
const usesTransparentSurface = label === "pet" || label === "quick-menu" || label === "region-picker";

document.documentElement.classList.toggle("transparent-window", usesTransparentSurface);
document.body.classList.toggle("transparent-window", usesTransparentSurface);

function CurrentWindow() {
  if (label === "pet") return <PetWindow />;
  if (label === "quick-menu") return <QuickMenuWindow />;
  if (label === "region-picker") return <RegionPickerWindow />;
  if (label === "annotate") return <AnnotateWindow />;
  return <App />;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CurrentWindow />
  </StrictMode>,
);
