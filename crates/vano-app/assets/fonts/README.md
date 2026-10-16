# Bundled fonts

Geist and Geist Mono, from the official Vercel `geist-font` GitHub release, embedded in the
binary with `include_bytes!` and registered at startup (`crates/postino-app/src/theme/mod.rs`).

- Source: <https://github.com/vercel/geist-font>
- Release used: `v1.7.2`
- Download URL: <https://github.com/vercel/geist-font/releases/download/v1.7.2/geist-font-v1.7.2.zip>
- License: SIL Open Font License 1.1, full text in `OFL.txt` (copied unmodified from the release
  archive).

Files kept here, static TTFs from the archive's `Geist/ttf/` and `GeistMono/ttf/` folders (the
weights the UI type scale (`docs/design-system.md`) actually uses; the archive also
ships Thin, ExtraLight, Light, ExtraBold, Black and italics for both families, plus a separate
`GeistPixel` family, none of which Postino uses, so they are not copied here):

- `Geist-Regular.ttf`
- `Geist-Medium.ttf`
- `Geist-SemiBold.ttf`
- `Geist-Bold.ttf`
- `GeistMono-Regular.ttf`
- `GeistMono-Medium.ttf`
- `GeistMono-SemiBold.ttf`
