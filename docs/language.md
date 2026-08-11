# The bgraph language

A diagram is a plain text file. Five kinds of statement: `type`, `graph`,
`style`, `node`, and edges. Statements are separated by newlines (`;` and `,`
also work). Comments start with `//` — `#` is reserved for hex colors.

```bgraph
graph { background: #0b1120 }

style box { fill: #1e293b stroke: #38bdf8 1.5 }

node a "Alpha" .box at (0, 0)
node b "Beta"  .box right-of a gap 60

a -> b "carries"
```

---

## Composing files

`include` merges another file whole, resolved relative to the file that wrote
it.

```
// main.bd — a bgraph-tagged fence promises one complete file, and this is
// three. A test writes this set out and renders it.
include "house.bds"          // a shared palette: styles only
include "core.bd" as core    // a diagram, under a namespace

node ui "Browser" .card
ui -> core.api "HTTPS"
```

Without `as`, every declared name must be unique across the whole set — a
clash is an error naming **both** files, never a silent shadowing. With `as`,
the included file's names arrive under a prefix and a clash becomes
impossible: `core.api` refers into it, and the file's own internal edges and
anchors are renamed with it, so prefixing never breaks the file it prefixes.

A qualified name is one token. That is what keeps `.card` unambiguously a
class reference: the lexer takes the longest match, so the dot in `core.api`
is never seen on its own.

A `.bds` holds `style` declarations and nothing else — that is what the
extension promises, and a file that could also declare nodes would be a `.bd`
under a misleading name.

A circular include is an error naming the chain. Diagnostics point at the file
and line that wrote the statement, not at the file that pulled it in.

The library keeps the split: `render_source` takes text and refuses an
`include`, because it has no path to resolve one against — the wasm build and
the semver promise both rest on it doing no I/O. `render_file` takes a path
and resolves them.

### Components

A shape declared once and placed many times. A component *is* an include of a
file you did not have to make: the same prefixing, so two placings cannot
collide.

```
component service {
  node app "{{label}}"
  node db  "Database" below app gap 34
  app -> db "reads"
}

use service as web
use service as jobs right-of web gap 90 { label: "Jobs" }

web.app -> jobs.app "enqueues"
```

The body goes through the same template engine a file with holes does, so a
binding is escaped for wherever its hole sits — a label with a quote in it
cannot become a statement. A component's holes belong to the component, not to
the file: they are bound by each `use`, and a file with a component is not
thereby a template.

**An instance is a frame.** That is what makes it placeable: a group is
already anchorable and already sized by what it holds, so `use service as jobs
right-of web` needs no placement vocabulary of its own. `web` names the
instance; `web.app` names a box inside it.

Deliberately no conditionals and no loops. Past that it is a macro system, and
a much larger promise than this makes — the library fills a table for the
cases that need one.

`bgraph init free` prints a working file to start from; this snippet is
complete on its own and the suite renders it.

### Notes

Text that belongs to no box — a caption, a warning, an aside:

```bgraph
style caption { font: 11 #b45309 }

node api "API"
node db "Database" below api gap 60

note "Figures are March; April is not in yet." .caption right-of api gap 30
note "Replica, not the primary." below db gap 14
```

It is placed by the same anchors as everything else, so it costs no new
placement vocabulary, and it takes a style class the same way a box does.

A note **declares no name**. That is the design rather than an omission: it is
not a thing the diagram can point at, so there is nothing for an edge to land
on and no way to write one that tries. An anchor that names nothing is an
error, as everywhere else.

Notes are solved last, against the boxes as they finally sit — so a note
anchored to something a constraint moved moves with it.

### A legend

The chart archetypes build a key from their own series. Every other genre had
no way to declare one:

```bgraph
style healthy  { fill: #dcfce7 stroke: #16a34a legend: "Healthy" }
style degraded { fill: #fef3c7 stroke: #d97706 legend: "Degraded" }
style plain    { radius: 2 }

node api "API" .healthy
node jobs "Workers" .degraded below api

legend right-of api gap 70
```

It lists the style classes that declare a `legend:` text — nothing else. A
legend built from every class would list the ones that only round a corner,
and one built from the colours on the page would have nothing to call them.
So the file says which classes mean something, and the legend can only ever
describe a colour the file actually used.

The swatch carries the class's stroke as well as its fill, because a status
colour is usually a pale tint with the meaning in the border.

