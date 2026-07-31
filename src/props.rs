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

pub const PROPS: &[PropDef] = &[
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
        name: "value",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "value: <number>",
        doc: "Magnitude of a slice. Required by `type pie`, ignored elsewhere.",
    },
    PropDef {
        name: "points",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "points: <n> <n> …",
        doc: "One number per category, in order. Required by `type bar`.",
    },
    PropDef {
        name: "wave",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "wave: \"<characters>\"",
        doc: "One character per period: `0` `1` `l` `h` levels, `p` `n` clocks, \
              `x` unknown, `z` high-impedance, `=` and `2`–`9` data, `.` repeats \
              the previous, `|` a gap. Required by `type wave`.",
    },
    PropDef {
        name: "data",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "data: \"<first>\" \"<second>\" …",
        doc: "Names for the data segments of a wave, in order.",
    },
    PropDef {
        name: "x",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "x: <number>",
        doc: "Horizontal position. Required by `type quadrant`, ignored elsewhere.",
    },
    PropDef {
        name: "y",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "y: <number>",
        doc: "Vertical position. Required by `type quadrant`, ignored elsewhere.",
    },
    PropDef {
        name: "bits",
        alias: false,
        ctx: Ctx::Node,
        words: &[],
        syntax: "bits: <first> <last>",
        doc: "Inclusive bit range of a field. Required by `type packet`, ignored \
              elsewhere; `bits: 8` is shorthand for a single bit.",
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
        name: "padding",
        alias: false,
        ctx: Ctx::Both,
        words: &[],
        syntax: "padding: <number>",
        doc: "Margin between the drawing and the canvas edge.",
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
    pub props: &'static [PropDef],
    /// Values written after the label, in order, and the property each one
    /// fills. `slice a "A" 4820` is sugar for `slice a "A" { value: 4820 }`.
    pub slots: &'static [Slot],
    /// The same, for edges: `a -> b 40` where the archetype's datum rides on
    /// the link rather than on what it joins.
    pub edge_slots: &'static [Slot],
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
        slots: &[],
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "pie",
        item: "slice",
        doc: "A pie or donut chart. Each slice needs a value; its `fill` \
              overrides the automatic palette.",
        props: PIE_PROPS,
        slots: &[Slot {
            prop: "value",
            arity: 1,
        }],
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "xy",
        item: "series",
        doc: "Lines, areas or dots over a shared x axis. One series per \
              statement, its numbers evenly spaced across the plot.",
        props: XY_PROPS,
        slots: &[Slot {
            prop: "points",
            arity: VARIADIC,
        }],
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "bar",
        item: "series",
        doc: "Bars grouped or stacked across named categories. One series per \
              statement, its numbers in category order.",
        props: BAR_PROPS,
        slots: &[Slot {
            prop: "points",
            arity: VARIADIC,
        }],
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "sankey",
        item: "node",
        doc: "Flows between stages, sized by magnitude. The value rides on the \
              link, not on what it joins: a node's height is whatever passes \
              through it.",
        props: SANKEY_PROPS,
        slots: &[],
        edge_slots: &[Slot {
            prop: "value",
            arity: 1,
        }],
    },
    ArchetypeDef {
        name: "wave",
        item: "signal",
        doc: "A timing diagram. Each signal carries a wave string, one \
              character per period, in the WaveDrom alphabet.",
        props: WAVE_PROPS,
        slots: &[Slot {
            prop: "wave",
            arity: 1,
        }],
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "quadrant",
        item: "point",
        doc: "Points on two axes, divided into four named regions. A Wardley \
              map is this archetype with its own axis names.",
        props: QUADRANT_PROPS,
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
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "treemap",
        item: "box",
        doc: "Nested rectangles whose area is proportional to a value. The \
              hierarchy is group nesting; a group's area is the sum of what \
              it contains.",
        props: TREEMAP_PROPS,
        slots: &[Slot {
            prop: "value",
            arity: 1,
        }],
        edge_slots: &[],
    },
    ArchetypeDef {
        name: "packet",
        item: "field",
        doc: "A packet / bitfield diagram. Each field needs an inclusive bit \
              range; one crossing a row boundary is split across rows.",
        props: PACKET_PROPS,
        slots: &[Slot {
            prop: "bits",
            arity: 2,
        }],
        edge_slots: &[],
    },
];

pub const PIE_LABELS: &[&str] = &["percent", "value", "both", "none"];
pub const TOGGLE: &[&str] = &["on", "off", "true", "false"];

pub const PIE_PROPS: &[PropDef] = &[
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
