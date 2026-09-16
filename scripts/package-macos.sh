#!/bin/bash
# 产出 GitHub Release 附件：Application-Snapshot-{ver}-macos-universal.zip
# 必须在 macOS 上运行。缺 arm64 或 x86_64 切片即失败。
set -euo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
    echo "error: macOS package must be built on macOS" >&2
    exit 1
fi

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT_DIR"

VERSION="${RELEASE_VERSION:-$(tr -d ' \n\r' < VERSION)}"
VERSION="${VERSION#v}"
PREFIX="Application-Snapshot-${VERSION}"
ASSET="${PREFIX}-macos-universal.zip"
APP_NAME="应用快照"
APP_DIR="$ROOT_DIR/dist/${APP_NAME}.app"
STAGE="$ROOT_DIR/dist/release"
BINARY="$APP_DIR/Contents/MacOS/WindowSnap"

export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"
export WINDOWSNAP_PACKAGE_ONLY=1
zsh "$ROOT_DIR/scripts/build-app.sh"

if [ ! -f "$BINARY" ]; then
    echo "error: missing $BINARY" >&2
    exit 1
fi

INFO="$(lipo -info "$BINARY")"
echo "$INFO"
echo "$INFO" | grep -q "arm64" || { echo "error: missing Apple Silicon (arm64) slice" >&2; exit 1; }
echo "$INFO" | grep -q "x86_64" || { echo "error: missing Intel (x86_64) slice" >&2; exit 1; }

mkdir -p "$STAGE"
rm -f "$STAGE/$ASSET"
ditto -c -k --keepParent "$APP_DIR" "$STAGE/$ASSET"

# 解压复查：zip 里仍是双切片，且顶层就是 .app
VERIFY="$(mktemp -d)"
trap 'rm -rf "$VERIFY"' EXIT
ditto -x -k "$STAGE/$ASSET" "$VERIFY"
UNPACKED="$VERIFY/${APP_NAME}.app/Contents/MacOS/WindowSnap"
if [ ! -f "$UNPACKED" ]; then
    echo "error: zip did not contain ${APP_NAME}.app" >&2
    exit 1
fi
UNPACKED_INFO="$(lipo -info "$UNPACKED")"
echo "$UNPACKED_INFO" | grep -q "arm64" || { echo "error: zip arm64 slice missing" >&2; exit 1; }
echo "$UNPACKED_INFO" | grep -q "x86_64" || { echo "error: zip x86_64 slice missing" >&2; exit 1; }

echo "OK macos-universal  $STAGE/$ASSET"