### Saying what something is

`description` is one property with one meaning: what this is, in a sentence.

```bgraph
graph { description: "How an order reaches the warehouse." }

node cart "Cart" { description: "What the customer has chosen but not paid for." }
```

Every genre emits it as an SVG `<title>` — a tooltip to a reader with a mouse,
a label to one with a screen reader — and it also reaches the JSON export.
`type block` additionally *draws* it, under the technology line, because that
genre has room for it.

At the diagram level it becomes `aria-label` on the SVG root, beside
`role="img"`. Without those an SVG is announced as a group of anonymous shapes
or skipped entirely. A chart's own `title` serves as the label when no
`description` is given, so a pie chart names itself without being told twice.

### Where the heading goes

```bgraph
graph { title: "Order to cash" title-align: center }

node a "A" at (0, 0)
node b "B" at (400, 0)
```

`left` (the default), `center` or `right`. **One setting for every genre**: a
chart names itself in its own block and everything else in `graph`, but *where*
the name goes is the same question either way, so it is asked once. The default
is spelled rather than left as the absence of a word, so a theme can put it
back.

The heading is positioned once the drawing's width is known, and measured
against the drawing **without** it — measuring with it would let a title wider
than its diagram widen the diagram, which would move the centre, which would
move the title.

### Named themes

`--theme light`, `--theme dark`, `--theme print`. A theme **is** a style
file — the same `.bds` source, parsed by the same parser — laid *underneath*
what the diagram declares, so `--theme dark` on a file that colours one box
red gives a dark diagram with a red box. A theme that overrode the file would
make the flag destructive rather than useful.

The theme also supplies a `background`, but only when the file did not, for
the same reason.

`print` is the one worth knowing about: black and white, every distinction
carried by the stroke, so it survives a grayscale printer.

---

## Archetypes

`type` picks the **genre** of diagram, once, at the top of the file. Without
it the file is `free` — the node-and-edge graph everything else in this
document describes.

```
type free       // implicit; nodes and edges placed by anchors
type pie        // slice a "A" 42
type packet     // field a "A" 0 7        — bits
type kanban     // group todo "To do" { card a "A" }
type journey    // step pay "Pay" 2 4        — one score per actor
type radar      // series a "A" 92 48 70     — one per axis
type rack       // device srv "Compute" 38 41 — U38 to U41; a group is a rack
type network    // host fw "Firewall" firewall
type venn       // set a "A" 210, then a -> b 85
type harness    // connector j1 "J1" { - 1: +12V }, then j1 -> j2 1 3
type gitgraph   // commit c1 "init" "main", then c1 -> c2
type orgchart   // position cto "CTO", then cto -> lead
type mindmap    // topic root "Root", then root -> branch
type treeview   // entry src "src/", then src -> lib
type bytefield  // field a "A" 0 3        — bytes
type treemap    // box a "A" 42
type quadrant   // point a "A" 0.8 0.2
type bar        // series s 4 8 15
type xy         // series s 4 8 15, or scatter s 1 4 3 8
type sankey     // a -> b 40
type wave       // signal s "clk" "p...."
type gantt      // task a "A" 2026-01-01 5d
type class      // class a "A", then `- id: Uuid` on its own line
type er         // entity a "A", then a -> b "places" one many
type requirements // requirement r "R", then r2 -> r derive
type block      // block api "API" container { tech: "Rust" }
type timeline   // event launch "Launch" 2026-03-15
type state      // state a "A"
type sequence   // participant a "A"
type bpmn       // task t "Check" service, event e "Got it" start message,
                //   gateway g "OK?" exclusive, data d "Form" object
```

Every archetype reads the *same* statements, under its own word: a box is a
`node`, a `slice`, a `field`, an `entity`. The style cascade, the diagnostics
and the editor support are identical in all of them. What changes is which
properties carry meaning — `check` tells you when one is in the wrong place.

### A second word for a second shape

An archetype may have **more than one word**, when the same picture can be
described by data of a different shape. `xy` is the smallest case of it:

```bgraph
type xy { size: 400 200 legend: off }
series north 4 8 15 16 23        // spread evenly across the axis
```

```bgraph
type xy { size: 400 200 legend: off }
scatter north 1 4  3 8  7 15  9 23   // read two at a time: (1, 4), (3, 8) …
```

