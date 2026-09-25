#!/usr/bin/env bash
# 装包冒烟：装上 deb（或直接运行 AppImage），在无头 X（Xvfb）里启动应用，活过一段时间就算通过。
# 缺依赖、glibc 不够新、一启动就崩，都会在这里暴露。发版前在 Ubuntu 22.04 和 24.04 上各跑一遍，
# 确认同一份包在这两代系统上都装得上、起得来。会往系统里装包，只在 CI 或一次性的机器上用。
#
# 用法：scripts/linux/smoke-test.sh deb <包路径>
#       scripts/linux/smoke-test.sh appimage <包路径>
# 环境变量 SMOKE_SECONDS：要活过的秒数，默认 15。
set -euo pipefail

kind="${1:?用法：smoke-test.sh deb|appimage <包路径>}"
package="$(realpath "${2:?缺少包路径}")"
seconds="${SMOKE_SECONDS:-15}"

export DEBIAN_FRONTEND=noninteractive
sudo apt-get update -qq
# Xvfb 提供无头 X；dbus 提供会话总线（单实例、托盘都要用）
sudo apt-get install -y -qq xvfb xauth dbus >/dev/null

case "$kind" in
  deb)
    # 绝对路径才会被 apt 当成本地包；依赖按包里声明的从源里装，装不上说明依赖写错了
    sudo apt-get install -y -qq "$package" >/dev/null
    app=/usr/bin/snapshot
    ;;
  appimage)
    chmod +x "$package"
    app="$package"
    # CI 机器上不一定有 FUSE，解开再跑
    export APPIMAGE_EXTRACT_AND_RUN=1
    ;;
  *)
    echo "error: 不认识的包类型：$kind（只认 deb / appimage）" >&2
    exit 2
    ;;
esac

log="$(mktemp)"
# 放进单独的进程组，结束时连同 Xvfb、会话总线一起收掉
setsid dbus-run-session -- xvfb-run -a -s "-screen 0 1280x800x24" "$app" >"$log" 2>&1 &
group=$!
trap 'kill -TERM -- "-$group" 2>/dev/null || true' EXIT

sleep "$seconds"
if ! kill -0 "$group" 2>/dev/null; then
  echo "error: $kind 启动后不到 ${seconds} 秒就退出了" >&2
  cat "$log" >&2
  exit 1
fi
if grep -q "panicked" "$log"; then
  echo "error: $kind 启动时 panic 了" >&2
  cat "$log" >&2
  exit 1
fi
echo "OK：$kind 启动后活过了 ${seconds} 秒"
echo "--- 应用输出（最后 30 行）"
tail -n 30 "$log"
