#!/usr/bin/env bash
# Install the latest published release of Vano for Linux into a prefix, for the current user by
# default: the tarball from GitHub, checked against the release's SHA256SUMS, laid out like
# `make install` does (the binary, the desktop entry with an absolute Exec, the icons).
# Pre-releases and drafts are never picked. Used by `make install-latest`.
#
# Usage: packaging/linux/install-latest.sh [prefix]   (default: ~/.local)
set -euo pipefail

repo=tanisperez/vano
name=vano
app_id=codes.tanis.vano
prefix=${1:-$HOME/.local}

die() {
    echo "error: $*" >&2
    exit 1
}

arch=$(uname -m)
[[ $arch == x86_64 ]] || die "there are Linux builds for x86_64 only, not for $arch"
for tool in curl tar sha256sum; do
    command -v "$tool" >/dev/null || die "$tool not found"
done

# The latest release: GitHub's "latest" is never a pre-release or a draft.
releases=https://github.com/$repo/releases
json=$(curl -fsSL "https://api.github.com/repos/$repo/releases/latest") ||
    die "no published release found ($releases)"
tag=$(sed -n 's/^ *"tag_name": *"\([^"]*\)".*/\1/p' <<<"$json")
[[ -n $tag ]] || die "could not read the latest release ($releases)"
version=${tag#v}
tarball=$name-$version-linux-$arch.tar.gz

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
echo "Downloading $tarball"
curl -fL --progress-bar -o "$work/$tarball" "$releases/download/$tag/$tarball" ||
    die "could not download $tarball"
curl -fsSL -o "$work/SHA256SUMS" "$releases/download/$tag/SHA256SUMS" ||
    die "could not download the SHA256SUMS of $tag"
sum=$(awk -v file="$tarball" '$2 == file' "$work/SHA256SUMS")
[[ -n $sum ]] || die "$tarball is not listed in SHA256SUMS"
(cd "$work" && sha256sum --check --quiet <<<"$sum") || die "$tarball does not match its checksum"

tar -xzf "$work/$tarball" -C "$work"
src=$work/$name-$version-linux-$arch
install -Dm755 "$src/bin/$name" "$prefix/bin/$name"
sed "s|^Exec=$name\$|Exec=$prefix/bin/$name|" "$src/share/applications/$app_id.desktop" |
    install -Dm644 /dev/stdin "$prefix/share/applications/$app_id.desktop"
mkdir -p "$prefix/share/icons"
cp -R "$src/share/icons/." "$prefix/share/icons/"
echo "Installed $name $version in $prefix"