The word decides the shape — never the number of values you wrote. Two shapes
of the same length would be indistinguishable, and the error message could not
tell you which one it thought you meant.

A plot is one or the other. `series` takes its x from a mark's *position in the
line*, `scatter` from the numbers written; putting both on one axis would give
x two meanings at once, so it is refused rather than blended. A scatter also
draws dots by default — joining scattered points would claim a continuity the
data does not have — and `mark: line` sorts them by x first, so the line runs
left to right whatever order you wrote them in.

`bpmn` is the widest case: **four** words, because a task, an event, a gateway
and a data artefact are four different things carrying four different shapes.

```bgraph
type bpmn { title: "Order to cash" }

event   got   "Order received" start message
task    check "Check credit"   service right-of got gap 44
gateway ok    "Approved?"      exclusive right-of check gap 44
data    form  "Claim form"     object below check gap 40
event   done  "Approved"       end right-of ok gap 44

got   -> check
check -> ok
ok    -> done "yes"
check -> form association
```

A task's kind, an event's stage and trigger, a gateway's gate and a datum's
artefact are all drawn as marks: the shape says what the element *is* and the
mark says which one. A `group` is a pool or a lane. Because the shape carries
the meaning, the colour is free for whatever you are tracking.

A **boundary event** sits on a task rather than in the flow, and says so by
naming it — `event late "14 days" boundary timer { attach: pay }`. That is
declared placement like every other: you say which box, and where on it is a
fixed rule rather than something solved. Several on one task spread along its
edge in the order you wrote them.

Sequence flows are orthogonal by default and route around the boxes in the way;
a message flow is dashed and straight, an association dotted and headless. Two
limits worth knowing: a lane's name is drawn horizontally rather than turned on
its side, and a message flow carries no circle at its source.

The two sections below walk through `pie` and `packet` in prose.
**[`reference.md`](reference.md) is the exhaustive list** — every archetype,
every property, generated from the compiler's own tables.

### `type pie`

Its elements are `slice`s, and the value comes straight after the label.
Colors come from a fixed categorical palette, stepped for a light or a dark
`background`; a declared `fill` overrides it.

```bgraph
type pie {
  title: "Traffic sources"
  radius: 130          // outer radius, default 130
  donut: 0.55          // inner hole as a fraction; 0 is a full pie
  start: 0             // angle of the first slice; 0 is twelve o'clock
  labels: percent      // percent | value | both | none
  legend: on           // on by default
}

slice organic "Organic search" 4820
slice direct  "Direct"         2410
slice paid    "Paid"            410 { fill: #f43f5e }
```

The legend is on by default and slices too thin for text are left unlabelled
on purpose: identity must never rest on color alone, and an unreadable label
is worse than none.

Angles are hard to compare. `check` warns when a pie has fewer than three
slices — a bar or a single number reads better — and when it has more than
eight, which exhausts the palette.

A pie can also read its slices from a table:

```bgraph
type pie { title: "Traffic sources" from: "sales.csv" }
```

```
name,value
Organic search,4820
Direct,2410
```

Columns are matched by **header name**, not by position, so a table whose
columns are in a different order still reads correctly and one missing a
column says which. The path resolves relative to the file that wrote it.

This carries a consequence worth knowing rather than discovering: a diagram
that reads the disk at render time is **no longer autonomous**. `check` can
pass on the text and the render still fail because the table moved — so the
diagnostic names both files. The template engine remains the answer when the
*shape* varies; `from` is for when only the rows do.

`top: 8` takes that advice for you: the largest eight are kept and the rest
are folded into one slice, named `"Other"` unless `other:` says otherwise.
The total stays honest, and the fold is the same on every run — sorted by
value, ties broken by the order written.

### Activation bars in `type sequence`

The box down a participant's lifeline, for as long as a call is outstanding:

```bgraph
type sequence
participant browser "Browser"
participant api "API"

browser -> api "POST /checkout"
activate api {
  api -> browser "202 Accepted"
}
```

`activate` is `group` under another word. Its block holds the messages the bar
spans, exactly as a `loop` frame's does — and its identifier names a
participant that is **already declared**, rather than declaring a new one.

The bar is not inferred from a call and its reply. "The matching reply" is a
guess the moment two calls are in flight, and a guess about which one closed
is worse than writing a block.

### `type packet`

Its elements are `field`s, and the inclusive bit range comes straight after
the label. Fields are laid out in rows of `width` bits; one that crosses a row
boundary is split across rows and named once.

