# Choose a diagram type

A file chooses its type once, at the beginning, with `type`.
Without a type declaration it uses `free`.

To list the types available in your build, then create a packet example:

```sh
bgraph init
bgraph init packet -o header.bd
```

`init` writes a complete, working source file. The
[gallery](galerie.md) shows all 30 types in the full build, with source
files and rendered images. Start from one of those examples when a type
is new to you.

## Which type should I use?

| I need… | Types to try |
|---|---|
| Architecture, services or networks | `free`, `block`, `network` |
| Software or data models | `class`, `er`, `state`, `requirements` |
| Processes and interactions | `bpmn`, `sequence`, `journey`, `kanban` |
| Parts of a whole or comparisons | `pie`, `bar`, `xy`, `radar`, `quadrant`, `treemap` |
| Flows and intersections | `sankey`, `venn` |
| Dates and history | `gantt`, `timeline`, `gitgraph` |
| Hierarchies | `orgchart`, `mindmap`, `treeview` |
| Electronics and infrastructure | `packet`, `bytefield`, `wave`, `harness`, `rack` |

## A pie chart in a few lines

```bgraph
type pie {
  title: "Traffic sources"
  donut: 0.55
  labels: percent
}

slice organic "Organic search" 4820
slice direct "Direct" 2410
slice referral "Referrals" 1180
```

The number after each label is its value. The type calculates the geometry,
so do not add `layout`. Colors come from the default palette unless you
declare a `fill`. Pie charts include a key derived from their slices.
To replace it with descriptive entries and a heading, set `legend: off` in
the `type pie` block and [add a legend](styles.md#add-a-legend).

For more than eight categories, group the tail of the distribution. See
`top` in [the pie reference](reference.md#type-pie).

## Overlapping sets with a Venn diagram

```bgraph
type venn { title: "Shared readership" }

set developers "Developers" 420
set operations "Operations" 260
set architects "Architects" 180

developers -> operations 120
developers -> architects 140
operations -> architects 70
```

Each set's number is its **total membership**, including overlaps. Circle
areas scale with these totals. The letters A, B and C and their colors match
the set key; long names wrap there instead of covering the circles.

The **Shared members** key names each pair. Here, 120 people belong to both
Developers and Operations; some may also be Architects. The value does not
describe a region belonging only to those two sets. An omitted link means
zero shared members.

With two sets, the overlap area is exact. With three, the renderer fits the
pairwise areas when the distances allow it, then falls back to a schematic
arrangement otherwise. The warning and the drawing both say that three-set
areas are indicative: the seven individual regions are not solved. Four or
more sets are rejected. See [the Venn reference](reference.md#type-venn).

## A binary packet

```bgraph
type packet { title: "Small header" width: 16 }

field version "Version" 0 3
field flags "Flags" 4 7
field length "Length" 8 15
```

Bit boundaries are **inclusive**: `0 3` represents four bits.
The type arranges the fields according to the declared row width.

## Two ways to control positions

| Geometry | Types | What you control |
|---|---|---|
| Declared placement | `free`, `class`, `block`, `er`, `state`, `harness`, `bpmn`, `network`, `requirements` | Positions and relationships in `layout` |
| Calculated by the type | The other 21 types | Data, written order and type settings |

A type with calculated geometry rejects `layout` blocks and `note`
declarations. A `legend` block works in both families: omit its position to
place it below the drawing, or use `at (x, y)` for an absolute position.
Relative legend anchors such as `below api` require declared placement.
See [Add a legend](styles.md#add-a-legend) for a complete example with a title.

Styles are shared by both families, but a property with no effect for the
chosen type produces a diagnostic.

Each type has its own declaration words, such as `slice`, `field`, `task`
or `host`. `node` remains a universal synonym. For your first attempt,
keep the words and data form used by `bgraph init`.
