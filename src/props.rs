//! The property vocabulary, in one place.
//!
//! Validation (`check.rs`), completion and hover (`lsp.rs`) all read these
//! tables, and a test asserts `style.rs` accepts exactly what they list — so
//! adding a property means editing one table and one `match` arm, and the
//! editor learns about it for free.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ctx {
    Node,
    Edge,
    Both,
}

impl Ctx {
    pub fn allows(self, other: Ctx) -> bool {
        self == Ctx::Both || self == other
    }
}

pub struct PropDef {
    pub name: &'static str,
    pub ctx: Ctx,
    /// Aliases stay valid and documented, but are kept out of completion
    /// lists — three ways to spell `stroke` is noise while you type.
    pub alias: bool,
    /// Accepted keywords, when the property is an enumeration.
    pub words: &'static [&'static str],
    pub syntax: &'static str,
    pub doc: &'static str,
}

pub const SHAPES: &[&str] = &[
    "rect",
    "box",
    "rectangle",
    "stadium",
    "pill",
    "rounded",
    "ellipse",
    "circle",
    "oval",
    "diamond",
    "rhombus",
    "decision",
    "hexagon",
    "hex",
    "cylinder",
    "database",
    "db",
];

pub const HEADS: &[&str] = &[
    "solid", "filled", "triangle", "open", "line", "thin", "dot", "circle", "none",
];

pub const ROUTERS: &[&str] = &["straight", "direct", "line", "ortho", "orthogonal", "elbow"];

pub const DASHES: &[&str] = &["solid", "dashed", "dotted", "none"];

pub const FLOWS: &[&str] = &["right", "left", "down", "up", "LR", "RL", "TB", "BT"];

/// Crow's-foot cardinality, read at the end nearest the entity it describes.
pub const CARDINALITIES: &[&str] = &[
    "one",
    "many",
    "zero-one",
    "optional",
    "zero-many",
    "one-many",
];

pub const PROPS: &[PropDef] = &[
    PropDef {
        name: "legend",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "legend: <text>",
        doc: "What this style class is called in a `legend` statement. Its \
              presence is what puts the class in one — a legend built from \
              every class would list the ones that only round a corner, and \
              one built from the colours in use would have nothing to call \
              them.",
    },
    PropDef {
        name: "description",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "description: <text>",
        doc: "What this is, in a sentence. Emitted as an SVG `<title>` in every \
              genre — a tooltip to a reader with a mouse, a label to one with a \
              screen reader — and additionally *drawn* under the technology \
              line in `type block`, which has room for it.",
    },
    PropDef {
        name: "desc",
        alias: true,
        ctx: Ctx::Both,
        words: &[],
        syntax: "desc: <text>",
        doc: "Alias for `description`.",
    },
    PropDef {
        name: "fill",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "fill: <color>",
        doc: "Interior color of the shape.",
    },
    PropDef {
        name: "background",
        alias: true,
        ctx: Ctx::Node,
        words: &[],
        syntax: "background: <color>",
        doc: "Alias of `fill`.",
    },
    PropDef {
        name: "stroke",
        alias: false,
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "stroke: <color> <width> <solid|dashed|dotted>",
        doc: "Outline. Accepts a color, a width and a dash pattern in any order; \
              omit whichever you don't need.",
    },
    PropDef {
        name: "border",
        alias: true,
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "border: <color> <width> <pattern>",
        doc: "Alias of `stroke`.",
    },
    PropDef {
        name: "color",
        alias: true,
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "color: <color> <width> <pattern>",
        doc: "Alias of `stroke`. For text color use `font-color`.",
    },
    PropDef {
        name: "stroke-width",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "stroke-width: <number>",
        doc: "Outline thickness in user units.",
    },
    PropDef {
        name: "weight",
        alias: true,
        ctx: Ctx::Both,
        words: &[],
        syntax: "weight: <number>",
        doc: "Alias of `stroke-width`.",
    },
    PropDef {
        name: "dash",
        alias: false,
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "dash: <solid|dashed|dotted> | dash: <on> <off>",
        doc: "Dash pattern. Named, or explicit on/off lengths such as `dash: 6 3`.",
    },
    PropDef {
        name: "radius",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "radius: <number>",
        doc: "Corner rounding — applies to the `rect` shape.",
    },
    PropDef {
        name: "corner",
        alias: true,
        ctx: Ctx::Node,
        words: &[],
        syntax: "corner: <number>",
        doc: "Alias of `radius`.",
    },
    PropDef {
        name: "shape",
        alias: false,
        ctx: Ctx::Node,
        words: SHAPES,
        syntax: "shape: <rect|stadium|ellipse|diamond|hexagon|cylinder>",
        doc: "Outline geometry. Edge endpoints are clipped against it.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "Fixed size. Without it the box is sized from the label and `padding`.",
    },
    PropDef {
        name: "width",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "width: <number>",
        doc: "Fixed width; the height still follows the label.",
    },
    PropDef {
        name: "height",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "height: <number>",
        doc: "Fixed height; the width still follows the label.",
    },
    PropDef {
        name: "padding",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "padding: <number>",
        doc: "Inset between the label and the outline; drives the auto-size.",
    },
    PropDef {
        name: "font",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "font: <size> \"<family>\" <color> <weight>",
        doc: "Shorthand read by value kind, so the order is free: a number is the \
              size, a string the family, a color the ink, a bare word the weight.",
    },
    PropDef {
        name: "font-size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-size: <number>",
        doc: "Text size in user units.",
    },
    PropDef {
        name: "font-family",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-family: \"<family>\"",
        doc: "Font stack, emitted verbatim into the SVG.",
    },
    PropDef {
        name: "font-color",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-color: <color>",
        doc: "Text color.",
    },
    PropDef {
        name: "text-color",
        alias: true,
        ctx: Ctx::Both,
        words: &[],
        syntax: "text-color: <color>",
        doc: "Alias of `font-color`.",
    },
    PropDef {
        name: "font-weight",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-weight: <weight>",
        doc: "Any SVG weight — `400`, `bold`, `600`…",
    },
    PropDef {
        name: "label-offset",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "label-offset: <dx> <dy>",
        doc: "Move the text by hand, in user units — positive is right and down. \
              `dy` may be omitted. On an edge it also **pins** the label: the \
              automatic avoidance stops moving it, and the offset is measured \
              from the middle of the line.",
    },
    PropDef {
        name: "text-offset",
        alias: true,
        ctx: Ctx::Both,
        words: &[],
        syntax: "text-offset: <dx> <dy>",
        doc: "Alias of `label-offset`.",
    },
    PropDef {
        name: "arrow",
        alias: false,
        ctx: Ctx::Edge,
        words: HEADS,
        syntax: "arrow: <solid|open|dot|none>",
        doc: "Arrow head style. The arrow token decides which ends carry one.",
    },
    PropDef {
        name: "head",
        alias: true,
        ctx: Ctx::Edge,
        words: HEADS,
        syntax: "head: <solid|open|dot|none>",
        doc: "Alias of `arrow`.",
    },
    PropDef {
        name: "router",
        alias: false,
        ctx: Ctx::Edge,
        words: ROUTERS,
        syntax: "router: <straight|ortho>",
        doc: "`straight` draws a direct line; `ortho` draws a right-angled elbow.",
    },
    PropDef {
        name: "route",
        alias: true,
        ctx: Ctx::Edge,
        words: ROUTERS,
        syntax: "route: <straight|ortho>",
        doc: "Alias of `router`.",
    },
    PropDef {
        name: "shadow",
        alias: false,
        ctx: Ctx::Node,
        words: &["on", "off", "true", "false"],
        syntax: "shadow: <on|off>",
        doc: "Soft drop shadow under the shape.",
    },
];

