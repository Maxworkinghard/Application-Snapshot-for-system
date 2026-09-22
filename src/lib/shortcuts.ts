import { invoke } from "@tauri-apps/api/core";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import type { ShortcutAction, ShortcutBinding } from "../types";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// 三端共用同一份前端 bundle，编译期分不了平台，只能看运行时的 UA：
// Windows 是 WebView2、macOS 是 WKWebView、Linux 是 WebKitGTK（UA 里带 X11; Linux）。
const isLinux =
  typeof navigator !== "undefined" &&
  /Linux/i.test(navigator.userAgent) &&
  !/Android/i.test(navigator.userAgent);

// 各平台支持的全局快捷键动作。滚动长截图只有 Linux/X11 有实现，
// Win/mac 侧连后端命令都不编译，绑了也只会拿到「未知快捷键动作」。
const SUPPORTED_ACTIONS: Record<ShortcutAction, boolean> = {
  snapshot: true,
  region: true,
  fullscreen: true,
  scrolling: isLinux,
  record: true,
  polish: true,
  ocr: true,
};

export function platformSupports(action: ShortcutAction): boolean {
  return SUPPORTED_ACTIONS[action];
}

export async function applyGlobalShortcuts(bindings: ShortcutBinding[]) {
  if (!inTauri) return;
  await unregisterAll();
  try {
    for (const binding of bindings) {
      if (!binding.accelerator) continue;
      if (!platformSupports(binding.action)) continue;
      await register(binding.accelerator, async (event) => {
        if (event.state !== "Pressed") return;
        await invoke("perform_action", { action: binding.action });
      });
    }
  } catch (error) {
    await unregisterAll();
    throw error;
  }
}
