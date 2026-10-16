#!/usr/bin/env bash
# Build the Linux release assets: a tarball, a .deb and a .rpm.
#
# Usage: packaging/linux/package.sh <version> <path-to-vano-binary> <out-dir> [arch]
#
# <version> is the bare version (0.1.0 or 0.1.0-rc.1, no leading "v").
# [arch] is x86_64 (default) or aarch64.
# Requires nfpm on PATH (https://nfpm.goreleaser.com).
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
    echo "usage: $0 <version> <vano-binary> <out-dir> [x86_64|aarch64]" >&2
    exit 2
fi

version=$1
binary=$2
out_dir=$3
arch=${4:-x86_64}

case "$arch" in
x86_64) deb_arch=amd64 ;;
aarch64) deb_arch=arm64 ;;
*)
    echo "error: unsupported arch '$arch' (use x86_64 or aarch64)" >&2
    exit 2
    ;;
esac

if ! command -v nfpm >/dev/null 2>&1; then
    echo "error: nfpm not found on PATH, install it from https://nfpm.goreleaser.com" >&2
    exit 1
fi
if [[ ! -f $binary ]]; then
    echo "error: binary not found: $binary" >&2
    exit 1
fi

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
linux_dir=$root/packaging/linux
icons_dir=$root/packaging/icons
mkdir -p "$out_dir"
out_dir=$(cd "$out_dir" && pwd)
binary=$(cd "$(dirname "$binary")" && pwd)/$(basename "$binary")

app_id=codes.tanis.vano
sizes=(16 24 32 48 64 128 256 512)

# 1. Tarball with a /usr-like prefix layout.
name=vano-$version-linux-$arch
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
top=$stage/$name
install -Dm755 "$binary" "$top/bin/vano"
install -Dm644 "$linux_dir/$app_id.desktop" "$top/share/applications/$app_id.desktop"
install -Dm644 "$icons_dir/vano.svg" \
    "$top/share/icons/hicolor/scalable/apps/$app_id.svg"
for size in "${sizes[@]}"; do
    install -Dm644 "$icons_dir/png/vano-$size.png" \
        "$top/share/icons/hicolor/${size}x${size}/apps/$app_id.png"
done
install -m644 "$root/LICENSE" "$top/LICENSE"
install -m644 "$root/NOTICE" "$top/NOTICE"
[[ -f $root/README.md ]] && install -m644 "$root/README.md" "$top/README.md"
tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='UTC 2020-01-01' \
    -C "$stage" -czf "$out_dir/$name.tar.gz" "$name"

# 2. deb and rpm with nfpm.
export VANO_VERSION=$version
export VANO_ARCH=$deb_arch
export VANO_BIN=$binary
export VANO_ROOT=$root
# The file names use <version> as given: nfpm's default names carry the "~" of a pre-release
# (0.1.0~rc.1), which GitHub turns into "." on upload, breaking SHA256SUMS. The version inside the
# packages keeps the "~", so a pre-release still sorts before the final release.
nfpm pkg --config "$linux_dir/nfpm.yaml" --packager deb \
    --target "$out_dir/vano_${version}_$deb_arch.deb"
nfpm pkg --config "$linux_dir/nfpm.yaml" --packager rpm \
    --target "$out_dir/vano-$version-1.$arch.rpm"

echo "Built in $out_dir:"
ls -1 "$out_dir"
