# Vano website

The landing page and install page served at <https://vano.tanis.codes>. Plain HTML, CSS and
JavaScript, no dependencies and no build step. Same structure and stylesheet as Postino's site,
without the user documentation.

- `index.html`, `install.html`: the pages. The download links point to the latest GitHub release.
- `css/style.css`: copied from Postino's site. Every color, radius and font comes from the Vano
  design system with the Indigo accent (`docs/design-system.md`,
  `crates/vano-app/src/theme/palette.rs`). Each color token is a `light-dark()` pair, picked by
  `color-scheme`. Every screenshot is in the page twice, `.light` and `.dark`, and CSS shows the
  one for the active theme.
- `js/theme.js`: follows the system theme until the visitor picks one with the header button,
  keeps that choice in `localStorage` and sets `data-theme` on `<html>`. Without JavaScript the
  page still follows the system and the button is hidden.
- `fonts/`: Geist and Geist Mono (SIL OFL 1.1, `fonts/OFL.txt`), subset to Latin, copied from
  Postino's site.
- `img/`: the logo (`packaging/icons/vano.svg`), the 512 px icon and the app screenshots, WebP
  only.
- `screenshots/`: the tooling that regenerates `img/*-light.webp` and `img/*-dark.webp`
  (`make screenshots`). It is not published.

The numbers of the "Fast and light" section were measured by hand on Linux (a home folder of
782,747 files and 208 GB, warm cache, release build): the scan time from the app's log, the
memory as the PSS of `/proc/<pid>/smaps_rollup`. Measure again before changing them.

Preview locally with `make site`, which serves the site on <http://localhost:8000>.

Deployed by `.github/workflows/website.yml` on every push to `main` that touches `site/`.
