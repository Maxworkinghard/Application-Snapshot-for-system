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
  fullscreen: true,
  scrolling: isLinux,
  record: true,
  recordings: true,
  polish: true,
  ocr: true,
};

export function platformSupports(action: ShortcutAction): boolean {
  return SUPPORTED_ACTIONS[action];
}

/** 上一次注册的尾巴，用来把并发调用排成队 */
let inFlight: Promise<unknown> = Promise.resolve();

/**
 * 重新注册全部全局快捷键。
 *
 * 必须串行：保存一次会连着触发好几轮设置更新——save() 自己调一次，
 * onSaved 引起的 state 更新一次，后端广播的 settings-changed 又一次。
 * 并发跑的话，前一轮刚注册完、后一轮的 unregisterAll 还没走到，
 * 两边就会抢着注册同一个键，后到的撞上「已注册」——那其实是应用
 * 自己和自己撞，却会被当成「被其他应用占用」报给用户。
 */
export async function applyGlobalShortcuts(bindings: ShortcutBinding[]) {
  if (!inTauri) return;
  const run = inFlight.catch(() => {}).then(() => registerAll(bindings));
  // 队列本身不能因为某次失败而断掉，所以这里吞掉异常，只把结果交给调用方
  inFlight = run.catch(() => {});
  return run;
}

async function registerAll(bindings: ShortcutBinding[]) {
  await unregisterAll();
  // 逐个注册：一个键被占用不该连累其它键，之前是一失败就 unregisterAll，
  // 结果一个冲突把刚注册好的全都注销了，按什么都没反应
  const failed: string[] = [];
  for (const binding of bindings) {
    if (!binding.accelerator) continue;
    if (!platformSupports(binding.action)) continue;
    try {
      await register(binding.accelerator, async (event) => {
        if (event.state !== "Pressed") return;
        await invoke("perform_action", { action: binding.action });
      });
    } catch {
      failed.push(binding.accelerator);
    }
  }
  if (failed.length > 0) {
    throw new Error(failed.join("、"));
  }
}
