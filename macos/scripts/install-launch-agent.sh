#!/bin/zsh

set -euo pipefail

PLIST_SRC="${0:A:h}/local.windowsnap.app.plist"
PLIST_DST="$HOME/Library/LaunchAgents/local.windowsnap.app.plist"
APP_PATH="/Applications/应用快照.app"

if [ ! -d "$APP_PATH" ]; then
    echo "错误：未找到 $APP_PATH。请先运行 macos/scripts/build-app.sh 并安装到 /Applications。" >&2
    exit 1
fi

mkdir -p "$(dirname "$PLIST_DST")"
cp "$PLIST_SRC" "$PLIST_DST"

# 若已加载则先卸载
launchctl unload "$PLIST_DST" 2>/dev/null || true
launchctl load "$PLIST_DST"

echo "已安装并加载 LaunchAgent：local.windowsnap.app"
echo "下次开机/登录后会自动启动应用快照。"
