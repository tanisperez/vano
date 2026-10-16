#!/bin/bash
# Renders the Open Graph card, site/img/og.png (1200x630), from og.html with headless Chrome.
# Run it after the screenshots change: the card shows img/hero-dark.webp.
set -u

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
CHROME="$(command -v google-chrome-stable || command -v google-chrome || command -v chromium)"
[ -n "$CHROME" ] || { echo "og-image.sh: Chrome or Chromium is required" >&2; exit 1; }

PROFILE="$(mktemp -d)"
trap 'rm -rf "$PROFILE"' EXIT

"$CHROME" --headless=new --user-data-dir="$PROFILE" --hide-scrollbars --allow-file-access-from-files \
    --window-size=1200,630 --virtual-time-budget=3000 --screenshot="$ROOT/site/img/og.png" \
    "file://$HERE/og.html" 2>/dev/null
[ -s "$ROOT/site/img/og.png" ] || { echo "og-image.sh: the card was not rendered" >&2; exit 1; }
magick "$ROOT/site/img/og.png" -strip -define png:compression-level=9 "$ROOT/site/img/og.png" 2>/dev/null
echo "wrote $ROOT/site/img/og.png"