```bgraph
type packet {
  title: "TCP header"
  width: 32            // bits per row, default 32
  cell: 20 46          // width of one bit, height of a row
  ruler: on            // bit indices above each row
}

field sport "Source port"       0 15
field dport "Destination port" 16 31
field seq   "Sequence number"  32 63
```

`check` reports overlapping ranges as errors and unassigned bits between two
fields as warnings, so a header that does not add up says so.

Edges and placement are ignored by both chart archetypes — and reported, not
silently dropped.

**`node` still works everywhere** as a universal synonym for the archetype's
own word, and every positional value can be written as a property instead —
`slice a "A" 10` and `slice a "A" { value: 10 }` are the same statement. The
positional form is the idiomatic one.

Every archetype ships a complete, working file: they are in
`bgraph init <genre>`, and each one is reproduced in
[`reference.md`](reference.md) under its genre.

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

### Sketch mode

```bgraph
graph { sketch: on }
```

A sketched architecture says *this is a proposal* where a crisp one says *this
is decided* — information rather than ornament, and the reason it is worth
having at all.

It is **deterministic**. The wobble is a hash of each shape's own geometry, so
the same file always produces the same file: the golden test would refuse
anything else, and a diagram that redrew itself on every save would be
unreviewable. Two boxes in different places deform differently, which is what
makes it look drawn rather than stamped.

Only the rectangles are wobbled. A diamond drawn freehand stops reading as a
decision and a circle stops reading as a circle — a shape whose meaning is its
outline is not one to wobble.

### A grid

Declare a step once and place in cells:

```bgraph
graph { grid: 170 110 }

node a "Ingest" at (col 0, row 0)
node b "Queue"  at (col 1, row 0)
node c "Store"  at (col 1, row 1)
```

`at (col 3, row 2)` is still `at`, in units the file declared — a coordinate
system, not a solver. Nothing is snapped to it: a box placed in pixels is
unaffected, and a grid that quietly moved a declared position would be exactly
the auto-placement this language refuses.

### Freezing a loose file

Write a diagram without placements, then run `bgraph layout` once and every
box says where it goes:

```
node ui   "Browser"          // the reader
node api  "API"
```

becomes

```
node ui   "Browser"          // the reader
node api  "API" right-of ui
```

It **inserts into lines** rather than rewriting them, so a comment, a column
lined up by hand and a trailing note all survive — `fmt` leaves interior
alignment alone and this goes further still.

**The picture does not change.** That is the whole promise, and it is checked
rather than asserted: the file is rendered before and after, and if the two
differ nothing is written. A box that already says where it goes is left
alone, and running it twice changes nothing the second time.

### Constraints

Three statements adjust what is already placed, and none of them reorders
anything:

```bgraph
node a "A"
node b "A much longer label" right-of a gap 40
node c "C" right-of b gap 40

same-width a b c
same-height a b c
align-group top a b c
```

`same-width` and `same-height` give a set one size — the **largest** wins,
because shrinking to match a narrower neighbour clips the label. They are
applied *before* placement, so widening a box pushes its neighbours along
instead of landing on top of them.

`align-group` gives a set one cross-axis coordinate. The **first** name
decides, not an average: an average moves every box, including the one the
others were being lined up against. The axis is read off the geometry — a row
lines up on `y`, a column on `x`.

```bgraph
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

```bgraph
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
| `label-offset` | both | two numbers — move the text by hand |
| `shadow` | nodes | `on` / `off` |

Colors are `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` or an SVG color name
(`white`, `tomato`). **The fourth channel is the alpha and defaults to `ff`** —
there is no separate `opacity` property, because transparency belongs to the
paint that carries it:

```bgraph
node card "Card" {
  fill: #1e293b80      // half-transparent
  stroke: #38bdf8      // opaque border on a translucent fill
}
```

A file written before that change says `opacity: 0.5` and now fails with
`unknown property`. **`bgraph fmt --migrate` rewrites it**: the fraction is
folded into the alpha of every paint in the same block, the line is cut out,
and nothing else in the file moves — comments and hand-aligned columns
included. It declines rather than guessing, naming the line, when a block has
no colour to carry the alpha or when the colour is a name like `tomato`, which
has no fourth channel to write into.

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