pub const GRAPH_PROPS: &[PropDef] = &[
    PropDef {
        name: "description",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "description: <text>",
        doc: "What the diagram is, in a sentence, for a reader who cannot see \
              it. Becomes `aria-label` on the SVG root, beside `role=\"img\"`.",
    },
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram. Every genre accepts it here — \
              the chart archetypes also take a `title` in their own settings, \
              and that one wins where both are given.",
    },
    PropDef {
        name: "padding",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "padding: <number>",
        doc: "Margin between the drawing and the canvas edge.",
    },
    PropDef {
        name: "sketch",
        alias: false,
        ctx: Ctx::Both,
        words: &["on", "off"],
        syntax: "sketch: <on|off>",
        doc: "Draw as if by hand. A sketched architecture says \"this is a \
              proposal\" where a crisp one says \"this is decided\" — \
              information rather than ornament. Deterministic: the wobble is a \
              hash of each shape's own geometry, so the same file always \
              produces the same file.",
    },
    PropDef {
        name: "grid",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "grid: <number> <number>",
        doc: "Column and row step for `at (col 3, row 2)`. One number sets \
              both. A coordinate system, not a solver: nothing is snapped to \
              it, and a box placed in pixels is unaffected.",
    },
    PropDef {
        name: "gap",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "gap: <number>",
        doc: "Default distance used by anchors that omit `gap`.",
    },
    PropDef {
        name: "background",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "background: <color>",
        doc: "Canvas fill. Also backs edge labels so they stay readable.",
    },
    PropDef {
        name: "bg",
        alias: true,
        ctx: Ctx::Both,
        words: &[],
        syntax: "bg: <color>",
        doc: "Alias of `background`.",
    },
    PropDef {
        name: "flow",
        alias: false,
        ctx: Ctx::Both,
        words: FLOWS,
        syntax: "flow: <right|left|down|up>",
        doc: "Direction used to place nodes that declare no placement.",
    },
    PropDef {
        name: "direction",
        alias: true,
        ctx: Ctx::Both,
        words: FLOWS,
        syntax: "direction: <right|left|down|up>",
        doc: "Alias of `flow`.",
    },
];

pub fn lookup(name: &str, table: &'static [PropDef]) -> Option<&'static PropDef> {
    table.iter().find(|p| p.name == name)
}

/// A property as this archetype means it: its own refinement first, then the
/// shared table.
pub fn item_lookup(def: &ArchetypeDef, name: &str) -> Option<&'static PropDef> {
    lookup(name, def.item_props).or_else(|| lookup(name, PROPS))
}

/// Which genres read a property, for an error that can say so. Empty when the
/// name is not anyone's — that is a typo, and a different message.
pub fn genres_reading(name: &str) -> Vec<&'static str> {
    ARCHETYPES
        .iter()
        .filter(|a| lookup(name, a.item_props).is_some())
        .map(|a| a.name)
        .collect()
}

/// Every property an item of this archetype accepts, with its refinements
/// shadowing the shared entries of the same name.
pub fn item_table(def: &ArchetypeDef) -> Vec<&'static PropDef> {
    def.item_props
        .iter()
        .chain(
            PROPS
                .iter()
                .filter(|p| lookup(p.name, def.item_props).is_none()),
        )
        .collect()
}

// The data each genre reads, kept with the genre. Off the shared table on
// purpose: `wave` means nothing to a pie, and offering it there is the noise
// that made completion get in the way (`D-019`).

pub const PIE_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "value",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "value: <number>",
    doc: "Magnitude of a slice. Usually written positionally — \
          `slice emea \"EMEA\" 4820`.",
}];

pub const TREEMAP_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "value",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "value: <number>",
    doc: "Area of a box, relative to its siblings. Usually written \
          positionally — `box api \"API\" 42`.",
}];

pub const SANKEY_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "value",
    alias: false,
    ctx: Ctx::Edge,
    words: &[],
    syntax: "value: <number>",
    doc: "How much flows along this link. Usually written positionally — \
          `coal -> grid 40`.",
}];

pub const PACKET_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "bits",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "bits: <first> <last>",
    doc: "Inclusive bit range of a field. `bits: 8` is shorthand for a single \
          bit. Usually written positionally — `field flags \"Flags\" 8 15`.",
}];

/// The same idea one order of magnitude up. Kept as its own property rather
/// than reusing `bits` so that writing the wrong one is an error naming the
/// genre that reads it (`D-022`) — the two are indistinguishable by value.
pub const BYTEFIELD_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "bytes",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "bytes: <first> <last>",
    doc: "Inclusive byte range of a field. `bytes: 4` is shorthand for a \
          single byte. Usually written positionally — `field magic \"Magic\" \
          0 3`, which is four bytes.",
}];

pub const QUADRANT_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "x",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "x: <number>",
        doc: "Horizontal position, on the axis range the archetype declares.",
    },
    PropDef {
        name: "y",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "y: <number>",
        doc: "Vertical position, on the axis range the archetype declares.",
    },
];

pub const WAVE_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "wave",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "wave: \"<characters>\"",
        doc: "One character per period: `0` `1` `l` `h` levels, `p` `n` clocks, \
              `x` unknown, `z` high-impedance, `=` and `2`–`9` data, `.` repeats \
              the previous, `|` a gap.",
    },
    PropDef {
        name: "data",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "data: \"<first>\" \"<second>\" …",
        doc: "Names for the data segments of a wave, in order.",
    },
];

pub const SERIES_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "points",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "points: <n> <n> …",
    doc: "One number per category, in order. Usually written positionally — \
          `series north 4 8 15`.",
}];

/// `xy` reads everything `bar` does, plus the coordinates its `scatter`
/// keyword carries. Kept apart from `SERIES_ITEM_PROPS` because `bar` has no
/// x axis to place a pair on, and offering `pairs` there is the noise `D-019`
/// removed.
pub const XY_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "points",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "points: <n> <n> …",
        doc: "One number per step, spread evenly across the x axis. Usually \
              written positionally — `series north 4 8 15`.",
    },
    PropDef {
        name: "pairs",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "pairs: <x> <y> <x> <y> …",
        doc: "Coordinates, read two at a time — the shape `scatter` carries. \
              `scatter north 1 4 3 8` is two points, at (1, 4) and (3, 8). An \
              odd count is an error: the last x would have no y.",
    },
];

pub const GANTT_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "start",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "start: <date> <date|duration>",
        doc: "When a task begins, and either when it ends or how long it runs \
              — read by value kind.",
    },
    PropDef {
        name: "length",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "length: <duration>",
        doc: "How long a task runs, when the start is given on its own.",
    },
];

pub const STATE_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "kind",
        alias: false,
        ctx: Ctx::Node,
        words: STATE_KINDS,
        syntax: "kind: <state|initial|final|choice>",
        doc: "What a state is. `initial` and `final` are drawn as marks rather \
              than boxes and take no label; `choice` is a decision diamond.",
    },
    PropDef {
        name: "guard",
        alias: false,
        ctx: Ctx::Edge,
        words: &[],
        syntax: "guard: <text>",
        doc: "A condition on a transition, shown in brackets after its label.",
    },
];

