#!/bin/sh
# 产出 GitHub Release 附件：
#   Application-Snapshot-{ver}-linux-x86_64.tar.gz
#   Application-Snapshot-{ver}-linux-aarch64.tar.gz
# 在 Linux 上交叉编译两个 GNU 目标；缺链接器或 ELF 架构不对即失败。
set -eu

if [ "$(uname -s)" != "Linux" ]; then
    echo "error: Linux packages must be built on Linux" >&2
    exit 1
fi

cd "$(dirname "$0")"
ROOT_DIR="$(cd .. && pwd)"
VERSION="${RELEASE_VERSION:-$(tr -d ' \n\r' < "$ROOT_DIR/VERSION")}"
VERSION="${VERSION#v}"
PREFIX="Application-Snapshot-${VERSION}"
STAGE="$ROOT_DIR/dist/release"
STAGING="$ROOT_DIR/dist/staging"

mkdir -p "$STAGE" "$STAGING"

if ! command -v cargo >/dev/null 2>&1; then
    echo "error: cargo not found" >&2
    exit 1
fi

if command -v rustup >/dev/null 2>&1; then
    rustup target add x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu
fi

if command -v aarch64-linux-gnu-gcc >/dev/null 2>&1; then
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
fi

elf_machine() {
    # ELF e_machine at offset 18, little-endian u16.
    python3 - "$1" <<'PY'
import struct, sys
path = sys.argv[1]
with open(path, "rb") as fh:
    header = fh.read(20)
if header[:4] != b"\x7fELF":
    raise SystemExit(f"not ELF: {path}")
machine = struct.unpack_from("<H", header, 18)[0]
names = {62: "x86_64", 183: "aarch64"}
print(names.get(machine, f"unknown({machine})"))
PY
}

pack_one() {
    triple="$1"
    expected="$2"
    asset_arch="$3"
    cargo build --release --target "$triple"
    bin="target/${triple}/release/windowsnap"
    if [ ! -f "$bin" ]; then
        echo "error: missing $bin" >&2
        exit 1
    fi
    got="$(elf_machine "$bin")"
    if [ "$got" != "$expected" ]; then
        echo "error: $triple ELF machine is $got, expected $expected" >&2
        exit 1
    fi

    dir_name="${PREFIX}-linux-${asset_arch}"
    dir="$STAGING/$dir_name"
    rm -rf "$dir"
    mkdir -p "$dir"
    cp "$bin" "$dir/windowsnap"
    chmod 755 "$dir/windowsnap"
    cp README.md "$dir/README.md"
    cp windowsnap.service "$dir/windowsnap.service"

    tar -C "$STAGING" -czf "$STAGE/${dir_name}.tar.gz" "$dir_name"
    echo "OK linux-${asset_arch}  $STAGE/${dir_name}.tar.gz"
}

pack_one x86_64-unknown-linux-gnu x86_64 x86_64
pack_one aarch64-unknown-linux-gnu aarch64 aarch64
