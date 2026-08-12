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
default), `pie`, `packet`. Every archetype reads the same statement *form*, so
the style cascade, the validator and the editor support are written once — but
each has its own *word* and its own positional data:

    slice organic "Organic search" 4820      // type pie
    field sport   "Source port"    0 15      // type packet
    node  a       "A"              at (0, 0) // type free

Adding an archetype means adding `item` and `slots` to its `ArchetypeDef` —
neither grammar changes. `node` stays a universal synonym and positional data
is sugar for the named property, so the long form always works. See
`docs/vocabulary.md` (`D-015`, `W-029`).

The premise: mermaid-class tools auto-place everything and you fight the
engine. Here **layout is declared** (relative anchors between nodes) and
**style is a real cascade**. See `docs/language.md` for the syntax and
`bgraph init <genre>` for a worked file per archetype — they live in
`src/examples.rs`, not in a directory (`W-116`).

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
- CLI: `clap` (**optional** — the binaries declare `required-features = ["cli"]`,
  so a library or wasm consumer compiles no argument parser) · Errors: `anyhow`
  · LSP wire format: `serde_json`
- Tree-sitter grammar: `tree-sitter-bgraph/` (JS grammar + generated parser)
- Zed extension: `editors/zed/`, compiled to `wasm32-wasip1`
- SVG, PDF, the watch server, base64 and the TrueType subsetter are all
  hand-rolled — no dependency for any of them
- Optional, each behind its own feature: `png` (a rasteriser, `resvg` — the
  only dependency tree left), `completions`, `wasm`, `typst`

## Setup commands
- `cargo build` — builds both `bgraph` and `bgraph-lsp`
- `cargo run --bin bgraph -- init free -o /tmp/a.bd && cargo run --bin bgraph -- /tmp/a.bd -o a.svg` (`.pdf` picks the PDF writer)
- `cargo run --bin bgraph -- watch /tmp/a.bd` → http://127.0.0.1:7878
- `cargo run --bin bgraph -- watch a.svg` — the same viewer, on a picture
- `cargo run --bin bgraph -- check /tmp/a.bd` — diagnostics as `file:line:col`
- `cargo run --bin bgraph -- docs -o docs/reference.md` — regenerate the manual
- `cargo test` (555 tests: `src/tests.rs` + `#[cfg(test)]` in `src/lsp.rs`)
- `cargo build --no-default-features` — the four-dependency library. **A CI
  step, and it has been red before**: a `#[cfg]` on a type without one on its
  users compiles under `default` and not here
- `./playground/build.sh` — the wasm module, into `playground/public/`
- `cd playground && pnpm install && pnpm dev` — the playground (Svelte + Vite);
  `pnpm build` writes `playground/dist`
- `./typst/build.sh` — the module Typst's plugin protocol expects (its imports
  make it unloadable anywhere else, hence a second feature and a second file)
- `cargo clippy --all-targets` / `cargo fmt`
- `cd tree-sitter-bgraph && tree-sitter generate && tree-sitter test` — 15 cases
  in `test/corpus/bgraph.txt`, which pin the **tree shape**. Separate from
  `tests/grammar/` (32 accept + 6 reject), which pins that pest and tree-sitter
  accept the *same language*; `cargo test` runs that one
