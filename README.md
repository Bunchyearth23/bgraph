# bgraph

Diagrams as text, where **you** decide the layout.

Mermaid and friends auto-place everything and leave you fighting the
engine. `bgraph` inverts that: every node says where it goes, but says it
*relative to its neighbours* — so you keep control without hand-editing
coordinates every time the diagram grows.

```bgraph
graph { background: #0b1120 gap: 70 }

style node    { fill: #1e293b stroke: #334155 1.5 font: 14 #e2e8f0 }
style service { fill: #172554 stroke: #3b82f6 1.5 }
style store extends service { shape: cylinder stroke: #10b981 1.5 }

node client  "Client"
node gateway "API\nGateway"   .service
node orders  "Order\nService" .service
node db      "Postgres"       .store
node cache   "Redis"          .store

layout {
  place client at (0, 0)
  place gateway right-of client gap 90
  place orders right-of gateway gap 90
  place db right-of orders gap 90
  place cache below db gap 55
}

client  -> gateway "https"
gateway -> orders
orders  -> db      "sql"
orders  -> cache   "read-through" { router: ortho }
```

```sh
bgraph init pie > traffic.bd       # a working file for any of the 29 specialised genres
bgraph examples/free.bd -o /tmp/free.svg # checked-in worked source/SVG gallery
bgraph diagram.bd -o diagram.svg   # …or .pdf, .png, .json — the extension picks
bgraph watch diagram.bd            # a browser preview, reloading on save
bgraph watch docs/diagrams         # …or a whole directory, as a build loop
bgraph check diagram.bd            # every mistake, with a caret under each
bgraph explain diagram.bd api      # why that box is where it is
bgraph diff old.bd new.bd          # what appeared, what left, what moved
bgraph layout diagram.bd           # freeze the placements into the file
bgraph fmt --migrate old.bd        # rewrite what a language change removed
```

## Start here

**Documentation utilisateur en français : [guide](docs/guide/index.md).**
Le site MkDocs ajoute une navigation, une recherche et la galerie des 30 types :

```sh
./docs.sh
```

Ouvrez <http://127.0.0.1:8000>. Pour produire le site statique dans `site/` :
`./docs.sh build`.
Voir [maintenir la documentation](docs/guide/documentation.md) pour Windows
et l'organisation des pages.

1. Create a file from a worked archetype:

   ```sh
   bgraph init free > architecture.bd
   ```

2. Render it, then keep the preview open while you edit:

   ```sh
   bgraph architecture.bd -o architecture.svg
   bgraph watch architecture.bd
   ```

3. Make it yours with a style and a relative placement — the opening example
   above shows both. Use `bgraph check architecture.bd` whenever a line does
   not draw as expected.

4. When the picture has data behind it, use `type pie`, `type bar`, `type xy`
   or one of the other specialised genres. For a calling program, start with
   the small Rust examples in [`src/lib.rs`](src/lib.rs); for a browser, use
   the [playground](playground).

The guided tour in [`docs/language.md`](docs/language.md) follows this same
order: source, placement, style, data, then composition.

## What you get

- **Relative anchors** — `right-of`, `left-of`, `above`, `below`, with
  `gap` and `align`. Forward references and insertions just work.
- **A real style cascade** — named styles, `extends`, implicit `node`/`edge`
  themes, inline overrides. Unknown properties are errors, not silence.
- **Shapes** — rect, stadium, ellipse, diamond, hexagon, cylinder, with edge
  endpoints clipped to the true outline.
- **Edge control** — straight routing, or orthogonal routing that goes **around
  the boxes in the way** rather than through them, four arrow heads, dash
  patterns, labels. Where there is no way round, the edge says so instead of
  pretending.
- **Self-sizing canvas** — the viewBox follows the content.
- **A declared grid** — `graph { grid: 4 3 32 48 }` and `at (col 3, row 2)`.
  Positions, gaps and waypoints use cells in grid mode; the checked-in
  [`examples/`](examples/) directory contains one `.bd` and one verified SVG
  for every archetype.
- **Constraints that never reorder** — `align`, `equal-width`,
  `equal-height`, `row` and `column`, inside `layout`. They adjust only what
  you declared; deciding the order stays your job.
- **Composition** — `include "house.bds"` for a shared palette,
  `include "core.bd" as core` for a diagram under a namespace, and
  `component`/`use` for a shape placed many times. No silent shadowing: a name
  in two files is an error naming both.
- **Text that belongs to no box** — `note`, and a `legend` derived from the
  style classes that asked to be in one.
- **Archetypes** — 29 specialised genres, plus `free`, reinterpret the same statements as
  chart data, reusing the whole style cascade. `top: 8` folds a long tail;
  `from: "sales.csv"` reads the rows from a table. `type bpmn` is the widest:
  tasks, events, gateways and data artefacts, with pools and lanes as groups.
- **Themes** — `--theme light|dark|print`, laid *underneath* what the file
  declares, so the one box you coloured on purpose survives.
- **Sketch mode** — `graph { sketch: on }`, and deterministic: the same file
  always produces the same file.
- **Outputs** — SVG, PDF (hand-written, no rasteriser), PNG behind a feature,
  the solved geometry as JSON, `--page a4` to tile a large diagram across
  sheets, `--embed-font` to carry the glyphs you used.
- **Accessible by default** — `description` becomes an SVG `<title>` in every
  genre, and `role="img"` with a label on the root.
