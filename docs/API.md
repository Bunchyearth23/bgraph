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