- `cd editors/zed && cargo build --target wasm32-wasip1`
- `./editors/zed/install.sh` — installs the server, regenerates + pins the grammar
- `./editors/vscode/package.sh` — builds `bgraph-<version>.vsix` (`--no-server` skips npm)

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
| `src/charts.rs` | Every chart archetype |
| `src/compose.rs` | `include` and `component` — files laid end to end, spans shifted, so one offset says which file *and* which line |
| `src/themes.rs` | `--theme`; a theme **is** a `.bds`, parsed by the same parser |
| `src/subset.rs` | TrueType subsetting for `--embed-font`. Conservative: ids are kept, unused outlines emptied |
| `src/render_json.rs` | The solved geometry, shape promised and values not (`D-028`) |
| `src/render_png.rs` | Raster, behind `png`, through `render_svg` so there is one picture |
| `src/diff.rs` | Two versions as one drawing, matched by identifier |
| `src/explain.rs` | Why a box is where it is — needs nothing from the solver |
| `src/relayout.rs` | `bgraph layout`: inserts into lines, never rewrites them |
| `src/report.rs` | The two forms of a diagnostic. The flat one is an interchange format and does not move |
| `src/init.rs` | `bgraph init <genre>`, generated from the same tables |
| `src/examples.rs` | **The worked file for every archetype, in the source.** Not a directory: `include_str!` made the crate unbuildable without one, and `init` makes one redundant. `docs/goldens.txt` pins what each renders to |
| `src/wasm.rs` | The wasm boundary. **The only `unsafe` in the crate**, and `forbid` is scoped around it |
| `src/docgen.rs` | `docs/reference.md`, generated from the `props.rs` tables |
| `src/check.rs` | **The validator** — spanned diagnostics, one source of truth |
| `src/style.rs` | Style cascade, `extends` flattening, `GraphOpts` |
| `src/layout.rs` | Anchor solver → absolute `Rect`s |
| `src/bpmn.rs` | `type bpmn` — the marks inside a shape (event glyph, gateway mark, task marker) and the boundary events that sit on a task's edge. Line segments, never a Unicode glyph: a `✉` is whatever the reader's font says it is |
| `src/ortho.rs` | `router: ortho` — A\* over a channel grid, so an elbow goes round a box. Integer costs, no hash container: the golden test is byte for byte |
| `src/render_svg.rs` | Shapes, edge routing, markers, text |
| `src/render_pdf.rs` | PDF output, hand-written — vector, so no rasteriser |
| `src/fmt.rs` | The formatter, and `--migrate`. Lexical, so comments and blank lines survive |
| `src/template.rs` | `{{holes}}` and the values a calling program binds |
| `src/watch.rs` | Dependency-free HTTP live preview, with pan and zoom |
| `src/lsp.rs` | Language server + `LineIndex` (byte ↔ UTF-16 position) |
| `tree-sitter-bgraph/` | Tree-sitter grammar, corpus tests, generated parser |
| `editors/zed/` | Zed extension: manifest, wasm shim, `.scm` queries |
| `editors/vscode/` | VS Code extension: TextMate grammar, LSP client, Markdown preview |
| `playground/` | The wasm module and a single-page playground |
| `typst/` | bgraph as a Typst plugin, plus the no-plugin form |

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
  check has already established. A diagnostic that needs the boxes *placed*
  goes in `check::after_layout`, which is the same validator's second pass —
  putting one in the solver would be the start of a second validator
- ALWAYS: XML-escape anything from the source before it reaches the SVG
  (`render_svg::esc`) — labels are user text
- ALWAYS: derive the viewBox from solved bounds; never hardcode a canvas
- ALWAYS: mirror a grammar change in **both** `src/syntax.pest` and
  `tree-sitter-bgraph/grammar.js`, add a case to `tests/grammar/accept` or
  `tests/grammar/reject`, and let `the_tree_sitter_grammar_agrees_with_pest`
  prove they still accept the same language
- ALWAYS: write an editor query against the **tokens** it is about, not the
  node types that hold them. `indents.scm` named `(block)` and silently missed
  `group_block`, so Enter behaved differently in two blocks that look the same;
  `(_ "{" "}" @end) @indent` cannot miss the next one. The same reasoning as the
  highlighter's (`X-014`)
- ALWAYS: when a tree-sitter node type is renamed, update
  `editors/zed/languages/bgraph/*.scm` in the same breath — the queries are
  part of the grammar's public surface, not decoration
