# AGENTS.md

Context for AI coding assistants. Read at the start of every session.

**What is left is in [`docs/INDEX.md`](docs/INDEX.md)** — one line per open question, planned
chantier and live risk, with an explicit state. Update it in the same commit as the work.

```
grep " open \| planned \| blocked " docs/INDEX.md
```

## Intent

`bgraph` — a diagram DSL (`*.bd`) that renders to SVG, plus a language server
and a Zed extension for it.

`type` picks the archetype once per file: `free` (nodes and edges, the
default), `pie`, `packet`. Every archetype reads the *same* statements — a
`node` is a box, a slice or a bitfield depending on the genre — so the style
cascade, the validator and the editor support are written once.

The premise: mermaid-class tools auto-place everything and you fight the
engine. Here **layout is declared** (relative anchors between nodes) and
**style is a real cascade**. See `docs/language.md` for the syntax and
`diagram.bd` for a worked example.

**Where this is going** (`D-014`): archetypes covering four audiences —
statistics, software, embedded and data — and, at the end of it, an
**embeddable Rust library** other programs call to generate documents on the
fly. `docs/INDEX.md` carries the surveyed archetype list and what each one
needs from the language.

Pipeline: `source → parser → AST → check → layout → render_svg`.
The language server reuses the same front half, so editor and CLI can never
disagree.

## Stack
- Language: Rust (edition 2024), package manager cargo
- Parsing: `pest` + `pest_derive`, grammar in `src/syntax.pest`
- CLI: `clap` · Errors: `anyhow` · LSP wire format: `serde_json`
- Tree-sitter grammar: `tree-sitter-bgraph/` (JS grammar + generated parser)
- Zed extension: `editors/zed/`, compiled to `wasm32-wasip1`
- SVG output and the watch server are hand-rolled — no dependency for either

## Setup commands
- `cargo build` — builds both `bgraph` and `bgraph-lsp`
- `cargo run --bin bgraph -- diagram.bd -o diag.svg`
- `cargo run --bin bgraph -- watch diagram.bd` → http://127.0.0.1:7878
- `cargo run --bin bgraph -- check diagram.bd` — diagnostics as `file:line:col`
- `cargo run --bin bgraph -- docs -o docs/reference.md` — regenerate the manual
- `cargo test` (86 tests: `src/tests.rs` + `#[cfg(test)]` in `src/lsp.rs`)
- `cargo clippy --all-targets` / `cargo fmt`
- `cd tree-sitter-bgraph && tree-sitter generate && tree-sitter test` (11 corpus cases)
- `cd editors/zed && cargo build --target wasm32-wasip1`
- `./editors/zed/install.sh` — installs the server, regenerates + pins the grammar

## Architecture
| Path | Purpose |
|---|---|
| `src/lib.rs` | Library root; `render_source` is the whole pipeline |
| `src/main.rs` | `bgraph` binary: render, `watch`, `check` |
| `src/bin/bgraph-lsp.rs` | `bgraph-lsp` binary: thin wrapper over `lsp::run` |
| `src/syntax.pest` | Grammar |
| `src/parser.rs` | pest pairs → AST, with spans and friendly error text |
| `src/ast.rs` | Untyped tree; `Span` / `Spanned<T>` on every declaration |
| `src/props.rs` | **The vocabulary** — properties, archetypes, palettes, docs |
| `src/charts.rs` | The `pie` and `packet` archetypes |
| `src/docgen.rs` | `docs/reference.md`, generated from the `props.rs` tables |
| `src/check.rs` | **The validator** — spanned diagnostics, one source of truth |
| `src/style.rs` | Style cascade, `extends` flattening, `GraphOpts` |
| `src/layout.rs` | Anchor solver → absolute `Rect`s |
| `src/render_svg.rs` | Shapes, edge routing, markers, text |
| `src/watch.rs` | Dependency-free HTTP live preview |
| `src/lsp.rs` | Language server + `LineIndex` (byte ↔ UTF-16 position) |
| `tree-sitter-bgraph/` | Tree-sitter grammar, corpus tests, generated parser |
| `editors/zed/` | Zed extension: manifest, wasm shim, `.scm` queries |

## Conventions
- Property **values stay raw in the AST** (`ast::Value`) and are interpreted in
  `style.rs` — a new property touches `props.rs` and one `match` arm
- `props.rs` is the vocabulary; `check.rs` enforces it; `style.rs` interprets
  it. A test (`every_documented_property_is_understood_by_the_style_engine`)
  fails if they drift apart — keep it that way
- Style layers are all-`Option` and combined with `Style::overlay`
- Unknown property / shape / class / router names are **hard errors**; a
  silent no-op would hide typos in a file you can't compile-check
- Comments on non-obvious code only

## Patterns
- ALWAYS: add validation to `check.rs`, never to `layout.rs`. `layout::solve`
  calls `check::first_error` and its own `bail!`s only guard invariants that
  check has already established
- ALWAYS: XML-escape anything from the source before it reaches the SVG
  (`render_svg::esc`) — labels are user text
