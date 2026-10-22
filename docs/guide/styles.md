# Styles and colors

A style lets you reuse appearance settings. Start by applying one to a box;
you can add shared palettes and themes later.

## Create a reusable style

```bgraph
style service {
  fill: #dbeafe
  stroke: #2563eb 1.5
  radius: 8
  font: 14 #0f172a
}

style store extends service {
  shape: cylinder
  fill: #d1fae5
}

node api "API" .service
node db "Database" .store { stroke: #059669 2 }
layout { place db right-of api gap 60 }
api -> db
```

`fill` is the background color. `stroke` combines a line color with an
optional thickness. `font` combines the text size and color.
`extends` inherits another style. A block written directly on an element
takes precedence over its named styles.

## Understand the cascade

For an element, settings are applied from general to specific:

1. Built-in defaults and the command-line theme.
2. `style node`, common to all elements.
3. The declaration word's style, such as `style slice` or `style task`.
4. Named classes such as `.service`, in their written order.
5. Properties written directly on the element inside `{ … }`.

`style edge` is the base for connections. An arbitrary named style such as
`service` applies where the class `.service` is used.

## Style all boxes or connections

```bgraph
style node { fill: #f1f5f9 stroke: #334155 1 font: 14 #0f172a }
style edge { stroke: #64748b 1.5 router: ortho }

node a "Start"
node b "Finish"
layout { place b right-of a gap 60 }
a -> b "request"
```

Common shapes include `rect`, `stadium`, `ellipse`, `diamond`,
`hexagon` and `cylinder`. The `ortho` router looks for a right-angle route
around boxes; `straight` draws a direct connection.

## Add a legend

Write `title: "…"` inside a `legend` block to give the legend a heading.
Each `entry` describes a color or shape with a label and a small swatch:

```bgraph
type free

style status { stroke-width: 1.5 radius: 6 }
style live extends status { fill: #dcfce7 stroke: #166534 }
style planned extends status { fill: #fef3c7 stroke: #92400e dash: dashed }

node api "API" .live
node worker "Worker" .planned
layout { place worker right-of api gap 60 }
api -> worker

legend {
  title: "Service status"
  font-size: 12
  entry "Available to users" .live
  entry "Planned for a later release" .planned
}
```

The heading here is **Service status**. Entries appear in their written
order. The same `.live` and `.planned` classes style both the boxes and their
swatches, including the properties inherited through `extends`. To style a
swatch directly, an entry can instead use a block such as
`entry "Available" { fill: #dcfce7 stroke: #166534 }`. Inline properties
override the entry's classes.

With no position, the legend sits below the drawing. This works in every
diagram type, including types that have no built-in key. Add
`at (0, 240)` after `legend` to set an absolute position. In a type with
[declared placement](graphiques.md#two-ways-to-control-positions), you can also
write `legend below api gap 32 align left { … }` to anchor it to a node or
group.

`title` belongs to the legend; it does not change the diagram title. The
legend's `font`, `font-size`, `font-family`, `font-color` and `font-weight`
settings control its text. Entry blocks accept `fill`, `stroke`,
`stroke-width`, `dash`, `radius` and `shape` for the swatches. See
[Legends in the reference](reference.md#legends).

A chart's built-in key remains independent. Where the type supports
`legend: off`, put that setting in its `type` block to hide its generated key
when providing your own legend. Without explicit entries, a `legend` statement
still builds its rows from styles with `legend: "description"`. Explicit
entries replace that list; no custom legend appears unless you declare one.

## Add space around an element

`padding` reserves room **inside** a box. `margin` reserves room **outside**
a box or frame, using the CSS side order:

```text
margin: 16
margin: 8 16
margin: 8 16 12
margin: 8 16 12 20
margin-top: 24
```

The forms mean: all sides; vertical/horizontal; top/horizontal/bottom;
top/right/bottom/left. Individual sides (`margin-top`, `margin-right`,
`margin-bottom`, `margin-left`) combine through named styles, `extends`
and direct properties.

```bgraph
style node { margin: 8 16 }
node api "API"
node db "Database"
layout {
  place api at (0, 0)
  place db right-of api gap 40
}
api -> db
```

Here the border-to-border gap is **72**: API's right margin of 16, the gap
of 40 and Database's left margin of 16. Visible box sizes do not change.
A group also contains its children's margins.

Absolute coordinates and explicit alignments refer to visible borders;
a margin does not move a required `at` position.

For a sequence diagram, styles can include:

```text
style participant { margin-bottom: 12 }
style group { margin: 16 8 }
```

Sequence layout reserves room for frame titles, margins and messages before
drawing lifelines. This keeps the first frame clear of the participants and
separates nested frames.

Margins are available in the nine declared-placement types and in
`sequence`. Other types reject them. Values must be finite, nonnegative
distances. `auto`, negative margins and CSS margin collapsing are not
supported. Unit suffixes are currently descriptive: `50%` means 50 drawing
units, without a relative percentage calculation.

## Apply a theme

```sh
bgraph architecture.bd --theme light -o light.svg
bgraph architecture.bd --theme dark -o dark.svg
bgraph architecture.bd --theme print -o print.pdf
```

A theme is applied underneath the file's own settings. Colors already
declared in the source keep their precedence. To let a theme choose the
background, omit `background` from your source.

Without a theme or a declared background, the background is white.
For a transparent export:

```bgraph
graph { background: none }
node a "Transparent background"
```

For a partially transparent background:

```bgraph
graph { background: #ffffff80 }
node a "Partly transparent background"
```

The format is `#RRGGBBAA`: the last two digits control background opacity
independently of shapes and text.

| Alpha | Approximate opacity |
|---|---|
| `00` | 0% — transparent |
| `40` | 25% |
| `80` | 50% |
| `bf` | 75% |
| `ff` | 100% — opaque |

This works in SVG, PDF and PNG output. The browser preview shows a
checkerboard behind the background so transparency is visible.
The short `#RGBA` form is also accepted, for example `#fff8`
(about 53% opacity).

## Share a palette

Put your `style` declarations in `house.bds`, then add
`include "house.bds"` to your diagrams. A `.bds` file contains styles only.
See [Files, components and data](composition.md).

Use the [generated property reference](reference.md#property-reference)
for every accepted setting. `bgraph explain file.bd id` shows which style
layer supplied each value.