- ALWAYS: a new archetype's item word must reach
  `editors/vscode/syntaxes/bgraph.tmLanguage.json` too. A stale `.scm` breaks
  Zed loudly; a stale TextMate grammar just stops colouring a word, which
  nobody notices — `the_textmate_grammar_knows_every_word_the_compiler_does`
  is what makes it loud
- ALWAYS: read an archetype's positional values with `NodeDecl::props_for(def)`,
  never `effective_props(def.slots)`. The shape follows the **word the author
  wrote** (`D-023`), and `def.slots` is only the primary word's. The free path
  had this wrong from `W-048` until `W-102`, invisible the whole time because
  `xy` was the only genre with a second word and `xy` is a chart
- ALWAYS: ask the archetype for **all** its words — `def.items()` — anywhere a
  declaration is looked for by keyword. Taking `def.item` alone means a box
  declared under a second word is invisible; that is what broke go-to-definition
  and anchor completion in `W-102`'s catch-up
- ALWAYS: when an archetype is meant to choose a `shape`, a `router` or a
  `head`, leave that field **unset in the base style** the way `topology` and
  `process` do. `Style::node_defaults()` and `edge_defaults()` already fill
  them, so an archetype's `style.x = style.x.or(…)` silently loses and the
  genre supplies nothing. Five archetypes have made this mistake now, twice on
  the same afternoon
- ALWAYS: keep completion quiet. It must not fire inside strings or comments,
  must not offer aliases, and must not offer a property the current archetype
  does not read. A suggestion that appears while you type prose is worse than
  no suggestion at all — that feedback is what drove the current rules
- ALWAYS: regenerate `docs/reference.md` after touching `props.rs`. A test
  compares the checked-in file to `docgen::reference()` and fails on drift
- NEVER: edit `docs/reference.md` by hand — prose included. It is generated,
  it says so in its own banner, and the edit fails the same drift test. Write
  the prose into `src/docgen.rs` and regenerate
- NEVER: add a color to a chart by eye. The categorical palette in `props.rs`
  is validated (adjacent-pair CVD ΔE ≥ 8 in both modes); adding a ninth hue is
  not an option — `check` tells the author to merge the tail instead
- NEVER: use `#` for anything but hex colors in the DSL (comments are `//`)
- NEVER: let a node's placement depend on render-time state

## Memories & Lessons Learned

_Append via `/start learn <type>: <lesson>`. NEVER delete this section on update._

### Critical Warnings
- [2026-08-12] **Every GitHub Release carries auto-generated source archives**
  and there is no way to turn them off. Fine while the repository is private,
  because the release audience is the source audience (`D-040`). If a build
  ever has to reach somebody without repo access, do not look for a flag —
  there isn't one; publish from a second repository that holds only
  artefacts.
- [2026-08-12] **The licence is `LICENSE`, written for this project**
  (`D-038`): noncommercial use, builds may be passed on unchanged, the source
  is confidential, no changes. Five manifests say
  `LicenseRef-bgraph-noncommercial-1.0` — it is on nobody's list, so do not
  invent an SPDX id for it. A test asserts each clause is still present:
  every one of them was somebody's answer to a question, and a heading going
  missing is that answer being dropped without anyone deciding to. It is
  drafted in the PolyForm *shape* and says so; never let it read as though it
  were one of theirs.
- [2026-08-11] **Releasing is one channel, and it is named.** `D-035` turned
  the GitHub Release on and left crates.io, the VS Code Marketplace, Open VSX
  and the Zed registry off — the workflow names all four and a test asserts it
  runs none of them. `LICENSE` still grants nothing, so the notes say the
  artefacts come with no permission to run them. Do not add a publish step to
  a second channel without a decision recorded first.
- [2026-08-11] **A picture checked in beside its source goes stale silently.**
  `docs/diagrams/` held nine `.svg` files; the day the renderer changed, eight
  were out of date and the manual illustrated a language it no longer
  described. The manual renders them through `typst/bgraph.typ` now
  (`./compile.sh` builds the module when missing) — so a diagram that stops
  parsing stops the build. Never re-add a rendered picture beside a `.bd`; a
  test refuses it.
