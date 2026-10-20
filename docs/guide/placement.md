# Place and group elements

Start with [Your first diagram](demarrage.md) if `node` and `layout` are
new to you. This page explains how to control positions in a larger drawing.

## Declare elements, then place them

Declarations describe what exists. A `layout` block describes where it goes.
It is supported by `free`, `class`, `block`, `er`, `state`, `harness`,
`bpmn`, `network` and `requirements`.

```bgraph
node ui "User interface"
node api "API"
node db "Storage"

layout {
  place ui at (0, 0)
  place api right-of ui gap 48 align top
  place db below api gap 32
}

ui -> api
api -> db
```

`at (x, y)` addresses the element's top-left corner. Coordinates increase
to the right and downward. Distances use drawing units. See
[the grid](language.md#a-grid) for cell-based coordinates.

| Instruction | Effect |
|---|---|
| `right-of api` | Place to the right of `api` |
| `left-of api` | Place to its left |
| `below api` | Place below it |
| `above api` | Place above it |
| `gap 40` | Leave space between the elements' borders |
| `align top` | Align top edges during horizontal placement |
| `align left` | Align left edges during vertical placement |

Without an explicit alignment, relative placement centers the elements
across the direction of travel. An element without a placement follows
the file's implicit flow, for example `graph { flow: right gap: 40 }`.
Declare a relationship when the element's position matters.

## Make a row or column

```bgraph
node api "API"
node cache "Cache"
node db "Database"
node audit "Audit log"

layout {
  place api at (0, 0)
  row api cache db gap 32
  column db audit gap 24
}
```

The first element anchors the row or column. The others receive their
placement from that instruction; do not give them a second `place`.

## Match sizes and align edges

Add these instructions inside an existing `layout` block:

```text
equal-width api cache db
equal-height api cache db
align top api cache db
```

Equal sizes preserve the room needed by the largest label.
`equal-width` and `equal-height` also accept groups, including a mixture
of nodes and groups. A group must still contain its children.

For alignment, `top`, `bottom` and `center-y` control the vertical axis;
`left`, `right` and `center-x` control the horizontal axis.

## Group part of a drawing

```bgraph
group backend "Backend" {
  padding: 24
  node api "API"
  node worker "Worker"
  layout {
    place api at (0, 0)
    place worker below api gap 32
  }
}

node db "Database"
layout {
  place backend at (0, 0)
  place db right-of backend gap 60
}

api -> db
```

A group sizes itself around its contents. Moving it moves its subtree.
A `layout` block inside a group addresses that group's direct members.

You can require a group's `width`, `height` or `size`. For example,
`group pool "Pool" { width: 600 … }` reserves 600 drawing units.
If the contents cannot fit, the size requirement produces a constraint error.

In BPMN, a group whose direct members are all groups forms a pool of lanes.
Those lanes fill its inner width after pool padding and lane margins are
deducted. The parent is painted before its children. The common
`style node` layer also applies to groups. Use
`style group { fill: none }` for transparent frames and
`style task { size: 176 74 }` to size tasks alone.

## Leave room for labels

In BPMN, events and gateways carry their labels outside their shapes.
Automatic placement looks for nearby room while avoiding flows, other shapes,
labels and group titles. For example, a vertical flow below a gateway can
move its caption beside the line.

Relative placements reserve the caption's footprint as well as the gap
and margins. Frames include the caption at its chosen position. Explicit
`at` coordinates and alignments still refer to visible shape borders.

Edge labels look for room near the middle of a segment, both along it and
on either side. They also account for annotations and group titles in
diagram types using the shared router.

Within a `type bpmn` file, these declarations choose a caption position:

```text
gateway decision "Approve?" exclusive { label-offset: 24 0 }
event received "Message received" start message { label-position: right }
```

`label-position` accepts `top`, `right`, `bottom` or `left` on BPMN events
and gateways only. The chosen side remains fixed; `label-offset` can add
a further adjustment. Without either property, placement is automatic.

An explicit `label-offset` fixes a BPMN caption or edge label in place.
If there is still insufficient room, a warning reports the collision.
Increase the gap, add margins or adjust the offset.

## BPMN flows and ports

Sequence flows leaving a boundary event first move away from its host task.
Automatic task ports avoid attached boundary events; associations and
messages do not shift its sequence-flow ports. Start/end warnings count
sequence flows, including `default` and `conditional` flows.

The `ortho` router avoids boxes, outside captions and group titles,
including diagrams with more than 60 obstacles. Ports and `via` waypoints
take part in its search. An impossible route remains visible with a warning;
an unreachable `via` does not silently turn into a diagonal.

A fixed width wraps node labels. If a fixed height or very small width still
prevents the text from fitting, a warning identifies the element.

## Understand an unexpected position

```sh
bgraph explain architecture.bd api
```

This explains the element's geometry and where its style values came from.
The browser preview's inspector exposes this information too.

bgraph reports overlaps and contradictory requirements. If a drawing needs
more space, change the relationships or gaps in the source.

!!! note "Charts calculate their positions"
    Types such as `pie`, `packet` and `gantt` reject `layout` blocks.
    Use their data and type settings instead.
    See [Choose a diagram type](graphiques.md).
