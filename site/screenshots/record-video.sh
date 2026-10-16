#!/bin/bash
# Records the website demo video into site/video/ (WebM AV1, MP4 H.264 and a poster).
# With --cold the scan reads a cold disk: it asks for sudo first, to drop the page cache.
# See site/screenshots/README.md for the requirements.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
BIN="$(command -v vano)"
# Larger than the screenshots' window, at a larger scale, so the video is sharp at about 1900
# pixels wide while the layout stays the same.
SCALE=2.0
WIDTH=2240
HEIGHT=1334
# Every file of the made-up home is allocated at this fraction of its size (204 GB / 4).
DIVISOR=4
WORK="$ROOT/target/video"
OUT="$ROOT/site/video"
HOME_DIR="$WORK/home"
DEMO_HOME=/home/alex
REC_PID=""
COLD=0
[ "${1:-}" = "--cold" ] && COLD=1

cleanup() {
    [ -n "$REC_PID" ] && kill -INT "$REC_PID" 2>/dev/null
    pkill -x vano 2>/dev/null
    # About 50 GB of allocated files.
    rm -rf "${HOME_DIR:?}"
}
trap cleanup EXIT INT TERM

die() { echo "record.sh: $*" >&2; exit 1; }

[ -n "$BIN" ] || die "vano is not in the PATH"
for tool in gpu-screen-recorder spectacle xdotool magick ffmpeg python3 cargo bwrap; do
    command -v "$tool" >/dev/null || die "missing required tool: $tool"
done
if [ "$(busctl --user call org.freedesktop.ScreenSaver /ScreenSaver org.freedesktop.ScreenSaver GetActive 2>/dev/null)" = "b true" ]; then
    die "the screen is locked, unlock it first"
fi
pgrep -x vano >/dev/null && die "a vano process is already running, close it first"
if [ "$COLD" = 1 ]; then
    sudo -v || die "--cold needs sudo to drop the page cache"
fi

