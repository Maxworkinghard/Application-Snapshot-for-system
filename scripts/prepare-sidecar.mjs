// Tauri 开发/构建前置：macOS 上准备 ScreenCaptureKit 录制 sidecar，
// 其它平台直接跳过。
// 不放在 beforeBuildCommand 里直接调 bash，是为了不拖累 Windows 构建。
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

if (process.platform !== "darwin") {
  process.exit(0);
}

const root = dirname(dirname(fileURLToPath(import.meta.url)));
// sidecar 的文件名必须带上本次构建的目标 triple（universal 包要的是 -universal-apple-darwin），
// tauri build 会把目标写进 TAURI_ENV_TARGET_TRIPLE；手工跑本脚本时交给脚本退回 host triple。
const target = process.env.TAURI_ENV_TARGET_TRIPLE;
const args = [join(root, "scripts/build-recorder-sidecar.sh")];
if (target) {
  args.push(target);
}
execFileSync("bash", args, { stdio: "inherit" });
