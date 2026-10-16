//! The reference manual, generated from the same tables the compiler and the
//! editor read. `bgraph docs` writes it; a test asserts the checked-in copy
//! still matches, so the documentation cannot drift away from the code.

use std::fmt::Write;

use serde_json::json;

use crate::props::{self, Ctx, PropDef};

pub fn reference() -> String {
    let mut m = String::new();

    m.push_str(HEAD);
    properties(&mut m);
    themes(&mut m);
    graph_settings(&mut m);
    archetypes(&mut m);
    vocabulary(&mut m);
    m.push_str(DIAGNOSTICS);
    m.push_str(TAIL);

    m
}

/// Machine-readable vocabulary for integrations. Field names and the schema
/// version are stable; the vocabulary itself follows the compiler version.
pub fn schema() -> String {
    let context = |ctx: Ctx| match ctx {
        Ctx::Node => "node",
        Ctx::Edge => "edge",
        Ctx::Both => "both",
    };
    let property = |def: &PropDef| {
        json!({
            "name": def.name,
            "context": context(def.ctx),
            "alias": def.alias,
            "words": def.words,
            "syntax": def.syntax,
            "description": def.doc,
        })
    };
    let archetypes: Vec<_> = props::ARCHETYPES
        .iter()
        .map(|def| {
            json!({
                "name": def.name,
                "item": def.item,
                "items": def.items().collect::<Vec<_>>(),
                "description": def.doc,
                "layoutCapability": def.layout_capability().as_str(),
                "layoutStatements": def.layout.statements(),
                "layoutGuides": def.layout.guides(),
                "settings": def.props.iter().map(&property).collect::<Vec<_>>(),
                "itemProperties": def.item_props.iter().map(&property).collect::<Vec<_>>(),
                "slots": def.slots.iter().map(|slot| json!({
                    "property": slot.prop,
                    "arity": if slot.arity == props::VARIADIC { None } else { Some(slot.arity) },
                    "variadic": slot.arity == props::VARIADIC,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "schemaVersion": 1,
        "compilerVersion": env!("CARGO_PKG_VERSION"),
        "properties": props::PROPS.iter().map(property).collect::<Vec<_>>(),
        "archetypes": archetypes,
    }))
    .expect("the vocabulary is JSON")
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

fn themes(m: &mut String) {
    m.push_str(
        "\n## Themes\n\n\
         A **theme** is a style the renderer looks up on its own: it paints \
         without being referenced. Everything else is a **class**, and a class \
         paints where `.name` is written and nowhere else. The list is fixed, \
         which is what lets a style whose name is on neither list be \
         reported rather than silently drawn nowhere.\n\n\
         Each genre's item words are theme targets. A single-word genre uses \
         that word (`style slice` themes a pie); a multi-form genre gives each \
         form its own target (`style task`, `style event`, `style gateway` and \
         `style data` in BPMN). `style node` remains the common layer.\n",
    );

    m.push_str("\n| Theme | Reads | What it paints |\n|---|---|---|\n");
    for t in props::THEME_STYLES {
        let reads = if !t.only.is_empty() {
            t.only
                .iter()
                .filter(|n| props::lookup(n, props::PROPS).is_some_and(|d| !d.alias))
                .map(|n| format!("`{n}`"))
                .collect::<Vec<_>>()
                .join(", ")
        } else {
            match t.ctx {
                Some(Ctx::Node) => "the node vocabulary".into(),
                Some(Ctx::Edge) => "the edge vocabulary".into(),
                _ => "everything".into(),
            }
        };
        let _ = writeln!(
            m,
            "| `style {}` | {reads} | {} |",
            t.name,
            cell(&squash(t.doc))
        );
    }

    m.push_str(
        "\nA property outside what a theme reads is a warning, not a line that \
         quietly draws nothing.\n",
    );
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
         pretending. Alpha also works here: `background: #ffffff80` gives \
         a white surface at about 50% opacity. The final two hex digits are \
         `00` for transparent through `ff` for opaque; `#rgba` is the short form. \
         SVG, PDF and PNG retain the alpha; the live preview shows it over \
         its checkerboard.\n",
    );
}

pub(crate) use crate::examples::EXAMPLES;

fn archetypes(m: &mut String) {
    m.push_str(
        "\n## Archetypes\n\n\
         `type` picks the genre of diagram, once, at the top of the file. \
         Without it the file is `free`.\n\n\
         Every archetype reads the *same* statements: a `node` is a box, a \
         slice or a bitfield depending on the genre. The style cascade, the \
         diagnostics and the editor support share the same vocabulary tables. \
         The geometry contract below determines which placements are meaningful.\n",
    );

    m.push_str(
        "\n### Geometry ownership\n\n\
         `Declared` accepts the complete `layout` language: `place`, `align`, \
         `equal-width`, `equal-height`, `row` and `column`, including local blocks \
         inside groups. `ArchetypeManaged` computes geometry from its data and \
         rejects `layout` blocks and `note` statements. `Hybrid` is reserved for \
         renderers with an explicit contract of named guides; no current genre \
         is Hybrid, and the current grammar has no named guide statements.\n\n\
         | Archetype | Geometry | Supported layout statements | Named guides |\n\
         |---|---|---|---|\n",
    );
    for def in props::ARCHETYPES {
        let statements = def.layout.statements().join(", ");
        let guides = def.layout.guides().join(", ");
        let _ = writeln!(
            m,
            "| `{}` | `{}` | {} | {} |",
            def.name,
            def.layout_capability().as_str(),
            if statements.is_empty() {
                "none"
            } else {
                &statements
            },
            if guides.is_empty() { "none" } else { &guides },
        );
    }

    for def in props::ARCHETYPES {
        let _ = write!(m, "\n### `type {}`\n\n{}\n", def.name, squash(def.doc));
        let _ = writeln!(m, "\nGeometry: `{}`.", def.layout_capability().as_str());

        let shape = slot_shape(def.slots);
        let _ = write!(
            m,
            "\n```\n{} <id> [\"label\"]{}{}\n```\n",
            def.item,
            if shape.is_empty() { "" } else { " " },
            shape
        );

        // A second word carries a second positional shape (`D-023`). It is
        // written out in full rather than as a note, because the shape *is*
        // the difference between the two.
        for alt in def.alt_items {
            let shape = slot_shape(alt.slots);
            let _ = write!(
                m,
                "\n```\n{} <id> [\"label\"]{}{}\n```\n\n{}\n",
                alt.item,
                if shape.is_empty() { "" } else { " " },
                shape,
                squash(alt.doc)
            );
        }

        if !def.edge_slots.is_empty() {
            let _ = write!(
                m,
                "\n```\n<from> -> <to> [\"label\"] {}\n```\n",
                slot_shape(def.edge_slots)
            );
        }

        if def.props.is_empty() {
            m.push_str("\nIt takes no settings.\n");
        } else {
            m.push_str("\n| Setting | Syntax | Meaning |\n|---|---|---|\n");
            for prop in def.props {
                row(m, prop, false);
            }
        }

        // Properties this genre adds to its items, or gives its own meaning.
        // They are looked up before the shared table and never appear in
        // another archetype's completion, so this is the only place they are
        // written down.
        if !def.item_props.is_empty() {
            let article = if def.item.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            };
            let _ = write!(
                m,
                "\nOn {article} `{}` — these are its own, and are not offered elsewhere:\n",
                def.item
            );
            m.push_str("\n| Property | Where | Syntax | Meaning |\n|---|---|---|---|\n");
            for prop in def.item_props {
                row(m, prop, true);
            }
        }

        if let Some((_, source)) = EXAMPLES.iter().find(|(name, _)| *name == def.name) {
            let _ = write!(
                m,
                "\n<details>\n<summary>A complete file — <code>bgraph init {}</code></summary>\n\n```\n{}\n```\n\n</details>\n",
                if def.name == "free" {
                    "groups"
                } else {
                    def.name
                },
                source.trim_end()
            );
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

There are **29 specialised genres, plus `free`**. `free` is the plain
node-and-edge language; each genre gives those statements a domain-specific
meaning without creating a second grammar.

## Commands

```
bgraph diagram.bd -o diagram.svg    # render
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
bgraph check diagram.bd             # diagnostics only; non-zero on error
bgraph check diagram.bd --ci        # …the flat form, whatever stdout is
bgraph check diagram.bd --json      # …a stable JSON array for tools and CI
bgraph diff old.bd new.bd -o d.svg  # what appeared, what left, what moved
bgraph explain diagram.bd api     # why that box is where it is
bgraph layout diagram.bd            # write the placements in; the picture is unchanged
bgraph report.bd --data q3.json     # fill a template's holes from JSON
bgraph report.bd --set year=2026    # …or bind a scalar on the command line
bgraph report.bd --rows r=sales.csv # …or one hole from the rows of a CSV
bgraph watch report.bd --data q3.json  # live preview of a filled template
bgraph fmt diagram.bd               # reformat; --check reports instead
bgraph fmt --migrate old.bd         # a removed `opacity` becomes the colour's alpha
bgraph init pie                     # a working file for a genre; no genre lists them
bgraph completions fish             # a completion script; bash|zsh|fish|elvish|powershell
bgraph completions man              # …or the man page, from the same flags
bgraph docs -o docs/reference.md    # regenerate this file
bgraph schema -o vocabulary.json    # vocabulary metadata for editor integrations
bgraph-lsp                          # language server, LSP over stdin/stdout
```

The watch page reloads on every save and reports parse and layout errors in
place, so a broken file never costs you the preview. Scroll to zoom, drag to
pan, `f` to fit, `0` for actual size — and the view survives the reload, or
zooming while editing would be pointless.

## The five statements

```
type <archetype> { … }      // the genre of diagram; optional, defaults to free
graph { … }                 // diagram-wide settings
style <name> [extends <name>] { … }
group <id> ["<label>"] [.class …] [<placement>] { … }
activate <id> { … }         // type sequence: a bar down an existing lifeline
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
  node auth "Auth"
  layout {
    place gw at (0, 0)
    place auth below gw gap 30
  }
}

group core "Core" {
  node orders "Orders"
}

node db "Postgres"

layout {
  place edge at (0, 0)
  place core right-of edge gap 60
  place db right-of core gap 60
}
```

**A group is sized by its members, never the other way round** — its rectangle
is their bounding box plus its `padding`. That keeps the rule this language is
built on: the items say where they go.

A root `layout` block may place a group, and then it **translates its whole
subtree** once its members have landed. Members of a placed group only become
anchorable after that translation, so nothing outside can latch onto a
position that is about to move.

Groups and items share one namespace, so a group can be anchored to by name.
An edge to a group is refused — a frame has no obvious endpoint. Style them
with `style group { … }` or a `.class`; the default is a dashed border, a
transparent fill and the label set into the top-left.

Parent frames paint before their children. `style node` is the common layer
for groups too; `style group { fill: none }` makes frames transparent while
leaving node fills intact. `width`, `height` and `size` on declared groups are
required dimensions, with a constraint error when the contents cannot fit.
`equal-width` and `equal-height` accept both nodes and groups.

In BPMN, a group containing only other groups defines a pool of lanes. Its
direct child groups fill the pool's inner width, excluding parent padding and
their own external margins. No invisible spacer nodes are needed.

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

Anchors may point **forward**: a `place` relation may reference a node declared
later. The solver resolves the declared relations together; contradictory
relations are reported as errors.

A node with no placement flows from the previous one in source order, using
the graph's `flow` direction and `gap`. The first node lands at `(0, 0)`.

Sizes come from the label plus `padding` unless `size`, `width` or `height`
says otherwise. The canvas is derived from the content — nothing is ever
clipped, and negative coordinates are fine.
Labels wrap to a declared width using measured advances. Fixed heights and
extremely narrow widths can still overflow; the validator reports those cases.

Orthogonal paths search from their actual ports through every `via`, avoiding
boxes, external captions and group titles. Boundary-event ports leave away
from the host and automatic task ports avoid its attached events. Message and
association ports do not displace BPMN sequence-flow ports. No obstacle-count
threshold disables avoidance. An impossible route retains a visible fallback
and a diagnostic. BPMN start/end warnings count sequence flows only.

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

The implicit theme is the word written for the item — `style slice { … }` in a
pie, or `style event { … }` for BPMN events — with `style node` and `style edge`
remaining universal aliases, applied first so the specific name wins where both are declared. `extends` flattens a chain
child-first. An unknown property or an unknown `.class` is an error.

### External spacing

BPMN event and gateway captions choose a nearby position clear of flows,
nodes and other labels. Their measured footprints also contribute to relative
gaps and containing frames. Explicit `at` coordinates and alignments still
refer to visible shapes. A declared `label-offset` pins the caption.
`label-position: top|right|bottom|left` pins the side of an external BPMN
event or gateway caption; `label-offset` then adjusts that position. Other
elements reject `label-position` rather than ignoring it.
Edge labels also avoid annotations and group titles and can slide along
their segment. Avoidance is bounded; remaining collisions produce diagnostics
rather than silently moving declared nodes.

`margin` reserves space outside a box or frame, while `padding` reserves space
inside its outline. It follows CSS side order:

```
margin: 16            // all sides
margin: 8 16         // vertical, horizontal
margin: 8 16 12      // top, horizontal, bottom
margin: 8 16 12 20   // top, right, bottom, left
margin-top: 24       // override one side
```

The four individual properties participate in the same style cascade as the
shorthand. Values are finite nonnegative drawing units; `auto` and collapsing
margins are not supported. Unit suffixes remain advisory: `50%` means 50 drawing
units, not a percentage of the parent. Adjacent margins add to `gap`.
Absolute `at` coordinates and explicit alignment still address visible outlines.

Supported by the nine declared-layout genres and by `sequence` participants
and group frames. Other managed genres reject the property. Sequence frames
reserve their title band and external margins before scheduling messages, so
a first frame cannot touch participant headers and nested frames stay separated.

### Themes, and everything else

A **theme** is a style the renderer looks up on its own; it paints without
being referenced. Everything else is a **class**, and a class paints where
`.name` is written and nowhere else. There is a fixed list of themes, and a
style whose name is not on it and is never applied is reported — that is the
difference between `style grid` and a typo.

Each theme reads a named part of the vocabulary, and a property outside it is
a warning rather than a line that draws nothing.
"##;

const ARCHETYPE_NOTES: &str = r#"
Edges and placement are ignored by the chart archetypes — and reported, not
silently dropped.

A chart's own furniture is not any item's. `style grid` paints the rules a
chart draws behind its data — a rack's unit rules, a chart's gridlines — and
the numbers and axis labels along them; `style title` paints the heading. Both
used to be hardcoded to the surface, and the heading used to take the colour of
whatever the genre calls its items, which is the one thing on the drawing it is
certainly not.

On-slice text defaults to white, which reads on all eight palette hues;
everything else defaults to the theme's ink for the surface.
"#;

const DIAGNOSTICS: &str = r#"
## Diagnostics

`check` and the language server report exactly the same things — one
validator serves both, so the editor can never disagree with the build.

`bgraph check diagram.bd --json` is the stable interchange form for CI and
editor integrations. It writes one JSON array to standard output, including
when there are no diagnostics. Every entry has `file`, `line`, `column`,
`severity` (`error` or `warning`) and `message`; `line` and `column` are
`null` only when composition prevents a source location from existing. It
exits 1 if the array contains an error, otherwise 0. The same form is used
for a composed diagram and attributes an included-file diagnostic to its own
path.

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
- two boxes in a declared-layout diagram that overlap; bgraph names them but
  never chooses a direction in which to move either one
- a style declared more than once
- text whose contrast against what sits behind it is below 3:1
- `type pie` — a slice with value 0; fewer than three slices, which a bar or
  a single number compares better; more than eight, which exhausts the palette
- `type packet` — bits left unassigned between two fields
- an edge or a placement in an archetype that ignores it
"#;

const TAIL: &str = r#"
## Templates

A `.bd` file may leave holes for a program to fill:

```
type pie { title: "Revenue by region, {{year}}" }

slice emea "EMEA" {{emea}}
slice amer "{{amer_label}}" {{amer}}
```

`{{name}}` parses, so a template stays editable — the editor would otherwise
light up every line of it — and `check` skips the validation that depends on a
value nobody has bound yet.

From the shell:

```
bgraph revenue.bd -o revenue.svg --set year=2026 --set emea=4820 --set amer_label=Americas --set amer=6310
bgraph revenue.bd --emit --set …          # print the filled source instead
```

From Rust:

```rust
use bgraph::{Bindings, Template};

let template = Template::parse(&source)?;
let svg = template.render(
    &Bindings::new()
        .set("year", 2026i64)
        .set("emea", 4820i64)
        .set("amer_label", "Americas")
        .set("amer", 6310i64),
)?;
```

A bound value is escaped for **where it lands**: inside a string literal it is
escaped for one, in a value position text is quoted. Blind replacement would
let a stray quote turn a label into a syntax error, or into different
statements. A list spreads space-separated, which is how every list in this
language reads — so `{{points}}` bound to `[10, 20, 30]` fills a whole series.

An unbound hole is an error naming it. Rendering a blank would put a wrong
figure in a document, which is worse than failing.

`Value::Raw` inserts verbatim, and is the only way to fill an identifier or a
colour — both of which would otherwise be quoted as text.

### Repetition

Substitution cannot give a chart a number of items that is not known when the
template is written — a pie with a variable number of slices. So a line that
holds a `{{row.field}}` hole is written **once per row**:

```
type pie
slice {{region.id}} "{{region.name}}" {{region.revenue}}
```

```rust
template.render(&Bindings::new().rows("region", vec![
    Bindings::new().set("id", Value::Raw("emea".into()))
                   .set("name", "EMEA").set("revenue", 4_820i64),
    Bindings::new().set("id", Value::Raw("amer".into()))
                   .set("name", "Americas").set("revenue", 6_310i64),
]))?;
```

The line is the unit because it is the unit the language already uses: a
statement ends at a newline. So "repeat this" needs no block syntax, and there
is no way to get the nesting wrong.

That is deliberately the whole of it — **one line, one list, no nesting and no
conditions**. A line naming two lists is refused when the template is read,
because there is no number of repetitions a reader could predict. No rows
means the line is not written at all: a chart of nothing draws nothing.

Rows can come from `Bindings::rows` in Rust, an array of objects supplied with
`--data`, or a CSV bound with `--rows name=file.csv`. `--set` binds scalars.

## Building only what you use

Every archetype is a cargo feature, so a program that needs one chart does not
compile the unused genres. The project is not published on crates.io; with
access to the source repository, use a path dependency:

```toml
bgraph = { path = "/path/to/bgraph", default-features = false, features = ["pie"] }
```

`free` is not a feature — it is the language itself, and a file that declares
no `type` is a free graph. An archetype that is **not** compiled in is
*unknown*, not broken: `check` reports it and lists what is available, rather
than accepting the file and failing to draw it.

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

What it offers follows the block you are in:

| Inside | You are offered |
|---|---|
| `graph { }` | the graph properties |
| `type <genre> { }` | that genre's own settings |
| `style node { }` / `style edge { }` | one side's vocabulary — the side the theme applies to |
| `style <name> { }` | both sides, since a generic class may be applied to either |
| `node a { }`, `use … { }`, `note … { }` | the node properties |
| `a -> b { }` | the edge properties |
| `group { }`, `component { }`, `activate { }` | **statements and properties both** — declaring first, since that is what a container is for |

The last row is the one worth knowing. A group's block nests a whole diagram,
so completion there offers the genre's declaring word, the statement keywords,
anchors that name the group's own members, and the group's properties after
them. Asking on a blank line — indentation and nothing else — answers with
that same list rather than with nothing.

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