# XWayland runs at the largest display scale: X11 coordinates are physical pixels times it, and
# xdotool pointer moves are divided by it again. The screenshots and the recording are physical.
X_SCALE=$(kscreen-doctor -o 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g' | awk '/Scale:/ {print $2}' | sort -g | tail -1)
X_SCALE=${X_SCALE:-1}

rm -rf "${WORK:?}"
mkdir -p "$HOME_DIR/.config/vano" "$HOME_DIR/.cache" "$HOME_DIR/.local/state" "$OUT"
(cd "$ROOT" && cargo run --release -q -p vano-rules --example demo_home -- \
    "$DEMO_HOME" "$WORK/config" "$WORK/cache" "$WORK/manifest.tsv") >/dev/null \
    || die "the demo home could not be written"
python3 -I "$HERE/materialize.py" "$WORK/manifest.tsv" "$HOME_DIR" "$DIVISOR" \
    || die "the demo home could not be created"

printf 'language = "en"\ntheme = "light"\naccent = "indigo"\ncheck_updates = false\n' \
    > "$HOME_DIR/.config/vano/settings.toml"
# /home becomes an empty tmpfs with only the real home (for the binary) and the demo one.
bwrap --dev-bind / / --tmpfs /home --bind "$HOME" "$HOME" --bind "$HOME_DIR" "$DEMO_HOME" \
    env -u WAYLAND_DISPLAY HOME="$DEMO_HOME" XDG_CONFIG_HOME="$DEMO_HOME/.config" \
    XDG_CACHE_HOME="$DEMO_HOME/.cache" XDG_STATE_HOME="$DEMO_HOME/.local/state" \
    GPUI_X11_SCALE_FACTOR=$SCALE "$BIN" >"$WORK/app.log" 2>&1 &
WID=""
for _ in $(seq 1 40); do
    WID=$(xdotool search --pid "$(pgrep -x vano | head -1)" 2>/dev/null | tail -1)
    [ -n "$WID" ] && break
    sleep 0.5
done
[ -n "$WID" ] || die "the vano window never appeared"
sleep 2
# At (20, 20) physical pixels of the top left display.
POS=$(python3 -c "print(round(20*$X_SCALE))")
xdotool windowsize "$WID" "$WIDTH" "$HEIGHT" windowmove "$WID" "$POS" "$POS"
sleep 3

# Keystrokes and clicks must never reach another application.
ensure_active() {
    [ "$(xdotool getactivewindow 2>/dev/null)" = "$WID" ] && return 0
    xdotool windowactivate --sync "$WID"; sleep 0.4
    [ "$(xdotool getactivewindow 2>/dev/null)" = "$WID" ] || die "Vano lost the focus, aborting"
}

# glide PX PY: moves the pointer smoothly to the screenshot pixel (PX, PY) of the window.
glide() {
    ensure_active
    python3 -I - "$1" "$2" "$POS" "$X_SCALE" "$(xdotool getmouselocation --shell)" <<'EOF'
import math, subprocess, sys, time
px, py, pos, scale = float(sys.argv[1]), float(sys.argv[2]), float(sys.argv[3]), float(sys.argv[4])
loc = dict(l.split("=") for l in sys.argv[5].split())
x0, y0 = float(loc["X"]) / scale, float(loc["Y"]) / scale
x1, y1 = pos / scale + px, pos / scale + py
steps = 36
for i in range(1, steps + 1):
    t = i / steps
    e = 0.5 - math.cos(math.pi * t) / 2
    subprocess.run(["xdotool", "mousemove", str(round(x0 + (x1 - x0) * e)), str(round(y0 + (y1 - y0) * e))])
    time.sleep(0.016)
EOF
    sleep 0.15
}

press() { ensure_active; xdotool mousedown 1; sleep 0.1; xdotool mouseup 1; }

# click PX PY WAIT: glides to the pixel, clicks and waits.
click() { glide "$1" "$2"; press; sleep "$3"; }

# The window's opaque area in the screenshot (the window without its shadow) is the area recorded.
ensure_active
spectacle -b -n -a -o "$WORK/window.png" || die "spectacle failed"
read -r CW CH CX CY < <(magick "$WORK/window.png" -alpha extract -threshold 99% -format '%@' info: \
    | sed 's/[x+]/ /g')
REGION="${CW}x${CH}+$((20 + CX))+$((20 + CY))"

glide 980 860
# The app reads nothing of the home before the scan, so the cache is dropped before recording.
if [ "$COLD" = 1 ]; then
    sync
    sudo -n sh -c 'echo 3 > /proc/sys/vm/drop_caches' || die "the page cache could not be dropped"
fi
gpu-screen-recorder -w "$REGION" -f 60 -k h264 -q ultra -fm cfr -cursor yes \
    -o "$WORK/raw.mp4" >"$WORK/recorder.log" 2>&1 &
REC_PID=$!
sleep 1.5

click 1711 69 1.0      # Settings
click 1474 572 1.0     # Dark
click 1474 940 1.3     # Electric
click 1608 107 0.8     # closes Settings
click 491 705 2.6      # Scan Home folder
# The poster: the results of the scan, colored by category, without the pointer.
spectacle -b -n -a -o "$WORK/poster.png" || die "spectacle failed"
click 200 650 2.2      # the atlas-api tile inside dev: zooms into it
click 183 473 1.8      # back to Home through the breadcrumb
click 798 380 2.0      # Rings
click 1169 380 2.0     # Ranking (the switch moves without the coloring switch)
click 955 380 1.2      # Treemap
click 1167 380 2.0     # Cleanup coloring
click 1450 555 2.2     # the "Rust build output" recommendation
click 1757 1003 2.0    # Move to Trash: the confirmation
click 1243 677 3.2     # confirms: the demo home's own files go to its own Trash

kill -INT "$REC_PID"
wait "$REC_PID"
REC_PID=""
COLD=0
[ "${1:-}" = "--cold" ] && COLD=1

# The window's rounded corners are cropped by the page (border-radius), not here.
ffmpeg -v error -y -i "$WORK/raw.mp4" -vf "scale=1600:-2:flags=lanczos" -an -c:v libx264 \
    -preset slow -crf 22 -pix_fmt yuv420p -movflags +faststart "$OUT/demo.mp4" \
    || die "the MP4 could not be encoded"
ffmpeg -v error -y -i "$WORK/raw.mp4" -vf "scale=1600:-2:flags=lanczos" -an -c:v libsvtav1 \
    -preset 4 -crf 38 -pix_fmt yuv420p "$OUT/demo.webm" || die "the WebM could not be encoded"
# The poster is the same area as the video, shown until it plays (or instead of it).
magick "$WORK/poster.png" -crop "${CW}x${CH}+${CX}+${CY}" +repage -alpha off \
    -filter Lanczos -resize 1600x -quality 85 "$OUT/demo-poster.webp" || die "the poster failed"
ls -l "$OUT"
