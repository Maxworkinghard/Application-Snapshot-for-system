#!/bin/sh
# 构建 Linux 版应用快照（本机架构）。
# 交叉编译见 README「多架构」一节。
set -eu

cd "$(dirname "$0")"
cargo build --release

BIN="target/release/windowsnap"
echo ""
echo "==> 构建成功：$BIN ($(uname -m))"
echo "安装：install -Dm755 $BIN ~/.local/bin/windowsnap"
