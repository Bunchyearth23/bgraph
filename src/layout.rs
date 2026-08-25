//! Anchor solver: turns declarative placement into absolute rectangles.
//!
//! Nodes are resolved by repeated passes — a node whose anchor is not placed
//! yet simply waits for the next round. If a full pass places nothing, the
//! remaining nodes form an anchor cycle.

use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::ast::*;
pub use crate::placement::Entity;
use crate::placement::{EffectivePlacement, PlacementPlan};
use crate::style::{GraphOpts, Registry, Shape, StateKind, Style};

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }

    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
}

pub struct PlacedNode {
    /// The identifier the author wrote, when the box is one they can name.
    /// Empty for the boxes a chart invents — a legend swatch, an axis plate —
    /// which nothing in the source addresses and nothing outside should try
    /// to correlate. What makes the JSON export and the visual diff possible
    /// at all: geometry without names is a picture, not data.
    pub id: String,
    pub lines: Vec<String>,
    pub rect: Rect,
    pub style: Style,
}

pub struct PlacedEdge {
    pub from: usize,
    pub to: usize,
    pub label: Option<String>,
    pub arrow: Arrow,
    pub style: Style,
    /// The places the line must go through, already solved to absolute points
    /// (`D-034`). Resolved here because this is the only stage that knows
    /// where a named anchor ended up.
    pub via: Vec<(f64, f64)>,
    /// Where the edge was written. Geometry-dependent diagnostics are found
    /// after the solve, and one that could not point at a line would be a
    /// diagnostic nobody could act on.
    pub span: Span,
}

pub struct PlacedGroup {
    /// As on a node: the declared name, or empty for a frame nobody wrote.
    pub id: String,
    pub lines: Vec<String>,
    pub rect: Rect,
    pub style: Style,
}

/// A plain line: an axis, a gridline, a divider. Drawn beneath everything —
/// the substrate `W-012` will need too, not a quadrant-only device.
pub struct Guide {
    pub from: (f64, f64),
    pub to: (f64, f64),
    pub color: String,
    pub width: f64,
    pub dash: Option<String>,
}

/// Free geometry: open for a line, closed for a filled shape. Enough for a
/// waveform without letting SVG path syntax leak into the layout.
pub struct Polyline {
    pub points: Vec<(f64, f64)>,
    pub closed: bool,
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub width: f64,
    /// An arrow at the far end, sharing the marker machinery edges use.
    pub head: Option<crate::style::Head>,
    pub dash: Option<String>,
    /// Drawn **after** the boxes rather than before them.
    ///
    /// Nearly every line here belongs underneath — a harness wire, a sankey
    /// ribbon, a tree's guides — so `false` is the answer for all but one kind.
    /// The exception is a mark that lives *inside* a shape: a BPMN gateway's
    /// cross is drawn on the diamond, and under it, it is simply invisible.
    pub over: bool,
}

pub struct Layout {
    /// Drawn first, under everything.
    pub guides: Vec<Guide>,
    /// Drawn over the guides, under the boxes.
    pub polylines: Vec<Polyline>,
    /// Drawn next, behind everything they enclose.
    pub groups: Vec<PlacedGroup>,
    pub nodes: Vec<PlacedNode>,
    pub edges: Vec<PlacedEdge>,
    pub opts: GraphOpts,
    pub chart: Chart,
    /// Free-standing text the renderer draws last: titles, slice labels,
    /// legend entries, bit rulers.
    pub annotations: Vec<Annotation>,
}

/// What the statements in the file were interpreted as.
pub enum Chart {
    /// Nodes and edges. `nodes` carries the geometry.
    Free,
    /// Bitfields. `nodes` already carries one rect per row segment, so the
    /// ordinary node renderer draws it.
    Packet,
    /// Nested tiles. Like `Packet`, the geometry is already in `nodes` and
    /// `groups`; the variant exists so the intent is legible.
    Treemap,
    /// Points on two axes. `guides` carries the frame and the cross.
    Quadrant,
    /// Timing lanes. `polylines` carries the waveforms.
    Wave,
    /// Flows between stages. `polylines` carries the ribbons.
    Sankey,
    /// Bars on a value axis. `guides` carries the frame.
    Bar,
    /// Lines, areas or dots on two axes.
    Xy,
    /// Participants, lifelines and messages in source order.
    Sequence,
    /// Bars on a calendar.
    Gantt,
    /// Events on a date line, labels staggered either side of it.
    Timeline,
    /// Slices, which need arcs the node renderer cannot make.
    Pie(Pie),
}

pub struct Pie {
    pub cx: f64,
    pub cy: f64,
    pub radius: f64,
    /// Inner radius; `0` is a full pie.
    pub inner: f64,
    pub slices: Vec<Slice>,
    /// Drawn between slices as a 2px gap so adjacent fills never touch.
    pub gap_color: String,
}

pub struct Slice {
    /// Degrees, `0` at twelve o'clock, growing clockwise.
    pub start: f64,
    pub end: f64,
    pub fill: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

pub struct Annotation {
    /// Whether this is the diagram's heading.
    ///
    /// Marked rather than found: a heading is repositioned once the drawing's
    /// width is known, which is after every genre has finished placing things,
    /// and picking it out of the pile by its size and weight would be a guess
    /// that goes wrong the day somebody styles a label the same (`W-128`).
    pub title: bool,
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub anchor: Anchor,
    pub size: f64,
    pub color: String,
    pub weight: String,
    pub family: String,
}

const MIN_WIDTH: f64 = 76.0;
const MIN_HEIGHT: f64 = 44.0;
const LINE_HEIGHT: f64 = 1.35;

/// Advance width per printable ASCII character, in thousandths of an em, for a
/// Helvetica-class proportional sans — which is what the default stack
/// resolves to almost everywhere.
///
/// Indexed from `0x20`. This is what replaced a flat `0.56` for every glyph:
/// `l` is a fifth of an em and `W` is nearly a whole one, so a label of thin
/// letters was being given half again the room it needs and a label of wide
/// ones was overflowing its box.
const ADVANCE: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
    278, // ␣ ! " # $ % & ' ( ) * + , - . /
    556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584,
    556, // 0-9 : ; < = > ?
    1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722, 778, // @ A-O
    667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469,
    556, // P-Z [ \ ] ^ _
    333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, // ` a-o
    556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // p-z { | } ~
];

/// Width of a run of text at a given size.
///
/// **Still an estimate, and it cannot stop being one** (`X-005`): the SVG names
/// a font by family and whoever renders it decides what that resolves to, so
/// measuring against any particular file would only be right by luck. What this
/// does remove is the larger of the two errors — the variation *within* a
/// typeface, which is a factor of four between `l` and `W`.
pub fn advance(text: &str, font_size: f64) -> f64 {
    let em: f64 = text
        .chars()
        .map(|c| match c as u32 {
            code @ 0x20..=0x7e => ADVANCE[(code - 0x20) as usize] as f64 / 1000.0,
            // Anything outside printable ASCII keeps the old flat guess: a
            // table for one typeface has nothing to say about CJK or emoji.
            _ => 0.56,
        })
        .sum();
    em * font_size
}

/// The widest of a set of lines. Character *count* used to stand in for width
/// here, which meant the longest line and the widest line could be different
/// lines — and the box was sized for the wrong one.
pub fn text_width(lines: &[String], font_size: f64) -> f64 {
    lines
        .iter()
        .map(|l| advance(l, font_size))
        .fold(0.0_f64, f64::max)
}

/// Round shapes need extra room: their corners cut into the text box. Shared,
/// because a second sizing path that forgets this puts the label on the edge —
/// which is exactly what `type block` did until it was looked at.
fn shape_allowance(shape: Option<Shape>, mut w: f64, mut h: f64) -> (f64, f64) {
    match shape.unwrap_or(Shape::Rect) {
        Shape::Ellipse => {
            w *= 1.30;
            h *= 1.30;
        }
        Shape::Diamond => {
            w *= 1.55;
            h *= 1.55;
        }
        // The slanted sides eat width at the top and bottom of the text box,
        // which a single line never reaches and a second one does.
        Shape::Hexagon => {
            w *= 1.34;
            h += 6.0;
        }
        Shape::Cylinder => h += 14.0,
        // A stadium's ends are semicircles of radius h/2, so the width a line
        // of text can actually use is `w - h`. Nothing was added for it, which
        // is why `type block`'s widest person box had its name running out of
        // both curved ends (`W-118`). Half the height each side, and a floor so
        // a short label in a tall box does not double the box.
        Shape::Stadium => w += h.min(w * 0.6),
        _ => {}
    }
    (w, h)
}

/// Put the heading where the file asked for it, now that the width is known.
///
/// Left is the default and is a no-op, so every drawing that says nothing about
/// this is byte-identical to what it was. Centring measures against the drawing
/// **without** the heading: measuring with it would make a title wider than its
/// diagram widen the diagram, which would move the centre, which would move the
/// title (`W-128`).
fn place_title(layout: &mut Layout) {
    let Some(align) = layout.opts.title_align else {
        return;
    };
    // **Everything drawn, lines included.** Boxes and text alone are not the
    // drawing: a rack's rails and unit rules are `Guide`s, a chart's axes are
    // `Guide`s, and an empty rack of a bank contributes nothing else at all.
    // Centring on the boxes left the heading of a six-rack bank sitting over
    // the fourth rack, because the two racks holding equipment and the six
    // sets of U numbers were the only things the box knew about (`W-133`).
    let content = bbox(
        layout
            .nodes
            .iter()
            .map(|n| n.rect)
            .chain(layout.groups.iter().map(|g| g.rect))
            .chain(
                layout
                    .annotations
                    .iter()
                    .filter(|a| !a.title)
                    .map(crate::charts::annotation_bounds),
            )
            .chain(layout.guides.iter().map(|g| span(&[g.from, g.to])))
            .chain(layout.polylines.iter().map(|p| span(&p.points))),
    );
    let Some(area) = content else { return };

    for a in layout.annotations.iter_mut().filter(|a| a.title) {
        a.x = match align {
            Anchor::Middle => area.cx(),
            Anchor::End => area.right(),
            Anchor::Start => area.x,
        };
        a.anchor = align;
    }
}

