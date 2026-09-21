import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { App } from "./App";
import { PetWindow } from "./windows/PetWindow";
import { QuickMenuWindow } from "./windows/QuickMenuWindow";
import { applyTheme, readTheme } from "./lib/theme";
import "./styles/variables.css";
import "./styles/prototype-port.css";
import "./styles.css";
import "./styles/responsive.css";

// 三个 WebView 各自独立，每个都要在首帧前套上主题，避免浅色闪一下
applyTheme(readTheme());

const label = "__TAURI_INTERNALS__" in window ? getCurrentWindow().label : "main";
const usesTransparentSurface = label === "pet" || label === "quick-menu";

document.documentElement.classList.toggle("transparent-window", usesTransparentSurface);
document.body.classList.toggle("transparent-window", usesTransparentSurface);

function CurrentWindow() {
  if (label === "pet") return <PetWindow />;
  if (label === "quick-menu") return <QuickMenuWindow />;
  return <App />;
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CurrentWindow />
  </StrictMode>,
);