- [2026-08-11] **A style's *name* decides whether it paints anything.** A fixed
  set — `props::THEME_STYLES` plus the genre's item word — is looked up by the
  renderer itself; everything else paints only where `.name` is written. Add a
  theme by adding it to that table, never by calling `registry.get` with a new
  string in one chart. Resolve a theme with `registry.get`, **not**
  `Registry::resolve`: resolve overlays `style node` onto everything, and a
  diagram that fills its boxes must not thereby fill its gridlines.
- [2026-08-11] **A block that holds statements is not a block that holds
  properties.** The grammar has two: `block` (properties, members) and
  `group_block` (properties *and* statements — `group`, `component`,
  `activate`). Anything reading a document lexically has to keep the two
  apart, or a container looks like a node and everything you can only do by
  declaring becomes invisible.
- [2026-08-11] **A helper that returns a short list is worse than one that
  fails.** `resolved_nodes` gave back nothing when the style registry would not
  build; seventeen callers walk it beside `diagram.nodes`, most silently
  checking nothing and one panicking on an index. If a function's contract is
  "one per node", it has to keep that contract on the failure path too — and a
  hostile-input corpus has to cross its axes, because the bug lived where
  "broken style" met "genre that indexes" and neither alone reached it.
- [2026-08-11] **`include_str!` makes a data directory a build dependency.**
  Thirty-one of them pointed at `examples/`, so deleting a directory of *sample
  files* stopped the crate compiling. Anything the code needs at build time
  belongs in the code; a directory is an output. The examples are
  `src/examples.rs` now and `docs/goldens.txt` pins what each one draws.
- [2026-08-11] **A sweep over a directory passes when the directory is empty.**
  Half the suite's coverage was `read_dir("examples")` loops with a
  `seen >= 25` floor — which is a guard, but the floor is the only thing that
  was guarding. Iterate a table the compiler knows about, and a test cannot
  quietly check nothing.
- [2026-08-11] **The golden test compares bytes and says nothing about
  validity.** An SVG with an `<ellipse>` closed by a `</path>` was checked in
  and passed every run: browsers draw it, so the preview looked fine, and only
  resvg refused — which meant `--png` failed on one genre and
  nothing else noticed. `every_rendered_example_is_well_formed_xml` is the
  guard. Any new writer needs the same question asked of it: does anything
  check that what we emit *parses*, or only that it has not changed?
- [2026-08-11] **A drawing helper that assumes one element breaks on the two
  shapes made of two.** `described` took the tag name from the first `<` and
  the closing `/>` from the last. A cylinder is a `<path>` and an `<ellipse>`.
  Wrap, do not sniff.
- [2026-08-11] **`Annotation::y` is the text's middle, not its baseline.** Four
  pixels above a wire still drew the label across it. Anything positioning an
  annotation relative to a line has to subtract half the font size too.
- [2026-08-11] **Go-to-definition looked for `node` and nothing else.** It
  searched the literal keywords `node` and `style`, so in the twenty-eight
  genres with a word of their own the jump silently did nothing — and returning
  `null` is also what "no such name" looks like, so it never read as a bug. Two
  guards now cover it, both proven by reverting the fix. The general rule is
  now a Pattern above: ask the archetype for `items()`, never assume one word.
- [2026-08-11] **A default already in place beats an archetype's `.or()`.**
  `Style::node_defaults()` sets `shape: Some(Rect)` and `edge_defaults()` sets
  `router: Some(Straight)`, so `style.shape = style.shape.or(…)` in an
  archetype does nothing at all. Every BPMN element rendered as a rounded
  rectangle and every sequence flow as a diagonal, both silently correct-looking
  enough to miss. The base style must leave the field unset — see the Pattern.