/// The SysML relation vocabulary, verb stems. Read tail-to-head:
/// `r2 -> r1 derive` is r2 derives from r1.
/// What a box on a topology *is*. Drives the shape, so a reader tells a
/// router from a storage array before reading the label.
/// One engine, three genres. The settings are shared because the differences
/// are in the arrangement, not in what an author has to say about it.
pub const GITGRAPH_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "branches",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "branches: \"<name>\" \"<name>\" …",
        doc: "The rails, top to bottom. A commit on a branch not named here \
              gets a rail of its own at the bottom, in the order first seen.",
    },
    PropDef {
        name: "spacing",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "spacing: <along> <between>",
        doc: "Distance between two commits on a rail, and between two rails.",
    },
];

pub const GITGRAPH_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "branch",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "branch: \"<name>\"",
        doc: "Which rail the commit sits on. Usually written positionally — \
              `commit c3 \"Fix the ruler\" \"main\"`.",
    },
    PropDef {
        name: "tag",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "tag: \"<name>\"",
        doc: "A label drawn beside the commit — a release, a marker.",
    },
];

pub const TREE_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "row",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "row: <number>",
        doc: "Distance between one level and the next.",
    },
    PropDef {
        name: "spacing",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "spacing: <number>",
        doc: "Distance between two siblings.",
    },
];

pub const HARNESS_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "pin",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "pin: <width> <height>",
        doc: "Width of a connector body and the height of one pin row.",
    },
];

pub const HARNESS_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "wire",
    alias: false,
    ctx: Ctx::Edge,
    words: &[],
    syntax: "wire: <from-pin> <to-pin>",
    doc: "Which pin at each end a wire lands on, by the number the connector \
          declares as a member line (`D-025`). Usually written positionally — \
          `j1 -> j2 \"power\" 1 3`.",
}];

pub const VENN_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "radius",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "radius: <number>",
        doc: "How large the biggest set is drawn. The others are scaled \
              against it by area, not by radius — a circle read as twice the \
              size is one with twice the ink in it.",
    },
];

pub const VENN_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "count",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "count: <number>",
        doc: "How many things are in the set. Not `size`, which is on the \
              shared table and means a box's width and height. Usually \
              written positionally — `set rust \"Rust\" 120`.",
    },
    PropDef {
        name: "shared",
        alias: false,
        ctx: Ctx::Edge,
        words: &[],
        syntax: "shared: <number>",
        doc: "How many things two sets have in common, written on a link \
              between them — `rust -> embedded 30`. With two sets the lens is \
              solved to that exact area; with three it is labelled but not \
              solved, because three circles cannot in general be arranged to \
              honour all seven regions at once.",
    },
];

/// What a BPMN task *is*. `plain` is the unmarked box, and it is spelled
/// rather than left as the absence of a word, so a file can say "no, really,
/// nothing" and completion has something to offer.
pub const BPMN_TASK_KINDS: &[&str] = &[
    "plain",
    "user",
    "service",
    "script",
    "manual",
    "send",
    "receive",
    "rule",
    "subprocess",
];

/// Where an event sits in the process. `boundary` is the one that is not a
/// position in the flow but a position on a *box*, and it needs `attach`.
pub const BPMN_STAGES: &[&str] = &["start", "intermediate", "end", "boundary"];

/// What makes an event happen. Drawn as a glyph inside the circle, which is
/// the whole reason BPMN is worth more than an ordinary flowchart: `plain`
/// says "something", `timer` says "at four o'clock".
pub const BPMN_TRIGGERS: &[&str] = &[
    "plain",
    "message",
    "timer",
    "error",
    "signal",
    "escalation",
    "conditional",
    "terminate",
];

pub const BPMN_GATES: &[&str] = &["exclusive", "parallel", "inclusive", "event", "complex"];

pub const BPMN_ARTIFACTS: &[&str] = &["object", "store", "input", "output"];

pub const BPMN_FLOWS: &[&str] = &[
    "sequence",
    "message",
    "association",
    "default",
    "conditional",
];

pub const BPMN_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the diagram.",
}];

pub const BPMN_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "kind",
        alias: false,
        ctx: Ctx::Node,
        words: BPMN_TASK_KINDS,
        syntax: "kind: <plain|user|service|script|manual|send|receive|rule|subprocess>",
        doc: "What kind of task it is, drawn as a marker in the corner of the \
              box. `subprocess` takes the `+` instead, and an *expanded* \
              subprocess is a `group`. Usually written positionally — \
              `task check \"Check credit\" service`.",
    },
    PropDef {
        name: "stage",
        alias: false,
        ctx: Ctx::Node,
        words: BPMN_STAGES,
        syntax: "stage: <start|intermediate|end|boundary>",
        doc: "Where the event sits: a thin ring starts, a thick one ends, a \
              double ring is intermediate. `boundary` is the exception — it \
              is not a place in the flow but a place on a task, so it needs \
              `attach`.",
    },
    PropDef {
        name: "trigger",
        alias: false,
        ctx: Ctx::Node,
        words: BPMN_TRIGGERS,
        syntax: "trigger: <plain|message|timer|error|signal|escalation|conditional|terminate>",
        doc: "What makes the event happen, drawn inside the ring. Written \
              second, after the stage — `event due \"Overdue\" boundary timer`.",
    },
    PropDef {
        name: "attach",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "attach: <task>",
        doc: "The task a `boundary` event sits on. It is placement, declared \
              like every other placement (`D-001`): the event lands on that \
              box's bottom edge, and several on one task spread along it in \
              the order they are written. `label-offset` moves one by hand.",
    },
    PropDef {
        name: "gate",
        alias: false,
        ctx: Ctx::Node,
        words: BPMN_GATES,
        syntax: "gate: <exclusive|parallel|inclusive|event|complex>",
        doc: "Which gateway it is, drawn as the mark inside the diamond. \
              Usually written positionally — `gateway ok \"Approved?\" exclusive`.",
    },
    PropDef {
        name: "artifact",
        alias: false,
        ctx: Ctx::Node,
        words: BPMN_ARTIFACTS,
        syntax: "artifact: <object|store|input|output>",
        doc: "Which data artefact it is. An object is a page with a folded \
              corner, a store is a cylinder; `input` and `output` are objects \
              carrying the arrow that says which way the data went.",
    },
    PropDef {
        name: "flow",
        alias: false,
        ctx: Ctx::Edge,
        words: BPMN_FLOWS,
        syntax: "flow: <sequence|message|association|default|conditional>",
        doc: "What the connector means. A sequence flow is a solid arrow, a \
              message flow is dashed with a hollow head, an association is \
              dotted with none. `default` takes the slash and `conditional` \
              the diamond, both at the gateway end. Usually written \
              positionally — `check -> notify \"rejected\" default`.",
    },
];

pub const NET_ROLES: &[&str] = &[
    "server", "switch", "router", "firewall", "storage", "client", "cloud",
];

pub const NETWORK_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the diagram.",
}];

pub const NETWORK_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "role",
        alias: false,
        ctx: Ctx::Node,
        words: NET_ROLES,
        syntax: "role: <server|switch|router|firewall|storage|client|cloud>",
        doc: "What the box is. Chooses the shape — a reader tells a router \
              from a storage array before reading the label. Usually written \
              positionally — `host fw \"Edge firewall\" firewall`.",
    },
    PropDef {
        name: "addr",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "addr: \"<address>\"",
        doc: "An address, drawn on its own line under the name. Kept as text: \
              a diagram has no business validating a CIDR block.",
    },
    PropDef {
        name: "speed",
        alias: false,
        ctx: Ctx::Edge,
        words: &[],
        syntax: "speed: \"<rate>\"",
        doc: "What the link runs at, drawn in brackets under its label. \
              Usually written positionally — `sw -> srv \"uplink\" \"10 GbE\"`.",
    },
];

