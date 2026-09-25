#!/usr/bin/env bash
# 确认二进制要求的 glibc 不超过支持线（默认 2.35，即 Ubuntu 22.04）。
# glibc 只向后兼容：打包机一旦换成更新的系统，产物在 22.04 上会报「GLIBC_2.3x not found」起不来。
# 这里在打包时就拦下，而不是等用户打开才发现。
#
# 用法：scripts/linux/check-glibc.sh <二进制> [最高 glibc 版本，默认 2.35]
set -euo pipefail

binary="${1:?用法：check-glibc.sh <二进制> [最高 glibc 版本]}"
limit="${2:-2.35}"

need="$(objdump -T "$binary" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -uV | tail -n 1)"
echo "$binary 需要 glibc $need，支持线是 $limit"
if [[ "$(printf '%s\n%s\n' "$need" "$limit" | sort -V | tail -n 1)" != "$limit" ]]; then
  echo "error: 超过了支持线，Ubuntu 22.04 上会起不来。打包机要用支持线里最老的系统。" >&2
  exit 1
fi
