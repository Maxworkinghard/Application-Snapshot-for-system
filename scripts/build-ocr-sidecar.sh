#!/bin/bash
# 构建 snapshot-ocr（Vision OCR 桥）并复制成 Tauri sidecar 命名：
#   src-tauri/snapshot-ocr-<host-triple>
# 仅 macOS 有意义；其它平台的调用方应直接跳过本脚本。
set -euo pipefail

cd "$(dirname "$0")/../src-tauri/snapshot-ocr"
swift build -c release

triple="$(rustc -vV | sed -n 's/^host: //p')"
dest="../snapshot-ocr-${triple}"
cp ".build/release/snapshot-ocr" "$dest"
echo "sidecar 就绪：src-tauri/snapshot-ocr-${triple}"