pub const RACK_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the frame.",
    },
    PropDef {
        name: "units",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "units: <count>",
        doc: "How tall the frame is, in rack units. 42 by default.",
    },
    PropDef {
        name: "cell",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "cell: <width> <height>",
        doc: "Width of the frame and the height of one rack unit.",
    },
    PropDef {
        name: "ruler",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "ruler: <on|off>",
        doc: "Unit numbers down the rails. On by default.",
    },
];

pub const RACK_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "u",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "u: <lowest> <highest>",
    doc: "The inclusive span of rack units a device occupies, counted from **1 \
          at the bottom** as a rack is. `u: 12` is a single unit. Usually \
          written positionally — `device srv \"Compute node\" 38 41`.",
}];

pub const RADAR_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "categories",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "categories: \"<name>\" \"<name>\" …",
        doc: "One axis per name, clockwise from the top, in the order each \
              series writes its numbers. The same word `bar` uses, because it \
              is the same idea: the named slots the numbers line up with.",
    },
    PropDef {
        name: "radius",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "radius: <number>",
        doc: "Distance from the centre to the outer ring. 140 by default.",
    },
    PropDef {
        name: "range",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "range: <low> <high>",
        doc: "The scale every axis shares. Taken from the data when it is not \
              written — one scale, because a radar whose axes disagreed about \
              what a ring means would make its shape a lie.",
    },
    PropDef {
        name: "legend",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "legend: <on|off>",
        doc: "Name the series beside the diagram. On when there is more than one.",
    },
];

pub const JOURNEY_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "actors",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "actors: \"<name>\" \"<name>\" …",
        doc: "Who is being followed through the journey, in the order their \
              scores are written on each step. One line per actor is drawn.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "How wide the journey runs and how tall the score band is.",
    },
    PropDef {
        name: "range",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "range: <low> <high>",
        doc: "The score scale. `1 5` by default; the axis is labelled at every \
              whole step, because a satisfaction score has no meaning between \
              two of them.",
    },
    PropDef {
        name: "legend",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "legend: <on|off>",
        doc: "Name the actors beside the diagram. On when there is more than one.",
    },
];

pub const JOURNEY_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "scores",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "scores: <n> <n> …",
    doc: "One score per actor, in the order `actors` declares them. Usually \
          written positionally — `step pay \"Pay\" 2 4`.",
}];

pub const KANBAN_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the board.",
    },
    PropDef {
        name: "column",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "column: <width>",
        doc: "Width of one column. 220 by default.",
    },
];

pub const KANBAN_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "limit",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "limit: <count>",
    doc: "The work-in-progress limit of a **column** — the number of cards it \
          is allowed to hold. `check` reports a column over its limit, which \
          is the one thing a board is kept for. Written on the `group`, not on \
          a card; on a card it means nothing and is reported as such.",
}];

pub const REQ_LINKS: &[&str] = &["derive", "satisfy", "verify", "refine", "trace", "copy"];

pub const REQ_RISKS: &[&str] = &["low", "medium", "high"];

pub const REQ_VERIFY: &[&str] = &["analysis", "demonstration", "inspection", "test"];

pub const REQUIREMENT_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the diagram.",
}];

pub const REQUIREMENT_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "risk",
        alias: false,
        ctx: Ctx::Node,
        words: REQ_RISKS,
        syntax: "risk: <low|medium|high>",
        doc: "How likely this requirement is to go unmet. Drawn under the \
              name, not as a colour — risk is ordinal and the categorical \
              palette would imply it is not.",
    },
    PropDef {
        name: "verify",
        alias: false,
        ctx: Ctx::Node,
        words: REQ_VERIFY,
        syntax: "verify: <analysis|demonstration|inspection|test>",
        doc: "How the requirement will be shown to be met. A requirement with \
              no stated method is one nobody has planned to check.",
    },
    PropDef {
        name: "link",
        alias: false,
        ctx: Ctx::Edge,
        words: REQ_LINKS,
        syntax: "link: <derive|satisfy|verify|refine|trace|copy>",
        doc: "What the relation asserts, read tail to head — `r2 -> r1 derive` \
              is r2 derives from r1. Drawn in guillemets on the line. Usually \
              written positionally.",
    },
];

pub const ER_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "from-card",
        alias: false,
        ctx: Ctx::Edge,
        words: CARDINALITIES,
        syntax: "from-card: <one|many|zero-one|zero-many|one-many>",
        doc: "Cardinality at the tail of a relation, drawn as a crow's foot \
              against that entity. Usually written positionally — \
              `order -> line \"contains\" one many`.",
    },
    PropDef {
        name: "to-card",
        alias: false,
        ctx: Ctx::Edge,
        words: CARDINALITIES,
        syntax: "to-card: <one|many|zero-one|zero-many|one-many>",
        doc: "Cardinality at the head of a relation. A declared cardinality \
              replaces the arrowhead at that end.",
    },
];

// ---------------------------------------------------------------- archetypes

/// A diagram genre. `free` is the node-and-edge graph; the others reinterpret
/// the same statements as chart data, so the style cascade, the validator and
/// the editor support all carry over unchanged.
pub struct ArchetypeDef {
    pub name: &'static str,
    /// What this archetype calls its elements: `node`, `slice`, `field`.
    /// `node` stays a universal synonym everywhere.
    pub item: &'static str,
    pub doc: &'static str,
    /// Settings the `type X { … }` block accepts.
    pub props: &'static [PropDef],
    /// Properties this archetype adds to its *items*, or refines. Looked up
    /// before the global table, so a genre can give a shared name its own
    /// word list — `kind` means one thing to `state` and another to `block` —
    /// without either leaking into the other's completion.
    pub item_props: &'static [PropDef],
    /// Values written after the label, in order, and the property each one
    /// fills. `slice a "A" 4820` is sugar for `slice a "A" { value: 4820 }`.
    pub slots: &'static [Slot],
    /// Whether an item in this genre carries **member lines** — a `- ` or `+ `
    /// line in its block — drawn as a ruled compartment under the name.
    /// Declared here rather than checked against a list of archetype names in
    /// the layout, because such a list is one nobody remembers to extend.
    pub members: bool,
    /// Further keywords, each carrying its own positional shape (`D-023`).
    /// The form is chosen by the **word the author wrote**, never by how many
    /// values follow it: two shapes of the same length would be
    /// indistinguishable, and the diagnostic could not say which was meant.
    pub alt_items: &'static [AltItem],
    /// The same, for edges: `a -> b 40` where the archetype's datum rides on
    /// the link rather than on what it joins.
    pub edge_slots: &'static [Slot],
}

/// A second word for the same archetype, with a positional shape of its own.
pub struct AltItem {
    pub item: &'static str,
    pub doc: &'static str,
    pub slots: &'static [Slot],
}

impl ArchetypeDef {
    /// Whether this archetype answers to the word the author wrote. `node` is
    /// the universal synonym everywhere.
    pub fn accepts(&self, written: &str) -> bool {
        written == self.item
            || written == "node"
            || self.alt_items.iter().any(|a| a.item == written)
    }

    /// The positional shape that word carries. `node` takes the primary form,
    /// which is what keeps the universal synonym meaningful.
    pub fn slots_for(&self, written: &str) -> &'static [Slot] {
        self.alt_items
            .iter()
            .find(|a| a.item == written)
            .map_or(self.slots, |a| a.slots)
    }

    /// Every word this archetype answers to, primary first. `node` is left
    /// out: it is a synonym, not a form, and offering it would put it in
    /// completion twice.
    pub fn items(&self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.item).chain(self.alt_items.iter().map(|a| a.item))
    }
}

pub struct Slot {
    pub prop: &'static str,
    /// How many values this slot takes. `VARIADIC` takes everything left.
    pub arity: usize,
}