- ALWAYS: derive the viewBox from solved bounds; never hardcode a canvas
- ALWAYS: mirror a grammar change in **both** `src/syntax.pest` and
  `tree-sitter-bgraph/grammar.js`, add a case to `tests/grammar/accept` or
  `tests/grammar/reject`, and let `the_tree_sitter_grammar_agrees_with_pest`
  prove they still accept the same language
- ALWAYS: keep completion quiet. It must not fire inside strings or comments,
  must not offer aliases, and must not offer a property the current archetype
  does not read. A suggestion that appears while you type prose is worse than
  no suggestion at all — that feedback is what drove the current rules
- ALWAYS: regenerate `docs/reference.md` after touching `props.rs`. A test
  compares the checked-in file to `docgen::reference()` and fails on drift
- NEVER: add a color to a chart by eye. The categorical palette in `props.rs`
  is validated (adjacent-pair CVD ΔE ≥ 8 in both modes); adding a ninth hue is
  not an option — `check` tells the author to merge the tail instead
- NEVER: use `#` for anything but hex colors in the DSL (comments are `//`)
- NEVER: let a node's placement depend on render-time state

## Memories & Lessons Learned

_Append via `/start learn <type>: <lesson>`. NEVER delete this section on update._

### Critical Warnings
- [2026-07-31] Zed pins a tree-sitter grammar to a **git revision**, even for a
  `file://` repository. The generated parser must therefore be committed, and
  `extension.toml`'s `rev` must move after every `grammar.js` change or Zed
  silently reuses the cached grammar. `editors/zed/install.sh` does both.

### Gotchas
- [2026-07-31] Comments are `//`, not `#`. pest applies `COMMENT` as implicit
  skipping everywhere, so a `#` comment marker swallowed every `#rrggbb`
  color. The two cannot coexist.
- [2026-07-31] `ident` deliberately forbids a **trailing** hyphen, otherwise
  `a->b` lexes as ident `a-` and the arrow never matches. Keywords like
  `right-of` still work because the hyphen is followed by a letter. The
  tree-sitter `identifier` regex encodes the same rule.
- [2026-07-31] `WHITESPACE` covers only spaces/tabs — newlines are statement
  separators. Any pest rule spanning several lines must consume `sep`; in
  tree-sitter, `extras` must not include `\n`.
- [2026-07-31] `{ fill: #111 stroke: #222 }` on one line is ambiguous: is
  `stroke` a value of `fill` or a new key? pest solves it with a
  `!(ident ~ ":")` lookahead; tree-sitter cannot, so `property_name` is a
  single token that **includes the trailing colon**. Highlight queries must
  account for that.
- [2026-07-31] A tree-sitter rule built from `seq('"', repeat(...), '"')` lets
  `extras` leak whitespace and comments inside string literals. `string` is one
  `token(...)` for that reason.
- [2026-07-31] The npm `tree-sitter-cli` package downloads its binary in a
  postinstall script, which is blocked here. Use `cargo install tree-sitter-cli`.
- [2026-07-31] Zed extensions default to edition 2021 in a fresh Cargo.toml;
  let-chains need edition 2024.

- [2026-07-31] Completion trigger characters must NOT include a space: it fires
  a popup on every word boundary and reads as an obstacle. `.` is a trigger,
  but only opens the style list when the character before it is whitespace —
  otherwise `0.55` and `..>` pop the menu too.
- [2026-07-31] For `pie`, node styles are resolved on `charts::chart_defaults()`
  rather than `Style::node_defaults()`. The latter always sets `fill`, which
  would make a declared color indistinguishable from the default and defeat
  the palette.
- [2026-07-31] `packet` emits one `PlacedNode` per row segment, not per
  declared field, so a field crossing a row boundary needs no renderer
  support. Only the first segment carries the label.

- [2026-07-31] The SVG surface is **explicit**: `GraphOpts::default()` sets
  `background: Some("#ffffff")`. It used to be `None`, so a file with no
  `background` exported a transparent SVG while the watch page painted its own
  color behind it — the preview and the export disagreed. `background: none`
  is the way back to transparency, and the preview shows a checkerboard so it
  is visible rather than faked.
- [2026-07-31] `check_contrast` must resolve node styles on the **same base**
  the archetype uses (`charts::chart_defaults()` for pie, `node_defaults()`
  otherwise) or it reports colors the renderer never emits.
- [2026-07-31] A Rust `r#"…"#` literal ends at the first `"#`, which every
  `fill="#rrggbb"` assertion contains. Use `r##"…"##` in tests that match SVG.

- [2026-07-31] A pest rule used inside a `!` predicate must be **atomic**.
  `keyword = _{ ("style"|…) ~ !(ALNUM|"_"|"-") }` silently never fired: pest
  inserts implicit whitespace inside a non-atomic rule, so after `style` it
  skipped the space and matched the `-` of `->` as an identifier character.
  `@{ … }` fixes it. The guard looked right and did nothing for an hour.
- [2026-07-31] `bgraph check`'s exit code covers **semantic** errors too, so it
  cannot stand in for "did this parse". Cross-grammar probing must compare
  `parser::parse` against `tree-sitter parse`, never the CLI's exit status.

### Workarounds

### Performance Notes
