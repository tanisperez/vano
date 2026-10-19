# Vano design system

Reference for anyone (human or agent) touching the UI. It replaces the Claude Design HTML mockups
that used to live in `vano_design_system/`: the design is implemented, so the code is the source
of truth and this file is its readable summary. If this file and the code disagree, the code
wins; fix this file.

Vano uses telaio's tokens, metrics, fonts and standard components (`~/dev/telaio`, the same ones
as Postino) and extends them for a disk analyzer: three accent palettes, six category colors for the treemap, two
cleanup treatments and the motion of the scan.

Where things live (all under `crates/vano-app/src/`):

| What | File |
| --- | --- |
| Color tokens (three accents, light and dark) as telaio `Tokens`, category and cleanup colors | `theme/palette.rs` |
| Sizes, heights, widths, radii (telaio's, plus Vano's own layout) | `theme/metrics.rs` |
| Theme family from the settings, live settings apply | `theme/mod.rs` |
| Standard widgets (telaio, re-exported), accent and live view cards | `views/components/` |
| Panel card, card header, stats strip, section label, legend | `views/common.rs` |
| Treemap layout (two levels, "N more" tile) | `state/treemap.rs` |
| Spring motion of tiles and ranking rows | `state/motion.rs` |
| Tile colors and drawing | `views/treemap_view.rs` |

## Rules

- Colors only through `cx.palette()` (a `Palette`), never from `cx.theme()` and never as a literal.
- Sizes only from `theme::metrics`, never scattered as literals.
- Every hex literal of Vano is written once, in `theme/palette.rs`. The accent swaps only the neutrals
  and the accent tokens; the states, the category colors and the cleanup fills are shared by the
  three accents.
- Light or dark follows the system through `Theme::sync_system_appearance`, as in Postino, unless
  the user picks one in Settings.
- Panels are flat (border only). Cards use `raised` plus a border. Menus, popovers and modals use
  `overlay` plus the shadow token.
- The accent is for the primary action, the selected item, links and focus. The treemap carries
  the color; the accent is not decoration.
- Safe to remove is always `success` (green), Review first is always `warning` (amber), in every
  view.
- Every size and counter uses the mono font, so numbers do not jitter while they climb during a
  scan.
- No em dash anywhere (code, comments, docs, strings).

## Palette

Hex values copied from `theme/palette.rs`. The user picks the accent in Settings, Appearance.

### Accents

- **Indigo** (default): Postino's own palette, unchanged.
- **Graphite**: pure neutrals, the accent is ink. The treemap carries all the color.
- **Electric blue**: cooler neutrals and a saturated blue accent. Reads as a system utility.

### Neutrals and accent, light

| Token | Indigo | Graphite | Electric | Use |
| --- | --- | --- | --- | --- |
| `bg` | `#fcfcfd` | `#fcfcfc` | `#fbfcfe` | Window content |
| `surface` | `#f4f5f7` | `#f5f5f5` | `#f2f5f9` | Title bar, status bar, panel headers |
| `raised` | `#ffffff` | `#ffffff` | `#ffffff` | Cards, inputs, tiles without a category |
| `overlay` | `#ffffff` | `#ffffff` | `#ffffff` | Menus, popovers, modals |
| `border` | `#e3e5ea` | `#e5e5e5` | `#e0e6ee` | Default border and separator |
| `border_strong` | `#cdd0d8` | `#d0d0d0` | `#c9d2de` | Control borders |
| `fg` | `#1b1c22` | `#171717` | `#111827` | Primary text |
| `fg_muted` | `#5c606e` | `#5e5e5e` | `#556070` | Secondary text |
| `fg_subtle` | `#8b8e9a` | `#8f8f8f` | `#8792a2` | Captions, icons, status bar |
| `hover` | `#eceef2` | `#eeeeee` | `#e9eef5` | Hover background |
| `accent` | `#4f57d8` | `#171717` | `#0b63f6` | Primary button, focus, logo square |
| `accent_fg` | `#ffffff` | `#ffffff` | `#ffffff` | Text on `accent` |
| `accent_text` | `#3f47c4` | `#171717` | `#0052d6` | Links, selected label |
| `accent_subtle` | `#e8e9fd` | `#ececec` | `#e3edff` | Selected card or row background |

### Neutrals and accent, dark