/// A slot that swallows the rest of the line — a series of numbers.
pub const VARIADIC: usize = usize::MAX;

pub const ARCHETYPES: &[ArchetypeDef] = &[
    ArchetypeDef {
        name: "free",
        item: "node",
        doc: "Nodes and edges placed by anchors — the default when a file \
              declares no `type`.",
        props: &[],
        item_props: &[],
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "pie")]
    ArchetypeDef {
        name: "pie",
        item: "slice",
        doc: "A pie or donut chart. Each slice needs a value; its `fill` \
              overrides the automatic palette.",
        props: PIE_PROPS,
        item_props: PIE_ITEM_PROPS,
        slots: &[Slot {
            prop: "value",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "gantt")]
    ArchetypeDef {
        name: "gantt",
        item: "task",
        doc: "Bars on a calendar. A task says when it starts and either when \
              it ends or how long it runs; a `group` becomes a section.",
        props: GANTT_PROPS,
        item_props: GANTT_ITEM_PROPS,
        slots: &[Slot {
            prop: "start",
            arity: 2,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "class")]
    ArchetypeDef {
        name: "class",
        item: "class",
        doc: "Boxes with a name and a list of members, placed by anchors like \
              `free`. A bare string in the block is a member.",
        props: CLASS_PROPS,
        item_props: &[],
        slots: &[],
        members: true,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "timeline")]
    ArchetypeDef {
        name: "timeline",
        item: "event",
        doc: "Events on a date line. Each event needs a date; they are drawn \
              in date order with their labels staggered above and below, so a \
              cluster of dates stays readable without hand-placing anything.",
        props: TIMELINE_PROPS,
        item_props: TIMELINE_ITEM_PROPS,
        slots: &[Slot {
            prop: "date",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "block")]
    ArchetypeDef {
        name: "block",
        item: "block",
        doc: "Architecture and C4-style component views: typed elements with a \
              technology and a description, nested with `group`, joined by \
              relations that carry a protocol. Placed by anchors exactly as in \
              `free` — the archetype supplies the vocabulary and the shapes.",
        props: BLOCK_PROPS,
        item_props: BLOCK_ITEM_PROPS,
        slots: &[Slot {
            prop: "kind",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "er")]
    ArchetypeDef {
        name: "er",
        item: "entity",
        doc: "Entities and the relations between them. A bare string in the \
              block is a column, exactly as in `class`; what `er` adds is \
              cardinality at each end of a relation, written after the label \
              and read at the end nearest the entity it describes — \
              `order -> line \"contains\" one many` is one order, many lines.",
        props: ER_PROPS,
        item_props: ER_ITEM_PROPS,
        slots: &[],
        members: true,
        alt_items: &[],
        edge_slots: &[
            Slot {
                prop: "from-card",
                arity: 1,
            },
            Slot {
                prop: "to-card",
                arity: 1,
            },
        ],
    },
    #[cfg(feature = "state")]
    ArchetypeDef {
        name: "state",
        item: "state",
        doc: "A state machine. States are placed by anchors, exactly as in \
              `free` — the archetype supplies the vocabulary and the shapes, \
              not a layout engine. Nest them with `group`.",
        props: STATE_PROPS,
        item_props: STATE_ITEM_PROPS,
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "sequence")]
    ArchetypeDef {
        name: "sequence",
        item: "participant",
        doc: "Participants across the top, messages down the page in the order \
              they are written. A `group` frames the messages inside it.",
        props: SEQUENCE_PROPS,
        item_props: &[],
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "xy")]
    ArchetypeDef {
        name: "xy",
        item: "series",
        doc: "Lines, areas or dots over a shared x axis. One series per \
              statement, its numbers evenly spaced across the plot — or \
              `scatter`, which reads coordinates instead.",
        props: XY_PROPS,
        item_props: XY_ITEM_PROPS,
        slots: &[Slot {
            prop: "points",
            arity: VARIADIC,
        }],
        members: false,
        alt_items: &[AltItem {
            item: "scatter",
            doc: "The same plot, read as coordinates: values are taken two at \
                  a time, so `scatter north 1 4 3 8` places points at (1, 4) \
                  and (3, 8). Its x values are the ones written, not the \
                  position in the line — which is the whole difference from \
                  `series`.",
            slots: &[Slot {
                prop: "pairs",
                arity: VARIADIC,
            }],
        }],
        edge_slots: &[],
    },
    #[cfg(feature = "bar")]
    ArchetypeDef {
        name: "bar",
        item: "series",
        doc: "Bars grouped or stacked across named categories. One series per \
              statement, its numbers in category order.",
        props: BAR_PROPS,
        item_props: SERIES_ITEM_PROPS,
        slots: &[Slot {
            prop: "points",
            arity: VARIADIC,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "sankey")]
    ArchetypeDef {
        name: "sankey",
        item: "node",
        doc: "Flows between stages, sized by magnitude. The value rides on the \
              link, not on what it joins: a node's height is whatever passes \
              through it.",
        props: SANKEY_PROPS,
        item_props: SANKEY_ITEM_PROPS,
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[Slot {
            prop: "value",
            arity: 1,
        }],
    },
    #[cfg(feature = "wave")]
    ArchetypeDef {
        name: "wave",
        item: "signal",
        doc: "A timing diagram. Each signal carries a wave string, one \
              character per period, in the WaveDrom alphabet.",
        props: WAVE_PROPS,
        item_props: WAVE_ITEM_PROPS,
        slots: &[Slot {
            prop: "wave",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "quadrant")]
    ArchetypeDef {
        name: "quadrant",
        item: "point",
        doc: "Points on two axes, divided into four named regions. A Wardley \
              map is this archetype with its own axis names.",
        props: QUADRANT_PROPS,
        item_props: QUADRANT_ITEM_PROPS,
        slots: &[
            Slot {
                prop: "x",
                arity: 1,
            },
            Slot {
                prop: "y",
                arity: 1,
            },
        ],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "treemap")]
    ArchetypeDef {
        name: "treemap",
        item: "box",
        doc: "Nested rectangles whose area is proportional to a value. The \
              hierarchy is group nesting; a group's area is the sum of what \
              it contains.",
        props: TREEMAP_PROPS,
        item_props: TREEMAP_ITEM_PROPS,
        slots: &[Slot {
            prop: "value",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "packet")]
    ArchetypeDef {
        name: "packet",
        item: "field",
        doc: "A packet / bitfield diagram. Each field needs an inclusive bit \
              range; one crossing a row boundary is split across rows.",
        props: PACKET_PROPS,
        item_props: PACKET_ITEM_PROPS,
        slots: &[Slot {
            prop: "bits",
            arity: 2,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "gitgraph")]
    ArchetypeDef {
        name: "gitgraph",
        item: "commit",
        doc: "Commits, branches and merges. Parentage is the edges — \
              `c1 -> c2` is c2's parent — and a commit with two parents is a \
              merge. The rails are the archetype's to assign; what the author \
              declares is which branch a commit is on and what came before it.",
        props: GITGRAPH_PROPS,
        item_props: GITGRAPH_ITEM_PROPS,
        slots: &[Slot {
            prop: "branch",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "orgchart")]
    ArchetypeDef {
        name: "orgchart",
        item: "position",
        doc: "Reporting lines, laid out top-down. The hierarchy comes from the \
              edges — `ceo -> cto` — and the archetype places every box, so \
              adding a report never means renumbering anything.",
        props: TREE_PROPS,
        item_props: &[],
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "mindmap")]
    ArchetypeDef {
        name: "mindmap",
        item: "topic",
        doc: "A root with branches either side of it. The same tree the \
              orgchart draws, arranged outward instead of downward, so the \
              root stays central however lopsided the tree is.",
        props: TREE_PROPS,
        item_props: &[],
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "treeview")]
    ArchetypeDef {
        name: "treeview",
        item: "entry",
        doc: "The same tree as indented rows, one per line — a file listing, a \
              menu, a taxonomy. Depth is the indent and order is the order \
              written.",
        props: TREE_PROPS,
        item_props: &[],
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "harness")]
    ArchetypeDef {
        name: "harness",
        item: "connector",
        doc: "Wiring harnesses. A connector's pins are its member lines — \
              `- 1: +12V` — and a wire is an edge carrying the pin it lands on \
              at each end. Both mechanisms already existed; what this genre \
              adds is that a wire is checked against the pins that are really \
              there (`D-025`).",
        props: HARNESS_PROPS,
        item_props: HARNESS_ITEM_PROPS,
        slots: &[],
        members: true,
        alt_items: &[],
        edge_slots: &[Slot {
            prop: "wire",
            arity: 2,
        }],
    },
    #[cfg(feature = "venn")]
    ArchetypeDef {
        name: "venn",
        item: "set",
        doc: "Overlapping sets. Two or three of them: with two, the lens is \
              solved so its **area** is the number you wrote; with three the \
              overlaps are labelled but arranged rather than solved, because \
              three circles cannot in general honour all seven regions at \
              once. Four is refused rather than drawn wrongly.",
        props: VENN_PROPS,
        item_props: VENN_ITEM_PROPS,
        slots: &[Slot {
            prop: "count",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[Slot {
            prop: "shared",
            arity: 1,
        }],
    },
    #[cfg(feature = "bpmn")]
    ArchetypeDef {
        name: "bpmn",
        item: "task",
        doc: "A business process. Tasks, gateways, events, data artefacts and \
              the flows between them, with pools and lanes as `group`s. \
              Placed by anchors exactly as in `free`: the archetype supplies \
              the vocabulary, the shapes and the diagnostics, and the author \
              still says where everything goes. Four words, because four \
              things have four positional shapes (`D-023`) — a task has a \
              kind, an event has a stage and a trigger, a gateway has a gate \
              and a datum has an artefact.",
        props: BPMN_PROPS,
        item_props: BPMN_ITEM_PROPS,
        slots: &[Slot {
            prop: "kind",
            arity: 1,
        }],
        members: false,
        alt_items: &[
            AltItem {
                item: "event",
                doc: "Something that happens: `event got \"Order received\" start message`. \
                      The stage comes first and the trigger second, because a \
                      stage is what every event has and a trigger is what some do.",
                slots: &[
                    Slot {
                        prop: "stage",
                        arity: 1,
                    },
                    Slot {
                        prop: "trigger",
                        arity: 1,
                    },
                ],
            },
            AltItem {
                item: "gateway",
                doc: "A branch or a join: `gateway ok \"Approved?\" exclusive`.",
                slots: &[Slot {
                    prop: "gate",
                    arity: 1,
                }],
            },
            AltItem {
                item: "data",
                doc: "What the process reads or writes: `data inv \"Invoice\" object`.",
                slots: &[Slot {
                    prop: "artifact",
                    arity: 1,
                }],
            },
        ],
        edge_slots: &[Slot {
            prop: "flow",
            arity: 1,
        }],
    },
    #[cfg(feature = "network")]
    ArchetypeDef {
        name: "network",
        item: "host",
        doc: "Network topology: hosts, links and the segments that hold them. \
              Placed by anchors exactly as in `free` — the archetype supplies \
              the vocabulary, the shapes and the diagnostics, and a `group` is \
              a segment or a zone.",
        props: NETWORK_PROPS,
        item_props: NETWORK_ITEM_PROPS,
        slots: &[Slot {
            prop: "role",
            arity: 1,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[Slot {
            prop: "speed",
            arity: 1,
        }],
    },
    #[cfg(feature = "rack")]
    ArchetypeDef {
        name: "rack",
        item: "device",
        doc: "A rack elevation. Units are numbered from 1 at the **bottom**, \
              as a rack is, and a device occupies an inclusive span of them. \
              Empty units are left empty: unlike a packet, a rack with gaps is \
              a rack with room, not a rack that does not add up.",
        props: RACK_PROPS,
        item_props: RACK_ITEM_PROPS,
        slots: &[Slot {
            prop: "u",
            arity: 2,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "radar")]
    ArchetypeDef {
        name: "radar",
        item: "series",
        doc: "Axes radiating from a centre, one closed polygon per series. \
              Every axis shares one scale: the shape a reader compares is only \
              meaningful if a ring means the same thing all the way round.",
        props: RADAR_PROPS,
        item_props: SERIES_ITEM_PROPS,
        slots: &[Slot {
            prop: "points",
            arity: VARIADIC,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "journey")]
    ArchetypeDef {
        name: "journey",
        item: "step",
        doc: "A user journey: steps in the order they are taken, and how each \
              actor felt at each one. `group`s are its sections, drawn as a \
              band above the steps they hold. One line per actor, so the place \
              where two people diverge is the thing you see first.",
        props: JOURNEY_PROPS,
        item_props: JOURNEY_ITEM_PROPS,
        slots: &[Slot {
            prop: "scores",
            arity: VARIADIC,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "kanban")]
    ArchetypeDef {
        name: "kanban",
        item: "card",
        doc: "A board: `group`s are its columns, in the order they are \
              declared, and the cards inside each one stack in the order they \
              are declared. A column may carry a `limit`, and a column over it \
              is reported — a board nobody is told is over its limit is a \
              board that is only a picture.",
        props: KANBAN_PROPS,
        item_props: KANBAN_ITEM_PROPS,
        slots: &[],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
    #[cfg(feature = "requirements")]
    ArchetypeDef {
        name: "requirements",
        item: "requirement",
        doc: "Requirements and the typed links between them. A bare string in \
              the block is a line of the requirement — its identifier, then \
              what it demands — exactly as in `class`. What this genre adds is \
              the relation vocabulary: a link says what it asserts, read tail \
              to head, and is drawn in guillemets on the line. Placement stays \
              declared, as in `free`.",
        props: REQUIREMENT_PROPS,
        item_props: REQUIREMENT_ITEM_PROPS,
        slots: &[],
        members: true,
        alt_items: &[],
        edge_slots: &[Slot {
            prop: "link",
            arity: 1,
        }],
    },
    #[cfg(feature = "bytefield")]
    ArchetypeDef {
        name: "bytefield",
        item: "field",
        doc: "A wire-format diagram at byte granularity — `packet` with the \
              ruler counting bytes rather than bits. Each field needs an \
              inclusive byte range; one crossing a row boundary is split \
              across rows.",
        props: BYTEFIELD_PROPS,
        item_props: BYTEFIELD_ITEM_PROPS,
        slots: &[Slot {
            prop: "bytes",
            arity: 2,
        }],
        members: false,
        alt_items: &[],
        edge_slots: &[],
    },
];

pub const PIE_LABELS: &[&str] = &["percent", "value", "both", "none"];
pub const TOGGLE: &[&str] = &["on", "off", "true", "false"];

pub const PIE_PROPS: &[PropDef] = &[
    PropDef {
        name: "from",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "from: \"<file.csv>\"",
        doc: "Read the slices from a CSV, resolved relative to this file. The \
              header names the columns: `name` (or `label`) and `value`. \
              Carries a consequence worth knowing — a file that reads the disk \
              at render time is no longer autonomous, so `check` can pass and \
              the render still fail.",
    },
    PropDef {
        name: "top",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "top: <number>",
        doc: "Keep the largest n slices and fold the rest into one. The \
              palette has eight colours and `check` already advises this when \
              a pie has more; `top` is what lets the file do it instead of \
              someone doing the arithmetic by hand outside the tool.",
    },
    PropDef {
        name: "other",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "other: <text>",
        doc: "What the folded slice is called. `\"Other\"` unless the file \
              says otherwise.",
    },
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the chart.",
    },
    PropDef {
        name: "radius",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "radius: <number>",
        doc: "Outer radius in user units.",
    },
    PropDef {
        name: "donut",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "donut: <0..1>",
        doc: "Inner hole as a fraction of the radius. `0` is a full pie.",
    },
    PropDef {
        name: "start",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "start: <degrees>",
        doc: "Angle the first slice starts at. `0` is twelve o'clock.",
    },
    PropDef {
        name: "labels",
        alias: false,
        ctx: Ctx::Both,
        words: PIE_LABELS,
        syntax: "labels: <percent|value|both|none>",
        doc: "What to write on each slice. Slices too thin to read are skipped.",
    },
    PropDef {
        name: "legend",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "legend: <on|off>",
        doc: "Swatch-and-name list beside the chart. On by default — identity \
              must never rest on color alone.",
    },
];

pub const GANTT_TICKS: &[&str] = &["day", "week", "month"];

pub const GANTT_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the chart.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <row height>",
        doc: "How wide the calendar runs and how tall one task is.",
    },
    PropDef {
        name: "ticks",
        alias: false,
        ctx: Ctx::Both,
        words: GANTT_TICKS,
        syntax: "ticks: <day|week|month>",
        doc: "How often the calendar is marked. Chosen from the span if unset.",
    },
    PropDef {
        name: "today",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "today: <date>",
        doc: "Draws a line at one date, for reading progress against it.",
    },
];

pub const CLASS_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the diagram.",
}];

/// What an element *is*, in the C4 sense. Drives its shape and its colour, so
/// a reader tells a person from a datastore without reading the label.
pub const BLOCK_KINDS: &[&str] = &[
    "person",
    "system",
    "container",
    "component",
    "database",
    "queue",
    "external",
];

pub const TIMELINE_TICKS: &[&str] = &["day", "week", "month", "year"];

pub const TIMELINE_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <row-height>",
        doc: "How wide the date line runs, and how much room one row of \
              staggered labels takes.",
    },
    PropDef {
        name: "ticks",
        alias: false,
        ctx: Ctx::Both,
        words: TIMELINE_TICKS,
        syntax: "ticks: <day|week|month|year>",
        doc: "How often the axis is labelled. Chosen from the span when it is \
              not declared.",
    },
];

pub const TIMELINE_ITEM_PROPS: &[PropDef] = &[PropDef {
    name: "date",
    alias: false,
    ctx: Ctx::Node,
    words: &[],
    syntax: "date: <date>",
    doc: "When the event happened. Usually written positionally — \
          `event launch \"Launch\" 2026-03-15`. Events are drawn in date \
          order, and their labels staggered above and below the line so a \
          cluster stays readable.",
}];

pub const BLOCK_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the diagram.",
}];