- **Editor support** — a tree-sitter grammar and a language server, packaged
  for [Zed](editors/zed) and [VS Code](editors/vscode), and a few lines of
  configuration away in Neovim, Helix and Emacs — see
  [`editors/README.md`](editors/README.md). Diagnostics, context-aware
  completion, hover with resolved geometry, go-to-definition, outline — and a
  ` ```bgraph ` fence drawn in VS Code's Markdown preview.

## In a browser

The core is six dependencies of pure Rust and the SVG writer is hand-written,
so the renderer compiles to wasm nearly as it stands:

```sh
./playground/build.sh            # the wasm module, into playground/public/
cd playground && pnpm install && pnpm dev
```

Source on the left with a real editor, SVG on the right, diagnostics under
both, and the genre menu is the module's own example table rather than a copy
kept beside it. The wasm boundary is four
`extern "C"` functions over linear memory — no `wasm-bindgen`, which would be
a dependency and a build step for four functions whose job is to move bytes.

`include` and `from:` are refused there: both need a path to resolve against
and a page has no disk, which is why the wasm build exposes `render_source`
and nothing that touches a filesystem.

For a Rust integration, see the supported embedding boundary in
[`docs/API.md`](docs/API.md).

## In a Typst document

```sh
./typst/build.sh                 # → typst/bgraph.wasm
```

```typst
#import "typst/bgraph.typ": bg-diagram, bg-diagram-file
#bg-diagram-file(path("arch.bd"), width: 70%)
```

A diagram that does not parse **fails the build**, with the compiler's own
message — which matches what `check` does and beats shipping a document with a
hole where a diagram should be. See [`typst/README.md`](typst/README.md),
including the two-command form that needs no plugin at all.

## Export local sans les sources

Pour préparer un dossier installable sur une autre machine sans distribuer le
dépôt, lancez :

```sh
./export.sh
```

Le dossier `export/` contient les binaires Linux, Windows x86_64 et Windows ARM64, le paquet VS Code,
l'extension Zed et le module Typst. La compilation Windows doit être faite sur
une machine disposant du linker Windows. Sous Linux, `export.sh` installe et
utilise automatiquement `cargo-xwin` ; pour fournir vos propres exécutables,
passez `--windows-bin-dir /chemin/vers/binaires-windows`.
Sous Windows, l'installation de l'extension Zed se fait avec
`export/zed/install.ps1`.

## Other archetypes

`type`, declared once at the top of a file, picks the genre. `free` — nodes
and edges — is the default; the others read the same statements as data.

```bgraph
type pie { title: "Traffic sources" donut: 0.55 }

node organic "Organic search" { value: 4820 }
node direct  "Direct"         { value: 2410 }
node paid    "Paid"           { value: 410 }
```

```bgraph
type packet { title: "TCP header" width: 32 }

node sport "Source port"      { bits: 0 15 }
node dport "Destination port" { bits: 16 31 }
node seq   "Sequence number"  { bits: 32 63 }
```

Guided tour: [`docs/language.md`](docs/language.md).
Exhaustive reference (generated from the compiler's own tables):
[`docs/reference.md`](docs/reference.md).
Worked examples: `bgraph init <genre>` prints a complete file for any of the
29 specialised genres, plus `free`, and the reference reproduces every one of them. They live in
`src/examples.rs`, so the crate needs no directory of sample files to build.

## Build

CI builds both binaries and the library without default features on Linux,
macOS and Windows. These compilation jobs are separate from runtime parity.

Run `./tests/runtime/gate.sh --extensions` to compare diagnostics and SVG bytes
across the CLI, LSP, browser wasm, WASI, live preview, Typst and the packaged
VS Code Markdown renderer, and build the Zed shim. This requires Node 24,
Typst, `zip`/`unzip`, and Rust's `wasm32-unknown-unknown` and `wasm32-wasip1`
targets; npm installs the VS Code language client when needed. Omit
`--extensions` to check the library and compiler runtimes only. The gate also
builds the library without default features and repeats native processes and
fresh wasm instances to check deterministic output. It covers 17 focused
fixtures, SVG parity for all 30 archetypes and four native file-context cases.

`./tests/runtime/vscode-host.sh` additionally activates the extension in a real
VS Code host and compares its diagnostics with the CLI. It requires VS Code
and Xvfb and uses a temporary profile. The separate
[`zed-host.py` check](editors/zed/README.md#isolated-host-verification-linux)
loads the extension component and grammar in real Zed and compares its LSP
diagnostics with the CLI, also with an isolated profile.

```sh
cargo build --release        # bgraph and bgraph-lsp
cargo test
./editors/zed/install.sh     # editor support
```

## Licence

[bgraph Noncommercial License 1.0](LICENSE) — two pages, written for this
project and meant to be read.

**Noncommercial use is permitted**: personal work, study, research, hobby
projects, and use inside a charity, a school, a public research body or a
government. Running a shared instance for the people around you is use like
any other and is covered.

**Commercial use is not**, and needs a separate written agreement. Use by or
for a company in the course of its own work is commercial, whatever the
diagram is of and whether or not anything is sold.

**You may pass on a build** you were given — unchanged, free, with the licence,
to someone whose use is also noncommercial.

**You may not change it, and the source is confidential.** Access to this
repository is personal to whoever was given it, and carries no right to copy
the source, publish it, or pass it on.