| Token | Indigo | Graphite | Electric |
| --- | --- | --- | --- |
| `bg` | `#15161a` | `#141414` | `#0f131a` |
| `surface` | `#1b1c21` | `#1a1a1a` | `#141922` |
| `raised` | `#23252c` | `#232323` | `#1b222d` |
| `overlay` | `#262830` | `#262626` | `#1f2632` |
| `border` | `#2c2e36` | `#2d2d2d` | `#263040` |
| `border_strong` | `#3b3e48` | `#3d3d3d` | `#344156` |
| `fg` | `#e9eaee` | `#ededed` | `#e8edf5` |
| `fg_muted` | `#a0a3ae` | `#a3a3a3` | `#9aa6b8` |
| `fg_subtle` | `#6f7280` | `#717171` | `#677489` |
| `hover` | `#25272e` | `#262626` | `#1f2733` |
| `accent` | `#5b63e6` | `#ededed` | `#2f7bff` |
| `accent_fg` | `#ffffff` | `#141414` | `#ffffff` |
| `accent_text` | `#a3a9ff` | `#ffffff` | `#8db8ff` |
| `accent_subtle` | `#272b4d` | `#2e2e2e` | `#15284a` |

Derived at runtime (not hex literals): `pressed` is `hover` blended with 2/3 of `border_strong`.
`accent_hover` and `danger_hover` shift the base color one step (0.07 lightness) toward black in
light and toward white in dark; `accent_pressed` and `danger_pressed` shift two steps.

### States (shared by every accent)

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `success` | `#1d8657` | `#4fc88f` | Safe to remove |
| `success_subtle` | `#e2f4ea` | `#15302a` | Safe badge background |
| `warning` | `#9a6500` | `#e8b75a` | Review first |
| `warning_subtle` | `#fbf0d9` | `#352b15` | Review badge background, review tile fill |
| `danger` | `#cf3535` | `#f27171` | Errors, destructive actions |
| `info` | `#1c6fb0` | `#62b3f0` | Informational (stock widgets) |
| `scrim` | `#0a0b1073` | `#0a0b1073` | Backdrop behind a modal |

### Treemap categories (shared by every accent)

One hue per category, with the same lightness and chroma across hues. The design wrote them in
`oklch()`; the code has them converted to sRGB hex. Each category has four colors: `bg` for a
parent tile (a folder that shows its children), `fill` for a leaf tile, `line` for the border of
both, and `dot` for legends, ring segments and ranking bars.

| Category | Name | Hue | Light `bg` / `fill` / `line` / `dot` | Dark `bg` / `fill` / `line` / `dot` |
| --- | --- | --- | --- | --- |
| `media` | Documents & media | 55 | `#fff4ec` `#ffe2ce` `#ebaf86` `#eb7c33` | `#2a1d15` `#412714` `#7a4b29` `#f0995b` |
| `other` | Dev tools & other | 260 | `#f5f7f9` `#e9ebef` `#c0c4cc` `#858d99` | `#1e1f21` `#292b2f` `#494d54` `#8b939f` |
| `code` | Code projects | 285 | `#f5f5ff` `#e2e3ff` `#b1b0ef` `#796ce0` | `#1e1d2d` `#2b294b` `#535088` `#9c96f8` |
| `sdk` | SDKs & emulators | 215 | `#ebfafe` `#cbeff9` `#7cc8db` `#01aac6` | `#132226` `#0c333c` `#1d6270` `#4dc3dd` |
| `ai` | AI models | 350 | `#fff2f8` `#ffdcec` `#eba6c6` `#db509c` | `#2a1a21` `#422232` `#7d415f` `#f080b8` |
| `caches` | Package caches | 255 | `#f0f8ff` `#d5e8ff` `#95baea` `#2a80e2` | `#17202c` `#1c2f46` `#365987` `#6aa7f4` |

A folder's category is its dominant one by bytes (`Results::dominant`).

### Cleanup treatments

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `safe_fill` | `raised` + 9% `success` | `raised` + 12% `success` | Fill of a safe tile, in both colorings |
| `safe_hatch` | `success` at 0.38 | `success` at 0.34 | Diagonal hatch over a safe tile |
| `safe_line` | `success` at 0.62 | `success` at 0.55 | Border of a safe tile |
| `review_fill` | `warning_subtle` | `warning_subtle` | Fill of a review tile (Cleanup coloring) |
| `review_line` | `warning` at 0.50 | `warning` at 0.45 | Border of a review tile |

