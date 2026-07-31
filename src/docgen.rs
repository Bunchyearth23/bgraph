//! The reference manual, generated from the same tables the compiler and the
//! editor read. `bgraph docs` writes it; a test asserts the checked-in copy
//! still matches, so the documentation cannot drift away from the code.

use std::fmt::Write;

use crate::props::{self, Ctx, PropDef};

pub fn reference() -> String {
    let mut m = String::new();

    m.push_str(HEAD);
    properties(&mut m);
    graph_settings(&mut m);
    archetypes(&mut m);
    vocabulary(&mut m);
    m.push_str(DIAGNOSTICS);
    m.push_str(TAIL);

    m
}

fn row(m: &mut String, def: &PropDef, with_ctx: bool) {
    let name = if def.alias {
        format!("`{}` *(alias)*", def.name)
    } else {
        format!("`{}`", def.name)
    };
    let ctx = match (with_ctx, def.ctx) {
        (false, _) => String::new(),
        (true, Ctx::Node) => " node |".into(),
        (true, Ctx::Edge) => " edge |".into(),
        (true, Ctx::Both) => " both |".into(),
    };
    let _ = writeln!(
        m,
        "| {name} |{ctx} `{}` | {} |",
        cell(def.syntax),
        cell(&squash(def.doc))
    );
}

/// A table cell is one line, and an unescaped `|` would end it early — which
/// matters here because half the syntax strings spell out alternatives.
fn cell(text: &str) -> String {
    text.replace('|', "\\|")
}

