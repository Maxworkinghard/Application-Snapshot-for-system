#!/usr/bin/env bash
# 给 PipeWire 早于 1.0 的系统（Ubuntu 22.04 是 0.3.48）补一份 1.0.5 的头文件，只供编译用。
#
# 为什么要：xcap（窗口截图、Wayland 录制）依赖 pipewire-rs 0.10，它用 bindgen 按系统头文件生成绑定，
# 代码里用到的字段和函数（spa_video_info_raw.flags、spa_meta_first 等）0.3.48 的头文件里没有，编译直接失败。
#
# 为什么这样补是安全的：
# - SPA 全是头文件里的内联代码，会直接编进程序，运行时不依赖系统里的 SPA 版本。
# - libpipewire 仍然链接系统自带的那一份。程序若用到了系统库里没有的函数，链接这一步就会报错，
#   不会编出一个在用户机器上起不来的包。
#
# 头文件取自 Ubuntu 24.04 发行时的 libspa-0.2-dev / libpipewire-0.3-dev 1.0.5-1，版本和 sha256 都写死。
# 两个架构共用 amd64 的包：里面只取头文件，和架构无关。
#
# 用法：scripts/linux/pipewire-headers.sh
# 装好后编译前要 export PKG_CONFIG_PATH=/opt/snapshot-build/pipewire-1.0.5/pkgconfig（脚本最后会打印）；
# 在 GitHub Actions 里会自动写进 $GITHUB_ENV，后面的步骤直接生效。
set -euo pipefail

prefix=/opt/snapshot-build/pipewire-1.0.5
pool=http://archive.ubuntu.com/ubuntu/pool/main/p/pipewire
debs=(
  "libspa-0.2-dev_1.0.5-1_amd64.deb 6c99d162e23e13cacf8bc3edc312c60545281078398e52e34dc5ebb4209649dc"
  "libpipewire-0.3-dev_1.0.5-1_amd64.deb c0b34ed6a34391a94c0711966dbea9b6b5a98758d88813824c05d6662e0b60c9"
)

# 链接用的仍是系统库，所以先按系统自己的 .pc 找到它在哪；这一步不能被下面写的 .pc 影响
libdir="$(env -u PKG_CONFIG_PATH pkg-config --variable=libdir libpipewire-0.3)" || {
  echo "error: 系统里没有 libpipewire-0.3 的开发包，先跑 scripts/linux/install-deps.sh" >&2
  exit 1
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
for entry in "${debs[@]}"; do
  read -r name sha <<<"$entry"
  curl -fsSL --retry 3 -o "$work/$name" "$pool/$name"
  echo "$sha  $work/$name" | sha256sum -c --quiet
  dpkg-deb -x "$work/$name" "$work/root"
done

sudo rm -rf "$prefix"
sudo mkdir -p "$prefix/include" "$prefix/pkgconfig"
sudo cp -r "$work/root/usr/include/spa-0.2" "$work/root/usr/include/pipewire-0.3" "$prefix/include/"

sudo tee "$prefix/pkgconfig/libspa-0.2.pc" >/dev/null <<EOF
prefix=$prefix
includedir=\${prefix}/include

Name: libspa
Description: Simple Plugin API (PipeWire 1.0.5 headers, build only)
Version: 0.2
Cflags: -I\${includedir}/spa-0.2 -D_REENTRANT
EOF

sudo tee "$prefix/pkgconfig/libpipewire-0.3.pc" >/dev/null <<EOF
prefix=$prefix
includedir=\${prefix}/include
libdir=$libdir

Name: libpipewire
Description: PipeWire 1.0.5 headers, linked against the system library
Version: 1.0.5
Requires: libspa-0.2
Libs: -L\${libdir} -lpipewire-0.3
Cflags: -I\${includedir}/pipewire-0.3 -D_REENTRANT
EOF

echo "==> PipeWire 1.0.5 头文件已就绪：$prefix（链接仍用系统库 $libdir）"
if [[ -n "${GITHUB_ENV:-}" ]]; then
  echo "PKG_CONFIG_PATH=$prefix/pkgconfig" >>"$GITHUB_ENV"
else
  echo "编译前先执行：export PKG_CONFIG_PATH=$prefix/pkgconfig"
fi
