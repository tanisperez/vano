# Website screenshots

`capture.sh` regenerates every screenshot of the website into `site/img/` as WebP, one light and
one dark version of each: `hero`, `explore`, `cleanup`, `trash`, `rings`, `home` and `settings`.
Run it from anywhere, or with `make screenshots`.

## What it does

- Writes a made-up home folder as a saved scan with the `demo_home` example of `vano-rules`
  (`crates/vano-rules/examples/demo_home.rs`): about 340,000 files and 204 GB of Rust, web,
  Android, Python and Flutter projects, caches, SDKs, AI models and media, built so that most
  rules recommend something. No real folder is scanned, so nothing of the machine taking the
  screenshots shows up.
- Launches `target/release/vano` (built first) under XWayland at 1.5x scale, inside bubblewrap,
  with that home mounted as `/home/alex` and its config, cache and state inside it, so the paths
  on screen read `/home/alex` and your real settings and scans are never touched. The UI is
  English, the accent Indigo. The work folder is `target/screenshots/`, on the real disk: the free
  and used space the app shows come from the file system of the scanned folder.
- Sizes the window to 1680x1000, so every click lands on the same spot, opens the results with
  `VANO_OPEN=results` (or Settings with `VANO_OPEN=settings`), and drives the rest with `xdotool`
  clicks (zoom into `dev`, Cleanup coloring, a recommendation, the Trash confirmation, Rings).
  It never confirms the Trash.
- Captures the window with `spectacle` and converts it with ImageMagick.
- Stops the app on exit, also on Ctrl+C or failure.

The disk numbers in the screenshots (free space, "Not scanned", the status bar) are those of the
machine's own root file system.

## Requirements

- Linux, KDE Plasma on Wayland with XWayland. X11 pointer requests are scaled by the display
  scale, read from `kscreen-doctor -o`; the window geometry and the screenshots are in physical
  pixels. The click coordinates are screenshot pixels of the 1680x1000 window.
- `bwrap` (bubblewrap), `spectacle`, `xdotool`, `python3`, `cargo` and ImageMagick (`magick`)
  built with WebP support.
- An unlocked screen. Do not use the keyboard or mouse while it runs (about 2 minutes): the
  script aborts if Vano loses the focus, so clicks never reach another application.

# Website demo video

`record-video.sh` records the demo video of the website into `site/video/`: `demo.webm` (AV1),
`demo.mp4` (H.264, for browsers without AV1) and `demo-poster.webp`. Run it from anywhere, or
with `make video`. It takes about 2 minutes. With `--cold`
(`site/screenshots/record-video.sh --cold`, from a terminal) it asks for `sudo` first and drops the
page cache right before recording, so the scan reads a cold disk and the live view stays on screen
a little longer.

## What it does

- Writes the same made-up home as the screenshots, but as real files: `demo_home` also writes a
  manifest of its tree, and `materialize.py` creates every folder and file of it under
  `target/video/home`, each file allocated with `fallocate` at a quarter of its size (about
  340,000 files and 50 GB of real disk space, removed when the script ends).
- Launches the `vano` of the `PATH` (not a fresh build) the same way as `capture.sh`: under
  XWayland, inside bubblewrap, with that home mounted as `/home/alex`, in English, light theme and
  Indigo accent. The window is 2240x1334 at 2x scale, so the video is sharp at 1600 pixels wide.
- Records the window with `gpu-screen-recorder` (a screen region, KMS capture, no portal dialog)
  while `xdotool` drives it with smooth pointer moves: Settings (Dark, Electric), a live scan of
  the home, a zoom into a project and back, Rings, Ranking, Cleanup coloring, a recommendation, and
  Move to Trash, confirmed. The files moved are the made-up home's own, into its own Trash.
- Encodes the recording with `ffmpeg` (SVT-AV1 and x264, no audio) and takes the poster from the
  results right after the scan.

## Requirements

The same as `capture.sh`, plus `vano` in the `PATH`, `gpu-screen-recorder`, `ffmpeg` built with
`libsvtav1` and `libx264`, and about 50 GB free on the disk of the repository. XWayland runs at
the largest display scale, read from `kscreen-doctor -o`. The window is placed on the display at
the top left corner of the desktop.
