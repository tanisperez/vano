#!/usr/bin/env bash
# Install Vano for Linux from its GitHub releases, for the current user by default:
#
#   curl -fsSL https://vano.tanis.codes/install.sh | bash
#
# It downloads the release tarball, checks it against the release's SHA256SUMS and lays it out
# under a prefix (~/.local by default): the binary, the desktop entry with an absolute Exec and
# the icons. A manifest of the installed files makes updates and
# --uninstall exact. Run it again to update. Options: see usage below, or pass --help.
#
# The release tarball ships this script too, in its top folder: Vano's in-app updater runs
# it with --tarball on the tarball it downloaded and verified, and --prefix of the running copy.
#
# VANO_RELEASES_URL replaces https://github.com/tanisperez/vano/releases, and
# VANO_PACKAGED_BINARY replaces /usr/bin/vano (the copy of a package manager), for tests.

# Everything runs from main, called on the last line, so a download cut short by the network
# never runs half a script.

set -euo pipefail

name=vano
app_id=codes.tanis.vano
site=https://vano.tanis.codes

usage() {
    cat <<EOF
Install Vano for Linux from its GitHub releases.

Usage:
  curl -fsSL $site/install.sh | bash
  curl -fsSL $site/install.sh | bash -s -- [options]

Options:
  --version X.Y.Z  Install that release instead of the latest one
  --prefix DIR     Install under DIR instead of ~/.local (for example /usr/local, with sudo)
  --tarball FILE   Install from a release tarball already downloaded, without checking it
                   (Vano's updater uses it, after verifying the file)
  --uninstall      Remove Vano. Your settings and logs stay
  --purge          With --uninstall, also remove your settings and logs
  -h, --help       Show this help

Running it again updates Vano to the latest release. Your settings and saved scans stay.
Documentation: $site/docs/installation.html#install-script
EOF
}

die() {
    echo "error: $*" >&2
    exit 1
}

say() {
    echo "vano: $*"
}

# Refresh the menu and icon caches of the prefix. Desktops that watch these folders pick
# the changes up anyway, so a missing tool is fine.
refresh_caches() {
    local prefix=$1
    if command -v update-desktop-database >/dev/null && [[ -d $prefix/share/applications ]]; then
        update-desktop-database -q "$prefix/share/applications" || true
    fi
    local icons=$prefix/share/icons/hicolor
    if [[ -f $icons/icon-theme.cache ]] && command -v gtk-update-icon-cache >/dev/null; then
        gtk-update-icon-cache -q -t "$icons" || true
    fi
}

# The version recorded in a manifest, empty when there is none.
installed_version() {
    local manifest=$1
    [[ -f $manifest ]] || return 0
    sed -n '1s/^# Vano //p' "$manifest"
}

uninstall() {
    local prefix=$1 purge=$2 manifest=$3
    if [[ -f $manifest ]]; then
        local version file
        version=$(installed_version "$manifest")
        while IFS= read -r file; do
            [[ -n $file && $file != \#* ]] || continue
            rm -f "$file"
        done <"$manifest"
        rm -f "$manifest"
        rmdir "$(dirname "$manifest")" 2>/dev/null || true
        refresh_caches "$prefix"
        say "removed Vano $version from $prefix"
    else
        say "Vano is not installed in $prefix by this script, nothing to remove"
    fi

    local config=${XDG_CONFIG_HOME:-$HOME/.config}/$name
    local state=${XDG_STATE_HOME:-$HOME/.local/state}/$name
    if [[ $purge == 1 ]]; then
        rm -rf "$config" "$state"
        say "removed your settings ($config) and logs ($state)"
    else
        say "your settings ($config) and logs ($state) are kept, --purge removes them"
    fi
}

# Vano installed by a package manager (deb, rpm, AUR) lives in /usr/bin. A second copy under
# another prefix would never be updated by it, and which one runs would depend on PATH.
check_no_package() {
    local manifest=$1
    local packaged=${VANO_PACKAGED_BINARY:-/usr/bin/$name}
    [[ -e $packaged ]] || return 0
    if [[ -f $manifest ]] && grep -Fxq "$packaged" "$manifest"; then
        return 0
    fi
    die "Vano is already installed in $packaged, most likely by your package manager.
It keeps that copy up to date. To use this script instead, remove it first
(sudo apt remove vano, sudo dnf remove vano or sudo pacman -R vano)."
}

# Says what is about to happen: an install, a reinstall or an update from the version in the
# manifest.
announce() {
    local prefix=$1 version=$2 manifest=$3
    local previous
    previous=$(installed_version "$manifest")
    if [[ -z $previous ]]; then
        say "installing Vano $version in $prefix"
    elif [[ $previous == "$version" ]]; then
        say "reinstalling Vano $version in $prefix"
    else
        say "updating Vano $previous to $version in $prefix"
    fi
}

install_release() {
    local prefix=$1 version=$2 manifest=$3 releases=$4 local_tarball=$5
    local arch
    arch=$(uname -m)
    [[ $arch == x86_64 ]] || die "there are Linux builds for x86_64 only, not for $arch"
    local tool tools=(tar)
    [[ -n $local_tarball ]] || tools+=(curl sha256sum)
    for tool in "${tools[@]}"; do
        command -v "$tool" >/dev/null || die "$tool not found, install it and try again"
    done
    check_no_package "$manifest"

    local work
    work=$(mktemp -d)
    # shellcheck disable=SC2064 # expand $work now, it is local to this function.
    trap "rm -rf '$work'" EXIT

    local src
    if [[ -n $local_tarball ]]; then
        # A tarball already downloaded and verified, by Vano's updater: its top folder names
        # the version.
        [[ -f $local_tarball ]] || die "$local_tarball not found"
        tar -xzf "$local_tarball" -C "$work" || die "could not unpack $local_tarball"
        local found=("$work/$name"-*-linux-"$arch")
        src=${found[0]}
        [[ -x $src/bin/$name ]] || die "$local_tarball has an unexpected layout"
        version=${src##*/"$name"-}
        version=${version%"-linux-$arch"}
        announce "$prefix" "$version" "$manifest"
    else
        # The latest release: GitHub's releases/latest is never a pre-release or a draft. Its
        # SHA256SUMS names the tarball, and so the version, without calling the rate limited API.
        local base
        if [[ -n $version ]]; then
            base=$releases/download/v$version
        else
            base=$releases/latest/download
        fi
        curl -fsSL -o "$work/SHA256SUMS" "$base/SHA256SUMS" ||
            if [[ -n $version ]]; then
                die "no release $version found ($releases)"
            else
                die "no published release found ($releases)"
            fi
        local tarball
        tarball=$(awk -v suffix="-linux-$arch.tar.gz" \
            'substr($2, length($2) - length(suffix) + 1) == suffix && $2 ~ /^vano-/ { print $2 }' \
            "$work/SHA256SUMS" | head -n 1)
        [[ -n $tarball ]] || die "the release has no Linux tarball for $arch"
        if [[ -z $version ]]; then
            version=${tarball#"$name-"}
            version=${version%"-linux-$arch.tar.gz"}
            base=$releases/download/v$version
        fi
        announce "$prefix" "$version" "$manifest"

        curl -fL --progress-bar -o "$work/$tarball" "$base/$tarball" ||
            die "could not download $tarball"
        (cd "$work" && awk -v file="$tarball" '$2 == file' SHA256SUMS |
            sha256sum --check --quiet --strict) ||
            die "$tarball does not match its checksum, nothing was installed"

        tar -xzf "$work/$tarball" -C "$work"
        src=$work/$name-$version-linux-$arch
        [[ -x $src/bin/$name ]] || die "$tarball has an unexpected layout"
    fi

    mkdir -p "$prefix" 2>/dev/null || true
    [[ -w $prefix ]] || die "cannot write to $prefix, run it with sudo for a system prefix"

    # Every file of the tarball's share/ goes under the prefix, the desktop entry with the
    # absolute path of the binary, so the menu entry works even when prefix/bin is not on the
    # session's PATH.
    local files=()
    install -Dm755 "$src/bin/$name" "$prefix/bin/$name"
    files+=("$prefix/bin/$name")
    local rel
    while IFS= read -r rel; do
        if [[ $rel == applications/$app_id.desktop ]]; then
            sed -E "s#^Exec=$name( |\$)#Exec=$prefix/bin/$name\1#" "$src/share/$rel" |
                install -Dm644 /dev/stdin "$prefix/share/$rel"
        else
            install -Dm644 "$src/share/$rel" "$prefix/share/$rel"
        fi
        files+=("$prefix/share/$rel")
    done < <(cd "$src/share" && find . -type f | sed 's|^\./||' | LC_ALL=C sort)

    # Files of the previous version that this one no longer has.
    local new_manifest=$work/manifest
    {
        echo "# Vano $version"
        printf '%s\n' "${files[@]}"
    } >"$new_manifest"
    if [[ -f $manifest ]]; then
        local file
        while IFS= read -r file; do
            [[ -n $file && $file != \#* ]] || continue
            grep -Fxq "$file" "$new_manifest" || rm -f "$file"
        done <"$manifest"
    fi
    install -Dm644 "$new_manifest" "$manifest"
    refresh_caches "$prefix"

    say "installed Vano $version"
    case ":$PATH:" in
    *":$prefix/bin:"*) say "start it from your applications menu or with: $name" ;;
    *)
        say "start it from your applications menu or with: $prefix/bin/$name"
        say "$prefix/bin is not on your PATH, add it to run it as: $name"
        ;;
    esac
}

main() {
    local prefix='' version='' tarball='' uninstall=0 purge=0
    while [[ $# -gt 0 ]]; do
        case $1 in
        -h | --help)
            usage
            return 0
            ;;
        --version)
            [[ $# -ge 2 && -n $2 ]] || die "--version needs a version, for example --version 1.0.0"
            version=${2#v}
            shift
            ;;
        --version=*)
            version=${1#--version=}
            version=${version#v}
            ;;
        --prefix)
            [[ $# -ge 2 && -n $2 ]] || die "--prefix needs a folder"
            prefix=$2
            shift
            ;;
        --prefix=*) prefix=${1#--prefix=} ;;
        --tarball)
            [[ $# -ge 2 && -n $2 ]] || die "--tarball needs a file"
            tarball=$2
            shift
            ;;
        --tarball=*) tarball=${1#--tarball=} ;;
        --uninstall) uninstall=1 ;;
        --purge) purge=1 ;;
        *) die "unknown option '$1', see --help" ;;
        esac
        shift
    done

    [[ $(uname -s) == Linux ]] || die "this script is for Linux, see $site/install.html"
    [[ $purge == 0 || $uninstall == 1 ]] || die "--purge only goes with --uninstall"
    [[ $uninstall == 0 || -z $version ]] || die "--version does not go with --uninstall"
    [[ $uninstall == 0 || -z $tarball ]] || die "--tarball does not go with --uninstall"
    [[ -z $version || -z $tarball ]] || die "--version does not go with --tarball"
    [[ -z $tarball || $tarball == /* ]] || tarball=$PWD/$tarball
    if [[ -z $prefix ]]; then
        # curl ... | sudo bash would install into root's home, where nobody finds it.
        [[ $EUID -ne 0 ]] ||
            die "run it as your user, or pass --prefix (for example --prefix /usr/local) as root"
        [[ -n ${HOME:-} ]] || die "HOME is not set, pass --prefix"
        prefix=$HOME/.local
    fi
    [[ $prefix == /* ]] || prefix=$PWD/$prefix
    prefix=${prefix%/}

    local manifest=$prefix/share/$name/installed-files
    local releases=${VANO_RELEASES_URL:-https://github.com/tanisperez/vano/releases}
    if [[ $uninstall == 1 ]]; then
        uninstall "$prefix" "$purge" "$manifest"
    else
        install_release "$prefix" "$version" "$manifest" "$releases" "$tarball"
    fi
}

main "$@"
