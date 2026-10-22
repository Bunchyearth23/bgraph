# Commands and exports

These examples assume that you created `architecture.bd` in
[Your first diagram](demarrage.md) and that your terminal is in its folder.
Quote filenames or paths containing spaces.

Use `bgraph --help` or `bgraph <command> --help` to read the help supplied
by your installed version.

## Choose an output format

```sh
bgraph architecture.bd -o architecture.svg
bgraph architecture.bd -o architecture.pdf
bgraph architecture.bd -o architecture.json
```

The output filename's extension selects the format. Without `-o`, the
output is `out.svg` in the terminal's current folder. A render replaces
an existing output file only after producing the complete result. The renderer
refuses to replace its source, includes or data files; choose a distinct output
filename. A failed render leaves an existing export unchanged.

| Format | Use |
|---|---|
| SVG | A scalable image for a browser, documentation or graphics editor |
| PDF | A vector document for printing or sharing |
| PNG | A raster image; requires a build with the `png` feature |
| JSON | Solved geometry for inspection and integrations |

Geometry JSON exposes inspection details. Numeric values and element order
are not a stable API contract. This is separate from the stable diagnostic
JSON returned by `check --json`.

Geometry uses the exported canvas coordinates. `canvas.layout_origin.x`
and `.y` give the translation from solved layout to canvas: subtract them
from an exported point to recover layout coordinates. For waypoints, prefer
relative forms such as `via (above api gap 40)` so the route follows its
anchor. See the [waypoint reference](reference.md#edge-waypoints-via).

### PNG and resolution

After [enabling PNG](installation.md#optional-enable-png-output):

```sh
bgraph architecture.bd -o architecture.png --scale 2
```

`--scale` multiplies raster dimensions. Vector exports have no raster
resolution to choose.

### Fonts and printing

```sh
bgraph architecture.bd -o architecture.svg --embed-font /path/to/Font.ttf
bgraph architecture.bd -o architecture.pdf --page a4
```

Replace `/path/to/Font.ttf` with a real TrueType font file. Embedding carries
the selected font so another computer does not substitute its own.
`--page a4` tiles a large drawing across A4 sheets. Other accepted values
include `a3`, `letter` and sizes in points such as `600x800`. Dimensions must
be finite and larger than the 24-point overlap; exports above 10,000 sheets
are refused.

Without embedding, PDF text is limited to the standard font encoding.
Characters outside it produce an error naming the character. Use
`--embed-font` with a TrueType font containing every required character.
Missing glyphs are rejected instead of being silently replaced by `?`.
For SVG, select the embedded file's stem in the diagram's `font:` property
(for example `font: "DejaVuSans"` for `DejaVuSans.ttf`). Embedding alone does
not resize nodes or perform complex-script text shaping.

## Preview while editing

```sh
bgraph watch architecture.bd --open --port 0
```

Leave the terminal running and save the source in your editor to update
the drawing. Open the printed address if the browser does not open.
The viewer offers pan, zoom and, for bgraph source, geometry/style inspection.
Stop it with `Ctrl+C` in that terminal.

You can choose a fixed port or view an existing SVG instead:

```sh
bgraph watch architecture.bd --port 7879
bgraph watch architecture.svg --open --port 0
```

Without `--port`, the port is 7878. `--port 0` chooses a free port.
`--open` opens the browser after the server starts. If opening fails,
the server stays available at the printed URL. The watcher updates its
served preview; it does not overwrite an exported image.

## Build a whole folder

```sh
bgraph watch diagrams --out images
bgraph watch diagrams --out images --once
```

Create `diagrams` and put your `.bd` files there first. In folder mode,
`watch` runs an SVG build loop. `--once` builds once and exits with a nonzero
status if any file fails. `--open` is not available in folder mode.
Includes and external chart data trigger rebuilds too. Identical SVG output
is not rewritten, so unchanged diagrams keep their modification times.

## Check and format

```sh
bgraph check architecture.bd
bgraph check architecture.bd --ci
bgraph check architecture.bd --json
bgraph fmt architecture.bd --check
```

`check --ci` prints `file:line:column: severity: message`.
`check --json` prints a stable JSON array for automation. An error gives
a nonzero exit status; warnings alone do not make `check` fail.

`fmt --check` reports formatting differences without changing the file.
To rewrite it in canonical format:

```sh
bgraph fmt architecture.bd
```

`fmt --migrate` handles the old `opacity` property. It does not convert
old inline placements into the current `layout` language.

## Explain and compare

```sh
bgraph explain architecture.bd api
bgraph layout architecture.bd --check
```

`explain` describes an element and the source of its styles.
`layout` makes implicit placements explicit in the source; `--check`
checks whether that would be necessary without writing the file.

Given two saved versions, compare them by matching identifiers:

```sh
bgraph diff before.bd after.bd -o changes.svg
```

The image shows additions, removals and movement.

## Generate reference material

```sh
bgraph docs -o reference.md
bgraph schema -o schema.json
bgraph completions bash > bgraph.bash
```

The manual and vocabulary schema come from the compiler's tables.
The command is `bgraph docs` (plural). Its generated manual includes
[legend blocks, titles and descriptions](reference.md#legends), alongside
the property and type reference. Run it again after updating bgraph to
refresh your saved manual.

For the guide with navigation and search, follow
[Open the local site](documentation.md#open-the-local-site).

`completions` requires the feature of the same name, enabled by default.