`router: ortho` produces right angles instead of a straight line, and it
**routes around the boxes in the way** rather than through them — shortest
first, then fewest corners among the paths of that length. It avoids *boxes*,
not other edges: two lines whose tidiest routes share a channel will share it.
Where there is no way round at all — a box wholly enclosed by others — the edge
falls back to a plain elbow and `check` reports that it crosses something,
because a drawing that admits it is wrong beats a missing edge.
`bgraph init free` and a `router: ortho` on an edge is the shortest way to see it.

### Telling a line where to go

Two controls, and they are different in kind. **`from` and `to`** name the side
a line leaves and arrives by; **`via`** names the places it goes through on the
way. The route between any two of those is still the engine's (`D-033`).

```bgraph
node a "A" at (0, 0)
node b "B" at (420, 0)
node c "C" at (200, 220)

a -> b "over the top"
a -> b "under" { router: ortho } via (left-of c gap 30) (below c gap 40)
a -> c { from: bottom to: left }
```

`from` and `to` take `top`, `bottom`, `left`, `right` or `auto` — `auto` is the
default and is spelled, so a class can put a side back to the engine's choice.
Lines arriving at one side of one box **spread along it** in declaration order
rather than converging on its midpoint; `lane: 2` reorders them by hand. A line
arriving and a line leaving are not in each other's way, so those are not
spread against one another.

`via` takes a **list**, and each stop is parenthesised — that is what lets the
list need no separator, since `,` already separates statements. A stop is one
of three things, written the same way:

| | |
|---|---|
| `(300, 120)` | a point. The escape hatch: `D-001` refuses absolute coordinates for boxes for the same reason they are brittle here |
| `(col 3, row 2)` | a cell of the grid `graph { grid }` declared — absolute in units you chose, so it survives a change of scale |
| `(left-of c gap 20)` | a place beside a box, in the anchor vocabulary the rest of the language uses. **The form that survives an insertion** |

### Arrow heads, and the three UML needs

`head:` sets the mark at the arrow's head end; `from-head:` and `to-head:` name
an end explicitly, which is what lets the two differ.

| | |
|---|---|
| `solid` `open` `dot` `none` | direction |
| `triangle` (`generalisation`, `extends`) | UML generalisation: a hollow triangle at the parent |
| `diamond` (`aggregation`) | UML aggregation: a hollow diamond at the whole |
| `composition` (`filled-diamond`) | UML composition: the same diamond, filled |
| `one` `many` `zero-one` `zero-many` `one-many` | crow's-foot cardinality, in `type er` |

```bgraph
type class
class vehicle "Vehicle"
class car "Car" below vehicle gap 60
class wheel "Wheel" right-of car gap 60

car -> vehicle { head: triangle }
car -> wheel { from-head: composition head: none }
```

The diamond belongs at the **whole**, which is nearly always the tail — that is
what `from-head` is for. Before it, a composition had to be written backwards
to be drawn right.

`triangle` used to be a third spelling of the solid arrow. It is the hollow
generalisation head now: a word spent on a shape the language already had, put
to a shape it did not.

### The shape of the line itself

```bgraph
node a "A" at (0, 0)
node b "B" at (300, 160)
node c "C" at (300, 0)

a -> b { router: ortho bend: 16 }
a -> c { router: curved }
c -> a "back"
```

`bend:` rounds an `ortho` elbow — a style on the router rather than a second
router, so every other property keeps meaning what it meant. `router: curved`
bows between the two ends. Both are **sampled into the polyline** rather than
emitted as arcs, so the bounds, the label solver, the PDF writer and the JSON
export all keep working on the one shape they already understand.

Two `ortho` lines no longer run along each other either. Edges are routed in
the order they are declared, and each remembers the runs it took; a later one
pays for overlapping them and takes another channel where one exists —
contention creates a lane either side of a run already taken, so there usually
is one. Where there genuinely is not, the two still share, because refusing
would mean a missing edge rather than a doubled line. Which edge gets the
direct channel follows the order you wrote them in.

Two lines between the same pair bow apart on their own, without a word: drawn
straight they lie on top of each other, and a reader sees one line. The bow's
side follows the *pair* and not the direction each was written in, or `a -> b`
and `b -> a` would lean the same way.

