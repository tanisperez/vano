#!/usr/bin/env bash
# Builds Vano.app, its .dmg and its .app.tar.gz. Run on macOS (needs iconutil, codesign,
# hdiutil). Usage: packaging/macos/bundle.sh <version> <path-to-vano-binary> <out-dir>
set -euo pipefail

if [[ $# -ne 3 ]]; then
    echo "usage: $0 <version> <path-to-vano-binary> <out-dir>" >&2
    exit 2
fi

version=$1
binary=$2
out_dir=$3

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo_root=$(cd "$script_dir/../.." && pwd)
icons_dir=$repo_root/packaging/icons/png

if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-[0-9A-Za-z.-]+)?$ ]]; then
    echo "error: version '$version' is not X.Y.Z or X.Y.Z-prerelease" >&2
    exit 2
fi
# CFBundleShortVersionString and CFBundleVersion only accept up to three numeric components.
short_version="${BASH_REMATCH[1]}.${BASH_REMATCH[2]}.${BASH_REMATCH[3]}"
build_version=$short_version
[[ -x "$binary" ]] || { echo "error: $binary is not an executable file" >&2; exit 2; }

mkdir -p "$out_dir"
out_dir=$(cd "$out_dir" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

app=$work/Vano.app
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$binary" "$app/Contents/MacOS/vano"
chmod 755 "$app/Contents/MacOS/vano"

sed -e "s/@SHORT_VERSION@/$short_version/g" -e "s/@BUILD_VERSION@/$build_version/g" \
    "$script_dir/Info.plist" > "$app/Contents/Info.plist"

# Iconset: <size>x<size>.png is @1x, the next size up is its @2x.
iconset=$work/vano.iconset
mkdir -p "$iconset"
cp "$icons_dir/vano-16.png" "$iconset/icon_16x16.png"
cp "$icons_dir/vano-32.png" "$iconset/icon_16x16@2x.png"
cp "$icons_dir/vano-32.png" "$iconset/icon_32x32.png"
cp "$icons_dir/vano-64.png" "$iconset/icon_32x32@2x.png"
cp "$icons_dir/vano-128.png" "$iconset/icon_128x128.png"
cp "$icons_dir/vano-256.png" "$iconset/icon_128x128@2x.png"
cp "$icons_dir/vano-256.png" "$iconset/icon_256x256.png"
cp "$icons_dir/vano-512.png" "$iconset/icon_256x256@2x.png"
cp "$icons_dir/vano-512.png" "$iconset/icon_512x512.png"
cp "$icons_dir/vano-1024.png" "$iconset/icon_512x512@2x.png"
iconutil --convert icns --output "$app/Contents/Resources/vano.icns" "$iconset"

# Ad-hoc signature, so Apple Silicon accepts the binary. Not a Developer ID signature.
codesign --force --deep --sign - "$app"

tarball=$out_dir/Vano-$version-macos-arm64.app.tar.gz
tar -C "$work" -czf "$tarball" Vano.app

dmg_root=$work/dmg
mkdir -p "$dmg_root"
cp -R "$app" "$dmg_root/Vano.app"
ln -s /Applications "$dmg_root/Applications"
dmg=$out_dir/Vano-$version-macos-arm64.dmg
rm -f "$dmg"
hdiutil create -volname "Vano" -srcfolder "$dmg_root" -ov -format UDZO "$dmg"

echo "Created $dmg"
echo "Created $tarball"
