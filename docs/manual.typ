// RAPPORT A4 — jamais une présentation. `bunchy-theme.with()` sans `presentation: true`
// est le mode document. Ne jamais ajouter `presentation: true`.
//
// Public : quelqu'un qui va écrire des fichiers .bd. Il doit pouvoir CONSTRUIRE.
// La référence exhaustive des propriétés reste générée ; ce document est le
// manuel qui apprend à s'en servir.
//
// Compilation : ./compile.sh docs/manual.typ

#import "btyp/btyp.typ": *

#show: bunchy-theme.with()

#v(1fr)
#cover(
  title: "bgraph",
  subtitle: "A diagram language where you decide the layout",
  author: "Soundnodes",
  date: "10 August 2026 · version 1.2",
)
#v(1fr)
#pagebreak(weak: true)

// ══════════════════════════════════════════════════════════════════════════════
#slide(title: "What it is, and what it will not do")[

#lead[
A diagram is a plain text file. It renders to SVG or PDF. Every box says where
it goes — but says it *relative to its neighbours*, so inserting one moves what
is anchored to it instead of making you renumber coordinates.
]

Mermaid and its relatives place everything for you and leave you fighting the
engine. bgraph inverts that trade. Two things follow from it, and everything
else in this manual is a consequence of one or the other.

#dtable(
  header: ([Principle], [What it buys], [What it costs]),
  columns: (auto, 1fr, 1fr),

  [*Layout is declared*],
  [The picture is stable. A diagram you arranged once stays arranged when you
   add to it.],
  [You place things. There is no button that guesses for you.],

  [*Style is a real cascade*],
  [Defaults, a theme, named classes and one-off exceptions compose, the way
   they do in a stylesheet.],
  [Four layers to hold in your head — which is why hovering a box in the editor
   reports what won.],
)

#v(0.8em)

*What it refuses to do.* These are not gaps waiting to be filled. They are
positions.

#dtable(
  header: ([It will not], [Because]),
  columns: (auto, 1fr),

  [Guess a layout for you],
  [That is the fight this language exists to end. An archetype may arrange its
   own items — a pie has no anchors to honour — but nothing overrides a
   placement you wrote.],

  [Silently ignore a name it does not know],
  [`fll: #fff` is an error, not a no-op. In a file with no compiler, a silent
   typo is a diagram that is quietly wrong.],

  [Colour a chart by eye],
  [The categorical palette is validated for colour-vision deficiency. Past
   eight series it tells you to merge the tail rather than repeating a hue.],

  [Emit a raster image],
  [SVG and PDF are vector and need no rasteriser. Anything that needs a bitmap
   can rasterise those with a tool built for it.],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "The five statements")[

That is the whole grammar. Statements end at a newline; `;` and `,` also
separate. Comments are `//` — `#` is reserved for hex colours, and the two
cannot coexist.

#two-col(
  [
```
type <archetype> { … }
graph { … }
style <name> [extends <name>] { … }
group <id> ["<label>"] [.class] [<where>] { … }
<word> <id> ["<label>"] <data…> [.class] [<where>] { … }
<id> <arrow> <id> ["<label>"] <data…> { … }
```

#v(0.4em)

#dtable(
  header: ([Statement], [What it declares]),
  columns: (auto, 1fr),

  [`type`], [The genre, once, at the top. Absent means `free`.],
  [`graph`], [Diagram-wide settings: padding, gap, background, flow.],
  [`style`], [A named, reusable block of properties.],
  [`group`], [A container, sized by what it holds.],
  [item], [A box, a slice, a field — the archetype decides the word.],
  [edge], [A link. `->`, `-->`, `..>`, `<->`, `--`, `..`],
)
  ],
  [
```
graph { background: #f8fafc }

style box { fill: #eef3ff stroke: #2563eb 1.5 }

node a "Alpha" .box at (0, 0)
node b "Beta"  .box right-of a gap 60

a -> b "carries"
```

#v(0.6em)

#callout(kind: "info", title: "Two things to notice")[
`node` works in every genre as a universal synonym, and every positional value
can be written as a property instead. `slice a "A" 10` and
`slice a "A" { value: 10 }` are the same statement — the positional form is the
idiomatic one.
]
  ],
  columns: (1.05fr, 1fr),
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Placement")[

