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
