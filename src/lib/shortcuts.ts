import { invoke } from "@tauri-apps/api/core";
import { register, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import type { ShortcutBinding } from "../types";

const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function applyGlobalShortcuts(bindings: ShortcutBinding[]) {
  if (!inTauri) return;
  await unregisterAll();
  try {
    for (const binding of bindings) {
      if (!binding.accelerator) continue;
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
