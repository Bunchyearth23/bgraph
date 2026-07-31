# The bgraph language

A diagram is a plain text file. Five kinds of statement: `type`, `graph`,
`style`, `node`, and edges. Statements are separated by newlines (`;` and `,`
also work). Comments start with `//` — `#` is reserved for hex colors.

```
graph { background: #0b1120 }

style box { fill: #1e293b stroke: #38bdf8 1.5 }

node a "Alpha" .box at (0, 0)
node b "Beta"  .box right-of a gap 60

a -> b "carries"
```

---

## Archetypes

`type` picks the **genre** of diagram, once, at the top of the file. Without
it the file is `free` — the node-and-edge graph everything else in this
document describes.

```
type free       // implicit; nodes and edges placed by anchors
type pie        // slice a "A" 42
type packet     // field a "A" 0 7
type treemap    // box a "A" 42
type quadrant   // point a "A" 0.8 0.2
type bar        // series s 4 8 15
type xy         // series s 4 8 15
type sankey     // a -> b 40
type wave       // signal s "clk" "p...."
type gantt      // task a "A" 2026-01-01 5d
type class      // class a "A", then `- id: Uuid` on its own line
type er         // entity a "A", then a -> b "places" one many
type block      // block api "API" container { tech: "Rust" }
type timeline   // event launch "Launch" 2026-03-15
type state      // state a "A"
type sequence   // participant a "A"
```

Every archetype reads the *same* statements, under its own word: a box is a
`node`, a `slice`, a `field`, an `entity`. The style cascade, the diagnostics
and the editor support are identical in all of them. What changes is which
properties carry meaning — `check` tells you when one is in the wrong place.

The two sections below walk through `pie` and `packet` in prose.
**[`reference.md`](reference.md) is the exhaustive list** — every archetype,
every property, generated from the compiler's own tables.

### `type pie`

Each node is a slice and needs a `value`. Colors come from a fixed
categorical palette, stepped for a light or a dark `background`; a declared
`fill` overrides it.

```
type pie {
  title: "Traffic sources"
  radius: 130          // outer radius, default 130
  donut: 0.55          // inner hole as a fraction; 0 is a full pie
  start: 0             // angle of the first slice; 0 is twelve o'clock
  labels: percent      // percent | value | both | none
  legend: on           // on by default
}

node organic "Organic search" { value: 4820 }
node direct  "Direct"         { value: 2410 }
node paid    "Paid"           { value: 410 fill: #f43f5e }
```

The legend is on by default and slices too thin for text are left unlabelled
on purpose: identity must never rest on color alone, and an unreadable label
is worse than none.

Angles are hard to compare. `check` warns when a pie has fewer than three
slices — a bar or a single number reads better — and when it has more than
eight, which exhausts the palette; merge the tail into an `Other` slice
rather than repeating a hue.

### `type packet`

Each node is a field and needs `bits`, an inclusive range. Fields are laid
out in rows of `width` bits; one that crosses a row boundary is split across
rows and named once.

```
type packet {
  title: "TCP header"
  width: 32            // bits per row, default 32
  cell: 20 46          // width of one bit, height of a row
  ruler: on            // bit indices above each row
}

node sport "Source port"      { bits: 0 15 }
node dport "Destination port" { bits: 16 31 }
node seq   "Sequence number"  { bits: 32 63 }
```

`check` reports overlapping ranges as errors and unassigned bits between two
fields as warnings, so a header that does not add up says so.

Edges and placement are ignored by both chart archetypes — and reported, not
silently dropped.

Worked examples: [`examples/pie.bd`](../examples/pie.bd) and
[`examples/packet.bd`](../examples/packet.bd).

---

## Layout

Placement is explicit, but expressed **relative to other nodes** — so
inserting a node shifts everything anchored to it instead of forcing you to
renumber coordinates.

| Form | Meaning |
|---|---|
| `at (x, y)` | absolute top-left corner |
| `right-of <id>` | to the right of another node |
| `left-of <id>` | to its left |
| `below <id>` | underneath it |
| `above <id>` | over it |
| `gap <n>` | distance to the anchor (default: the graph `gap`) |
| `align <kw>` | cross-axis alignment against the anchor |

`align` takes `left` / `right` / `center` for vertical placements and
`top` / `bottom` / `center` (or `middle`) for horizontal ones. It defaults
to `center`.

```
node a "A" at (0, 0)
node b "B" right-of a gap 80 align top      // tops line up
node c "C" below a gap 40 align left        // left edges line up
```

**Anchors may point forward.** The solver runs repeated passes, so
`node a right-of b` before `node b` is declared works fine. A genuine loop
is reported as `anchor cycle between: a, b`.

**A node with no placement** flows from the previous one in source order,
using the graph's `flow` direction and `gap`. The first node lands at
`(0, 0)`. That makes a quick sketch as terse as mermaid:

```
node a "A"
node b "B"
node c "C"
a -> b -> c    // (write these as two lines: a -> b, then b -> c)
```