Tile colors (`views/treemap_view.rs::tile_colors`):

- **Category coloring**: parents `cat.bg` with `cat.line`, leaves `cat.fill` with `cat.line`, safe
  leaves hatched with the safe treatment.
- **Cleanup coloring**: parents `raised` with `border_strong`; safe leaves hatched; review leaves
  `review_fill`; any other leaf `surface` with `border`, plus bands rising from the bottom sized by
  the Safe and Review bytes inside it (`Results::reclaimable`).
- The "N more" tile is `surface` with `border`. The current tile gets an `accent` border.

### Shadow (menus, popovers, modals)

Two layers, horizontal offset always 0. The same as Postino.

| Layer | Light | Dark |
| --- | --- | --- |
| 1 | y 12, blur 32, spread 0, `rgba(20,22,40,0.14)` | y 12, blur 32, spread 0, `rgba(0,0,0,0.45)` |
| 2 | y 0, blur 0, spread 1, `rgba(20,22,40,0.06)` | y 0, blur 0, spread 1, `rgba(255,255,255,0.04)` |

The values are CSS. gpui's `blur_radius` is the gaussian's standard deviation, half a CSS blur, so
`shadow_layer` passes half the blur to gpui.

## Typography

UI font **Geist**, mono font **Geist Mono**, both bundled by telaio-ui (OFL). The user can
change the UI font, its size (base 13) and the mono font in Settings.

| Spec | Use |
| --- | --- |
| 26 / 500 | "Vano" on the home screen |
| 15 / 600 | Card and dialog titles ("Recommendations") |
| 13 / 500 | Buttons, row names |
| 13 / 400 | Body and descriptions |
| 12 / 400 | Card subtitles, legend |
| 11 / 600 uppercase, 0.06em | Section labels ("WHAT TO SCAN") |
| mono 26 / 500 | Big numbers of the stats strip ("92.0 GB") |
| mono 13 / 500 | Sizes in rows ("13.4 GB") |
| mono 11.5 / 400 | Paths |

Status bar text is 11.5 in `fg_subtle`. gpui has no letter spacing, so the mockup's tracking on
the title and the big numbers is not rendered.

## Metrics (`theme/metrics.rs`)

Heights (px): title bar 40, status bar 24, controls 30, title bar controls (target switcher,
gear, Rescan) 26, segmented item 22, status badge 22, stat cell 90, card header 68, breadcrumb
38, recommendation row 56, ranking row 42, location card (home) 110, recent row 40, modal header
and footer 52.

Widths (px): home column 680, right column (Recommendations, Found so far) 342, Move to Trash
confirmation 482, ring 250 with a 20 thickness.

Spacing: page padding and gap between cards 16.

Radii (px): 4 chips and checkboxes (`RADIUS_XS`), 6 segmented items, rows and badges
(`RADIUS_SM`), 8 inputs, buttons and small cards (`RADIUS_MD`), 10 cards, menus and popovers
(`RADIUS_LG`), 14 modals (`RADIUS_MODAL`). gpui-kit's own widgets get 8 general and 14 large.

Treemap (`state/treemap.rs`): a parent tile has a 27 px header with its name and size and 5 px
of padding; a tile needs at least 64 x 68 to show its children. Up to 90 tiles at the top level
and 40 inside each parent; smaller entries are grouped into one "N more" tile.

## Screens

Every screen has the title bar on top and the status bar at the bottom.

1. **Title bar** (`surface`, bottom border): the "V" logo square in `accent` (a click goes home),
   "Vano", the scan target switcher (home folder, whole disk or a folder), Rescan on the results,
   the settings gear, and the window controls drawn by gpui-kit's `TitleBar`.
2. **Home** (`views/home.rs`): one centered column, 680 wide. Header with a 48 logo square
   (radius 12, "V" 24/600) beside "Vano" and the tagline in 14 `fg_muted`. Then "What to scan"
   with two location cards side by side (the disk, with a usage bar, and "Any folder"); the
   selected one is `accent_subtle` with a 2 px `accent` border. Then the primary "Scan ..."
   button, the recent scans (icon, name, size and date, "Open results" link), and a footer with a
   top border that says Vano only reads sizes and removes nothing without confirmation.
3. **Scanning** (`views/scanning.rs`): the stats strip (Files scanned, Counted, Elapsed,
   Throughput), then the "Live view" card on the left (Treemap, Rings or Ranking, chosen with a
   segmented control) and "Found so far" on the right (342 wide), where recommendations appear as
   each folder finishes. "Cancel scan" while it runs, "View results" when it ends; the results
   open by themselves 1.4 s after the end.