#diagram(
  image("diagrams/anchors.svg", width: 76%),
  caption: [Four boxes, three of them placed against a neighbour.],
)

#v(0.3em)

```
node a "a"                    at (0, 0)
node b "right-of a"           right-of a gap 44
node c "below a gap 30"       below a gap 30
node d "right-of b align top" right-of b gap 44 align top
```

#v(0.5em)

#two-col(
  [
#dtable(
  header: ([Form], [Meaning]),
  columns: (auto, 1fr),

  [`at (x, y)`], [Absolute top-left corner.],
  [`right-of <id>`], [To the right of another item.],
  [`left-of` `above` `below`], [The other three directions.],
  [`gap <n>`], [Distance to the anchor. Defaults to the graph's `gap`.],
  [`align <kw>`], [Cross-axis alignment against the anchor.],
)
  ],
  [
`align` takes `left` / `right` / `center` for a vertical placement and
`top` / `bottom` / `center` for a horizontal one. It defaults to `center`.

*Anchors may point forward.* The solver runs repeated passes, so anchoring to
something declared further down works. A genuine loop is reported by name:
`anchor cycle between: a, b`.

*A box with no placement* flows from the previous one, using the graph's `flow`
and `gap`. The first lands at the origin — which makes a quick sketch as terse
as anything that places for you.
  ],
)

#v(0.5em)

#callout(kind: "warn", title: "A group is sized by its members, never the reverse")[
Give a group a placement and it translates everything inside it, once its
members have landed. Sizing a group first and fitting members into it would be
auto-layout by another name.
]
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Style is a cascade")[

#diagram(
  image("diagrams/cascade.svg", width: 92%),
  caption: [Each layer overwrites only what it mentions.],
)

#v(0.6em)

#two-col(
  [
```
style service {
  fill: #172554
  stroke: #3b82f6 1.5
  radius: 10
}

style store extends service {
  shape: cylinder
  fill: #0f2e26
}

node db "Postgres" .store {
  stroke: #10b981
}
```
  ],
  [
`db` ends up a cylinder with a green outline and a dark green fill: `radius`
came from `service`, `shape` and `fill` from `store`, `stroke` from the
statement itself.

*Two style names are implicit.* `style node` and `style edge` apply to every
box and every link without being referenced. Use them for the diagram's theme
and keep named classes for what differs.

*Classes apply in the order written.* `.a .b` lets `b` win, which is the only
reading that makes the list mean anything.

#v(0.4em)

#callout(kind: "info", title: "When the answer is not obvious")[
Hover a box in the editor. It reports the paint the cascade settled on — four
layers deep, and otherwise visible only by rendering the file.
]
  ],
  columns: (0.85fr, 1fr),
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Properties")[

Every property is valid on a `style` block, on a statement's inline block, or
both. Anything else is an error rather than a silent no-op. These are the ones
reached for most; the generated reference carries all of them, with the exact
syntax of each.

#dtable(
  header: ([Family], [Properties], [Notes]),
  columns: (auto, 1.1fr, 1fr),

  [*Paint*],
  [`fill` `stroke` `stroke-width` `dash` `shadow`],
  [`stroke` takes a colour, a width and a pattern in any order — omit whichever
   you do not need.],

  [*Geometry*],
  [`shape` `size` `width` `height` `padding` `radius`],
  [Six shapes: rect, stadium, ellipse, diamond, hexagon, cylinder. Endpoints
   are clipped against the real outline, not a bounding box.],

  [*Text*],
  [`font` `font-size` `font-family` `font-color` `font-weight` `label-offset`],
  [`font` is read by value kind, so the order is free: a number is the size, a
   string the family, a colour the ink, a bare word the weight.],

  [*Edges*],
  [`arrow` `router` `label-offset`],
  [`router: ortho` draws a right-angled elbow. `label-offset` on an edge also
   *pins* the label, so automatic avoidance stops moving it.],
)

