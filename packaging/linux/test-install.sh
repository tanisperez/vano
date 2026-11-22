#!/usr/bin/env bash
# Tests site/install.sh end to end against fake releases served from file:// URLs, with a
# temporary HOME. No network, no root, nothing outside a temporary folder.
#
# Usage: packaging/linux/test-install.sh
set -euo pipefail

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
script=$root/site/install.sh
app_id=codes.tanis.vano

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
releases=$tmp/releases
failures=0

fail() {
    echo "FAIL: $*" >&2
    failures=$((failures + 1))
}

check() {
    local what=$1
    shift
    "$@" || fail "$what"
}

# Succeeds when the command fails.
refused() {
    ! "$@" >/dev/null 2>&1
}

# A release tarball with the layout of packaging/linux/package.sh, and a stub binary that prints
# its version. Extra files under share/ go in as given.
make_release() {
    local version=$1
    shift
    local name=vano-$version-linux-x86_64
    local top=$tmp/stage/$name dir=$releases/download/v$version
    rm -rf "$tmp/stage"
    mkdir -p "$top/bin" "$dir"
    printf '#!/bin/sh\necho "Vano %s"\n' "$version" >"$top/bin/vano"
    chmod 755 "$top/bin/vano"
    install -Dm644 "$root/packaging/linux/$app_id.desktop" "$top/share/applications/$app_id.desktop"
    install -Dm644 "$root/packaging/icons/vano.svg" \
        "$top/share/icons/hicolor/scalable/apps/$app_id.svg"
    install -m755 "$script" "$top/install.sh"
    local extra
    for extra in "$@"; do
        install -Dm644 /dev/null "$top/share/$extra"
    done
    tar -C "$tmp/stage" -czf "$dir/$name.tar.gz" "$name"
    echo "unrelated" >"$dir/vano_${version}_amd64.deb"
    (cd "$dir" && sha256sum -- * >SHA256SUMS)
}

publish_latest() {
    rm -rf "$releases/latest"
    mkdir -p "$releases/latest"
    cp -R "$releases/download/v$1" "$releases/latest/download"
}

run() {
    env -i PATH=/usr/bin:/bin HOME="$tmp/home" XDG_CONFIG_HOME="$tmp/home/.config" \
        XDG_STATE_HOME="$tmp/home/.local/state" VANO_RELEASES_URL="file://$releases" \
        VANO_PACKAGED_BINARY="$tmp/usr/bin/vano" bash "$script" "$@"
}

prefix=$tmp/home/.local
manifest=$prefix/share/vano/installed-files
old_icon=icons/hicolor/1024x1024/apps/$app_id.png
mkdir -p "$tmp/home"

make_release 0.9.0 "$old_icon"
make_release 0.9.1
publish_latest 0.9.1

# Help and argument errors.
check "--help exits 0" run --help >/dev/null
check "--help mentions --uninstall" grep -q -- --uninstall <<<"$(run --help)"
check "--purge alone is refused" refused run --purge
check "an unknown option is refused" refused run --nope

# Install the latest release.
check "install latest" run >/dev/null 2>&1
check "binary installed" test -x "$prefix/bin/vano"
check "binary is 0.9.1" grep -q 0.9.1 "$prefix/bin/vano"
check "absolute Exec" grep -qx "Exec=$prefix/bin/vano" \
    "$prefix/share/applications/$app_id.desktop"
check "icon installed" test -f "$prefix/share/icons/hicolor/scalable/apps/$app_id.svg"
check "manifest records 0.9.1" grep -qx "# Vano 0.9.1" "$manifest"
check "manifest lists the binary" grep -qx "$prefix/bin/vano" "$manifest"

# A specific version, then an update that drops a file.
check "install --version 0.9.0" run --version 0.9.0 >/dev/null 2>&1
check "binary is 0.9.0" grep -q 0.9.0 "$prefix/bin/vano"
check "0.9.0 icon installed" test -f "$prefix/share/$old_icon"
out=$(run 2>&1) || fail "update to latest"
check "update message" grep -q "updating Vano 0.9.0 to 0.9.1" <<<"$out"
check "binary is 0.9.1 again" grep -q 0.9.1 "$prefix/bin/vano"
check "file dropped by 0.9.1 removed" test ! -e "$prefix/share/$old_icon"
check "missing version is refused" refused run --version 9.9.9