- [2026-08-11] **A polyline is drawn under the boxes, always was, and nobody
  needed otherwise until a mark had to go *inside* a shape.** A BPMN gateway's
  cross rendered behind an opaque diamond, which looks exactly like "the glyph
  code never ran". `Polyline::over` splits the pass in both writers; a test
  asserts the emission order rather than the picture.
- [2026-07-31] **A hard-coded list of keywords in the LSP goes stale silently.**
  `Context::at` matched a block's owner against five words, so all sixteen
  archetype item keywords read as *edges* — wrong completions in every genre
  but `free`, from `W-029` until it was looked at. Ask a structural question
  (does the statement carry an arrow?) rather than enumerating names. The same
  applies to `relevant_here`, now deleted: a filter listing what is special is
  a filter that forgets.
- [2026-07-31] **The example files were covered by nothing.** The suite named
  `pie.bd` and `packet.bd` by hand, so the fifteen added since could have been
  broken for weeks. `every_example_still_works` sweeps the directory — a list
  written by hand is a list that stops being true.
- [2026-07-31] **`check_props` validated enumerated words only for a hard-coded
  allow-list of property names.** Anything not on it — `kind`, `ticks`, `mark`,
  `numbered`, `stack`, `grid` — accepted any word silently, against `D-005`.
  The rule is now inverted: **strict by default**, with the compound paint
  properties (`stroke`, `border`, `color`) named as the exceptions, because a
  bare word there may be a named colour. A new enumeration is validated by
  construction; nobody has to remember.
- [2026-07-31] **`pest` parses by recursive descent, so nesting depth is stack
  depth.** About 1500 nested `{` overflowed the stack and aborted the process
  with `SIGABRT` — not a panic, so no caller could catch it, in a library meant
  to be embedded. `parser::check_nesting` now counts brace depth *lexically*,
  before pest runs, and refuses past `MAX_NESTING`. Any future grammar rule
  that can nest inherits this: the guard counts braces, so a nestable
  construct written with different delimiters would slip past it.
- [2026-07-31] **Never publish anything outward without being asked.** Adding
  crates.io metadata was taken as preparing to release, and it was not the
  Archmagos's decision to make. `Cargo.toml` carries `publish = false` and
  omits `license`/`authors` so `cargo publish` *refuses*, rather than relying
  on anyone reading a comment. Same rule for any registry, remote or service.
- [2026-07-31] **Check a command's exit code, never grep its output for a
  success word.** `cargo test 2>&1 | grep 'test result: ok'` matches the
  passing suites and hides the failing one; `tree-sitter test`'s summary line
  counts parses rather than matches. Both have already let a broken tree be
  reported as green. Run the command, then read `$?`.
- [2026-07-31] `bgraph docs` runs the **built** binary. Editing `docgen.rs` and
  regenerating without `cargo build` first writes the old manual and the drift
  test then fails on the next run — after the commit has gone out.
- [2026-07-31] **A `*_defaults()` base hides whether the author declared
  anything.** `node_defaults` always sets `fill`, `edge_defaults` always sets
  `stroke` — so `style.fill.unwrap_or(palette)` silently never reaches the
  palette. This has now bitten three times: the pie's slice colours, the
  quadrant's contrast check, the sankey's ribbons. Any archetype that wants to
  distinguish *declared* from *default* must resolve on `chart_defaults()`,
  which sets neither.
- [2026-07-31] `tree-sitter test`'s summary line — "failed parses: 0" — counts
  **parses**, not expectation matches. It says nothing about whether the trees
  are what the corpus claims, and reading it let seven expectations go stale
  across a rename. Only the **exit code** tells the truth;
  `the_grammar_corpus_expectations_still_hold` now asserts it.
- [2026-07-31] A `.scm` query naming a node type the grammar no longer has does
  **not** degrade gracefully: Zed fails to load the language and stops
  recognising `.bd` files entirely — no highlighting, no LSP, nothing. Renaming
  `node_declaration` to `item_declaration` in `W-029` broke it exactly this
  way and shipped. `the_editor_queries_match_the_grammar` now compiles every
  query against the generated parser on each `cargo test`.