4. **Results** (`views/results.rs`): the stats strip (Free after cleanup, Safe to remove, Needs
   review, Not scanned), then the "Explorer" card (breadcrumb, the treemap, rings or ranking of
   the focused folder, Category or Cleanup coloring, and the legend) and the "Recommendations"
   panel on the right, grouped in Safe to remove and Review first, with checkboxes and "Select
   safe". A click on a recommendation or a tile opens its detail (badge, description, reason,
   "Largest inside" bars, Files, Last used, "Covered by" links, "Show in folder", "Move to
   Trash").
5. **Move to Trash** (`views/feedback.rs`): a modal (482 wide) with the list of items and the
   total, and a danger button "Move N to Trash". Then a toast at the bottom center, "N freed"
   with "Undo", for 8 s.
6. **Settings** (`views/settings.rs`): a modal with a left nav and sections for Appearance
   (language, theme, accent, fonts), Scanning (default live view), Advanced (log level) and About
   (version, updates). Every control applies and saves at once; there is no Save button.
7. **Status bar** (24, `surface`, top border): `hard-drive` icon with the disk's free and total
   space (and the space the file system reserves, when there is any); at the right, the scan
   progress, the results summary or "Moving to the Trash"; and "Vano X is ready · Restart" when
   an update is downloaded.

Every dialog is a modal card drawn by telaio's `framed_dialog`: `overlay` fill, radius 14, a 1 px
`border` border and the shadow token, `scrim` behind, a 52 px header and footer. See
`docs/gpui-notes.md`, "Dialogs".

The app's minimum window size is 900 x 560. The mockup also had narrow layouts down to 560 wide;
they were not implemented.

## Motion

- **Treemap**: every tile springs toward its target with gpui's analytic springs
  (`state/motion.rs`): `SPRING` (stiffness 260, damping 32, critically damped, about 350 ms) for
  zooms, `LIVE_SPRING` (140, 24) for the live scan, whose targets change every poll (100 ms). A
  change in the middle of another keeps each tile's position and velocity. Zooming in grows the
  new tiles out of the clicked one and sends the old ones outward; zooming out is the reverse.
  New tiles fade in, removed ones fade out.
- **Rings**: the category arcs ease toward the counted bytes, and a comet sweeps the ring while
  the scan reads; it stops when the scan ends.
- **Ranking**: the top ten rows spring to their new places; bars follow the bytes.
- **Found so far**: recommendations enter in the order they were found.
- **Idle rule**: as in Postino, no timer or animation frame outside a scan or a moving tile. The
  home screen and the results repaint only on input.

The mockup used CSS easing (450 ms `cubic-bezier(.2,.7,.2,1)`, re-layout every 250 ms); the
springs replace it so that interrupted motion never jumps.

## Components (`views/components/`)

From telaio: buttons (primary, secondary, danger, ghost), `IconButton`, the modal frame every dialog
shares (`framed_dialog`), `SegmentedControl`, `StatusBadge` and `ThemeCard`. Vano's own:
`PreviewCard` (the accent and live view cards of Settings). Reuse one before writing a new
widget; a fix to a standard one belongs in telaio.

Icons are Lucide, through `gpui_kit::assets::IconName`. The app must keep
`.with_assets(gpui_kit::assets::AllAssets)` or icons render empty.

## App icon

Built like Postino's: an accent square, radius 71 of 256, with a "V" in Geist Bold outlined to a
path, at the same cap height. The master is `packaging/icons/vano.svg`; the PNGs, the `.ico` and
the macOS icon are generated from it (`packaging/icons/README.md`).
`packaging/icons/alternatives/` keeps the Graphite and Electric variants; Graphite needs its 1 px
outline to stay visible on dark backgrounds.

## History

The mockups (Claude Design HTML files in `vano_design_system/`: `Disk Analyzer.dc.html`, the
interactive prototype of every screen, and `Design System.dc.html`, the tokens page, plus the
Geist web fonts and the logo for a future website) are kept only in git history, under the tag
`design-mockups`:

```
git checkout design-mockups -- vano_design_system
```

Open `Disk Analyzer.dc.html` with Chrome and change the props at the end of the file (palette,
theme, scan style, start screen, update state, window width) to see a given state.