#v(0.8em)

*Colours* are `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` or an SVG colour name.

*There is no `opacity`.* Alpha is the fourth channel of the colour, defaulting
to opaque — which is what lets a shape keep a solid border over a translucent
fill. `#38bdf880` is that colour at half alpha.

#v(0.4em)

#callout(kind: "danger", title: "An unknown property is a hard error")[
So is an unknown shape, class, router or enumerated word. The message names the
nearest real property, and where a name belongs to another genre it says which:
`bits` written outside a packet reports `` `type packet` `` by name.
]
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "One form, many words")[

`type` picks the genre once per file. Every genre reads the *same* statement
shape — so the cascade, the diagnostics and the editor support are written
once — but each has its own word for its items, and its own data after the
label.

```
slice organic "Organic search" 4820      // type pie
field sport   "Source port"    0 15      // type packet
node  a       "A"              at (0, 0) // type free
```

An archetype may also declare a *second* word, when the same picture can be
described by data of a different shape. The word decides the shape, never the
number of values written — two shapes of the same length would be
indistinguishable, and the error message could not say which one it thought you
meant.

#v(0.5em)

#dtable(
  header: ([`type`], [word], [data after the label], [`type`], [word], [data after the label]),
  columns: (auto, auto, 1fr, auto, auto, 1fr),

  [free], [`node`], [—],                       [xy], [`series`], [numbers…],
  [pie], [`slice`], [value],                   [xy], [`scatter`], [x y pairs…],
  [bar], [`series`], [numbers…],               [radar], [`series`], [numbers…],
  [treemap], [`box`], [value],                 [sankey], [`node`], [value],
  [quadrant], [`point`], [x y],                [venn], [`set`], [count],
  [journey], [`step`], [scores…],              [timeline], [`event`], [date],
  [gantt], [`task`], [start, end or length],   [kanban], [`card`], [—],
  [class], [`class`], [—],                     [er], [`entity`], [—],
  [sequence], [`participant`], [—],            [state], [`state`], [—],
  [block], [`block`], [kind],                  [requirements], [`requirement`], [—],
  [packet], [`field`], [first, last bit],      [bytefield], [`field`], [first, last byte],
  [wave], [`signal`], [wave string],           [harness], [`connector`], [—],
  [rack], [`device`], [lowest, highest U],     [network], [`host`], [role],
  [gitgraph], [`commit`], [branch],            [orgchart], [`position`], [—],
  [mindmap], [`topic`], [—],                   [treeview], [`entry`], [—],
)

#v(0.4em)

Twenty-eight genres and `free`. Six of them put data on the *edge* instead of
on the item, because that is where the fact lives:

#dtable(
  header: ([`type`], [written], [meaning]),
  columns: (auto, auto, 1fr),

  [er], [`a -> b "places" one many`], [Cardinality at each end, read at the end nearest the entity it describes.],
  [sankey], [`a -> b 40`], [The magnitude that flows. A stage's height is whatever passes through it.],
  [venn], [`a -> b 85`], [How many things the two sets share.],
  [harness], [`j1 -> j2 "power" 1 3`], [The pin the wire lands on at each end.],
  [requirements], [`r2 -> r1 derive`], [What the relation asserts, read tail to head.],
  [network], [`sw -> srv "uplink" "10 GbE"`], [What the link runs at.],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Charts")[

`pie` · `bar` · `xy` · `radar` · `treemap` · `sankey` · `quadrant` · `venn`

#diagram(
  image("diagrams/family-charts.svg", width: 78%),
  caption: [`type bar` — one series per statement, its numbers in category order.],
)

#v(0.3em)