/// Fold a wrapped doc string back onto one line.
fn squash(doc: &str) -> String {
    doc.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn properties(m: &mut String) {
    m.push_str(
        "\n## Property reference\n\n\
         Every property below is valid on a `style` block, on a `node` or edge \
         statement's inline block, or both. Anything else is an error, not a \
         silent no-op — a typo surfaces the moment you save.\n\n\
         Aliases are marked. They behave identically to the property they \
         alias; the editor keeps them out of completion so the list stays \
         short.\n",
    );

    m.push_str("\n| Property | Applies to | Syntax | Meaning |\n|---|---|---|---|\n");
    for def in props::PROPS {
        row(m, def, true);
    }
}

fn graph_settings(m: &mut String) {
    m.push_str(
        "\n## Graph settings\n\n\
         Declared once, in `graph { … }`. These are diagram-wide and are not \
         part of the style cascade.\n",
    );

    m.push_str("\n| Setting | Syntax | Meaning |\n|---|---|---|\n");
    for def in props::GRAPH_PROPS {
        row(m, def, false);
    }

    m.push_str(
        "\nThe surface is **explicit by default**: without a `background` the \
         renderer still paints `#ffffff`, so the live preview and the exported \
         file agree. Write `background: none` (or `transparent`) for a \
         transparent SVG — the preview then shows a checkerboard rather than \
         pretending.\n",
    );
}

fn archetypes(m: &mut String) {
    m.push_str(
        "\n## Archetypes\n\n\
         `type` picks the genre of diagram, once, at the top of the file. \
         Without it the file is `free`.\n\n\
         Every archetype reads the *same* statements: a `node` is a box, a \
         slice or a bitfield depending on the genre. The style cascade, the \
         diagnostics and the editor support are identical in all of them — \
         what changes is which properties carry meaning.\n",
    );

    for def in props::ARCHETYPES {
        let _ = write!(m, "\n### `type {}`\n\n{}\n", def.name, squash(def.doc));

        let shape = slot_shape(def.slots);
        let _ = write!(
            m,
            "\n```\n{} <id> [\"label\"]{}{}\n```\n",
            def.item,
            if shape.is_empty() { "" } else { " " },
            shape
        );

        if !def.edge_slots.is_empty() {
            let _ = write!(
                m,
                "\n```\n<from> -> <to> [\"label\"] {}\n```\n",
                slot_shape(def.edge_slots)
            );
        }

        if def.props.is_empty() {
            m.push_str("\nIt takes no settings.\n");
            continue;
        }

        m.push_str("\n| Setting | Syntax | Meaning |\n|---|---|---|\n");
        for prop in def.props {
            row(m, prop, false);
        }
    }

    m.push_str(ARCHETYPE_NOTES);
}

/// How a slot list is written: one placeholder per value, or `…` when the
/// slot takes the rest of the line.
fn slot_shape(slots: &[props::Slot]) -> String {
    slots
        .iter()
        .map(|s| {
            if s.arity == props::VARIADIC {
                format!("<{}> …", s.prop)
            } else {
                std::iter::repeat_n(format!("<{}>", s.prop), s.arity)
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn word_list(m: &mut String, title: &str, words: &[&str], note: &str) {
    let quoted: Vec<String> = words.iter().map(|w| format!("`{w}`")).collect();
    let _ = write!(m, "\n**{title}** — {}\n\n{note}\n", quoted.join(" · "));
}

fn vocabulary(m: &mut String) {
    m.push_str("\n## Accepted words\n");

    word_list(
        m,
        "Shapes",
        props::SHAPES,
        "Several names denote the same shape; pick whichever reads best.",
    );
    word_list(
        m,
        "Arrow heads",
        props::HEADS,
        "Used by `arrow:` on an edge.",
    );
    word_list(
        m,
        "Routers",
        props::ROUTERS,
        "Used by `router:` on an edge.",
    );
    word_list(
        m,
        "Dash patterns",
        props::DASHES,
        "Accepted by `dash:` and inside a compound `stroke:`.",
    );
    word_list(
        m,
        "Alignments",
        props::ALIGNMENTS,
        "Used by `align` in a placement. `left`/`right` for a vertical \
         placement, `top`/`bottom` for a horizontal one, `center` (or \
         `middle`) for either — and the default.",
    );
    word_list(
        m,
        "Flow directions",
        props::FLOWS,
        "Used by `flow:` in `graph { … }`, for nodes that declare no placement.",
    );
    word_list(
        m,
        "Pie labels",
        props::PIE_LABELS,
        "Used by `labels:` in `type pie { … }`.",
    );

    m.push_str(
        "\n### Categorical palette\n\n\
         Pie slices that declare no `fill` take these hues, in order, never \
         cycled. Two columns of the same eight hues, stepped for a light or a \
         dark `background`; the renderer picks the column from the surface's \
         luminance.\n\n\
         The order is validated: worst adjacent-pair separation is ΔE 9.1 for \
         protanopia on light and 8.4 on dark (OKLab ×100, ≥ 8 target), and \
         19.6 / 19.3 for normal vision (≥ 15 floor).\n",
    );

    m.push_str("\n| Slot | Light | Dark |\n|---|---|---|\n");
    for (i, (light, dark)) in props::PALETTE_LIGHT
        .iter()
        .zip(props::PALETTE_DARK)
        .enumerate()
    {
        let _ = writeln!(m, "| {} | `{light}` | `{dark}` |", i + 1);
    }

    m.push_str(
        "\nThere is no ninth hue. Past eight slices `check` tells you to merge \
         the tail into an `Other` slice rather than repeat a color.\n",
    );
}

const HEAD: &str = r##"# bgraph reference

<!-- Generated by `bgraph docs`. Edit src/docgen.rs, not this file. -->

Diagrams as text, where **you** decide the layout.

Mermaid and friends auto-place everything and leave you fighting the engine.
`bgraph` inverts that: every node says where it goes, but says it *relative to
its neighbours* — so you keep control without hand-editing coordinates every
time the diagram grows.

- A prose introduction lives in [`../README.md`](../README.md).
- A worked tour of the syntax lives in [`language.md`](language.md).
- **This file is the exhaustive reference**, generated from the compiler's own
  tables, so it always describes the binary you have.

## Commands

```
bgraph diagram.bd -o diagram.svg    # render
bgraph watch diagram.bd             # live preview on :7878, reloads on save
bgraph watch diagram.bd --port 9000
bgraph check diagram.bd             # diagnostics only; non-zero on error
bgraph docs -o docs/reference.md    # regenerate this file
bgraph-lsp                          # language server, LSP over stdin/stdout
```

## The five statements

```
type <archetype> { … }      // the genre of diagram; optional, defaults to free
graph { … }                 // diagram-wide settings
style <name> [extends <name>] { … }
group <id> ["<label>"] [.class …] [<placement>] { … }
<keyword> <id> ["<label>"] <data…> [.class …] [<placement>] { … }
<id> <arrow> <id> ["<label>"] [.class …] { … }
```

The fourth is an **item**, and each archetype has its own word for it —
`node` in `free`, `slice` in `pie`, `field` in `packet`. `node` stays a
universal synonym everywhere, so nothing is ever forced.

```
type pie
slice organic "Organic search" 4820
slice referral 1180                    // no label: the id is displayed
node b "Direct" { value: 2410 }        // the long form, still valid
```

Values after the label fill the archetype's slots in order, and are **sugar
for the properties it declares** — `slice a 4820` and `slice a { value: 4820 }`
build the same thing. The first string after the identifier is always the
label, so an archetype whose data is a string obliges you to write one.

A statement runs to its separator. `node a node b` on one line is therefore
one item with two stray values, not two items, and `check` says so.

Statements are separated by newlines; `;` and `,` also work. Comments start
with `//` — `#` is reserved for hex colors and cannot also open a comment.

Identifiers may contain letters, digits, `_` and an interior `-`. A trailing
hyphen is not allowed, which is what lets `a->b` split into three tokens.
The four statement keywords — `type`, `graph`, `style`, `node` — are reserved
and cannot name a node.

Labels are quoted. `\n` splits a label into centred lines; `\t` and `\"` also
work. A node with no label falls back to its identifier. Label text is
XML-escaped on output, so `<`, `&` and quotes are safe.

## Groups

A `group` is a frame. Its block holds properties *and* nested statements, so
groups nest:

```
group edge "Edge" {
  padding: 22
  node gw   "Gateway"
  node auth "Auth" below gw gap 30
}

group core "Core" right-of edge gap 60 {
  node orders "Orders"
}

node db "Postgres" right-of core gap 60
```

**A group is sized by its members, never the other way round** — its rectangle
is their bounding box plus its `padding`. That keeps the rule this language is
built on: the items say where they go.

A group may still carry a placement, and then it **translates its whole
subtree** once its members have landed. Members of a placed group only become
anchorable after that translation, so nothing outside can latch onto a
position that is about to move.

Groups and items share one namespace, so a group can be anchored to by name.
An edge to a group is refused — a frame has no obvious endpoint. Style them
with `style group { … }` or a `.class`; the default is a dashed border, a
transparent fill and the label set into the top-left.

## Placement

| Form | Meaning |
|---|---|
| `at (x, y)` | absolute top-left corner |
| `right-of <id>` | to the right of another node |
| `left-of <id>` | to its left |
| `below <id>` | underneath it |
| `above <id>` | over it |
| `gap <n>` | distance to the anchor; defaults to the graph `gap` |
| `align <kw>` | cross-axis alignment against the anchor |

Anchors may point **forward**: the solver runs repeated passes, so a node may
be anchored to one declared later. A genuine loop is reported as an error.

A node with no placement flows from the previous one in source order, using
the graph's `flow` direction and `gap`. The first node lands at `(0, 0)`.

Sizes come from the label plus `padding` unless `size`, `width` or `height`
says otherwise. The canvas is derived from the content — nothing is ever
clipped, and negative coordinates are fine.

## Arrows

| Token | Line | Heads |
|---|---|---|
| `->` | solid | end |
| `<-` | solid | start |
| `<->` | solid | both |
| `--` | solid | none |
| `-->` | dashed | end |
| `..>` | dotted | end |
| `..` | dotted | none |

The token's dash pattern is a default; `{ dash: solid }` overrides it.

## Colors

`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, or an SVG color name (`white`,
`tomato`).

**The fourth channel is the alpha**, and it defaults to `ff` — opaque. There
is no separate `opacity` property: transparency belongs to the color that
carries it, which is what lets a shape have an opaque border and a translucent
fill.

```
node card "Card" {
  fill: #1e293b80      // half-transparent slate
  stroke: #38bdf8      // fully opaque, alpha omitted
  font-color: #e2e8f0c0
}
```

On output the alpha becomes its own attribute — `fill="#1e293b" fill-opacity="0.5"`
rather than an eight-digit hex. Both render identically in a modern viewer, but
several SVG-to-PDF paths and older renderers drop the inline alpha silently.

Note that this is per-paint, not per-element: a translucent fill lets the inner
half of a thick stroke show through, which a single element opacity would have
composited away first.

## The style cascade

```
built-in defaults  →  style node / style edge  →  .classes (in order)  →  inline { }
```

The implicit theme is the archetype's own word — `style slice { … }` in a
pie — with `style node` and `style edge` remaining universal aliases, applied
first so the specific name wins where both are declared. `extends` flattens a chain
child-first. An unknown property or an unknown `.class` is an error.
"##;

const ARCHETYPE_NOTES: &str = r#"
Edges and placement are ignored by the chart archetypes — and reported, not
silently dropped.

`style node { … }` is the only text hook a chart has, so it governs the
title, the on-slice labels and the legend alike. On-slice text defaults to
white, which reads on all eight palette hues; everything else defaults to the
theme's ink for the surface.
"#;

const DIAGNOSTICS: &str = r#"
## Diagnostics

`check` and the language server report exactly the same things — one
validator serves both, so the editor can never disagree with the build.

**Errors**

- an unknown property, style, node, shape, router, arrow head or archetype
  (property typos come with a suggestion)
- a node declared twice, or `type` declared twice
- a node named after a statement keyword (`type`, `graph`, `style`, `node`) —
  an edge cannot start with one, so the name would be half-usable at best
- a node anchored to itself, or a cycle of anchors
- a style that extends an unknown style, or extends itself in a cycle
- a property with no value
- `type pie` — a node with no `value`, or a negative one
- `type packet` — a node with no `bits`, or ranges that overlap

**Warnings**

- a property with no effect where it is written (`shape` on an edge,
  `value` outside `type pie`, a setting on an archetype that takes none)
- a style declared more than once
- text whose contrast against what sits behind it is below 3:1
- `type pie` — a slice with value 0; fewer than three slices, which a bar or
  a single number compares better; more than eight, which exhausts the palette
- `type packet` — bits left unassigned between two fields
- an edge or a placement in an archetype that ignores it
"#;

const TAIL: &str = r#"
## Editor support

`bgraph-lsp` speaks LSP over stdin/stdout with no arguments, so any client can
use it. A Zed extension is packaged in [`../editors/zed`](../editors/zed)
together with a tree-sitter grammar.

It provides diagnostics, go-to-definition from any reference to its
declaration, a symbol outline, hover (a property's syntax and meaning, a
node's label and **resolved position and size**, a style's flattened property
list), and context-aware completion.

Completion is deliberately quiet: it never fires inside a label or a comment,
`.` only opens the style list where a class reference can actually go, aliases
are hidden, and `value` / `bits` appear only under the archetype that reads
them.

## Grammar

The authoritative grammar is [`../src/syntax.pest`](../src/syntax.pest); the
tree-sitter grammar in [`../tree-sitter-bgraph`](../tree-sitter-bgraph) mirrors
it and is covered by a corpus test suite.

Two details are worth knowing because they shape the syntax:

- Newlines separate statements, so horizontal whitespace and newlines are not
  interchangeable.
- Inside a `{ }` block, a property's value list ends at the next `name:`. That
  is what lets `{ fill: #111 stroke: #222 }` sit on one line.
"#;
