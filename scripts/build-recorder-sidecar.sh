#!/bin/bash
# 构建 snapshot-recorder（ScreenCaptureKit 窗口录制桥）并复制成 Tauri sidecar 命名：
#   src-tauri/snapshot-recorder-<target-triple>
# 用法与 build-ocr-sidecar.sh 相同。
set -euo pipefail

target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"

case "$target" in
  universal-apple-darwin) archs=(arm64 x86_64) ;;
  aarch64-apple-darwin)   archs=(arm64) ;;
  x86_64-apple-darwin)    archs=(x86_64) ;;
  *)
    echo "不支持的目标 triple：$target（sidecar 只编 macOS）" >&2
    exit 1
    ;;
esac

cd "$(dirname "$0")/../src-tauri/snapshot-recorder"
args=(-c release)
for arch in "${archs[@]}"; do
  args+=(--arch "$arch")
done
swift build "${args[@]}"

bin="$(swift build "${args[@]}" --show-bin-path)"
names=("$target")
if [ "$target" = universal-apple-darwin ]; then
  names+=(aarch64-apple-darwin x86_64-apple-darwin)
fi

for name in "${names[@]}"; do
  cp "$bin/snapshot-recorder" "../snapshot-recorder-${name}"
  echo "sidecar 就绪：src-tauri/snapshot-recorder-${name}"
done