/// Whether box `i` is one of an edge's own ends, and so not something for that
/// edge to route around or be reported for crossing.
///
/// The two endpoints are obvious. The third case is a **boundary event's host**:
/// the event is drawn sitting *on* that box, so every flow leaving it starts
/// inside one — treating the host as solid would make the router take a detour
/// around the very box the event belongs to, and make `check` report a crossing
/// that is the notation working correctly.
///
/// Shared so the router and the validator cannot disagree about it, which is
/// the same reason `route_all` is public.
pub fn endpoint_of(layout: &Layout, edge: &PlacedEdge, i: usize) -> bool {
    if i == edge.from || i == edge.to {
        return true;
    }
    let host = |end: usize| layout.nodes[end].style.attach.as_deref();
    let id = layout.nodes[i].id.as_str();
    host(edge.from) == Some(id) || host(edge.to) == Some(id)
}

fn natural_size(lines: &[String], style: &Style) -> (f64, f64) {
    let font_size = style.font_size.unwrap_or(14.0);
    let padding = style.padding.unwrap_or(14.0);

    let mut w = text_width(lines, font_size) + padding * 2.0;
    let mut h = lines.len().max(1) as f64 * font_size * LINE_HEIGHT + padding * 2.0;

    (w, h) = shape_allowance(style.shape, w, h);

    (
        style.width.unwrap_or(w.max(MIN_WIDTH)),
        style.height.unwrap_or(h.max(MIN_HEIGHT)),
    )
}

/// How an element of a given kind is drawn.
struct BlockLook {
    fill: &'static str,
    stroke: &'static str,
    ink: &'static str,
    shape: Shape,
    dashed: bool,
}

/// C4 encodes *scope*, not identity: a ramp of blues from the person outward
/// to the component, and grey for anything outside the system being described.
/// So this is a sequential ramp plus a neutral, not the categorical palette —
/// two elements sharing a hue is the point, it means they share a level.
fn block_look(kind: Option<crate::style::BlockKind>, dark: bool) -> BlockLook {
    use crate::style::BlockKind as K;
    let (fill, ink) = match (kind, dark) {
        (Some(K::Person), false) => ("#08427b", "#ffffff"),
        (Some(K::Person), true) => ("#1e3a8a", "#e2e8f0"),
        (Some(K::System), false) => ("#1168bd", "#ffffff"),
        (Some(K::System), true) => ("#1d4ed8", "#f8fafc"),
        (Some(K::Component), false) => ("#85bbf0", "#0b0b0b"),
        (Some(K::Component), true) => ("#93c5fd", "#0b1120"),
        (Some(K::External), false) => ("#8b8b8b", "#ffffff"),
        (Some(K::External), true) => ("#475569", "#e2e8f0"),
        // Container, database and queue sit at one level and share its hue;
        // their shape is what tells them apart.
        (_, false) => ("#438dd5", "#ffffff"),
        (_, true) => ("#3b82f6", "#0b1120"),
    };

    // Every other kind reads by its fill alone; `external` earns a visible
    // dashed border, which is the whole point of marking it as outside.
    let stroke = match (kind, dark) {
        (Some(K::External), false) => "#5f5f5f",
        (Some(K::External), true) => "#94a3b8",
        _ => fill,
    };

    BlockLook {
        fill,
        stroke,
        ink,
        shape: match kind {
            Some(K::Database) => Shape::Cylinder,
            Some(K::Queue) => Shape::Hexagon,
            Some(K::Person) => Shape::Stadium,
            _ => Shape::Rect,
        },
        dashed: kind == Some(K::External),
    }
}

/// The lines a C4 box carries: its name, then `[Kind: Technology]`, then a
/// wrapped description. Returned with a size factor and whether the line is
/// secondary, because the three are not drawn at one weight.
fn block_lines(name: &[String], style: &Style) -> Vec<(String, f64, bool)> {
    use crate::style::BlockKind as K;
    let mut out: Vec<(String, f64, bool)> = name.iter().map(|l| (l.clone(), 1.0, false)).collect();

    let kind_word = match style.block_kind {
        Some(K::Person) => Some("Person"),
        Some(K::System) => Some("System"),
        Some(K::Container) => Some("Container"),
        Some(K::Component) => Some("Component"),
        Some(K::Database) => Some("Database"),
        Some(K::Queue) => Some("Queue"),
        Some(K::External) => Some("External"),
        None => None,
    };
    let bracket = match (kind_word, style.tech.as_deref()) {
        (Some(k), Some(t)) => Some(format!("[{k}: {t}]")),
        (Some(k), None) => Some(format!("[{k}]")),
        (None, Some(t)) => Some(format!("[{t}]")),
        (None, None) => None,
    };
    if let Some(text) = bracket {
        out.push((text, 0.78, true));
    }

    if let Some(description) = &style.description {
        for line in wrap(description, 30) {
            out.push((line, 0.82, true));
        }
    }

    out
}

