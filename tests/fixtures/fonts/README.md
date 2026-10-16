# Fixed font fixture

`DejaVuSans.ttf` is the unmodified DejaVu Sans Book font, internal version
`Version 2.37`, copied from the Arch Linux `ttf-dejavu`
`2.37+18+g9b5d1b2f-8` package (`/usr/share/fonts/TTF/DejaVuSans.ttf`).

- Upstream: <https://dejavu-fonts.github.io/>
- Package's upstream revision: `9b5d1b2ffeec20c7b46aa89c0223d783c02762cf`
- Licence at that revision:
  <https://github.com/dejavu-fonts/dejavu-fonts/blob/9b5d1b2ffeec20c7b46aa89c0223d783c02762cf/LICENSE>
- Font SHA-256:
  `6038a160b491e121c1f12c7bccb4a9c8730296e3adc1086a059404ed84b7451c`
- Font size: 759,720 bytes.

`LICENSE-DejaVu.txt` preserves the package's complete upstream licence and
copyright notices. This fixture is separately licensed from bgraph.

The subsetter regression needs a font with TrueType `glyf`/`loca` outlines;
choosing an arbitrary installed `.ttf` can select a colour font without them.
PNG snapshots embed these exact bytes under the CSS alias `Bgraph PNG Fixture`
so text rendering does not depend on a runner's installed fonts.