# A tarball that does not match SHA256SUMS installs nothing.
make_release 0.9.2
echo corrupt >>"$releases/download/v0.9.2/vano-0.9.2-linux-x86_64.tar.gz"
check "bad checksum is refused" refused run --version 0.9.2
check "bad checksum keeps 0.9.1" grep -q 0.9.1 "$prefix/bin/vano"

# The in-app updater: the install.sh of a downloaded tarball, with --tarball and --prefix.
check "install --version 0.9.0 before the local update" run --version 0.9.0 >/dev/null 2>&1
local_dir=$tmp/updates
mkdir -p "$local_dir"
cp "$releases/download/v0.9.1/vano-0.9.1-linux-x86_64.tar.gz" "$local_dir/"
tar -xzf "$local_dir/vano-0.9.1-linux-x86_64.tar.gz" -C "$local_dir"
shipped=$local_dir/vano-0.9.1-linux-x86_64/install.sh
check "the tarball ships install.sh" test -x "$shipped"
out=$(env -i PATH=/usr/bin:/bin HOME="$tmp/home" \
    VANO_PACKAGED_BINARY="$tmp/usr/bin/vano" bash "$shipped" \
    --tarball "$local_dir/vano-0.9.1-linux-x86_64.tar.gz" --prefix "$prefix" 2>&1) ||
    fail "update from a local tarball ($out)"
check "local update message" grep -q "updating Vano 0.9.0 to 0.9.1" <<<"$out"
check "local update installed 0.9.1" grep -q 0.9.1 "$prefix/bin/vano"
check "local update dropped the 0.9.0 file" test ! -e "$prefix/share/$old_icon"
check "local update rewrote the manifest" grep -qx "# Vano 0.9.1" "$manifest"
check "local update keeps the absolute Exec" grep -qx "Exec=$prefix/bin/vano" \
    "$prefix/share/applications/$app_id.desktop"
check "a missing tarball is refused" refused run --tarball "$tmp/nope.tar.gz"
check "--tarball with --version is refused" \
    refused run --tarball "$local_dir/vano-0.9.1-linux-x86_64.tar.gz" --version 0.9.1
echo "not a tarball" >"$tmp/bad.tar.gz"
check "a broken tarball is refused" refused run --tarball "$tmp/bad.tar.gz"
check "a broken tarball keeps 0.9.1" grep -q 0.9.1 "$prefix/bin/vano"

# Uninstall keeps settings and logs, --purge removes them.
mkdir -p "$tmp/home/.config/vano" "$tmp/home/.local/state/vano"
touch "$tmp/home/.config/vano/settings.toml" "$tmp/home/.local/state/vano/vano.log"
touch "$prefix/share/applications/other.desktop"
check "uninstall" run --uninstall >/dev/null
check "binary removed" test ! -e "$prefix/bin/vano"
check "manifest removed" test ! -e "$manifest"
left=$(find "$prefix" -type f -name "*vano*" ! -path "$tmp/home/.local/state/*")
check "no installed file left ($left)" test -z "$left"
check "other files kept" test -f "$prefix/share/applications/other.desktop"
check "settings kept" test -f "$tmp/home/.config/vano/settings.toml"
check "logs kept" test -f "$tmp/home/.local/state/vano/vano.log"
check "uninstall twice is fine" run --uninstall >/dev/null
check "purge" run --uninstall --purge >/dev/null
check "settings purged" test ! -e "$tmp/home/.config/vano"
check "logs purged" test ! -e "$tmp/home/.local/state/vano"

# A copy owned by a package manager blocks the script.
mkdir -p "$tmp/usr/bin"
touch "$tmp/usr/bin/vano"
check "a packaged copy is refused" refused run
rm -rf "${tmp:?}/usr"

# Another prefix.
check "install --prefix" run --prefix "$tmp/opt" >/dev/null 2>&1
check "binary in the prefix" test -x "$tmp/opt/bin/vano"
check "uninstall --prefix" run --uninstall --prefix "$tmp/opt" >/dev/null 2>&1
check "prefix binary removed" test ! -e "$tmp/opt/bin/vano"

if [[ $failures -gt 0 ]]; then
    echo "$failures check(s) failed" >&2
    exit 1
fi
echo "install.sh: all checks passed"