- [2026-07-31] Zed pins a tree-sitter grammar to a **git revision**, even for a
  `file://` repository. The generated parser must therefore be committed, and
  `extension.toml`'s `rev` must move after every `grammar.js` change or Zed
  silently reuses the cached grammar. `editors/zed/install.sh` does both.

- [2026-08-10] **`def.slots` is the archetype's *primary* form, not the form in
  front of you.** Once an archetype has a second keyword (`D-023`), thirteen
  call sites reading `node.effective_props(def.slots)` would silently fold a
  `scatter`'s coordinates into `points` and draw the wrong plot — with no
  error, because both are lists of numbers. They all go through
  `NodeDecl::props_for(def)` now, which resolves from the **written keyword**.
  The method takes the whole `ArchetypeDef` rather than a slot list precisely
  so a caller cannot reach past it; that is the same shape of fix as deleting
  `relevant_here` — make the wrong thing unsayable rather than remembering not
  to say it.

- [2026-08-10] **`check_edges` validated the inline block only.** Positional
  edge data is folded into named properties by `effective_props`, and that
  folded form was never checked — so `a -> b nonsense` passed `check` in `er`
  as well as in `requirements`, which is `D-005` broken in exactly the form the
  language calls idiomatic. Any new validation over properties must run on the
  **folded** props, never on `.props`.

- [2026-08-10] **A hole could not stand where a name goes, and only the editor
  noticed.** `Template::parse` substitutes *before* parsing, so the CLI
  rendered `slice {{r.id}} "…"` happily — while both grammars refused it, so
  the file showed as a syntax error in Zed and VS Code with its highlighting
  collapsed. Anything the templating layer accepts must be accepted by the
  grammars too, or the editor and the renderer disagree about what a file is.
  `name = ident | placeholder` now, everywhere a name is declared or
  referenced — but **not** where the archetype goes, since the archetype is
  what makes the rest of the file checkable.

- [2026-08-10] **A scroll wheel's delta is not a number you can scale.** X11
  sends ±1 a notch; Wayland sends the axis in fractional pixels, about ±15.
  Multiplying that by a factor made one notch a 2.8× zoom in one direction and
  a *negative* scale in the other — which the clamp then displayed as 5%, so a
  wheel *up* shrank the picture to nothing. Read the sign, never the
  magnitude, and make the step multiplicative so one notch means the same
  thing at 20% as at 800%.

- [2026-08-10] **`resvg` does not consult fontconfig.** The default stack this
  project emits — `Inter, ui-sans-serif, system-ui, sans-serif` — is a *web*
  stack: two of those are CSS generics no font database has, and `Inter` is on
  few machines. So everything lands on the generic, and the generic was
  whatever usvg guessed, which is how the removed native window came out set
  in a face nothing else on the system uses. That resolver went with it; names
  the sans, serif and monospace families from what is actually installed.

### Gotchas
- [2026-07-31] `template::scan` is textual on purpose, so `{{…}}` written
  inside a **comment** is still a hole. Writing about the syntax in a `.bd`
  comment therefore invents holes; say `row.field` without the braces.
- [2026-07-31] The last line of a file carries no newline, so repeated copies
  of it run together into one statement. Any line-repeating transform has to
  put the separator back.
- [2026-07-31] `pick_tick` chose a calendar step from the **span alone**, so a
  ten-year timeline asked for forty quarterly labels in 800px and drew a
  smear. A tick step is a function of the span *and the room on the page*; it
  now takes how many labels fit. `gantt` had the same latent bug.
