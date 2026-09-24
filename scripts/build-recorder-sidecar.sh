#!/bin/bash
# 构建 snapshot-recorder（ScreenCaptureKit 窗口录制桥）并复制成 Tauri sidecar 命名：
#   src-tauri/snapshot-recorder-<target-triple>
# 用法：build-recorder-sidecar.sh [triple]
#   不带参数                → 按 rustc 的 host triple（本地开发路径）
#   universal-apple-darwin  → arm64 + x86_64 合成一个通用二进制
#   aarch64/x86_64-apple-darwin → 按对应 Swift 架构单独编
# 仅 macOS 有意义；其它平台的调用方应直接跳过本脚本。
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
