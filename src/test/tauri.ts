import { mockConvertFileSrc, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { InvokeArgs } from "@tauri-apps/api/core";

export type IPCHandler = (command: string, payload?: InvokeArgs) => unknown;

const userAgents = {
  linux: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36",
  windows: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/130.0.0.0 Safari/537.36",
  macos: "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_0) AppleWebKit/605.1.15 Version/18.0 Safari/605.1.15",
};

export function setupTauriMock(
  handler: IPCHandler = () => undefined,
  options: { currentWindow?: string; userAgent?: string; shouldMockEvents?: boolean } = {},
) {
  const currentWindow = options.currentWindow ?? "main";
  mockWindows(currentWindow);
  // 图片地址走 convertFileSrc（media:// 协议），测试里也要有它
  mockConvertFileSrc("windows");
  Object.defineProperty(window.navigator, "userAgent", {
    configurable: true,
    value: options.userAgent ?? userAgents.windows,
  });
  mockIPC(handler, { shouldMockEvents: options.shouldMockEvents ?? false });
}

export function invokeArgs(payload: InvokeArgs | undefined): Record<string, unknown> {
  return (payload ?? {}) as Record<string, unknown>;
}
