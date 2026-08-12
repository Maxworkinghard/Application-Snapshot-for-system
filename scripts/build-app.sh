#!/bin/zsh

set -euo pipefail

ROOT_DIR="${0:A:h:h}"
APP_NAME="应用快照"
APP_DIR="$ROOT_DIR/dist/$APP_NAME.app"
INSTALL_DIR="/Applications/$APP_NAME.app"
LAUNCH_AGENT_LABEL="local.windowsnap.app"
LAUNCH_AGENT_PLIST="$HOME/Library/LaunchAgents/$LAUNCH_AGENT_LABEL.plist"

cd "$ROOT_DIR"
# 同时编译 Apple Silicon (arm64) 与 Intel (x86_64)，产出 universal 2 二进制
swift build -c release --arch arm64 --arch x86_64
BINARY="$ROOT_DIR/.build/apple/Products/Release/WindowSnap"

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/Contents/MacOS" "$APP_DIR/Contents/Resources"
cp "$BINARY" "$APP_DIR/Contents/MacOS/WindowSnap"
cp "$ROOT_DIR/Resources/Info.plist" "$APP_DIR/Contents/Info.plist"
lipo -info "$APP_DIR/Contents/MacOS/WindowSnap"

# 有 WindowSnapDev 证书就用（按哈希解析，避免 keychain 中同名证书歧义）；
# 没有则退回 ad-hoc 签名（每次重建都需重新授予屏幕录制权限）
SIGN_IDENTITY=$(security find-identity -v -p codesigning 2>/dev/null | awk '/WindowSnapDev/{print $2; exit}')
if [ -z "$SIGN_IDENTITY" ]; then
    SIGN_IDENTITY="-"
    echo "未找到 WindowSnapDev 证书，使用 ad-hoc 签名"
fi
codesign --force --deep --sign "$SIGN_IDENTITY" "$APP_DIR"

# 同步安装到 /Applications（若已安装则覆盖）
if [ -d "$INSTALL_DIR" ]; then
    rm -rf "$INSTALL_DIR"
    cp -R "$APP_DIR" "$INSTALL_DIR"
    echo "已更新 /Applications/$APP_NAME.app"
fi

# 结束正在运行的旧进程。LaunchAgent 经 /usr/bin/open 启动应用，launchd 只跟踪瞬时的
# open 进程，unload/load 不会重启应用本体，必须显式 kill。
WAS_RUNNING=0
if pgrep -f "$APP_NAME.app/Contents/MacOS/WindowSnap" >/dev/null 2>&1; then
    WAS_RUNNING=1
    pkill -f "$APP_NAME.app/Contents/MacOS/WindowSnap" || true
    for _ in {1..30}; do
        pgrep -f "$APP_NAME.app/Contents/MacOS/WindowSnap" >/dev/null 2>&1 || break
        sleep 0.1
    done
fi

# 重载 LaunchAgent（RunAtLoad 会重新拉起应用）；无 LaunchAgent 时按需手动拉起
if [ -f "$LAUNCH_AGENT_PLIST" ]; then
    launchctl unload "$LAUNCH_AGENT_PLIST" 2>/dev/null || true
    launchctl load "$LAUNCH_AGENT_PLIST"
    echo "已重启 LaunchAgent ($LAUNCH_AGENT_LABEL)"
elif [ "$WAS_RUNNING" = 1 ] && [ -d "$INSTALL_DIR" ]; then
    open -a "$INSTALL_DIR"
    echo "已重启应用"
fi

echo "$APP_DIR"

