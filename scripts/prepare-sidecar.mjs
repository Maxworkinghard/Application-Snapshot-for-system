// tauri build 前置：macOS 上把 snapshot-ocr 编成 sidecar，其它平台直接跳过。
// 不放在 beforeBuildCommand 里直接调 bash，是为了不拖累 Windows 构建。
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

if (process.platform !== "darwin") {
  process.exit(0);
}

const root = dirname(dirname(fileURLToPath(import.meta.url)));
execFileSync("bash", [join(root, "scripts/build-ocr-sidecar.sh")], { stdio: "inherit" });
