import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { emit } from "@tauri-apps/api/event";
import type { Settings } from "../types";
import { setupTauriMock } from "../test/tauri";

const settings = {
  baseUrl: "",
  model: "",
  hasApiKey: false,
  templates: [{ id: "builtin-default", name: "默认", content: "x", builtin: true }],
  activeTemplateId: "builtin-default",
  selectedAppearanceId: "pet-1",
  petAssets: [
    { id: "pet-1", name: "三花", path: "", entry: "idle.gif", animations: ["idle.gif", "wave.gif"] },
  ],
  shortcuts: [],
  clipboardAutoClear: "60s",
  snapshotFormat: "png",
  saveDir: "",
  recordingDir: "",
  shutterSound: "crisp",
  customSoundPath: null,
  flashOnCapture: true,
  hideAfterCopy: false,
  autoSaveLocal: true,
  launchOnBoot: false,
  includeCursor: false,
  recordSystemAudio: false,
  recordMicrophone: false,
  afterCapture: "clipboard",
} as Settings;

describe("侧栏的猫跟着桌宠", () => {
  it("没收到桌宠消息时放默认动作，桌宠换动作这里也换", async () => {
    setupTauriMock(
      (command: string) => (command === "load_settings" ? settings : undefined),
      { currentWindow: "main", shouldMockEvents: true },
    );
    vi.resetModules();
    const { App } = await import("../App");
    const { container } = render(<App />);
    await screen.findByRole("button", { name: /三花/ });

    // 还没收到桌宠消息：放默认动作
    const cat = () => container.querySelector<HTMLImageElement>(".companion-cat img");
    await waitFor(() => expect(decodeURIComponent(cat()!.src)).toContain("pet/pet-1/idle.gif"));

    await emit("pet-animation", { assetId: "pet-1", entry: "wave.gif" });
    await waitFor(() => expect(decodeURIComponent(cat()!.src)).toContain("pet/pet-1/wave.gif"));
  });
});

// jsdom 未实现 Element.prototype.scrollTo；App 换页后会调用它回到顶部。
if (!Element.prototype.scrollTo) {
  Element.prototype.scrollTo = () => {};
}