A waypoint is a **passage, not a preference**: the line goes through each one,
in the order written. What survives between them is the obstacle avoidance —
every leg is routed by the same solver, so a line told to pass left of a box
still goes around whatever stands between it and there. A waypoint that lands
inside a box is reported rather than moved, because moving it would change the
line you asked for.
Endpoints are clipped against the real shape outline, so arrows meet
diamonds and ellipses on their edge, not on an invisible bounding box.

### Moving a label

An edge label lands on the middle of its line and slides sideways when that
would put it on top of a node, another line or a label already placed. The
solver is good, not clairvoyant — on a busy diagram it eventually runs out of
room. `label-offset` is the escape hatch:

```bgraph
node worker "Worker"
node db "Postgres" below worker gap 80

worker -> db "read / write" { label-offset: -30 12 }   // left 30, down 12
```

Two numbers, in user units, positive being right and down; the second may be
omitted for a purely horizontal nudge. On an edge the offset is measured from
the middle of the line and it **pins** the label — the automatic avoidance
stops moving it, so the position you write is the position you get, and it
does not drift when a neighbouring edge changes.

The same property moves a node's label inside its shape, and a group's title
along its frame:

```bgraph
group dmz "DMZ" {
  label-offset: 0 -22                    // title above the frame, not inside it
  node gateway "Gateway"
}

node store "PostgreSQL" below gateway gap 80 {
  shape: cylinder
  label-offset: 0 8                      // clear of the cylinder's cap
}
```

Nothing is clipped for it: the viewBox follows a label that has been moved
outside the shape it belongs to.

---

## Graph settings

```bgraph
graph {
  title: "Order pipeline"  // heading drawn above the diagram
  background: #0b1120   // canvas fill (also backs edge labels)
                        // omit it and the surface is #ffffff, explicitly;
                        // `background: none` gives a transparent SVG
  padding: 40           // margin around the drawing
  gap: 70               // default anchor distance
  flow: right           // right | left | down | up — for unplaced nodes
}
```

`title` works in every genre. The chart archetypes also take a `title` in
their own settings — `type pie { title: … }` — and that one wins where both
are given, being the more specific of the two.

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
bgraph diagram.bd -o diagram.png    # …with the `png` feature; --scale 2 for twice the size
bgraph diagram.bd -o diagram.json   # the solved geometry, for another program
bgraph diagram.bd -o d.svg --embed-font Inter.ttf   # text exact on any machine
bgraph diagram.bd -o big.pdf --page a4  # tile a large diagram across sheets
bgraph diagram.bd --theme dark      # a named palette under the file's own styles
bgraph watch diagram.bd             # show it, and reload when it changes
bgraph watch diagram.svg            # …a picture works too; the extension decides
bgraph watch diagram.bd --port 9000
bgraph watch docs/diagrams          # a directory: build every .bd as it changes
bgraph watch docs/diagrams --once   # …once, non-zero on failure — a build step
bgraph check diagram.bd             # diagnostics only, no output file
bgraph check diagram.bd --ci        # …the flat form, whatever stdout is
bgraph diff old.bd new.bd -o d.svg  # what appeared, what left, what moved
bgraph explain diagram.bd api     # why that box is where it is
bgraph layout diagram.bd            # write the placements in; the picture is unchanged
bgraph report.bd --data q3.json     # fill a template from a JSON object
bgraph report.bd --data - --emit    # …from a pipe, printing the filled source
bgraph report.bd --rows r=sales.csv # one hole from the rows of a CSV
bgraph watch report.bd --data q3.json   # live preview, data re-read on save
bgraph fmt diagram.bd               # reformat to the canonical shape
bgraph fmt --check *.bd            # …or report what would change, and exit non-zero
bgraph fmt --migrate old.bd         # rewrite a removed `opacity` as the colour's alpha
bgraph init pie                     # a working file for a genre; no genre lists them
bgraph completions fish             # a completion script; bash|zsh|fish|elvish|powershell
bgraph completions man              # …or the man page, from the same flag definitions
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

`bgraph-lsp` is a language server for this DSL. It reports the same
diagnostics as `check`, completes properties and values in context, shows a
node's resolved position on hover and jumps to declarations.

Packaged for [Zed](../editors/zed) and [VS Code](../editors/vscode); Neovim,
Helix and Emacs each need a few lines of configuration, which
[`editors/README.md`](../editors/README.md) gives in full alongside where the
tree-sitter queries live.

Any LSP client can use it — the binary speaks LSP over stdin/stdout with no
arguments and no configuration file.