```
type bar { title: "Requests served, by region" categories: "Q1" "Q2" "Q3" "Q4" }

series emea "EMEA" 120 185 240 310
series amer "AMER" 200 210 265 280
```

#v(0.4em)

#dtable(
  header: ([Genre], [What it is for], [Worth knowing]),
  columns: (auto, 1fr, 1.2fr),

  [`pie`], [Parts of a whole.],
  [Warns below three slices — a bar reads better — and above eight, which
   exhausts the palette.],

  [`bar` `xy`], [Series over named categories, or over an axis.],
  [`xy` also answers to `scatter`, which reads its values two at a time as
   coordinates. A plot is one or the other, never both.],

  [`radar`], [A shape compared across several axes.],
  [Every axis shares one scale and the rings start at zero. Per-axis scales are
   how this chart is normally made to lie.],

  [`treemap`], [Nested proportions.],
  [The hierarchy comes from group nesting. Colour identifies the top-level
   branch; depth darkens the same hue rather than inventing one.],

  [`sankey`], [Flows between stages.],
  [The value rides on the link, not on what it joins — a stage's height is
   whatever passes through it.],

  [`venn`], [Overlapping sets.],
  [With two sets the lens is *solved* to the area you wrote. With three it is
   arranged and labelled, and says so. Four is refused.],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Software design")[

`class` · `er` · `sequence` · `state` · `block` · `requirements`

#diagram(
  image("diagrams/family-software.svg", width: 80%),
  caption: [`type er` — a bare `- ` line in the block is a column.],
)

#v(0.4em)

#dtable(
  header: ([Genre], [What it is for], [Worth knowing]),
  columns: (auto, 1fr, 1.2fr),

  [`class` `er`], [Boxes with a ruled list of members.],
  [A member line runs to the end of its line, so it cannot share one with the
   closing brace. Placement stays declared in both.],

  [`sequence`], [Participants, lifelines and ordered messages.],
  [`activate api { … }` draws the bar down a lifeline for as long as the block's
   messages run. A frame spans message rows, not participants.],

  [`state`], [A machine.],
  [Pseudo-states are marks, guards go in brackets, nesting comes from groups —
   and the anchors still place everything.],

  [`block`], [C4 and architecture views.],
  [The element kind picks the shape and the colour, so a reader tells a person
   from a datastore before reading the label.],

  [`requirements`], [Requirements and their typed links.],
  [A relation with no kind, and a requirement with no `verify`, are both
   reported — they are the failures the genre exists to make visible.],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Wire formats and hardware")[

`packet` · `bytefield` · `wave` · `harness` · `rack` · `network`

#diagram(
  image("diagrams/family-wire.svg", width: 88%),
  caption: [`type bytefield` — the ruler counts bytes, and the range is inclusive.],
)

#v(0.3em)

```
type bytefield { title: "ELF64 header" width: 16 cell: 30 34 ruler: on }

field ident "e_ident" .magic 0 15      // sixteen bytes, one whole row
field etype "e_type"              16 17
```

#v(0.4em)