/// `block` refines `kind` and adds the two lines a C4 box carries under its
/// name. Kept off the global table so they never appear in another genre.
pub const BLOCK_ITEM_PROPS: &[PropDef] = &[
    PropDef {
        name: "kind",
        alias: false,
        ctx: Ctx::Node,
        words: BLOCK_KINDS,
        syntax: "kind: <person|system|container|component|database|queue|external>",
        doc: "What this element is. Usually written positionally — \
              `block api \"API\" container`. `external` draws a dashed, muted \
              box for something outside the system being described.",
    },
    PropDef {
        name: "tech",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "tech: <text>",
        doc: "The technology, drawn in brackets under the name — \
              `[Container: Rust]`. On an edge it reads as the protocol: \
              `web -> api \"reads\" { tech: \"HTTPS/JSON\" }`.",
    },
    PropDef {
        name: "technology",
        alias: true,
        ctx: Ctx::Both,
        words: &[],
        syntax: "technology: <text>",
        doc: "Alias of `tech`.",
    },
    // `description` and `desc` used to live here. They are on the shared
    // table now, because every genre emits one as an SVG `<title>` — `block`
    // is simply the genre that also *draws* it, under the technology line,
    // sizing the box around it. One property, one meaning (`D-015`), and
    // `D-022` forbids it appearing in both tables.
];

