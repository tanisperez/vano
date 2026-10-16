# Vano icon

`vano.svg` is the master: the title bar logo, an accent square
(`#4f57d8`, radius 71 of 256) with a "V" in Geist Bold converted to a path, so it renders the
same without the font installed. Same construction as Postino's icon: identical square, radius,
color and cap height (the letter spans y 74 to 188), so the two apps read as one family.
`crates/vano-app/assets/vano.ico` is the same design (16, 24, 32, 48, 64, 128 and 256 px PNG
entries). `alternatives/` holds the Graphite and Electric accent versions, unused.

The PNG sizes under `png/` are generated from it and committed:

```
for s in 16 24 32 48 64 128 256 512 1024; do
  rsvg-convert -w $s -h $s vano.svg -o png/vano-$s.png
done
```

The macOS `.icns` is built from these PNGs at bundle time (`iconutil`, see `packaging/macos`).
On Linux the 256 and 512 PNGs install as `share/icons/hicolor/<size>/apps/codes.tanis.vano.png`
and the SVG as `share/icons/hicolor/scalable/apps/codes.tanis.vano.svg`.
