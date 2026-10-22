# Embedding bgraph

The supported Rust API is deliberately small:

- `render_source(&str) -> Result<String>` renders autonomous source to SVG.
- `render_file(&Path) -> Result<String>` also resolves `include` files.
- `render_source_pdf(&str) -> Result<Vec<u8>>` is available with the `pdf`
  feature.
- `Template`, `Bindings` and `Value` fill safely escaped template holes before
  rendering.
- `parser` and `check` are supported when a caller needs an AST or all
  diagnostics rather than the first render error.

The modules that expose the engine (`ast`, `layout`, `style`, `charts`,
`render_svg`, `props`) remain public because the CLI, benchmarks and tooling
use them, but they are explicitly **unstable inspection surfaces**: their
types, fields, ordering and numeric values are not a compatibility promise.
Build integrations on the API above; use `bgraph schema` when an editor or
other tool needs vocabulary metadata. The CLI is the supported interface for
file-oriented operations such as formatting, composition, diff and JSON
export.

The doctests in `src/lib.rs` compile the supported direct-render and template
flows in CI.

Diagnostic position units are explicit at each interface:

| Interface | Line | Column |
|---|---|---|
| CLI, preview, playground display, Typst error text | One-based | One-based Unicode scalar values |
| WebAssembly `bgraph_check` JSON (`line`, `col`) | One-based | One-based UTF-16 code units |
| LSP ranges, including `relatedInformation` | Zero-based | Zero-based UTF-16 code units |

For a supplementary character such as `🦀`, UTF-16 counts two code units and
the human displays count one scalar. JavaScript consumers use `col - 1` for
string offsets. A human column is
`Array.from(sourceLine.slice(0, col - 1)).length + 1`.
Combining marks count as separate scalars. Rust `Span.start` and `Span.end`
remain zero-based UTF-8 byte offsets.

## Font metrics

Embedding a font makes that face portable; it does not rerun layout or move
the solved nodes and frames. Node sizing, wrapping, edge labels and collision
checks still use the built-in advance estimates, rather than the metrics of
arbitrary installed fonts (`X-005`).

SVG exports, and PNG through SVG, use the embedded face's horizontal glyph
advances for group-title background plates and the canvas bounds of group
and diagram titles. The title must explicitly select the embedded font as
the first family in its CSS font stack; the CLI names that family after the
embedded file's stem. Invalid fonts, missing glyphs and other font families
retain the existing estimates.

This targeted measurement reads `cmap` and `hmtx`; it is not a text-shaping
engine. Kerning, ligatures, complex scripts, variable-font axes and synthetic
weight or slant can differ from those simple advances. It does not promise
pixel-exact browser text metrics or global measurement of system fonts.

PDF exports reject characters missing from their selected encoding or embedded
TrueType face before producing bytes. Embedded text and group-title plates
use that face's advances; this does not recalculate node sizes. The inspection
functions `render_pdf::render` and `render_pdf::render_paged` therefore return
`Result<Vec<u8>>`. The supported `render_source_pdf` signature is unchanged.

Use `Template::fill_checked` when a complete binding is required before another
rendering pipeline. `Template::fill` deliberately retains partial substitutions
for editing; `Template::render` always performs the complete-binding check.