- [2026-07-31] Box sizing existed in three places — `natural_size`, the class
  compartment path, and `block`. The shape allowance (a cylinder's caps, a
  hexagon's slanted sides) lived only in the first, so a `block` cylinder drew
  its label on the curve. Extracted as `shape_allowance`; any fourth sizing
  path must call it.
- [2026-07-31] A `member` line (`- id: uuid`) runs to the **end of the line**,
  so it cannot share one with the closing brace. `class a "A" { - id: Uuid }`
  does not parse — and a doc example written that way is a doc that lies.
- [2026-07-31] Edge styles were resolved from `edge.props`, not
  `effective_props(edge_slots)`, so positional edge data never reached the
  cascade in the free solver. `sankey` did not notice because it builds its own
  layout in `charts.rs`. Any archetype with `edge_slots` on the free path needs
  the folded form.
- [2026-07-31] `#![warn(missing_docs)]` over the whole crate produced 290
  warnings, most of them on struct fields of the layout engine. Documenting
  them all would have been ceremony; the useful move was to *narrow the
  promise* — see `D-018`. `#[allow(missing_docs)]` on a `pub mod` item covers
  everything inside it, which is what makes the two tiers expressible.
- [2026-07-31] A `#[allow(...)]` on a `#[derive]`d struct does **not** cover
  the other items the macro emits. `pest_derive` emits `pub enum Rule`
  alongside the struct; silencing it needed a wrapping private module.
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

- [2026-07-31] Tree-sitter regexes have **no lookahead**. `color` cannot pin its
  length there (`#1234567` would lex as a six-digit color plus a stray `7`), so
  both grammars take any run of hex digits and `check` closes the set. The
  cross-check test found this — the second real divergence it has caught.
- [2026-07-31] There is no `opacity` property: alpha is the fourth channel of a
  color (`#38bdf880`), defaulting to `ff`. It is emitted as `fill-opacity`
  rather than inline eight-digit hex, which several SVG-to-PDF paths drop.
  Contrast checks must **composite** before measuring — a fill at alpha 20 is
  mostly its backdrop, and comparing against the nominal color reports text
  that is perfectly readable.

- [2026-07-31] A statement runs to its separator, and that is load-bearing:
  without it `slice a b` is ambiguous (data, or the next statement?) and
  tree-sitter refuses to generate. `node a node b` on one line is therefore
  one item with two stray values.
- [2026-07-31] "The first string is the label" is encoded in the tree-sitter
  grammar as a `choice`, not left to the reader — with a label, later strings
  are data; without one, data cannot begin with a string. Exactly one parse
  either way. pest gets it for free from PEG ordering.
- [2026-07-31] `pos_value` excludes the placement keywords in pest, or `at`
  and `right-of` would be eaten as bare-word positional data before
  `placement?` ever gets to look.

- [2026-07-31] A group is sized by its members, never the reverse — that is what
  keeps `D-001` intact. A group carrying a placement translates its subtree, so
  its members are `placed` before they are `settled`; an anchor inside the same
  translation unit needs only `placed`, one reaching outside needs `settled`.
  Getting that distinction wrong deadlocks the solver instantly.
- [2026-07-31] `group_block` mixes properties and statements, so its entries
  need a separator for the same reason statements do. pest and tree-sitter
  both read `padding: 22 node a` as one four-valued property — they agree, and
  the corpus case sits in `accept/` saying so.

- [2026-07-31] `type treemap` reads its hierarchy from **group nesting**, not
  from edges — the first archetype to consume `W-011`. A group's area is the
  sum of what it holds, so declaring a value on one is an error rather than a
  second, contradictory truth.
- [2026-07-31] Treemap colour identifies the **top-level branch only**; depth
  darkens the same hue rather than inventing one, because a new hue would
  claim to be a new category. Slots are taken in order and never cycled.

- [2026-07-31] `check::base_style(archetype)` exists because contrast checks
  kept comparing text against a fill the renderer never paints. `free` and
  `packet` draw boxes and take `node_defaults`; every chart archetype takes
  `chart_defaults`. Adding an archetype means adding it there too, or its
  labels will all be reported as unreadable.
- [2026-07-31] `Guide` is a plain line in `Layout`, drawn under everything.
  Introduced for the quadrant's frame and cross, but it is the primitive
  `W-012` will draw axes and gridlines with — not a quadrant-only device.

- [2026-07-31] In a wave string, `=.=` is **two** values and `=..` is one held
  for two periods. Merging runs on equal levels alone silently joins them —
  the expansion has to remember that a `.` *was* a repeat.
- [2026-07-31] A waveform transition straddles the period boundary: pull the
  previous point back by half a slew rather than appending one behind it, or
  the polyline doubles back and renders as a spike.

- [2026-07-31] `props::VARIADIC` is `usize::MAX`, and `docgen` used to render a
  slot by repeating its name `arity` times — which allocated a vector of
  `usize::MAX` strings and aborted. A variadic slot is *written* `<points> …`,
  never repeated.

- [2026-07-31] The watch page is one `format!` template, so **every CSS and JS
  brace in it is doubled**. A missed pair reaches the browser as `{{`, which is
  a syntax error in both languages — and the page still serves, so nothing
  else notices. `the_template_leaves_no_doubled_braces_behind` checks it.
- [2026-07-31] Pan and zoom transform a wrapper `<div>`, never the SVG: what
  is on screen stays exactly what the file exports. The view is persisted in
  `sessionStorage` because the page reloads on every save, and a view that
  resets each time makes zooming useless while editing.

- [2026-07-31] A `group` records `members` (items) **and** `edge_members`
  (indices into `Diagram::edges`). A sequence frame spans message rows, not
  participants, so counting only `members` called every `loop` block empty.

- [2026-07-31] `type state` is the first archetype that does **not** take over
  placement: the anchors still decide, and it supplies only the vocabulary,
  the shapes and the diagnostics. Adding an archetype does not oblige you to
  write a layout engine.
- [2026-07-31] Edge-label penalties are **proportional to the overlapped
  area**, not flat. A flat charge made a five-pixel clip as expensive as
  sitting on a node, and labels fled three rows to escape it — where they read
  as belonging to another edge.

- [2026-07-31] Every test that shells out to `tree-sitter` takes
  `tree_sitter_lock()` first. The CLI rebuilds the parser library into a
  shared path, and cargo's parallel test runner had two of them catching it
  mid-write — the corpus guard failed spuriously twice before this.

- [2026-07-31] A member line opens with `- ` or `+ `, **space included**. That
  space is what keeps `-40` a number, and the marker is what lets properties
  keep sharing a line — the first attempt used a bare string and needed a
  separator rule, which broke `{ fill: #111 stroke: #222 }`.
- [2026-07-31] `Guide`s are drawn beneath the boxes. Anything that must appear
  **over** a node — a class's compartment rule — is a thin `PlacedNode` pushed
  after it, not a guide.

- [2026-07-31] `date` and `duration` are lexed **before** `number`, or
  `2026-07-31` reads as three numbers and `12d` as a number plus an
  identifier. Civil-calendar arithmetic is twenty lines in `ast.rs`
  (Hinnant's algorithm) rather than a dependency.
- [2026-07-31] Every archetype clamps its `size`. A test asking for less than
  the floor silently gets the floor — which cost an hour of reading the wrong
  end of a division.

- [2026-07-31] A `{{hole}}` **parses**: it is a `Value::Placeholder`, not a
  pre-parse text hack. Otherwise every line of a template lights up in the
  editor. `check` skips the validation that depends on an unbound value, via
  `awaits_binding`.
- [2026-07-31] Substitution is context-aware. Text landing inside a string
  literal is escaped for one; text landing in a value position is quoted.
  Blind replacement lets a stray quote turn a label into different statements.
- [2026-07-31] `Template::parse` probes with **two** plausible bindings (`0`
  and an identifier), because no single one fits both a value position and an
  identifier position.

### Workarounds

### Performance Notes