#dtable(
  header: ([Genre], [What it is for], [Worth knowing]),
  columns: (auto, 1fr, 1.2fr),

  [`packet` `bytefield`], [Wire formats, at bit or byte granularity.],
  [One layout, two units. A field crossing a row boundary is split and named
   once. Overlaps are errors; unassigned bits are warnings.],

  [`wave`], [Timing lanes.],
  [The WaveDrom alphabet: clocks, buses with named segments, gaps. `=.=` is two
   values; `=..` is one held for two periods.],

  [`harness`], [Wiring looms.],
  [A connector's pins are its member lines, and a wire lands *on* the pin it
   names. A pin wired twice, or one that does not exist, is an error.],

  [`rack`], [Rack elevations.],
  [Units count from 1 at the bottom, as a rack is labelled. Empty units are
   room, not a mistake — unlike a packet's unassigned bits.],

  [`network`], [Topology.],
  [The role picks the outline and deliberately not the fill: a network
   diagram's colours usually already mean something to whoever drew it.],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Plans and histories")[

`gantt` · `timeline` · `journey` · `kanban` · `gitgraph`

#diagram(
  image("diagrams/family-planning.svg", width: 86%),
  caption: [`type gantt` — dates and durations are literals, not strings.],
)

#v(0.4em)

#dtable(
  header: ([Genre], [What it is for], [Worth knowing]),
  columns: (auto, 1fr, 1.2fr),

  [`gantt`], [Bars on a calendar.],
  [`2026-03-16` and `3w` are literals the language understands. A task takes a
   start and either an end or a length, read by value kind.],

  [`timeline`], [Events on a date line.],
  [Labels stagger either side, and the tick step is chosen against the room on
   the page — not against the calendar alone.],

  [`journey`], [Steps taken, and how each actor felt.],
  [Scores are positional against `actors`. The axis is labelled at every whole
   score, because a satisfaction score has no meaning between two.],

  [`kanban`], [Columns of cards.],
  [A column may carry a work-in-progress `limit`, and one over it is reported.
   That diagnostic is the reason the genre exists.],

  [`gitgraph`], [Commits, branches and merges.],
  [Only the rails are computed. A commit lands one step past its latest parent,
   and a parent declared after its child is refused.],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Trees")[

`orgchart` · `mindmap` · `treeview`

#two-col(
  [
#diagram(
  image("diagrams/family-trees.svg", width: 100%),
  caption: [`type orgchart` — the hierarchy is the edges.],
)

#v(0.2em)

```
type orgchart { row: 74 spacing: 20 }

position cto  "CTO"
position plat "Platform"
position sig  "DSP"

cto -> plat
cto -> sig
plat -> k8s
```
  ],
  [
Three genres over one placement pass, read on different axes: `orgchart` lays
the tree downward, `mindmap` outward from a central root, `treeview` as
indented rows.

*The hierarchy is the edges*, not group nesting. An org chart is written
`ceo -> cto`; nesting a manager inside a group *and* declaring them as a box
would be two statements for one fact.

Every subtree is measured bottom-up and handed a disjoint slice of its parent's
span, so siblings cannot overlap by construction.

Three things stop a tree being one, and all three are reported: a second parent,
a cycle, and several roots — the last drawn side by side, with a warning saying
so.
  ],
  columns: (1fr, 1.15fr),
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "What the compiler refuses")[

#diagram(
  image("diagrams/pipeline.svg", width: 100%),
  caption: [The language server reuses the first four stages, so the editor and the command line can never disagree.],
)

#v(0.5em)

One validator, two front ends. Rendering reports the first error; the editor
reports all of them, each with the span to underline. A diagnostic earns its
place by naming something a reader would otherwise get wrong.

#dtable(
  header: ([Severity], [Examples]),
  columns: (auto, 1fr),

  [*Error* — nothing is drawn],
  [An unknown property, shape, class, router or enumerated word · a wire on a
   pin that does not exist · two devices sharing a rack unit · overlapping
   packet fields · an anchor cycle · a duplicate identifier · a requirement
   link nobody defined · four sets in a venn],

  [*Warning* — it draws, but it will not read as intended],
  [A label too dark against its fill · unassigned bits between two fields · a
   pie of two slices · a series with fewer numbers than there are categories ·
   a kanban column over its limit · a host on no link and in no segment · a
   requirement nobody has planned to verify],
)

#v(0.5em)

#callout(kind: "info", title: "Contrast is checked, not assumed")[
A fill at low alpha is mostly its backdrop, so the check composites before
measuring — otherwise text that is genuinely unreadable gets reported as fine.
]
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Working with it")[