/// Greedy wrap at a word boundary. A fixed column rather than a measured one:
/// the box is sized from these lines, so measuring against a width that does
/// not exist yet would be circular.
pub(crate) fn wrap(text: &str, columns: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > columns {
            lines.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

pub fn solve(diagram: &Diagram) -> Result<Layout> {
    // `check` is the authority on what is valid; the bails below only guard
    // invariants it has already established.
    if let Some(diag) = crate::check::first_error(diagram) {
        bail!("{}", diag.message);
    }

    let mut opts = GraphOpts::from_props(&diagram.graph)?;
    // A chart's own `title` is the diagram's name too, and the SVG's
    // `aria-label` is built from `opts`. Lifted here, once, rather than in
    // each of the eighteen chart functions — a screen reader on a pie chart
    // is exactly the reader who needs the name most.
    if opts.title.is_none()
        && let Some(title) = diagram
            .archetype_props
            .iter()
            .find(|p| p.name == "title")
            .and_then(|p| p.values.first())
            .and_then(|v| v.as_text())
    {
        opts.title = Some(title.to_string());
    }
    let registry = Registry::build(&diagram.styles)?;

    // The archetype decides what the statements mean.
    let chart: Option<Result<Layout>> = match diagram
        .archetype
        .as_ref()
        .map(|a| a.as_str())
        .unwrap_or("free")
    {
        #[cfg(feature = "pie")]
        "pie" => Some(crate::charts::pie(diagram, opts.clone(), &registry)),
        #[cfg(feature = "packet")]
        "packet" => Some(crate::charts::packet(diagram, opts.clone(), &registry)),
        #[cfg(feature = "kanban")]
        "kanban" => Some(crate::charts::kanban(diagram, opts.clone(), &registry)),
        #[cfg(feature = "journey")]
        "journey" => Some(crate::charts::journey(diagram, opts.clone(), &registry)),
        #[cfg(feature = "radar")]
        "radar" => Some(crate::charts::radar(diagram, opts.clone(), &registry)),
        #[cfg(feature = "rack")]
        "rack" => Some(crate::charts::rack(diagram, opts.clone(), &registry)),
        #[cfg(feature = "venn")]
        "venn" => Some(crate::charts::venn(diagram, opts.clone(), &registry)),
        #[cfg(feature = "gitgraph")]
        "gitgraph" => Some(crate::charts::gitgraph(diagram, opts.clone(), &registry)),
        #[cfg(feature = "orgchart")]
        "orgchart" => Some(crate::charts::tree(
            diagram,
            opts.clone(),
            &registry,
            crate::charts::TreeKind::Org,
        )),
        #[cfg(feature = "mindmap")]
        "mindmap" => Some(crate::charts::tree(
            diagram,
            opts.clone(),
            &registry,
            crate::charts::TreeKind::Mind,
        )),
        #[cfg(feature = "treeview")]
        "treeview" => Some(crate::charts::tree(
            diagram,
            opts.clone(),
            &registry,
            crate::charts::TreeKind::View,
        )),
        #[cfg(feature = "bytefield")]
        "bytefield" => Some(crate::charts::bytefield(diagram, opts.clone(), &registry)),
        #[cfg(feature = "treemap")]
        "treemap" => Some(crate::charts::treemap(diagram, opts.clone(), &registry)),
        #[cfg(feature = "quadrant")]
        "quadrant" => Some(crate::charts::quadrant(diagram, opts.clone(), &registry)),
        #[cfg(feature = "wave")]
        "wave" => Some(crate::charts::wave(diagram, opts.clone(), &registry)),
        #[cfg(feature = "sankey")]
        "sankey" => Some(crate::charts::sankey(diagram, opts.clone(), &registry)),
        #[cfg(feature = "bar")]
        "bar" => Some(crate::charts::bar(diagram, opts.clone(), &registry)),
        #[cfg(feature = "xy")]
        "xy" => Some(crate::charts::xy(diagram, opts.clone(), &registry)),
        #[cfg(feature = "sequence")]
        "sequence" => Some(crate::charts::sequence(diagram, opts.clone(), &registry)),
        #[cfg(feature = "gantt")]
        "gantt" => Some(crate::charts::gantt(diagram, opts.clone(), &registry)),
        #[cfg(feature = "timeline")]
        "timeline" => Some(crate::charts::timeline(diagram, opts.clone(), &registry)),
        _ => None,
    };

    // One funnel for every genre. A heading can only be placed once the
    // drawing's width is known, and that is after the archetype has finished —
    // so it is done here rather than in each of the nineteen places a title is
    // pushed, which would have been nineteen chances to disagree (`W-128`).
    if let Some(chart) = chart {
        let mut chart = chart?;
        place_title(&mut chart);
        return Ok(chart);
    }

    let placement = PlacementPlan::new(diagram);
    let index = placement.index();

    // Pass 1 — resolve styles and intrinsic sizes; positions come later.
    let mut sizes = Vec::with_capacity(diagram.nodes.len());
    let mut styles = Vec::with_capacity(diagram.nodes.len());
    let mut labels = Vec::with_capacity(diagram.nodes.len());
    // The label of a shape that cannot hold one, kept for the pass that draws
    // it underneath. One entry per node, so the two stay in step.
    let mut captions: Vec<Option<String>> = Vec::with_capacity(diagram.nodes.len());

    let item = crate::check::archetype_of(diagram);
    let machine = item.name == "state";
    let architecture = item.name == "block";
    let topology = item.name == "network";
    let process = item.name == "bpmn";

    let dark = opts
        .background
        .as_deref()
        .is_some_and(crate::charts::is_dark);
    // An entity box is a class box: a name over a ruled list of members. Asked
    // of the archetype rather than matched against a list of names here — a
    // list like that is one nobody remembers to extend.
    let compartments = item.members;
    let harness = item.name == "harness";
    // A base that leaves `shape` unset, so a declared one is distinguishable
    // from the default and a state can fall back to a rounded box.
    let node_base = if machine {
        Style {
            shape: None,
            ..Style::node_defaults()
        }
    } else if topology || process {
        // Only `shape` is left unset: a topology's colours usually already
        // mean something to its author (environment, tenancy, trust zone), so
        // the archetype chooses the outline and keeps out of the palette. A
        // process is the same bargain for a different reason — in BPMN the
        // shape *is* the meaning, and the colour is free for whatever the
        // author is tracking.
        Style {
            shape: None,
            ..Style::node_defaults()
        }
    } else if architecture {
        // Everything the element kind decides is left unset, so a declared
        // value stays distinguishable from a default. Getting this wrong has
        // silently overridden an archetype four times now.
        Style {
            shape: None,
            fill: None,
            stroke: None,
            font_color: None,
            dash: None,
            ..Style::node_defaults()
        }
    } else {
        Style::node_defaults()
    };
    for node in &diagram.nodes {
        let mut style = registry
            .resolve(
                node_base.clone(),
                item.style_word(node.keyword.as_str()),
                &node.classes,
                // `props_for`, not `effective_props(item.slots)`: the
                // positional shape follows the **word the author wrote**
                // (`D-023`). The free path read the archetype's primary slots
                // instead, which was invisible for as long as `xy` was the only
                // genre with a second word — and `xy` is a chart, so it never
                // came through here. `bpmn` is the first to have four.
                &node.props_for(item),
            )
            .map_err(|e| e.context(format!("in node `{}`", node.id.as_str())))?;

        // A pseudo-state is a mark, not a box: it takes a shape and a fixed
        // size, and carries no label of its own.
        let mark = machine
            && matches!(
                style.kind,
                Some(StateKind::Initial) | Some(StateKind::Final) | Some(StateKind::Choice)
            );
        if machine {
            match style.kind {
                Some(StateKind::Initial) => {
                    style.shape = Some(Shape::Ellipse);
                    style.fill = style.stroke.clone();
                    style.width = Some(16.0);
                    style.height = Some(16.0);
                }
                Some(StateKind::Final) => {
                    style.shape = Some(Shape::Ellipse);
                    style.stroke_width = Some(4.0);
                    style.width = Some(22.0);
                    style.height = Some(22.0);
                }
                Some(StateKind::Choice) => {
                    style.shape = Some(Shape::Diamond);
                    style.width = Some(34.0);
                    style.height = Some(34.0);
                }
                _ => style.shape = style.shape.or(Some(Shape::Stadium)),
            }
        }

        // A BPMN element's shape *is* its meaning — a ring is an event and a
        // diamond is a gateway, and no label makes one into the other. So the
        // shape follows the element and the two smallest are given a fixed
        // size: a bigger circle does not mean a bigger event, and sizing one
        // from its label would turn two lengths of text into two importances.
        #[cfg(feature = "bpmn")]
        if process {
            use crate::bpmn::{EVENT, Element, GATE};
            match crate::bpmn::element_of(&style) {
                Element::Event => {
                    style.shape = style.shape.or(Some(Shape::Ellipse));
                    style.width = style.width.or(Some(EVENT));
                    style.height = style.height.or(Some(EVENT));
                    // The ring says where in the process it sits before the
                    // glyph says what it is: thin starts, thick ends.
                    if style.stage.as_deref() == Some("end") {
                        style.stroke_width = Some(3.6);
                    }
                }
                Element::Gateway => {
                    style.shape = style.shape.or(Some(Shape::Diamond));
                    style.width = style.width.or(Some(GATE));
                    style.height = style.height.or(Some(GATE));
                }
                Element::Data => {
                    style.shape =
                        style
                            .shape
                            .or(Some(if style.artifact.as_deref() == Some("store") {
                                Shape::Cylinder
                            } else {
                                Shape::Rect
                            }));
                    style.radius = Some(0.0);
                }
                Element::Task => {
                    style.shape = style.shape.or(Some(Shape::Rect));
                    style.radius = style.radius.or(Some(10.0));
                }
            }
        }

        // The role picks the outline and nothing else.
        if topology {
            use crate::style::NetRole as R;
            style.shape = style.shape.or(Some(match style.role {
                Some(R::Router) => Shape::Diamond,
                Some(R::Switch) | Some(R::Firewall) => Shape::Hexagon,
                Some(R::Storage) => Shape::Cylinder,
                Some(R::Client) => Shape::Stadium,
                Some(R::Cloud) => Shape::Ellipse,
                Some(R::Server) | None => Shape::Rect,
            }));
        }

        if architecture {
            let look = block_look(style.block_kind, dark);
            style.fill = style.fill.or(Some(look.fill.into()));
            style.stroke = style.stroke.or(Some(look.stroke.into()));
            style.font_color = style.font_color.or(Some(look.ink.into()));
            style.shape = style.shape.or(Some(look.shape));
            if look.dashed {
                style.dash = style.dash.or(Some("dashed".into()));
            }
        }

        // An event and a gateway are sized by the notation, not by their text,
        // so the label cannot go inside one — it is drawn underneath instead,
        // which is also where BPMN puts it.
        #[cfg(feature = "bpmn")]
        let outside = process
            && matches!(
                crate::bpmn::element_of(&style),
                crate::bpmn::Element::Event | crate::bpmn::Element::Gateway
            );
        #[cfg(not(feature = "bpmn"))]
        let outside = false;

        // A task carrying a corner marker needs room the label alone would not
        // ask for.
        #[cfg(feature = "bpmn")]
        let marked = process
            && crate::bpmn::element_of(&style) == crate::bpmn::Element::Task
            && style.task_kind.as_deref().is_some_and(|k| k != "plain");
        #[cfg(not(feature = "bpmn"))]
        let marked = false;

        let mut lines: Vec<String> = if mark || outside {
            captions.push(node.label.clone());
            Vec::new()
        } else {
            captions.push(None);
            node.label
                .clone()
                .unwrap_or_else(|| node.id.value.clone())
                .split('\n')
                .map(|l| l.trim().to_string())
                .collect()
        };

        // The address goes on its own line under the name, and the box is
        // sized from it like any other line — declared and not drawn would be
        // worse than refused.
        if topology && let Some(addr) = &style.addr {
            lines.push(addr.clone());
        }

        // A block is sized by every line it will draw, at the size each one
        // is drawn at — a description in 11px does not need a 14px column.
        let size = if architecture {
            let font = style.font_size.unwrap_or(14.0);
            let pad = style.padding.unwrap_or(14.0);
            let rows = block_lines(&lines, &style);
            let widest = rows
                .iter()
                .map(|(text, scale, _)| advance(text, font * scale))
                .fold(0.0, f64::max);
            let tall: f64 = rows.iter().map(|(_, scale, _)| font * scale * 1.45).sum();
            let (w, h) = shape_allowance(
                style.shape,
                (widest + pad * 2.0).max(MIN_WIDTH),
                (tall + pad * 2.0).max(MIN_HEIGHT),
            );
            (style.width.unwrap_or(w), style.height.unwrap_or(h))
        } else if compartments {
            let font = style.font_size.unwrap_or(14.0);
            let pad = style.padding.unwrap_or(14.0);
            let widest = lines
                .iter()
                .cloned()
                .chain(node.members.iter().map(|m| m.value.clone()))
                .collect::<Vec<_>>();
            let rows = lines.len().max(1) + node.members.len();
            (
                style
                    .width
                    .unwrap_or((text_width(&widest, font) + pad * 2.0).max(96.0)),
                style.height.unwrap_or(
                    rows as f64 * font * 1.45
                        + pad * 2.0
                        + if node.members.is_empty() { 0.0 } else { 8.0 },
                ),
            )
        } else if marked {
            // A task's marker sits in its top-left corner and its label is
            // centred, so a box sized to the text alone puts one on the other.
            // The floor is what BPMN draws anyway — a task is a chunky box —
            // and it is a floor rather than a size, so a long label still
            // grows it.
            let (w, h) = natural_size(&lines, &style);
            (
                style.width.unwrap_or(w.max(118.0)),
                style.height.unwrap_or(h.max(60.0)),
            )
        } else {
            natural_size(&lines, &style)
        };

        sizes.push(size);
        styles.push(style);
        labels.push(lines);
    }

    // Pass 2 — place. Nodes and groups share one namespace and one fixpoint:
    // a node waits for its anchor, a group waits for its members. A group
    // carrying a placement translates its whole subtree once solved, so its
    // members only count as anchorable after that translation.
    // Size constraints run *before* placement, because a box widened after
    // the fact does not push its neighbours along — it lands on top of them.
    // Placement already knows how to lay out boxes of a given size, so the
    // honest order is: decide the sizes, then place.
    constrain_sizes(&placement, &mut sizes);

    // What each node's drawing reaches past its own box. Only a caption does
    // that, and only two genres draw one — everywhere else this is zeros and
    // the group frames are exactly what they always were.
    let overhang: Vec<(f64, f64, f64, f64)> = captions
        .iter()
        .zip(sizes.iter())
        .zip(styles.iter())
        .map(|((caption, (w, _)), style)| {
            let Some(text) = caption else {
                return (0.0, 0.0, 0.0, 0.0);
            };
            let size = style.font_size.unwrap_or(14.0) * 0.85;
            let lines: Vec<String> = text.split('\n').map(|l| l.trim().to_string()).collect();
            let side = ((text_width(&lines, size) - w) / 2.0).max(0.0);
            let below = 6.0 + lines.len() as f64 * size * 1.3;
            (side, 0.0, side, below)
        })
        .collect();

    // Cassowary is now the source of geometry whenever every relation can be
    // lowered without approximation.  Frames still need their content bounds
    // before their own translation can be expressed, so those diagrams retain
    // the established fixpoint until group lowering lands.  This is a
    // semantic fallback, not a build-flag-dependent second engine.
    let solved = if let Some(nodes) =
        crate::constraint_layout::solve_nodes(diagram, &placement, &sizes, &opts)?
    {
        Solved {
            nodes: nodes.into_iter().map(Some).collect(),
            groups: vec![None; diagram.groups.len()],
        }
    } else {
        let mut solved = place_all(diagram, &placement, &sizes, &overhang, &opts)?;
        // Legacy alignment and group frames still need the historical pass.
        constrain_alignment(&placement, &mut solved);
        crate::constraint_layout::validate_frames(
            diagram,
            &placement,
            &solved.nodes,
            &solved.groups,
            &opts,
        )?;
        solved
    };
    let placed = solved.nodes;

    let mut nodes: Vec<PlacedNode> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| PlacedNode {
            id: node.id.as_str().to_string(),
            lines: labels[i].clone(),
            rect: placed[i].expect("every node is placed once the loop exits"),
            style: styles[i].clone(),
        })
        .collect();

    let mut edges = Vec::with_capacity(diagram.edges.len());
    for edge in &diagram.edges {
        let (Entity::Node(from), Entity::Node(to)) =
            (index[edge.from.as_str()], index[edge.to.as_str()])
        else {
            // `check` rejects an edge touching a group; nothing to draw.
            continue;
        };

        // `effective_props`, not `props`: an archetype with edge slots writes
        // its data after the label — `order -> line "contains" one many` — and
        // it has to reach the cascade like any other property.
        // A process leaves the router, the head and the dash unset in the
        // base, the way a topology leaves the shape unset: the connector's
        // *kind* decides all three, and a default already in place would win
        // the `or` below and the archetype would silently supply nothing.
        // Getting this wrong is the same mistake four archetypes have made.
        let edge_base = if process {
            Style {
                router: None,
                head: None,
                ..Style::edge_defaults()
            }
        } else if topology {
            // A topology reads as a circuit, so its links turn right angles —
            // and since `W-105` they go around the boxes as well. `router` has
            // to be unset here rather than `or`-ed below, because
            // `edge_defaults()` already fills it and a default in place beats
            // an archetype's `or` every time.
            Style {
                router: None,
                ..Style::edge_defaults()
            }
        } else {
            Style::edge_defaults()
        };
        let mut style = registry
            .resolve(
                edge_base,
                "edge",
                &edge.classes,
                &edge.effective_props(item.edge_slots),
            )
            .map_err(|e| {
                e.context(format!(
                    "in edge `{} -> {}`",
                    edge.from.as_str(),
                    edge.to.as_str()
                ))
            })?;

        // `[guard]` after the label is the notation a reader expects.
        let mut label = edge.label.clone();
        // The protocol goes on its own line under the relation, the way a C4
        // diagram reads it. Declared and never drawn would be worse than
        // refused, so this is not optional.
        if architecture && let Some(tech) = &style.tech {
            label = Some(match label {
                Some(text) => format!("{text}\n[{tech}]"),
                None => format!("[{tech}]"),
            });
        }
        // What the relation asserts, in guillemets — the notation SysML uses,
        // and the only thing on the line if no label was written.
        if let Some(link) = &style.link {
            label = Some(match label {
                Some(text) if !text.is_empty() => format!("{text}\n«{link}»"),
                _ => format!("«{link}»"),
            });
        }
        // What a link runs at, bracketed under its label — the same shape the
        // C4 view uses for a protocol, because it is the same kind of fact.
        if topology {
            style.router = style.router.or(Some(crate::style::Router::Ortho));
        }
        if topology && let Some(speed) = &style.speed {
            label = Some(match label {
                Some(text) if !text.is_empty() => format!("{text}\n[{speed}]"),
                _ => format!("[{speed}]"),
            });
        }
        if let Some(guard) = &style.guard {
            label = Some(match label {
                Some(text) if !text.is_empty() => format!("{text} [{guard}]"),
                _ => format!("[{guard}]"),
            });
        }

        // A BPMN connector says what it means with its *line*, which is the
        // one thing about the notation a reader learns without being taught: a
        // solid arrow is the process going on, a dashed one is a message
        // crossing between participants, a dotted one only associates. Each is
        // a default, so a class or an inline property still wins — the
        // archetype supplies the vocabulary and the author keeps the last word.
        #[cfg(feature = "bpmn")]
        if process {
            let (dash, head) = match style.flow.as_deref() {
                Some("message") => (Some("dashed"), Some(crate::style::Head::Open)),
                Some("association") => (Some("dotted"), Some(crate::style::Head::None)),
                _ => (None, None),
            };
            if let Some(d) = dash {
                style.dash = style.dash.or(Some(d.to_string()));
            }
            style.head = style.head.or(head).or(Some(crate::style::Head::Solid));
            // A sequence flow is orthogonal, because a process is read as a
            // circuit: a diagonal between two boxes looks like a suggestion
            // where a right angle looks like a route. A message flow is not —
            // it crosses the page between participants, and made orthogonal it
            // would be mistaken for part of the process it is talking to.
            let straight = matches!(style.flow.as_deref(), Some("message" | "association"));
            style.router = style.router.or(Some(if straight {
                crate::style::Router::Straight
            } else {
                crate::style::Router::Ortho
            }));
        }

        // A dashed/dotted arrow token sets the pattern unless a style already did.
        if let Some(dash) = edge.arrow.dash
            && style.dash.is_none()
        {
            style.dash = Some(dash.to_string());
        }

        // Waypoints, solved to absolute points. A relative one takes the
        // anchor's centre on the cross axis and steps off its edge by the gap,
        // which is the only reading that does not need a size the waypoint does
        // not have (`D-034`). An anchor `check` has already refused as unknown
        // cannot arrive here, so a miss is skipped rather than guessed at.
        let via: Vec<(f64, f64)> = edge
            .via
            .iter()
            .filter_map(|stop| match stop {
                Placement::At { .. } => {
                    let p = resolve_grid_placement(stop, &opts, &sizes);
                    match p {
                        Placement::At { x, y } => Some((x, y)),
                        _ => None,
                    }
                }
                Placement::Cell { .. } => {
                    let p = resolve_grid_placement(stop, &opts, &sizes);
                    match p {
                        Placement::At { x, y } => Some((x, y)),
                        _ => None,
                    }
                }
                Placement::Rel {
                    dir,
                    anchor,
                    gap,
                    align: _,
                } => {
                    let r = match index.get(anchor.as_str())? {
                        Entity::Node(i) => placed[*i]?,
                        Entity::Group(g) => solved.groups[*g]?,
                    };
                    let step = grid_distance(&opts, gap.unwrap_or(opts.gap), *dir, &sizes);
                    Some(match dir {
                        RelDir::RightOf => (r.right() + step, r.cy()),
                        RelDir::LeftOf => (r.x - step, r.cy()),
                        RelDir::Below => (r.cx(), r.bottom() + step),
                        RelDir::Above => (r.cx(), r.y - step),
                    })
                }
            })
            .collect();

        edges.push(PlacedEdge {
            from,
            to,
            label,
            arrow: edge.arrow,
            style,
            via,
            span: edge.span,
        });
    }

    let groups = diagram
        .groups
        .iter()
        .enumerate()
        .map(|(g, group)| {
            let style = registry
                .resolve(
                    Style::group_defaults(),
                    "group",
                    &group.classes,
                    &group.props,
                )
                .map_err(|e| e.context(format!("in group `{}`", group.id.as_str())))?;
            Ok(PlacedGroup {
                id: group.id.as_str().to_string(),
                lines: group
                    .label
                    .clone()
                    .map(|l| l.split('\n').map(|s| s.trim().to_string()).collect())
                    .unwrap_or_default(),
                rect: solved.groups[g].expect("every group is solved"),
                style,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    // A class's members are drawn as their own lines, under a rule that
    // separates them from the name.
    let guides = Vec::new();
    let mut annotations = Vec::new();
    let mut rules: Vec<PlacedNode> = Vec::new();
    // Where every member row ended up, per node. Recorded as it is drawn
    // rather than recomputed later: a second copy of the row formula is a
    // second place for the wire and the pin it names to drift apart.
    let mut member_rows: Vec<Vec<f64>> = vec![Vec::new(); nodes.len()];
    if compartments {
        for (n, (node, placed)) in diagram.nodes.iter().zip(nodes.iter_mut()).enumerate() {
            let s = placed.style.clone();
            let font = s.font_size.unwrap_or(14.0);
            let pad = s.padding.unwrap_or(14.0);
            let title_rows = placed.lines.len().max(1) as f64;
            let head = placed.rect.y + pad + title_rows * font * 1.45;

            // The name heads the box; taking it out of `lines` stops the
            // renderer centring it over the members.
            //
            // **Always centred.** `W-065` made it left-align once the box was
            // more than twice the title's width, on the grounds that a centred
            // name floats away from the left-aligned members beneath it. That
            // reading was right about one box and wrong about a diagram: the
            // threshold is crossed by some boxes and not others, so one picture
            // came out with `OrderLine` centred and `User`, `Order` and `Money`
            // not — and a rule that changes its mind between two boxes reads as
            // a defect even where each case on its own is defensible.
            // Consistency wins, and centred is what UML draws (`W-108`).
            //
            // Centred **in both directions** (`W-117`). Horizontally was only
            // half of it: the rows were laid from the top of the box downward,
            // so the name sat a padding's worth below the middle of its own
            // header band and the space above it never matched the space below.
            // The band is `rect.y` to `head`; the block of title rows is centred
            // in that.
            let block = title_rows * font * 1.45;
            let first = (placed.rect.y + head) / 2.0 - block / 2.0 + font * 1.45 / 2.0;
            for (i, line) in std::mem::take(&mut placed.lines).into_iter().enumerate() {
                annotations.push(Annotation {
                    text: line,
                    x: placed.rect.cx(),
                    y: first + i as f64 * font * 1.45,
                    anchor: Anchor::Middle,
                    size: font,
                    color: s.font_color.clone().unwrap_or_else(|| "#111827".into()),
                    weight: s.font_weight.clone().unwrap_or_else(|| "600".into()),
                    family: s.font_family.clone().unwrap_or_default(),
                    title: false,
                });
            }

            if node.members.is_empty() {
                continue;
            }

            // A thin filled rect, not a `Guide`: guides are drawn beneath the
            // boxes, so the rule would vanish under the class's own fill.
            let thickness = s.stroke_width.unwrap_or(1.5);
            rules.push(PlacedNode {
                id: String::new(),
                lines: Vec::new(),
                rect: Rect {
                    x: placed.rect.x,
                    y: head - thickness / 2.0,
                    w: placed.rect.w,
                    h: thickness,
                },
                style: Style {
                    fill: s.stroke.clone(),
                    stroke: Some("none".into()),
                    stroke_width: Some(0.0),
                    radius: Some(0.0),
                    shape: Some(Shape::Rect),
                    ..Style::default()
                },
            });

            member_rows[n] = (0..node.members.len())
                .map(|i| head + 8.0 + (i as f64 + 0.5) * font * 1.45)
                .collect();

            for (i, member) in node.members.iter().enumerate() {
                annotations.push(Annotation {
                    // `- ` and `+ ` are UML visibility on a class and mean
                    // something; on a column they are only the syntax that
                    // marked the line as a member, so `er` drops them.
                    text: if item.name == "er" {
                        member
                            .value
                            .trim_start_matches(['-', '+'])
                            .trim_start()
                            .to_string()
                    } else {
                        member.value.clone()
                    },
                    x: placed.rect.x + pad,
                    y: head + 8.0 + (i as f64 + 0.5) * font * 1.45,
                    anchor: Anchor::Start,
                    size: font * 0.92,
                    color: s.font_color.clone().unwrap_or_else(|| "#111827".into()),
                    weight: "400".into(),
                    family: s.font_family.clone().unwrap_or_default(),
                    title: false,
                });
            }
        }
    }

    // A block draws its own lines for the same reason a class does: the node
    // renderer sets one size for the whole label, and these are three tiers.
    if architecture {
        for placed in nodes.iter_mut() {
            let s = placed.style.clone();
            let font = s.font_size.unwrap_or(14.0);
            let rows = block_lines(&std::mem::take(&mut placed.lines), &s);
            let tall: f64 = rows.iter().map(|(_, scale, _)| font * scale * 1.45).sum();

            let ink = s.font_color.clone().unwrap_or_else(|| "#ffffff".into());
            let mut y = placed.rect.cy() - tall / 2.0;
            for (text, scale, secondary) in rows {
                let height = font * scale * 1.45;
                annotations.push(Annotation {
                    text,
                    x: placed.rect.cx(),
                    y: y + height / 2.0,
                    anchor: Anchor::Middle,
                    size: font * scale,
                    // The secondary tiers are the same ink at less weight, not
                    // a second colour: the fill already carries the meaning,
                    // and a third hue on top of it would only compete.
                    color: ink.clone(),
                    weight: if secondary { "400" } else { "600" }.into(),
                    family: s.font_family.clone().unwrap_or_default(),
                    title: false,
                });
                y += height;
            }
        }
    }

    // A harness draws its wires **to the pins**, not between the boxes. The
    // pin rows were recorded as they were laid out, so a wire lands on the
    // number it names and a reader can follow it with a finger — which is the
    // whole point of a loom drawing, and what `X-016` said was missing.
    let mut polylines = Vec::new();
    if harness {
        let by_id: HashMap<&str, usize> = diagram
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id.as_str(), i))
            .collect();

        // Which row carries which pin, per connector.
        let pin_row: Vec<HashMap<u32, usize>> = diagram
            .nodes
            .iter()
            .map(|node| {
                node.members
                    .iter()
                    .enumerate()
                    .filter_map(|(i, m)| pin_number(m.as_str()).map(|p| (p, i)))
                    .collect()
            })
            .collect();

        let wires = diagram.edges.len().max(1);
        let mut drawn = 0usize;
        for (edge, placed) in diagram.edges.iter().zip(edges.iter()) {
            let (Some(&a), Some(&b)) = (by_id.get(edge.from.as_str()), by_id.get(edge.to.as_str()))
            else {
                continue;
            };
            let props = edge.effective_props(item.edge_slots);
            let pins: Vec<u32> = props
                .iter()
                .filter(|p| p.name == "wire")
                .flat_map(|p| p.values.iter())
                .filter_map(|v| v.as_num())
                .map(|n| n.max(0.0) as u32)
                .collect();
            let (Some(&from_pin), Some(&to_pin)) = (pins.first(), pins.get(1)) else {
                continue;
            };
            let (Some(&ra), Some(&rb)) = (pin_row[a].get(&from_pin), pin_row[b].get(&to_pin))
            else {
                continue;
            };
            let (Some(&ya), Some(&yb)) = (
                member_rows[a].get(ra).as_ref(),
                member_rows[b].get(rb).as_ref(),
            ) else {
                continue;
            };

            // Out of one connector, across, into the other — the way a cable
            // actually leaves a shell. A straight diagonal would cross every
            // pin between the two and read as touching them.
            let (ba, bb) = (nodes[a].rect, nodes[b].rect);
            let (xa, xb, out) = if bb.cx() >= ba.cx() {
                (ba.right(), bb.x, 14.0)
            } else {
                (ba.x, bb.right(), -14.0)
            };
            // Each wire turns in its own column. Sharing one put every vertical
            // segment on top of the others, which is a bundle of wires drawn as
            // a single unreadable line — the opposite of what a loom drawing is
            // for. They fan across the gap in declaration order.
            let span = (xb - out) - (xa + out);
            let lane = (drawn as f64 + 1.0) / (wires as f64 + 1.0);
            // Past the name, whatever lane this wire drew. The name sits on the
            // horizontal run leaving the shell, and the first wire's lane put
            // the turn a third of the way along — straight through the last two
            // letters of it (`W-120`). The lane still decides the order; it no
            // longer decides whether the label is readable.
            let clear = edge
                .label
                .as_deref()
                .filter(|l| !l.is_empty())
                .map(|l| {
                    text_width(&[l.to_string()], placed.style.font_size.unwrap_or(11.0)) + 22.0
                })
                .unwrap_or(0.0);
            let along = (span * lane).max(clear).min(span.abs());
            let mid = xa + out + along * span.signum();
            drawn += 1;

            polylines.push(Polyline {
                points: vec![
                    (xa, *ya),
                    (xa + out, *ya),
                    (mid, *ya),
                    (mid, *yb),
                    (xb - out, *yb),
                    (xb, *yb),
                ],
                closed: false,
                fill: None,
                stroke: placed.style.stroke.clone(),
                width: placed.style.stroke_width.unwrap_or(1.4),
                head: None,
                dash: placed.style.dash.clone(),
                over: false,
            });

            // The pin pair is no longer written: the wire touches both, which
            // says it better than a label beside the line ever did.
            //
            // The name goes just clear of the shell, above the horizontal run
            // it leaves on — at the elbow it sat across the vertical segment,
            // which is the one place on the wire that is already busy.
            if let Some(label) = edge.label.as_ref().filter(|l| !l.is_empty()) {
                annotations.push(Annotation {
                    text: label.clone(),
                    x: xa + out * 1.4,
                    // `Annotation::y` is the text's **middle**, not its
                    // baseline, so four pixels above the wire still put the
                    // name across it — and a wire is the one thing in a loom
                    // drawing a reader follows with a finger. Half the text
                    // plus a little clears it.
                    y: ya - placed.style.font_size.unwrap_or(11.0) / 2.0 - 4.0,
                    anchor: if out > 0.0 {
                        Anchor::Start
                    } else {
                        Anchor::End
                    },
                    size: placed.style.font_size.unwrap_or(11.0),
                    color: placed
                        .style
                        .font_color
                        .clone()
                        .unwrap_or_else(|| "#94a3b8".into()),
                    weight: "400".into(),
                    family: placed.style.font_family.clone().unwrap_or_default(),
                    title: false,
                });
            }
        }
        // Every wire is drawn above; leaving the routed edges in would draw
        // each one twice, once between the boxes.
        edges.clear();
    }

    // After the boxes, so the compartment rules land on top of them.
    nodes.extend(rules);

    // Notes: free text placed by the ordinary anchors. Solved last, against
    // the boxes as they finally sit, because a note anchored to something the
    // constraints moved must move with it.
    for note in &diagram.notes {
        let style = registry.resolve(node_base.clone(), "note", &note.classes, &note.props)?;
        let size = style.font_size.unwrap_or(12.0);
        let lines: Vec<String> = note.text.split('\n').map(|l| l.to_string()).collect();
        let w = text_width(&lines, size);
        let h = lines.len() as f64 * size * LINE_HEIGHT;

        let rect = match note
            .placement
            .as_ref()
            .map(|p| resolve_grid_placement(p, &opts, &sizes))
        {
            Some(Placement::At { x, y }) => Rect { x, y, w, h },
            Some(Placement::Rel {
                dir,
                ref anchor,
                gap,
                align,
            }) => {
                let found = index.get(anchor.as_str()).and_then(|entity| match entity {
                    Entity::Node(i) => nodes.get(*i).map(|n| n.rect),
                    Entity::Group(g) => groups.get(*g).map(|f| f.rect),
                });
                // An anchor that does not resolve has already been reported by
                // `check`; drawing the note at the origin would add a second
                // wrong thing to look at, so it is left out.
                let Some(anchor_rect) = found else { continue };
                let candidate = offset_from(
                    &anchor_rect,
                    dir,
                    grid_distance(&opts, gap.unwrap_or(opts.gap), dir, &sizes),
                    align,
                    w,
                    h,
                );
                snap_to_grid(candidate, &opts, &sizes)
            }
            // Unreachable — `resolved` leaves no cells — and answered rather
            // than panicked, as everywhere else.
            Some(Placement::Cell { col, row }) => Rect {
                x: col,
                y: row,
                w,
                h,
            },
            None => continue,
        };

        for (i, line) in lines.iter().enumerate() {
            annotations.push(Annotation {
                text: line.clone(),
                x: rect.x,
                y: rect.y + (i as f64 + 0.5) * size * LINE_HEIGHT,
                anchor: Anchor::Start,
                size,
                color: style.font_color.clone().unwrap_or_else(|| {
                    if dark {
                        "#cbd5e1".into()
                    } else {
                        "#52514e".into()
                    }
                }),
                weight: "400".into(),
                family: style.font_family.clone().unwrap_or_default(),
                title: false,
            });
        }
    }

    // A legend, for the genres that have no chart to build one from. Derived
    // from the style classes that declare a `legend:` text, in the order they
    // were declared — so it lists what the file actually says rather than
    // guessing from the colours on the page.
    for legend in &diagram.legends {
        let style = registry.resolve(node_base.clone(), "note", &legend.classes, &legend.props)?;
        let size = style.font_size.unwrap_or(12.0);
        let swatch = size;
        let step = size * 2.0;
        let gap_to_text = size * 0.7;

        // The swatch takes the class's stroke as well as its fill: a pale
        // fill is common — a status colour is usually a tint with the meaning
        // in the border — and a swatch drawn from the fill alone would be a
        // key nobody can read.
        let entries: Vec<(String, String, String)> = diagram
            .styles
            .iter()
            .filter_map(|decl| {
                let resolved = registry.get(decl.name.as_str())?;
                let text = resolved.legend.clone()?;
                Some((
                    text,
                    resolved.fill.clone().unwrap_or_else(|| "#94a3b8".into()),
                    resolved.stroke.clone().unwrap_or_else(|| "#94a3b8".into()),
                ))
            })
            .collect();
        if entries.is_empty() {
            continue;
        }

        let widest = entries
            .iter()
            .map(|(text, _, _)| advance(text, size))
            .fold(0.0_f64, f64::max);
        let w = swatch + gap_to_text + widest;
        let h = entries.len() as f64 * step;

        let rect = match legend
            .placement
            .as_ref()
            .map(|p| resolve_grid_placement(p, &opts, &sizes))
        {
            Some(Placement::At { x, y }) => Rect { x, y, w, h },
            Some(Placement::Rel {
                dir,
                ref anchor,
                gap,
                align,
            }) => {
                let found = index.get(anchor.as_str()).and_then(|entity| match entity {
                    Entity::Node(i) => nodes.get(*i).map(|n| n.rect),
                    Entity::Group(g) => groups.get(*g).map(|f| f.rect),
                });
                let Some(anchor_rect) = found else { continue };
                offset_from(
                    &anchor_rect,
                    dir,
                    grid_distance(&opts, gap.unwrap_or(opts.gap), dir, &sizes),
                    align,
                    w,
                    h,
                )
            }
            Some(Placement::Cell { col, row }) => Rect {
                x: col,
                y: row,
                w,
                h,
            },
            None => continue,
        };

        for (i, (text, fill, stroke)) in entries.iter().enumerate() {
            let y = rect.y + i as f64 * step;
            nodes.push(PlacedNode {
                id: String::new(),
                lines: Vec::new(),
                rect: Rect {
                    x: rect.x,
                    y: y + (step - swatch) / 2.0,
                    w: swatch,
                    h: swatch,
                },
                style: Style {
                    fill: Some(fill.clone()),
                    stroke: Some(stroke.clone()),
                    stroke_width: Some(1.2),
                    radius: Some(3.0),
                    ..Style::default()
                },
            });
            annotations.push(Annotation {
                text: text.clone(),
                x: rect.x + swatch + gap_to_text,
                y: y + step / 2.0,
                anchor: Anchor::Start,
                size,
                color: style.font_color.clone().unwrap_or_else(|| {
                    if dark {
                        "#e2e8f0".into()
                    } else {
                        "#1a1a1a".into()
                    }
                }),
                weight: "500".into(),
                family: style.font_family.clone().unwrap_or_default(),
                title: false,
            });
        }
    }

    // A BPMN diagram's marks come last, because every one of them is placed
    // against a box that is already where it is going to be — including the
    // boundary events, which are moved onto their host's edge first so their
    // glyphs and captions follow them there.
    #[cfg(feature = "bpmn")]
    if process {
        crate::bpmn::attach_boundaries(&mut nodes);
        let (marks, labels) = crate::bpmn::decorate(&nodes, &captions);
        polylines.extend(marks);
        annotations.extend(labels);
    }

    // `graph { title }` on a genre that draws boxes rather than a chart. The
    // chart archetypes each reserve a band and shift their content down; here
    // nothing needs shifting, because the heading simply sits above the
    // content and `bounds` already grows the canvas to hold an annotation.
    // Flush with the left edge of the drawing, as every chart title is.
    if let Some(title) = opts.title.clone()
        && let Some(area) = bbox(
            nodes
                .iter()
                .map(|n| n.rect)
                .chain(groups.iter().map(|g| g.rect)),
        )
    {
        let theme = crate::charts::Theme::of(&opts);
        annotations.push(Annotation {
            text: title,
            x: area.x,
            // The same two-and-a-half-line clearance the charts use: at less
            // the heading sits on the top row of boxes and reads as one of
            // them rather than as a title over them.
            y: area.y - crate::charts::TITLE_SIZE * 1.5,
            anchor: Anchor::Start,
            size: crate::charts::TITLE_SIZE,
            color: theme.ink.clone(),
            weight: "600".into(),
            // Read off the drawing rather than restated here: a diagram that
            // declares its own font must not get a heading in a different
            // one, and an empty family falls through to the renderer's serif
            // default, which is never what the boxes are set in.
            family: nodes
                .first()
                .and_then(|n| n.style.font_family.clone())
                .unwrap_or_default(),
            title: true,
        });
    }

    let mut out = Layout {
        guides,
        polylines,
        groups,
        nodes,
        edges,
        opts,
        chart: Chart::Free,
        annotations,
    };
    // The same funnel the chart genres go through, for the same reason: a
    // heading can only be placed once the drawing's width is known.
    place_title(&mut out);
    Ok(out)
}

fn offset_from(a: &Rect, dir: RelDir, gap: f64, align: Option<Align>, w: f64, h: f64) -> Rect {
    let align = align.unwrap_or(Align::Center);

    match dir {
        RelDir::RightOf | RelDir::LeftOf => {
            let x = match dir {
                RelDir::RightOf => a.right() + gap,
                _ => a.x - gap - w,
            };
            let y = match align {
                Align::Start => a.y,
                Align::Center => a.cy() - h / 2.0,
                Align::End => a.bottom() - h,
            };
            Rect { x, y, w, h }
        }
        RelDir::Below | RelDir::Above => {
            let y = match dir {
                RelDir::Below => a.bottom() + gap,
                _ => a.y - gap - h,
            };
            let x = match align {
                Align::Start => a.x,
                Align::Center => a.cx() - w / 2.0,
                Align::End => a.right() - w,
            };
            Rect { x, y, w, h }
        }
    }
}

// ---------------------------------------------------------------- containers

pub struct Solved {
    pub nodes: Vec<Option<Rect>>,
    pub groups: Vec<Option<Rect>>,
}

/// One fixpoint over both kinds of entity.
///
/// A node waits for its anchor; a group waits for its members. A group is
/// *sized* by its members — never the other way round — which keeps `D-001`
/// intact: the nodes still say where they go. A group that carries a
/// placement translates its whole subtree once its members have landed, and
/// its members only become anchorable after that translation, so nothing
/// outside can latch onto a position that is about to move.
/// Apply the declared constraints to the solved rectangles.
///
/// A pass over what is already placed, in declaration order, adjusting one
/// axis and never the order — which is the whole of what `D-031` allows,
/// because `D-001` makes the order the author's to state. Nothing here can
/// move a box past another one.
///
/// `equal-width` and `equal-height`, applied to the intrinsic sizes.
fn constrain_sizes(plan: &PlacementPlan<'_>, sizes: &mut [(f64, f64)]) {
    use crate::ast::ConstraintKind;

    // Equality is transitive: `equal-width a b`, then `equal-width b c`
    // means all three share one width, regardless of statement order. The old
    // pass mutated each declaration in turn, so the first relation could be
    // false again after the second. Build each axis' connected components
    // before choosing its largest intrinsic dimension.
    let mut widths = DisjointSets::new(sizes.len());
    let mut heights = DisjointSets::new(sizes.len());
    for constraint_ref in plan.constraints() {
        let constraint = constraint_ref.declaration();
        let members = plan.constraint_members(constraint_ref);
        if members.len() < 2 {
            continue;
        }
        match constraint.kind {
            ConstraintKind::SameWidth => {
                for member in &members[1..] {
                    widths.union(members[0], *member);
                }
            }
            ConstraintKind::SameHeight => {
                for member in &members[1..] {
                    heights.union(members[0], *member);
                }
            }
            ConstraintKind::AlignGroup => {}
        }
    }

    // The largest wins, never the first: shrinking to a narrower neighbour
    // would clip text, which is worse than an unmet constraint.
    let mut widest = vec![0.0_f64; sizes.len()];
    let mut tallest = vec![0.0_f64; sizes.len()];
    for (node, (width, height)) in sizes.iter().copied().enumerate() {
        let width_root = widths.find(node);
        widest[width_root] = widest[width_root].max(width);
        let height_root = heights.find(node);
        tallest[height_root] = tallest[height_root].max(height);
    }
    for (node, size) in sizes.iter_mut().enumerate() {
        size.0 = widest[widths.find(node)];
        size.1 = tallest[heights.find(node)];
    }
}

/// Tiny deterministic union-find for equality components. No hash iteration
/// is involved, so equal-size resolution follows the diagram's node order.
struct DisjointSets {
    parent: Vec<usize>,
}

impl DisjointSets {
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
        }
    }

    fn find(&mut self, node: usize) -> usize {
        if self.parent[node] != node {
            let root = self.find(self.parent[node]);
            self.parent[node] = root;
        }
        self.parent[node]
    }

    fn union(&mut self, left: usize, right: usize) {
        let left = self.find(left);
        let right = self.find(right);
        if left != right {
            self.parent[right] = left;
        }
    }
}

/// `align`, applied to the solved rectangles.
fn constrain_alignment(plan: &PlacementPlan<'_>, solved: &mut Solved) {
    use crate::ast::{ConstraintAxis, ConstraintKind};

    for constraint_ref in plan.constraints() {
        let constraint = constraint_ref.declaration();
        if constraint.kind != ConstraintKind::AlignGroup {
            continue;
        }
        let rects: Vec<usize> = plan
            .constraint_members(constraint_ref)
            .into_iter()
            .filter(|i| solved.nodes[*i].is_some())
            .collect();
        if rects.len() < 2 {
            continue;
        }

        match constraint.kind {
            // The first named box decides, rather than an average: an average
            // moves every box including the one the author was lining the
            // others up against, which is never what was meant.
            ConstraintKind::AlignGroup => {
                let Some(lead) = solved.nodes[rects[0]] else {
                    continue;
                };
                // v1 discarded the axis for `center` / `middle`, so only that
                // ambiguous legacy spelling needs a geometry fallback. The
                // directional words must mean what they say: `top` operates
                // on y and `left` on x, regardless of the current picture.
                let spread = |f: fn(&Rect) -> f64| {
                    let values: Vec<f64> = rects
                        .iter()
                        .filter_map(|i| solved.nodes[*i])
                        .map(|r| f(&r))
                        .collect();
                    values.iter().cloned().fold(f64::MIN, f64::max)
                        - values.iter().cloned().fold(f64::MAX, f64::min)
                };
                let horizontal = match constraint.axis {
                    ConstraintAxis::Horizontal => false,
                    ConstraintAxis::Vertical => true,
                    ConstraintAxis::Inferred => spread(|r| r.x) >= spread(|r| r.y),
                };

                for i in &rects[1..] {
                    // `at` is a required coordinate. The old late pass
                    // silently overwrote it, despite the language contract
                    // saying an explicit position wins over a constraint.
                    if matches!(
                        plan.declared_placement(*i),
                        Some(Placement::At { .. } | Placement::Cell { .. })
                    ) {
                        continue;
                    }
                    let Some(rect) = solved.nodes[*i].as_mut() else {
                        continue;
                    };
                    if horizontal {
                        rect.y = match constraint.align {
                            Align::Start => lead.y,
                            Align::Center => lead.cy() - rect.h / 2.0,
                            Align::End => lead.bottom() - rect.h,
                        };
                    } else {
                        rect.x = match constraint.align {
                            Align::Start => lead.x,
                            Align::Center => lead.cx() - rect.w / 2.0,
                            Align::End => lead.right() - rect.w,
                        };
                    }
                }
            }
            // Handled before placement, where a size belongs.
            ConstraintKind::SameWidth | ConstraintKind::SameHeight => {}
        }
    }
}

fn resolve_grid_placement(
    placement: &Placement,
    opts: &GraphOpts,
    sizes: &[(f64, f64)],
) -> Placement {
    if let Some(spec) = opts.grid_spec {
        let (sx, sy) = spec.pitch(sizes);
        return match placement {
            Placement::At { x, y } | Placement::Cell { col: x, row: y } => Placement::At {
                x: x * sx,
                y: y * sy,
            },
            other => other.clone(),
        };
    }
    placement.resolved(opts.grid)
}

fn grid_distance(opts: &GraphOpts, distance: f64, dir: RelDir, sizes: &[(f64, f64)]) -> f64 {
    let Some(spec) = opts.grid_spec else {
        return distance;
    };
    let (sx, sy) = spec.pitch(sizes);
    if matches!(dir, RelDir::RightOf | RelDir::LeftOf) {
        distance * sx
    } else {
        distance * sy
    }
}

fn snap_to_grid(rect: Rect, opts: &GraphOpts, sizes: &[(f64, f64)]) -> Rect {
    let Some(spec) = opts.grid_spec else {
        return rect;
    };
    let (sx, sy) = spec.pitch(sizes);
    Rect {
        x: (rect.x / sx).round() * sx,
        y: (rect.y / sy).round() * sy,
        ..rect
    }
}

fn place_all(
    diagram: &Diagram,
    plan: &PlacementPlan<'_>,
    sizes: &[(f64, f64)],
    // How far each node's *drawing* reaches past its box — left, top, right,
    // bottom. A group is sized by what it holds, and a caption drawn beside a
    // box is part of what it holds: a BPMN event's ring is forty pixels and its
    // name is three times that, so a frame built from the rings alone cut the
    // names off (`W-112`). Zero for every node in every other genre, so nothing
    // else moves.
    overhang: &[(f64, f64, f64, f64)],
    opts: &GraphOpts,
) -> Result<Solved> {
    let index = plan.index();
    let mut node_rect: Vec<Option<Rect>> = vec![None; diagram.nodes.len()];
    let mut group_rect: Vec<Option<Rect>> = vec![None; diagram.groups.len()];

    // Placed means "has coordinates"; settled means "they will not move".
    let mut node_settled = vec![false; diagram.nodes.len()];
    let mut group_settled = vec![false; diagram.groups.len()];

    // Two entities that will be translated by the same delta may anchor to
    // each other as soon as they have coordinates — they move together. Only
    // an anchor reaching *out* of its translation unit has to wait.
    let unit = |e: Entity| plan.placed_unit(e);

    let settled = |e: Entity, ns: &[bool], gs: &[bool]| match e {
        Entity::Node(i) => ns[i],
        Entity::Group(i) => gs[i],
    };
    let rect_of = |e: Entity, nr: &[Option<Rect>], gr: &[Option<Rect>]| match e {
        Entity::Node(i) => nr[i],
        Entity::Group(i) => gr[i],
    };

    loop {
        let mut progress = false;

        for i in 0..diagram.nodes.len() {
            if node_rect[i].is_some() {
                continue;
            }
            let (w, h) = sizes[i];

            // No placement: flow from the previous sibling — the previous
            // declaration inside the same container, not merely the previous
            // line in the file.
            let placement = match plan.node(i, opts.flow) {
                EffectivePlacement::Declared(placement) => {
                    Some(resolve_grid_placement(placement, opts, sizes))
                }
                EffectivePlacement::Flow { dir, anchor } => Some(Placement::Rel {
                    dir,
                    anchor: anchor.clone(),
                    gap: None,
                    align: None,
                }),
                EffectivePlacement::Series { dir, anchor, gap } => Some(Placement::Rel {
                    dir,
                    anchor: anchor.clone(),
                    gap,
                    align: None,
                }),
                EffectivePlacement::Origin => None,
            };
            let rect = match placement {
                None => Rect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
                Some(Placement::At { x, y }) => Rect { x, y, w, h },
                // Unreachable: `resolved` above turned every cell into an
                // `At`. Answered rather than panicked, and answered with what
                // a step of one would give, because no input may bring the
                // renderer down.
                Some(Placement::Cell { col, row }) => Rect {
                    x: col,
                    y: row,
                    w,
                    h,
                },
                Some(Placement::Rel {
                    dir,
                    ref anchor,
                    gap,
                    align,
                }) => {
                    let target = index[anchor.as_str()];
                    let together =
                        unit(Entity::Node(i)).is_some() && unit(Entity::Node(i)) == unit(target);
                    let ready = if together {
                        rect_of(target, &node_rect, &group_rect).is_some()
                    } else {
                        settled(target, &node_settled, &group_settled)
                    };
                    if !ready {
                        continue;
                    }
                    let anchor_rect = rect_of(target, &node_rect, &group_rect)
                        .expect("readiness implies a rectangle");
                    offset_from(
                        &anchor_rect,
                        dir,
                        grid_distance(opts, gap.unwrap_or(opts.gap), dir, sizes),
                        align,
                        w,
                        h,
                    )
                }
            };

            node_rect[i] = Some(rect);
            // A node outside every placed group settles immediately.
            node_settled[i] = unit(Entity::Node(i)).is_none();
            progress = true;
        }

        for g in 0..diagram.groups.len() {
            if group_rect[g].is_some() {
                continue;
            }
            let group = &diagram.groups[g];

            let members: Vec<Entity> = group
                .members
                .iter()
                .filter_map(|m| index.get(m.as_str()).copied())
                .collect();
            if members
                .iter()
                .any(|m| rect_of(*m, &node_rect, &group_rect).is_none())
            {
                continue;
            }

            let Some(measure) =
                measure_frame(diagram, g, &members, &node_rect, &group_rect, overhang)
            else {
                // An empty group has nothing to enclose; `check` reports it.
                group_rect[g] = Some(Rect {
                    x: 0.0,
                    y: 0.0,
                    w: 0.0,
                    h: 0.0,
                });
                group_settled[g] = true;
                progress = true;
                continue;
            };
            let frame = measure.frame;

            let placed_at = plan
                .declared_group_placement(g)
                .map(|p| resolve_grid_placement(p, opts, sizes));
            let (frame, delta) = match &placed_at {
                None => (frame, (0.0, 0.0)),
                Some(Placement::At { x, y }) | Some(Placement::Cell { col: x, row: y }) => (
                    Rect {
                        x: *x,
                        y: *y,
                        ..frame
                    },
                    (x - frame.x, y - frame.y),
                ),
                Some(Placement::Rel {
                    dir,
                    anchor,
                    gap,
                    align,
                }) => {
                    let target = index[anchor.as_str()];
                    if !settled(target, &node_settled, &group_settled) {
                        continue;
                    }
                    let anchor_rect = rect_of(target, &node_rect, &group_rect)
                        .expect("a settled entity has a rectangle");
                    let moved = offset_from(
                        &anchor_rect,
                        *dir,
                        gap.unwrap_or(opts.gap),
                        *align,
                        frame.w,
                        frame.h,
                    );
                    (moved, (moved.x - frame.x, moved.y - frame.y))
                }
            };

            if delta != (0.0, 0.0) {
                translate_subtree(diagram, index, g, delta, &mut node_rect, &mut group_rect);
            }

            group_rect[g] = Some(frame);
            group_settled[g] = true;
            settle_subtree(diagram, index, g, &mut node_settled, &mut group_settled);
            progress = true;
        }

        let done = node_rect.iter().all(Option::is_some)
            && group_rect.iter().all(Option::is_some)
            && node_settled.iter().all(|s| *s)
            && group_settled.iter().all(|s| *s);
        if done {
            break;
        }
        if !progress {
            let stuck: Vec<&str> = diagram
                .nodes
                .iter()
                .enumerate()
                .filter(|(i, _)| node_rect[*i].is_none())
                .map(|(_, n)| n.id.as_str())
                .chain(
                    diagram
                        .groups
                        .iter()
                        .enumerate()
                        .filter(|(g, _)| group_rect[*g].is_none())
                        .map(|(_, g)| g.id.as_str()),
                )
                .collect();
            bail!("anchor cycle between: {}", stuck.join(", "));
        }
    }

    Ok(Solved {
        nodes: node_rect,
        groups: group_rect,
    })
}

/// Geometry measured before a frame participates in placement. Keeping this
/// separate from the fixpoint is important: the solver consumes a stable
/// frame size, while the frame's x/y may still be constrained by an anchor.
#[derive(Debug, Clone, Copy)]
struct FrameMeasure {
    frame: Rect,
}

fn measure_frame(
    diagram: &Diagram,
    group_index: usize,
    members: &[Entity],
    node_rect: &[Option<Rect>],
    group_rect: &[Option<Rect>],
    overhang: &[(f64, f64, f64, f64)],
) -> Option<FrameMeasure> {
    let rect_of = |entity: Entity| match entity {
        Entity::Node(i) => node_rect[i],
        Entity::Group(i) => group_rect[i],
    };
    let inner = bbox(members.iter().filter_map(|member| {
        let rect = rect_of(*member)?;
        let (left, top, right, bottom) = match member {
            Entity::Node(i) => overhang[*i],
            Entity::Group(_) => (0.0, 0.0, 0.0, 0.0),
        };
        Some(Rect {
            x: rect.x - left,
            y: rect.y - top,
            w: rect.w + left + right,
            h: rect.h + top + bottom,
        })
    }))?;
    let group = &diagram.groups[group_index];
    let padding = group_padding(diagram, group_index);
    // A titled group reserves a band at the top for its name. Its title is
    // part of the measured frame, just like a caption overhanging a node.
    let title = if group.label.is_some() { 16.0 } else { 0.0 };
    let named = group
        .label
        .as_deref()
        .map(|label| text_width(&[label.to_string()], 12.0) + padding * 2.0 + 10.0)
        .unwrap_or(0.0);
    Some(FrameMeasure {
        frame: Rect {
            x: inner.x - padding,
            y: inner.y - padding - title,
            w: (inner.w + padding * 2.0).max(named),
            h: inner.h + padding * 2.0 + title,
        },
    })
}

fn translate_subtree(
    diagram: &Diagram,
    index: &HashMap<&str, Entity>,
    group: usize,
    (dx, dy): (f64, f64),
    node_rect: &mut [Option<Rect>],
    group_rect: &mut [Option<Rect>],
) {
    for member in &diagram.groups[group].members {
        match index.get(member.as_str()) {
            Some(Entity::Node(i)) => {
                if let Some(r) = node_rect[*i].as_mut() {
                    r.x += dx;
                    r.y += dy;
                }
            }
            Some(Entity::Group(g)) => {
                if let Some(r) = group_rect[*g].as_mut() {
                    r.x += dx;
                    r.y += dy;
                }
                translate_subtree(diagram, index, *g, (dx, dy), node_rect, group_rect);
            }
            None => {}
        }
    }
}

fn settle_subtree(
    diagram: &Diagram,
    index: &HashMap<&str, Entity>,
    group: usize,
    node_settled: &mut [bool],
    group_settled: &mut [bool],
) {
    for member in &diagram.groups[group].members {
        match index.get(member.as_str()) {
            Some(Entity::Node(i)) => node_settled[*i] = true,
            Some(Entity::Group(g)) => {
                group_settled[*g] = true;
                settle_subtree(diagram, index, *g, node_settled, group_settled);
            }
            None => {}
        }
    }
}

/// The rectangle a run of points occupies — a line's own bounding box.
fn span(points: &[(f64, f64)]) -> Rect {
    let (mut x0, mut y0) = (f64::MAX, f64::MAX);
    let (mut x1, mut y1) = (f64::MIN, f64::MIN);
    for (x, y) in points {
        x0 = x0.min(*x);
        y0 = y0.min(*y);
        x1 = x1.max(*x);
        y1 = y1.max(*y);
    }
    if points.is_empty() {
        return Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0,
        };
    }
    Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    }
}

fn bbox(rects: impl Iterator<Item = Rect>) -> Option<Rect> {
    let mut bounds: Option<(f64, f64, f64, f64)> = None;
    for r in rects {
        bounds = Some(match bounds {
            None => (r.x, r.y, r.right(), r.bottom()),
            Some((x0, y0, x1, y1)) => (
                x0.min(r.x),
                y0.min(r.y),
                x1.max(r.right()),
                y1.max(r.bottom()),
            ),
        });
    }
    bounds.map(|(x0, y0, x1, y1)| Rect {
        x: x0,
        y: y0,
        w: x1 - x0,
        h: y1 - y0,
    })
}

fn group_padding(diagram: &Diagram, g: usize) -> f64 {
    diagram.groups[g]
        .props
        .iter()
        .find(|p| p.name == "padding")
        .and_then(|p| p.values.first())
        .and_then(|v| v.as_num())
        .unwrap_or(18.0)
}
