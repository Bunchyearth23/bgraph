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

pub const PROPS: &[PropDef] = &[
    PropDef {
        name: "fill",
        ctx: Ctx::Node,
        words: &[],
        syntax: "fill: <color>",
        doc: "Interior color of the shape.",
    },
    PropDef {
        name: "background",
        ctx: Ctx::Node,
        words: &[],
        syntax: "background: <color>",
        doc: "Alias of `fill`.",
    },
    PropDef {
        name: "stroke",
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "stroke: <color> <width> <solid|dashed|dotted>",
        doc: "Outline. Accepts a color, a width and a dash pattern in any order; \
              omit whichever you don't need.",
    },
    PropDef {
        name: "border",
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "border: <color> <width> <pattern>",
        doc: "Alias of `stroke`.",
    },
    PropDef {
        name: "color",
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "color: <color> <width> <pattern>",
        doc: "Alias of `stroke`. For text color use `font-color`.",
    },
    PropDef {
        name: "stroke-width",
        ctx: Ctx::Both,
        words: &[],
        syntax: "stroke-width: <number>",
        doc: "Outline thickness in user units.",
    },
    PropDef {
        name: "weight",
        ctx: Ctx::Both,
        words: &[],
        syntax: "weight: <number>",
        doc: "Alias of `stroke-width`.",
    },
    PropDef {
        name: "dash",
        ctx: Ctx::Both,
        words: DASHES,
        syntax: "dash: <solid|dashed|dotted> | dash: <on> <off>",
        doc: "Dash pattern. Named, or explicit on/off lengths such as `dash: 6 3`.",
    },
    PropDef {
        name: "radius",
        ctx: Ctx::Node,
        words: &[],
        syntax: "radius: <number>",
        doc: "Corner rounding — applies to the `rect` shape.",
    },
    PropDef {
        name: "corner",
        ctx: Ctx::Node,
        words: &[],
        syntax: "corner: <number>",
        doc: "Alias of `radius`.",
    },
    PropDef {
        name: "shape",
        ctx: Ctx::Node,
        words: SHAPES,
        syntax: "shape: <rect|stadium|ellipse|diamond|hexagon|cylinder>",
        doc: "Outline geometry. Edge endpoints are clipped against it.",
    },
    PropDef {
        name: "size",
        ctx: Ctx::Node,
        words: &[],
        syntax: "size: <width> <height>",
        doc: "Fixed size. Without it the box is sized from the label and `padding`.",
    },
    PropDef {
        name: "width",
        ctx: Ctx::Node,
        words: &[],
        syntax: "width: <number>",
        doc: "Fixed width; the height still follows the label.",
    },
    PropDef {
        name: "height",
        ctx: Ctx::Node,
        words: &[],
        syntax: "height: <number>",
        doc: "Fixed height; the width still follows the label.",
    },
    PropDef {
        name: "padding",
        ctx: Ctx::Node,
        words: &[],
        syntax: "padding: <number>",
        doc: "Inset between the label and the outline; drives the auto-size.",
    },
    PropDef {
        name: "opacity",
        ctx: Ctx::Both,
        words: &[],
        syntax: "opacity: <0..1>",
        doc: "Overall transparency.",
    },
    PropDef {
        name: "font",
        ctx: Ctx::Both,
        words: &[],
        syntax: "font: <size> \"<family>\" <color> <weight>",
        doc: "Shorthand read by value kind, so the order is free: a number is the \
              size, a string the family, a color the ink, a bare word the weight.",
    },
    PropDef {
        name: "font-size",
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-size: <number>",
        doc: "Text size in user units.",
    },
    PropDef {
        name: "font-family",
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-family: \"<family>\"",
        doc: "Font stack, emitted verbatim into the SVG.",
    },
    PropDef {
        name: "font-color",
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-color: <color>",
        doc: "Text color.",
    },
    PropDef {
        name: "text-color",
        ctx: Ctx::Both,
        words: &[],
        syntax: "text-color: <color>",
        doc: "Alias of `font-color`.",
    },
    PropDef {
        name: "font-weight",
        ctx: Ctx::Both,
        words: &[],
        syntax: "font-weight: <weight>",
        doc: "Any SVG weight — `400`, `bold`, `600`…",
    },
    PropDef {
        name: "arrow",
        ctx: Ctx::Edge,
        words: HEADS,
        syntax: "arrow: <solid|open|dot|none>",
        doc: "Arrow head style. The arrow token decides which ends carry one.",
    },
    PropDef {
        name: "head",
        ctx: Ctx::Edge,
        words: HEADS,
        syntax: "head: <solid|open|dot|none>",
        doc: "Alias of `arrow`.",
    },
    PropDef {
        name: "router",
        ctx: Ctx::Edge,
        words: ROUTERS,
        syntax: "router: <straight|ortho>",
        doc: "`straight` draws a direct line; `ortho` draws a right-angled elbow.",
    },
    PropDef {
        name: "route",
        ctx: Ctx::Edge,
        words: ROUTERS,
        syntax: "route: <straight|ortho>",
        doc: "Alias of `router`.",
    },
    PropDef {
        name: "shadow",
        ctx: Ctx::Node,
        words: &["on", "off", "true", "false"],
        syntax: "shadow: <on|off>",
        doc: "Soft drop shadow under the shape.",
    },
];

pub const GRAPH_PROPS: &[PropDef] = &[
    PropDef {
        name: "padding",
        ctx: Ctx::Both,
        words: &[],
        syntax: "padding: <number>",
        doc: "Margin between the drawing and the canvas edge.",
    },
    PropDef {
        name: "gap",
        ctx: Ctx::Both,
        words: &[],
        syntax: "gap: <number>",
        doc: "Default distance used by anchors that omit `gap`.",
    },
    PropDef {
        name: "background",
        ctx: Ctx::Both,
        words: &[],
        syntax: "background: <color>",
        doc: "Canvas fill. Also backs edge labels so they stay readable.",
    },
    PropDef {
        name: "bg",
        ctx: Ctx::Both,
        words: &[],
        syntax: "bg: <color>",
        doc: "Alias of `background`.",
    },
    PropDef {
        name: "flow",
        ctx: Ctx::Both,
        words: FLOWS,
        syntax: "flow: <right|left|down|up>",
        doc: "Direction used to place nodes that declare no placement.",
    },
    PropDef {
        name: "direction",
        ctx: Ctx::Both,
        words: FLOWS,
        syntax: "direction: <right|left|down|up>",
        doc: "Alias of `flow`.",
    },
];

pub fn lookup(name: &str, graph: bool) -> Option<&'static PropDef> {
    let table = if graph { GRAPH_PROPS } else { PROPS };
    table.iter().find(|p| p.name == name)
}

/// Keywords that open a statement.
pub const KEYWORDS: &[(&str, &str)] = &[
    (
        "graph",
        "Diagram-wide settings: padding, gap, background, flow.",
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