#two-col(
  [
*The command line*

```
bgraph diagram.bd -o diagram.svg
bgraph diagram.bd -o diagram.pdf
bgraph watch diagram.bd
bgraph check diagram.bd
bgraph fmt diagram.bd
bgraph completions fish
```

The output extension picks the writer. A flag would be a second way of saying
what the filename already says.

`check` prints `file:line:col: severity: message` and exits non-zero on an
error — the form editors and build systems already understand.

`fmt` fixes what is structural: indentation, trailing space, runs of blank
lines. It leaves columns lined up by hand exactly as written — indentation is
structural and so its business, alignment is taste and is not. `--check`
reports instead of writing, and exits non-zero, so one command serves a person
and a build.

`completions` writes a shell script — or the man page, with `completions man` —
generated from the same flag definitions the binary parses with, so neither can
describe an option it does not have.

*The live preview*

`watch` serves the diagram and reloads on every save, reporting parse and
layout errors in place so a broken file never costs you the preview. Scroll to
zoom, drag to pan, `f` to fit, `0` for actual size. The view survives the
reload, or zooming while editing would be pointless.

`watch --data q3.json` previews a *filled* template, re-reading the data on
every request — so editing the numbers reloads too. Without data it draws the
shape, holes and all, which is what you are editing before the data exists.

`watch` takes a picture as readily as a source: an `.svg` is shown as it is, so
it works on a file bgraph never wrote — and because the reload follows the
file's timestamp, it keeps up with whatever is regenerating it. The extension
decides which it is, the same rule `-o` uses.

Built with the `window` feature it opens a *native window* rather than
serving a page: no port, no browser, and the SVG rasterised directly. Scroll
zooms by 5% a notch, or 10% holding shift; drag pans; `f` fits, `0` is actual
size, `r` reloads. That is
the one place this project takes a dependency tree, which is why it is opt-in.
`--server` asks for the page instead, and a machine with no display is told
rather than left waiting.
  ],
  [
*The editors*

Both get the same language server, so they behave identically: diagnostics as
you type, completion that knows the genre, hover, go-to-definition and an
outline.

#dtable(
  header: ([], [Highlighting], [Server]),
  columns: (auto, 1fr, auto),
  [Zed], [tree-sitter grammar], [yes],
  [VS Code], [TextMate grammar], [yes],
)

#v(0.3em)

Completion writes whole statements rather than words: accepting `slice` leaves
`slice ${id} "${label}" ${value}`, with each slot named after the property it
fills. Signature help names the value you are typing — positional data is the
idiomatic form, and it was the one shape nothing could tell you about while you
wrote it.

Colour literals get a swatch and a picker, and an identifier lights up
everywhere it is written.

In a template it goes further: a hole is offered from the ones the file already
uses, hovering one says whether it is bound once or once per row, the outline
lists them as the file's parameters, and a line that would repeat over two
lists is a squiggle rather than a surprise at render time.
  ],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Calling it from a program")[

A diagram often has a fixed shape and changing numbers — a weekly report, a
dashboard, a generated page. For that, a file holds the shape and the calling
program binds the holes.

#two-col(
  [
```
type pie { title: "{{quarter}}" }

slice {{r.id}} "{{r.name}}" {{r.value}}
```

A `{{hole}}` *parses*. It is a value in the tree, not a pre-render text
substitution — otherwise every line of a template would light up red in the
editor. Checks that depend on an unbound value are skipped until it is bound.

A line holding a `{{row.field}}` hole is written once per row. The line is the
unit because a statement already ends at a newline, so it needs no block syntax
and cannot nest wrongly. A line naming two lists is refused when the template is
read, not at the first render.

*The same from the command line.* A JSON object binds the holes, so a template
is filled from a query result or a pipe without writing a program:

```
bgraph report.bd --data q3.json -o q3.svg
… | bgraph report.bd --data - --emit
```

An array of objects becomes repeated lines; an array of scalars becomes a list.
A CSV works too, and maps onto repetition exactly — a header names the fields,
every other line writes its statement once:

```
bgraph report.bd --rows source=sales.csv -o q3.svg
```
  ],
  [
*A hole may stand where a name goes*

`slice {{r.id}} "…"` is the point of repeating a line per row — the data
supplies the id of the statement it fills. Both grammars accept it, so the file
parses and highlights in the editor as well as it renders. Not where the
archetype goes: the archetype decides which properties exist, so it has to be
known before anything else can be checked.

*Where a value lands decides how it is written*

Text in an identifier position is written bare, because the grammar wants a
name there; anywhere else it is quoted, because a bare word would become a
second statement. Text that cannot be a name is refused *by name*, rather than
written and left to fail as a parse error further down the file.

*What is promised*

Four things carry a stability promise: rendering a source, the template API,
the parser and the validator. Everything else is public to be inspected, not to
be depended on — pinning it would mean the diagrams could never improve.

The vocabulary's stable form is the generated reference, produced from the same
tables the compiler and the editor read.

#v(0.4em)

#callout(kind: "ok", title: "Hostile input cannot bring down the host")[
A deeply nested file used to overflow the stack and abort the process — which a
caller cannot catch, in a library meant to be embedded. Nesting depth is now
counted before parsing and refused past a limit.
]