**Sizes** come from the label plus `padding`, unless you set them:
`{ size: 140 56 }`, or `{ width: 140 }` / `{ height: 56 }`.

---

## Styling

Styles are named blocks, applied with `.name`, and they compose.

```
style service {
  fill: #172554
  stroke: #3b82f6 1.5
  radius: 10
}

style store extends service {   // inherits everything above
  shape: cylinder
  fill: #0f2e26
}

node db "Postgres" .store { stroke: #10b981 }   // inline override
```

Two style names are **implicit**: `node` and `edge` apply to every node and
every edge without being referenced. Use them for your theme.

The cascade, weakest to strongest:

```
built-in defaults  →  style node / style edge  →  .classes (in order)  →  inline { }
```

An unknown property or an unknown `.class` is a hard error, not a silent
no-op — a typo surfaces the moment you save.

### Properties

The full table — every property, which of nodes and edges it applies to, its
syntax and its meaning — is in [`reference.md`](reference.md), generated from
the compiler's own tables so it cannot drift. The ones you reach for most:

| Property | Applies to | Values |
|---|---|---|
| `fill` | nodes | color |
| `stroke` | both | color, width, and/or `solid`\|`dashed`\|`dotted`, in any order |
| `radius` | nodes | number (corner rounding on `rect`) |
| `shape` | nodes | see below |
| `size` / `width` / `height` | nodes | numbers |
| `padding` | nodes | number (label inset, drives auto-size) |
| `font` | both | size, `"family"`, color, weight — in any order |
| `arrow` | edges | `solid` \| `open` \| `dot` \| `none` |
| `router` | edges | `straight` \| `ortho` |
| `shadow` | nodes | `on` / `off` |

Colors are `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` or an SVG color name
(`white`, `tomato`). **The fourth channel is the alpha and defaults to `ff`** —
there is no separate `opacity` property, because transparency belongs to the
paint that carries it:

```
node card "Card" {
  fill: #1e293b80      // half-transparent
  stroke: #38bdf8      // opaque border on a translucent fill
}
```

Numbers accept a `px` suffix; it is ignored.

### Shapes

`rect` · `stadium` (pill) · `ellipse` · `diamond` · `hexagon` · `cylinder`

Aliases exist where they read better: `box`, `rounded`, `circle`,
`decision`, `hex`, `database`.

---

## Edges

```
a -> b                      // arrow at the end
a <- b                      // arrow at the start
a <-> b                     // both ends
a -- b                      // plain line
a --> b                     // dashed, with arrow
a ..> b                     // dotted, with arrow
```

The token's dash pattern is only a default — `{ dash: solid }` overrides it.

An edge takes an optional label, classes and an inline block, exactly like a
node:

```
orders -> billing "charge" .critical { router: ortho, stroke: #f43f5e 2 }
```

`router: ortho` produces an elbow (right angles) instead of a straight line.
Endpoints are clipped against the real shape outline, so arrows meet
diamonds and ellipses on their edge, not on an invisible bounding box.

---

## Graph settings

```
graph {
  background: #0b1120   // canvas fill (also backs edge labels)
                        // omit it and the surface is #ffffff, explicitly;
                        // `background: none` gives a transparent SVG
  padding: 40           // margin around the drawing
  gap: 70               // default anchor distance
  flow: right           // right | left | down | up — for unplaced nodes
}
```

The canvas size is derived from the content: nothing is ever clipped, and
negative coordinates are fine.

---

## Labels

`"Order\nService"` splits into two centred lines. `\t` and `\"` also work.
A node with no label falls back to its identifier.

Label text is XML-escaped on output, so `<`, `&` and quotes are safe.

---

## Commands

```
bgraph diagram.bd -o diagram.svg    # render once
bgraph watch diagram.bd             # http://127.0.0.1:7878, reloads on save
bgraph watch diagram.bd --port 9000
bgraph check diagram.bd             # diagnostics only, no output file
bgraph docs -o docs/reference.md    # regenerate the reference manual
```

The watch page shows parse and layout errors in place, so a broken file
doesn't cost you the preview. Scroll to zoom, drag to pan, `f` to fit the
diagram, `0` for actual size; the view is kept across the reload that a save
triggers.

`check` prints `file:line:col: severity: message` and exits non-zero if any
error was found — the form editors and CI already understand:

```
diagram.bd:9:17: error: unknown style `.missing`
diagram.bd:12:8: error: unknown node `nope` 
diagram.bd:14:3: warning: `fill` has no effect on an edge
```

## Editor support

`bgraph-lsp` is a language server for this DSL, packaged for Zed in
[`editors/zed`](../editors/zed) along with a tree-sitter grammar. It reports
the same diagnostics as `check`, completes properties and values in context,
shows a node's resolved position on hover and jumps to declarations.

Any LSP client can use it — the binary speaks LSP over stdin/stdout with no
arguments.
