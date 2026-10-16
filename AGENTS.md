# AGENTS.md

Guidance for AI agents working on Vano.

## Project

Vano is a native desktop disk analyzer written in Rust with `gpui` (through `gpui-kit` 0.7.0,
the same stack as Postino in `~/dev/postino`). It scans the home folder, the whole disk or any
folder, shows what fills it in an animated treemap, and recommends what can be moved to the
Trash safely. Targets: Linux first, then macOS and Windows. Nothing is ever deleted directly:
items go to the system Trash and can be put back.

The design system (screens, palettes, icons, the interactive prototype) is in
`vano_design_system/`. `Disk Analyzer.dc.html` is the reference for every screen; open it with
Chrome and change the props at the end of the file to see a given state.

## Architecture

A Cargo workspace, UI kept thin. Dependency direction (no cycles):

```
vano-app ──> vano-scan ──> vano-core
    │──────> vano-rules ─> vano-core
    │──────> vano-core
    └──────> vano-update (standalone)
```

- `vano-core`: the scanned tree (one flat arena of 40 byte nodes, children of a folder have
  consecutive ids, names in one buffer), its binary encoding (`persist`), the squarified treemap
  layout, size units and categories. No IO.
- `vano-scan`: the parallel scanner. Work stealing threads (`crossbeam-deque`) read one folder
  per job with `rustix` (`openat`, `getdents`, `statat` relative to the folder), and one
  aggregator thread owns the tree behind a `RwLock`, so the UI can read it while it grows. Sizes
  are allocated bytes, hard links count once, other file systems and pseudo file systems are not
  entered. `examples/bench.rs` measures it: about 0.2 s for 750k files warm, 5 times `du`.
- `vano-rules`: the recommendation rules (Safe / Review), the category of every node, and the
  live rules that report recommendations as folders finish. Its texts are
  `crates/vano-rules/locales/rules.yml`, linked into the app's `locales/`.
- `vano-update`: the in-app updater, ported from `postino-update`.
- `vano-app`: the `vano` binary. `state/` is plain Rust and unit tested (settings, results,
  treemap layout, spring motion, recents, trash). `views/` only builds elements.

### The explorer and its animation

`state/treemap.rs` lays out two levels (the focused folder's children, and inside each big
enough tile its own children), grouping small entries into one "N more" tile.
`state/motion.rs` animates any set of rectangles with gpui's analytic springs: a change of
targets keeps each tile's position and velocity, so a change in the middle of another one never
jumps. Zooming in maps the new tiles from the clicked tile's rectangle to the full area and
sends the old ones outward by the same transform; zooming out is the reverse
(`views/treemap_view.rs::apply_layout`). The live treemap of the scan uses the same machinery,
and its tiles become the explorer's when the results open.

The explorer shows the focused folder as a treemap, rings or a ranking (the results start in
the view the scan was showing). Category coloring uses each folder's dominant category by
bytes (`Results::dominant`, one backwards pass over the ids, since a child's id is always
higher than its parent's). Cleanup coloring paints each tile that is not a recommendation with
bands rising from the bottom, sized by the Safe and Review bytes inside it
(`Results::reclaimable`).

## Rules

- Performance rules from Postino apply: an idle window does no work (no timers or animation
  frames unless something moves; with the app idle for 10 s the main thread CPU ticks, fields
  14 and 15 of `/proc/<pid>/task/<pid>/stat`, must barely move), and `render` never costs in
  proportion to the tree size: layouts are computed only when their key (area size, focus,
  generation) changes.
- Colors only from `theme::Palette` (`cx.palette()`), sizes only from `theme::metrics`.
- Translations: English, Spanish, Galician, Italian, every key in all four
  (`state/locale.rs` tests check it). Never the em dash anywhere.
- `unsafe` is forbidden in the whole workspace. Clippy runs with `-D warnings`
  (`make lint`), `make format` and `make test` before considering a task done.
- Read `docs/gpui-notes.md` before touching theming, dialogs or inputs.

## Testing the UI by hand

`VANO_OPEN` starts in a given state: `scan` (scan the home folder), `scan:<path>`, `results`
(the last saved scan), `settings`, `settings-scanning`, `settings-advanced`, `settings-about`.
`VANO_LOG=debug` logs scans, rule timings, tile clicks and every treemap animation with its
frame count and fps.

On KDE Wayland, run the app through XWayland (`env -u WAYLAND_DISPLAY`) to drive it with
`xdotool`. xdotool works in logical coordinates (physical divided by the display scale, see
`kscreen-doctor -o`), and `spectacle -b -n -f -o file.png` captures the screen; `import` and
`ffmpeg x11grab` return stale frames of a gpui window.

## Working rules

- The repository is `git@github.com:tanisperez/vano.git` (branch `main`). Commit messages,
  issues and pull requests in English.
- Keep changes small and traceable to the request.