#v(0.3em)

Each genre is a build option. A genre not compiled in is *unknown*, not broken,
and the binary is smaller by its absence.
  ],
)
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "What it cannot do")[

Stated, because finding these out by being surprised is worse than reading them
here.

#dtable(
  header: ([Limit], [What it means in practice], [Why it stands]),
  columns: (auto, 1fr, 1.1fr),

  [*Text width is estimated*],
  [Auto-sized boxes are slightly wrong for a font unlike the one measured
   against.],
  [A file names a font *family* and the viewer decides what that resolves to.
   No measurement can be right by anything but luck. Per-character advances
   removed the larger half of the error; the rest cannot be removed. It does
   not apply to PDF, where the font is guaranteed.],

  [*Orthogonal routing avoids nothing*],
  [An `ortho` link can pass straight through a box — but it says so.],
  [A router that avoids obstacles is a different piece of work. Drawing a wrong
   picture *in silence* was the part worth fixing first, so the crossing is
   reported and the message names the ways out.],

  [*Edge labels avoid lines, not each other*],
  [On a busy diagram a label can end up somewhere no cost function would have
   chosen.],
  [The placement is local by design. `label-offset` is the escape hatch: it
   moves the label and *pins* it, because the author can see what the cost
   function cannot.],

  [*No raster output*],
  [No PNG.],
  [SVG and PDF are vector and need no rasteriser. A bitmap is one command away
   from either, with a tool built for it.],
)

#v(0.6em)

#line-sep()

#v(0.4em)

#text(fill: muted, size: 0.92em)[
None of these is a defect being hidden until it is fixed. Three of them are
positions; the first is a property of the format itself.
]
]

// ══════════════════════════════════════════════════════════════════════════════
#pagebreak(weak: true)
#slide(title: "Revision history")[

#dtable(
  header: ([Version], [Date], [What changed]),
  columns: (auto, auto, 1fr),

  [*1.2*], [10 August 2026], [`watch` shows a picture as readily as a source,
   and opens a native window when the build has one. Records that the viewer is
   the one place the project takes a dependency tree, and that it is therefore
   opt-in.],

  [*1.1*], [10 August 2026], [Corrected a claim that had become false: an
   `ortho` link crossing a box is now reported, where this document said there
   was no diagnostic. Added `bgraph fmt`, `watch --data`, `--rows` for CSV, and
   the rule that a hole may stand where a name goes — with what the editor now
   knows about a template.],

  [*1.0*], [10 August 2026], [First edition. Written for someone about to write
   `.bd` files: the five statements, placement, the cascade, the twenty-eight
   genres by family, the diagnostics, the tools, embedding, and the limits.
   Diagrams are drawn in bgraph itself and checked by its own validator.],
)

#v(0.6em)

#line-sep()

#v(0.4em)

#text(fill: muted, size: 0.9em)[
This manual teaches the language. The exhaustive property reference is
generated from the compiler's own tables and always describes the binary you
have — regenerate it rather than trusting a copy.
]
]
