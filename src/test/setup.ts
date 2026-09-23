import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach, vi } from "vitest";

afterEach(async () => {
  cleanup();
  vi.useRealTimers();
  // Window components unregister Tauri listeners in a Promise continuation on unmount.
  await Promise.resolve();
  await Promise.resolve();
  clearMocks();
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
  Reflect.deleteProperty(window, "__TAURI_EVENT_PLUGIN_INTERNALS__");
  localStorage.clear();
  vi.restoreAllMocks();
});