pub const ER_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the diagram.",
}];

pub const STATE_KINDS: &[&str] = &["state", "initial", "final", "choice"];

pub const STATE_PROPS: &[PropDef] = &[PropDef {
    name: "title",
    alias: false,
    ctx: Ctx::Both,
    words: &[],
    syntax: "title: <text>",
    doc: "Heading drawn above the machine.",
}];

pub const SEQUENCE_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "column",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "column: <number>",
        doc: "Width of a participant's column. 150 by default.",
    },
    PropDef {
        name: "row",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "row: <number>",
        doc: "Vertical space one message takes. 46 by default.",
    },
    PropDef {
        name: "numbered",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "numbered: <on|off>",
        doc: "Number the messages in order.",
    },
];

pub const XY_MARKS: &[&str] = &["line", "area", "dot"];

pub const XY_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the chart.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "The plot area, before axis labels and legend. 560 by 320 by default.",
    },
    PropDef {
        name: "categories",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "categories: \"<first>\" \"<second>\" …",
        doc: "Labels for the x positions. One per point, in order.",
    },
    PropDef {
        name: "x-axis",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "x-axis: <min> <max>",
        doc: "A numeric x span instead of named categories; the points are \
              spread evenly across it.",
    },
    PropDef {
        name: "y-axis",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "y-axis: <min> <max>",
        doc: "Force the value axis. Otherwise it is rounded outward from the \
              data to human numbers.",
    },
    PropDef {
        name: "mark",
        alias: false,
        ctx: Ctx::Both,
        words: XY_MARKS,
        syntax: "mark: <line|area|dot>",
        doc: "How a series is drawn. `line` by default; `area` fills beneath \
              it, `dot` plots the points alone.",
    },
    PropDef {
        name: "legend",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "legend: <on|off>",
        doc: "Swatch-and-name list. Shown from two series up.",
    },
];

pub const BAR_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the chart.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "The plot area, before axis labels and legend. 560 by 320 by default.",
    },
    PropDef {
        name: "categories",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "categories: \"<first>\" \"<second>\" …",
        doc: "What the bars stand along. Each series gives one number per \
              category, in this order.",
    },
    PropDef {
        name: "stack",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "stack: <on|off>",
        doc: "Stack the series instead of standing them side by side.",
    },
    PropDef {
        name: "y-axis",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "y-axis: <min> <max>",
        doc: "Force the value axis. Otherwise it is rounded outward from the \
              data to human numbers.",
    },
    PropDef {
        name: "legend",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "legend: <on|off>",
        doc: "Swatch-and-name list. Shown from two series up; identity must \
              never rest on colour alone.",
    },
];

pub const SANKEY_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "The area the flows fill. 680 by 400 by default.",
    },
    PropDef {
        name: "gap",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "gap: <number>",
        doc: "Space between stacked nodes in a column.",
    },
    PropDef {
        name: "width",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "width: <number>",
        doc: "Thickness of a node bar.",
    },
    PropDef {
        name: "labels",
        alias: false,
        ctx: Ctx::Both,
        words: TREEMAP_LABELS,
        syntax: "labels: <name|value|both|none>",
        doc: "What to write beside each node.",
    },
];

