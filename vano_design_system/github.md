repo: tanisperez/postino
branch: main

## Last sync
date: 2026-10-16T07:45:11Z

### Updated in this project
- Disk analyzer prototype built on Postino's design tokens, metrics and components
- Design system with three accent palettes (Indigo, Graphite, Electric) in light and dark
- Geist and Geist Mono fonts copied from site/fonts
- Vano app icon built like packaging/icons/postino.svg (Geist Bold glyph outlined)

## Screen map
| Project screen | Repo files |
| --- | --- |
| Disk Analyzer.dc.html (title bar, status bar, cards, buttons, segmented control, badges, modal) | crates/postino-app/src/theme/palette.rs, crates/postino-app/src/theme/metrics.rs, crates/postino-app/src/views/title_bar.rs, crates/postino-app/src/views/status_bar.rs, crates/postino-app/src/views/components/{buttons,card,segmented_control,section_label,status_badge}.rs, docs/design-system.md |
| Disk Analyzer.dc.html (home screen) | docs/design-system.md (Welcome tab section) |
| packaging/icons/vano.svg, png/, vano.ico, site/img/logo.svg | packaging/icons/postino.svg, packaging/icons/README.md, packaging/macos/bundle.sh, crates/postino-app/build.rs |
| Disk Analyzer.dc.html (Settings modal) | crates/postino-app/src/views/settings.rs, views/settings/about.rs, views/components/{theme_card,select_trigger}.rs, locales/settings.yml, locales/update.yml |
| Design System.dc.html | crates/postino-app/src/theme/palette.rs, crates/postino-app/src/theme/metrics.rs, docs/design-system.md |
