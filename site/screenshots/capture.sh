#!/bin/bash
# Regenerates every website screenshot into site/img/ (WebP, light and dark).
# See site/screenshots/README.md for the requirements.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$ROOT/target/release/vano"
OUT="$ROOT/site/img"
SCALE=1.5
# The window is resized to this, so every click lands on the same pixel. The screenshot is the
# same size, with the transparent margin around the window's shadow.
WIDTH=1680
HEIGHT=1000
# Under target/, on the real disk: the app reads the free and used space of the scanned folder's
# file system, and a tmpfs would show nonsense next to the made-up home.
WORK="$ROOT/target/screenshots"
# The made-up home, mounted as /home/alex for the app (bubblewrap), so every path it shows
# (the home card, Settings) reads /home/alex and the scan it opens is the one written by the
# demo_home example, never a real folder. Its config, cache and state live inside it.
HOME_DIR="$WORK/home"
DEMO_HOME=/home/alex

cleanup() {
    pkill -x vano 2>/dev/null
}
trap cleanup EXIT INT TERM

die() { echo "capture.sh: $*" >&2; exit 1; }

for tool in spectacle xdotool magick python3 cargo bwrap; do
    command -v "$tool" >/dev/null || die "missing required tool: $tool"
done
if [ "$(busctl --user call org.freedesktop.ScreenSaver /ScreenSaver org.freedesktop.ScreenSaver GetActive 2>/dev/null)" = "b true" ]; then
    die "the screen is locked, unlock it first"
fi
pgrep -x vano >/dev/null && die "a vano process is already running, close it first"

# X11 pointer requests are scaled by the display scale here (KDE Plasma on Wayland), while the
# window geometry and the screenshot are in physical pixels.
POINTER_SCALE=$(kscreen-doctor -o 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | awk '/Scale:/ {print $2; exit}')
POINTER_SCALE=${POINTER_SCALE:-1}

(cd "$ROOT" && cargo build --release -p vano-app) || die "cargo build failed"

rm -rf "${WORK:?}"
mkdir -p "$HOME_DIR/.local/state" "$OUT"
(cd "$ROOT" && cargo run --release -q -p vano-rules --example demo_home -- \
    "$DEMO_HOME" "$HOME_DIR/.config" "$HOME_DIR/.cache") || die "the demo home could not be written"

WID=""

# launch THEME [VAR=value ...]: starts Vano with an isolated config, sizes its window and waits.
launch() {
    local theme="$1"; shift
    printf 'language = "en"\ntheme = "%s"\naccent = "indigo"\ncheck_updates = false\n' "$theme" \
        > "$HOME_DIR/.config/vano/settings.toml"
    # /home becomes an empty tmpfs with only the real home (for the binary) and the demo one.
    bwrap --dev-bind / / --tmpfs /home --bind "$HOME" "$HOME" --bind "$HOME_DIR" "$DEMO_HOME" \
        env -u WAYLAND_DISPLAY HOME="$DEMO_HOME" XDG_CONFIG_HOME="$DEMO_HOME/.config" \
        XDG_CACHE_HOME="$DEMO_HOME/.cache" XDG_STATE_HOME="$DEMO_HOME/.local/state" \
        GPUI_X11_SCALE_FACTOR=$SCALE "$@" "$BIN" >"$WORK/app.log" 2>&1 &
    local i
    for i in $(seq 1 40); do
        WID=$(xdotool search --pid "$(pgrep -x vano | head -1)" 2>/dev/null | tail -1)
        [ -n "$WID" ] && break
        sleep 0.5
    done
    [ -n "$WID" ] || die "the vano window never appeared"
    sleep 2
    xdotool windowsize "$WID" "$WIDTH" "$HEIGHT" windowmove "$WID" 20 10
    sleep 3
}

quit_app() {
    pkill -x vano
    local i
    for i in $(seq 1 20); do pgrep -x vano >/dev/null || break; sleep 0.25; done
    WID=""
}

# Keystrokes and clicks must never reach another application.
ensure_active() {
    [ "$(xdotool getactivewindow 2>/dev/null)" = "$WID" ] && return 0
    xdotool windowactivate --sync "$WID"; sleep 0.4
    [ "$(xdotool getactivewindow 2>/dev/null)" = "$WID" ] || die "Vano lost the focus, aborting"
}

# pointer PX PY: moves the pointer to the screenshot pixel (PX, PY).
pointer() {
    eval "$(xdotool getwindowgeometry --shell "$WID")"
    local rx ry
    rx=$(python3 -c "print(round(($X+$1)/$POINTER_SCALE))")
    ry=$(python3 -c "print(round(($Y+$2)/$POINTER_SCALE))")
    ensure_active
    xdotool mousemove $((rx-2)) $((ry-2)); sleep 0.2
    xdotool mousemove "$rx" "$ry"; sleep 0.3
    ensure_active
}

click() { pointer "$1" "$2"; xdotool mousedown 1; sleep 0.15; xdotool mouseup 1; sleep 1.2; }

# Parks the pointer on the status bar so no hover state is captured.
park() { pointer 900 951; }

# shot NAME THEME: captures the active window into site/img/NAME-THEME.webp
shot() {
    local name="$1" theme="$2"
    park
    sleep 0.6
    ensure_active
    spectacle -b -n -a -o "$WORK/$name.png" || die "spectacle failed"
    magick "$WORK/$name.png" -quality 85 -define webp:method=6 -define webp:alpha-quality=90 \
        "$OUT/$name-$theme.webp" || die "magick failed (no WebP support?)"
    echo "wrote $OUT/$name-$theme.webp"
}

for theme in light dark; do
    # The results of the made-up home, colored by category.
    launch "$theme" VANO_OPEN=results
    shot hero "$theme"
    click 110 458      # the "dev" tile: zooms into it
    shot explore "$theme"
    quit_app

    # Cleanup coloring, a recommendation's detail and the confirmation of the Trash.
    launch "$theme" VANO_OPEN=results
    click 1008 328     # Cleanup
    click 1300 486     # the "Rust build output" recommendation
    shot cleanup "$theme"
    click 1517 868     # Move to Trash: only opens the confirmation, nothing is confirmed
    shot trash "$theme"
    quit_app

    launch "$theme" VANO_OPEN=results
    click 691 328      # Rings
    shot rings "$theme"
    quit_app

    launch "$theme"
    shot home "$theme"
    quit_app

    launch "$theme" VANO_OPEN=settings
    shot settings "$theme"
    quit_app
done

echo "done"