/// The WaveDrom wave alphabet, as supported here.
pub const WAVE_ALPHABET: &str = "01lhpnPNxz=23456789.|";

pub const WAVE_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the lanes.",
    },
    PropDef {
        name: "period",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "period: <number>",
        doc: "Width of one wave character. 40 by default.",
    },
    PropDef {
        name: "lane",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "lane: <height> <gap>",
        doc: "Height of a signal lane and the space beneath it.",
    },
    PropDef {
        name: "ruler",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "ruler: <on|off>",
        doc: "Period numbers above the lanes. On by default.",
    },
    PropDef {
        name: "every",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "every: <number>",
        doc: "Label a ruler tick every N periods. 4 by default.",
    },
    PropDef {
        name: "sketch",
        alias: false,
        ctx: Ctx::Both,
        words: &["on", "off"],
        syntax: "sketch: <on|off>",
        doc: "Draw as if by hand. A sketched architecture says \"this is a \
              proposal\" where a crisp one says \"this is decided\" — \
              information rather than ornament. Deterministic: the wobble is a \
              hash of each shape's own geometry, so the same file always \
              produces the same file.",
    },
    PropDef {
        name: "grid",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "grid: <on|off>",
        doc: "Vertical lines on the period boundaries. On by default.",
    },
];

pub const QUADRANT_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the chart.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "The plot area. 460 square by default.",
    },
    PropDef {
        name: "range",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "range: <min> <max>",
        doc: "The span both axes cover. `0 1` by default.",
    },
    PropDef {
        name: "x-axis",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "x-axis: \"<left>\" \"<right>\"",
        doc: "What the horizontal axis means at each end.",
    },
    PropDef {
        name: "y-axis",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "y-axis: \"<bottom>\" \"<top>\"",
        doc: "What the vertical axis means at each end.",
    },
    PropDef {
        name: "quadrants",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "quadrants: \"<top-right>\" \"<top-left>\" \"<bottom-left>\" \"<bottom-right>\"",
        doc: "Names for the four regions, anticlockwise from the top right.",
    },
    PropDef {
        name: "labels",
        alias: false,
        ctx: Ctx::Both,
        words: &["name", "none"],
        syntax: "labels: <name|none>",
        doc: "Whether each point carries its name.",
    },
];

pub const TREEMAP_LABELS: &[&str] = &["name", "value", "both", "none"];

pub const TREEMAP_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the map.",
    },
    PropDef {
        name: "size",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "The rectangle the map fills. 640 by 400 by default.",
    },
    PropDef {
        name: "gap",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "gap: <number>",
        doc: "Surface showing between sibling tiles. Adjacent fills must never \
              touch, so this is 2 by default and never 0.",
    },
    PropDef {
        name: "header",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "header: <number>",
        doc: "Height reserved inside a group's tile for its name.",
    },
    PropDef {
        name: "labels",
        alias: false,
        ctx: Ctx::Both,
        words: TREEMAP_LABELS,
        syntax: "labels: <name|value|both|none>",
        doc: "What to write on each tile. Tiles too small to read are skipped.",
    },
];

pub const BYTEFIELD_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "width",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "width: <bytes>",
        doc: "Bytes per row. 16 by default — the width a hex dump is read at.",
    },
    PropDef {
        name: "cell",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "cell: <width> <height>",
        doc: "Width of one byte and the height of a row.",
    },
    PropDef {
        name: "ruler",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "ruler: <on|off>",
        doc: "Byte offsets above each row. On by default.",
    },
];

pub const PACKET_PROPS: &[PropDef] = &[
    PropDef {
        name: "title",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "title: <text>",
        doc: "Heading drawn above the diagram.",
    },
    PropDef {
        name: "width",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "width: <bits>",
        doc: "Bits per row. 32 by default.",
    },
    PropDef {
        name: "cell",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "cell: <width> <height>",
        doc: "Width of one bit and the height of a row.",
    },
    PropDef {
        name: "ruler",
        alias: false,
        ctx: Ctx::Both,
        words: TOGGLE,
        syntax: "ruler: <on|off>",
        doc: "Bit indices above each row. On by default.",
    },
];

pub fn archetype(name: &str) -> Option<&'static ArchetypeDef> {
    ARCHETYPES.iter().find(|a| a.name == name)
}

/// Categorical slices, in fixed order — never cycled, never generated.
/// Validated for both surfaces (adjacent-pair CVD ΔE 9.1 light / 8.4 dark).
/// Past eight, `check` tells you to merge the tail rather than invent a hue.
pub const PALETTE_LIGHT: &[&str] = &[
    "#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#008300", "#4a3aa7", "#e34948",
];

pub const PALETTE_DARK: &[&str] = &[
    "#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#008300", "#9085e9", "#e66767",
];

/// Keywords that open a statement.
pub const KEYWORDS: &[(&str, &str)] = &[
    (
        "include",
        "Merge another file — `include \"house.bds\"`, or `include \"core.bd\" \
         as core` to bring its names in under a prefix. Without a prefix every \
         name must be unique across the set; a clash names both files.",
    ),
    (
        "legend",
        "A key to the colours: `legend right-of core gap 30`. Lists the style \
         classes that declare a `legend:` text, so it can only ever describe a \
         colour the file actually used. The chart archetypes build their own \
         from their series and need no statement.",
    ),
    (
        "note",
        "Text that belongs to no box: `note \"Figures are March\" below api gap 20`. \
         Placed by the same anchors as everything else, and it declares no name \
         — so nothing can point at it, which is the whole difference between a \
         note and a box with no border.",
    ),
    (
        "align-group",
        "Give a set of boxes one cross-axis coordinate: `align-group top a b c`. \
         A pass over the solved layout, never a reordering — the first name \
         decides, and an explicit `at` wins.",
    ),
    (
        "same-width",
        "Give a set of boxes one width: `same-width a b c`. The widest wins, \
         because shrinking to match a narrower neighbour clips the label. \
         Applied before placement, so nothing is pushed on top of anything.",
    ),
    ("same-height", "The same, for height."),
    (
        "component",
        "A fragment declared once and placed many times — `component service \
         { … }`, then `use service as web`. Its holes are bound by each \
         placing, and its names arrive under the placing's prefix.",
    ),
    (
        "use",
        "Place a component: `use service as web right-of jobs { label: \"Web\" }`. \
         The instance is a frame, so it is anchored like any other group.",
    ),
    (
        "type",
        "Diagram archetype, declared once at the top of the file — `type pie`. \
         Defaults to `free`.",
    ),
    (
        "graph",
        "Diagram-wide settings: padding, gap, background, flow.",
    ),
    (
        "group",
        "A container. Its block holds properties and nested statements; its \
         rectangle is the bounding box of its members plus its `padding`.",
    ),
    (
        "style",
        "A named, reusable style block. `style x extends y { … }`.",
    ),
    (
        "node",
        "Declare a node: `node <id> \"<label>\" .class <placement> { … }`.",
    ),
];

pub const PLACEMENTS: &[(&str, &str)] = &[
    ("at", "Absolute top-left corner: `at (x, y)`."),
    ("right-of", "Place to the right of another node."),
    ("left-of", "Place to the left of another node."),
    ("above", "Place above another node."),
    ("below", "Place below another node."),
    ("gap", "Distance to the anchor, overriding the graph `gap`."),
    ("align", "Cross-axis alignment against the anchor."),
    ("extends", "Inherit every property of another style."),
];

pub const ALIGNMENTS: &[&str] = &["left", "right", "center", "middle", "top", "bottom"];
