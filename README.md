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

node client  "Client"         at (0, 0)
node gateway "API\nGateway"   .service right-of client  gap 90
node orders  "Order\nService" .service right-of gateway gap 90
node db      "Postgres"       .store   right-of orders  gap 90
node cache   "Redis"          .store   below db         gap 55

client  -> gateway "https"
gateway -> orders
orders  -> db      "sql"
orders  -> cache   "read-through" { router: ortho }
```

```sh
bgraph diagram.bd -o diagram.svg
bgraph watch diagram.bd            # live preview on :7878, reloads on save
bgraph check diagram.bd            # diagnostics as file:line:col
bgraph docs -o docs/reference.md   # regenerate the reference manual
```

## What you get

- **Relative anchors** — `right-of`, `left-of`, `above`, `below`, with
  `gap` and `align`. Forward references and insertions just work.
- **A real style cascade** — named styles, `extends`, implicit `node`/`edge`
  themes, inline overrides. Unknown properties are errors, not silence.
- **Shapes** — rect, stadium, ellipse, diamond, hexagon, cylinder, with edge
  endpoints clipped to the true outline.
- **Edge control** — straight or orthogonal routing, four arrow heads, dash
  patterns, labels.
- **Self-sizing canvas** — the viewBox follows the content.
- **Archetypes** — `type pie` and `type packet` reinterpret the same
  statements as chart data, reusing the whole style cascade.
- **Editor support** — a tree-sitter grammar and a language server, packaged
  for [Zed](editors/zed) and [VS Code](editors/vscode), and a few lines of
  configuration away in Neovim, Helix and Emacs — see
  [`editors/README.md`](editors/README.md). Diagnostics, context-aware
  completion, hover with resolved geometry, go-to-definition, outline.

## In a browser

The core is four dependencies of pure Rust and the SVG writer is hand-written,
so the renderer compiles to wasm nearly as it stands:

```sh
./playground/build.sh --inline   # → playground/standalone.html, open it
```

Source on the left, SVG on the right, no server. The wasm boundary is four
`extern "C"` functions over linear memory — no `wasm-bindgen`, which would be
a dependency and a build step for four functions whose job is to move bytes.

`include` and `from:` are refused there: both need a path to resolve against
and a page has no disk, which is why the wasm build exposes `render_source`
and nothing that touches a filesystem.

## In a Typst document

```sh
./typst/build.sh                 # → typst/bgraph.wasm
```

```typst
#import "typst/bgraph.typ": diagram
#diagram(read("arch.bd"), width: 70%)
```

A diagram that does not parse **fails the build**, with the compiler's own
message — which matches what `check` does and beats shipping a document with a
hole where a diagram should be. See [`typst/README.md`](typst/README.md),
including the two-command form that needs no plugin at all.

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
Worked examples: [`examples/`](examples) — one complete file per archetype.

## Build

```sh
cargo build --release        # bgraph and bgraph-lsp
cargo test
./editors/zed/install.sh     # editor support
```
