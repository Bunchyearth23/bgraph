//! Anchor solver: turns declarative placement into absolute rectangles.
//!
//! Nodes are resolved by repeated passes — a node whose anchor is not placed
//! yet simply waits for the next round. If a full pass places nothing, the
//! remaining nodes form an anchor cycle.

use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::ast::*;
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
}

pub struct PlacedGroup {
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
        Shape::Hexagon => w *= 1.20,
        Shape::Cylinder => h += 14.0,
        _ => {}
    }
    (w, h)
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

    let opts = GraphOpts::from_props(&diagram.graph)?;
    let registry = Registry::build(&diagram.styles)?;

    // The archetype decides what the statements mean.
    match diagram
        .archetype
        .as_ref()
        .map(|a| a.as_str())
        .unwrap_or("free")
    {
        #[cfg(feature = "pie")]
        "pie" => return crate::charts::pie(diagram, opts, &registry),
        #[cfg(feature = "packet")]
        "packet" => return crate::charts::packet(diagram, opts, &registry),
        #[cfg(feature = "kanban")]
        "kanban" => return crate::charts::kanban(diagram, opts, &registry),
        #[cfg(feature = "journey")]
        "journey" => return crate::charts::journey(diagram, opts, &registry),
        #[cfg(feature = "radar")]
        "radar" => return crate::charts::radar(diagram, opts, &registry),
        #[cfg(feature = "rack")]
        "rack" => return crate::charts::rack(diagram, opts, &registry),
        #[cfg(feature = "venn")]
        "venn" => return crate::charts::venn(diagram, opts, &registry),
        #[cfg(feature = "gitgraph")]
        "gitgraph" => return crate::charts::gitgraph(diagram, opts, &registry),
        #[cfg(feature = "orgchart")]
        "orgchart" => {
            return crate::charts::tree(diagram, opts, &registry, crate::charts::TreeKind::Org);
        }
        #[cfg(feature = "mindmap")]
        "mindmap" => {
            return crate::charts::tree(diagram, opts, &registry, crate::charts::TreeKind::Mind);
        }
        #[cfg(feature = "treeview")]
        "treeview" => {
            return crate::charts::tree(diagram, opts, &registry, crate::charts::TreeKind::View);
        }
        #[cfg(feature = "bytefield")]
        "bytefield" => return crate::charts::bytefield(diagram, opts, &registry),
        #[cfg(feature = "treemap")]
        "treemap" => return crate::charts::treemap(diagram, opts, &registry),
        #[cfg(feature = "quadrant")]
        "quadrant" => return crate::charts::quadrant(diagram, opts, &registry),
        #[cfg(feature = "wave")]
        "wave" => return crate::charts::wave(diagram, opts, &registry),
        #[cfg(feature = "sankey")]
        "sankey" => return crate::charts::sankey(diagram, opts, &registry),
        #[cfg(feature = "bar")]
        "bar" => return crate::charts::bar(diagram, opts, &registry),
        #[cfg(feature = "xy")]
        "xy" => return crate::charts::xy(diagram, opts, &registry),
        #[cfg(feature = "sequence")]
        "sequence" => return crate::charts::sequence(diagram, opts, &registry),
        #[cfg(feature = "gantt")]
        "gantt" => return crate::charts::gantt(diagram, opts, &registry),
        #[cfg(feature = "timeline")]
        "timeline" => return crate::charts::timeline(diagram, opts, &registry),
        _ => {}
    }

    let mut index: HashMap<&str, Entity> = HashMap::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        index.insert(node.id.as_str(), Entity::Node(i));
    }
    for (i, group) in diagram.groups.iter().enumerate() {
        index.insert(group.id.as_str(), Entity::Group(i));
    }

    // Pass 1 — resolve styles and intrinsic sizes; positions come later.
    let mut sizes = Vec::with_capacity(diagram.nodes.len());
    let mut styles = Vec::with_capacity(diagram.nodes.len());
    let mut labels = Vec::with_capacity(diagram.nodes.len());

    let item = crate::check::archetype_of(diagram);
    let machine = item.name == "state";
    let architecture = item.name == "block";
    let topology = item.name == "network";

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
    } else if topology {
        // Only `shape` is left unset: a topology's colours usually already
        // mean something to its author (environment, tenancy, trust zone), so
        // the archetype chooses the outline and keeps out of the palette.
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
                item.item,
                &node.classes,
                &node.effective_props(item.slots),
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

        let mut lines: Vec<String> = if mark {
            Vec::new()
        } else {
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
    let solved = place_all(diagram, &index, &sizes, &opts)?;
    let placed = solved.nodes;

    let mut nodes: Vec<PlacedNode> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, _node)| PlacedNode {
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
        let mut style = registry
            .resolve(
                Style::edge_defaults(),
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

        // A dashed/dotted arrow token sets the pattern unless a style already did.
        if let Some(dash) = edge.arrow.dash
            && style.dash.is_none()
        {
            style.dash = Some(dash.to_string());
        }

        edges.push(PlacedEdge {
            from,
            to,
            label,
            arrow: edge.arrow,
            style,
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
            for (i, line) in std::mem::take(&mut placed.lines).into_iter().enumerate() {
                annotations.push(Annotation {
                    text: line,
                    x: placed.rect.cx(),
                    y: placed.rect.y + pad + (i as f64 + 0.5) * font * 1.45,
                    anchor: Anchor::Middle,
                    size: font,
                    color: s.font_color.clone().unwrap_or_else(|| "#111827".into()),
                    weight: s.font_weight.clone().unwrap_or_else(|| "600".into()),
                    family: s.font_family.clone().unwrap_or_default(),
                });
            }

            if node.members.is_empty() {
                continue;
            }

            // A thin filled rect, not a `Guide`: guides are drawn beneath the
            // boxes, so the rule would vanish under the class's own fill.
            let thickness = s.stroke_width.unwrap_or(1.5);
            rules.push(PlacedNode {
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
            let mid = (xa + out + xb - out) / 2.0;

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
            });

            // The pin pair is no longer written: the wire touches both, which
            // says it better than a label beside the line ever did.
            if let Some(label) = edge.label.as_ref().filter(|l| !l.is_empty()) {
                annotations.push(Annotation {
                    text: label.clone(),
                    x: mid,
                    y: (ya + yb) / 2.0 - 4.0,
                    anchor: Anchor::Middle,
                    size: placed.style.font_size.unwrap_or(11.0),
                    color: placed
                        .style
                        .font_color
                        .clone()
                        .unwrap_or_else(|| "#94a3b8".into()),
                    weight: "400".into(),
                    family: placed.style.font_family.clone().unwrap_or_default(),
                });
            }
        }
        // Every wire is drawn above; leaving the routed edges in would draw
        // each one twice, once between the boxes.
        edges.clear();
    }

    // After the boxes, so the compartment rules land on top of them.
    nodes.extend(rules);

    Ok(Layout {
        guides,
        polylines,
        groups,
        nodes,
        edges,
        opts,
        chart: Chart::Free,
        annotations,
    })
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entity {
    Node(usize),
    Group(usize),
}

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
fn place_all(
    diagram: &Diagram,
    index: &HashMap<&str, Entity>,
    sizes: &[(f64, f64)],
    opts: &GraphOpts,
) -> Result<Solved> {
    let mut node_rect: Vec<Option<Rect>> = vec![None; diagram.nodes.len()];
    let mut group_rect: Vec<Option<Rect>> = vec![None; diagram.groups.len()];

    // Placed means "has coordinates"; settled means "they will not move".
    let mut node_settled = vec![false; diagram.nodes.len()];
    let mut group_settled = vec![false; diagram.groups.len()];

    let owner = ownership(diagram, index);
    let previous_sibling = siblings(diagram, &owner);

    // Two entities that will be translated by the same delta may anchor to
    // each other as soon as they have coordinates — they move together. Only
    // an anchor reaching *out* of its translation unit has to wait.
    let unit = |e: Entity| placed_unit(diagram, &owner, e);

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
            let node = &diagram.nodes[i];
            let (w, h) = sizes[i];

            // No placement: flow from the previous sibling — the previous
            // declaration inside the same container, not merely the previous
            // line in the file.
            let placement = node.placement.clone().or_else(|| {
                previous_sibling[i].map(|prev| Placement::Rel {
                    dir: opts.flow,
                    anchor: diagram.nodes[prev].id.clone(),
                    gap: None,
                    align: None,
                })
            });

            let rect = match placement {
                None => Rect {
                    x: 0.0,
                    y: 0.0,
                    w,
                    h,
                },
                Some(Placement::At { x, y }) => Rect { x, y, w, h },
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
                    offset_from(&anchor_rect, dir, gap.unwrap_or(opts.gap), align, w, h)
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

            let padding = group_padding(diagram, g);
            let Some(inner) = bbox(
                members
                    .iter()
                    .filter_map(|m| rect_of(*m, &node_rect, &group_rect)),
            ) else {
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

            let frame = Rect {
                x: inner.x - padding,
                y: inner.y - padding,
                w: inner.w + padding * 2.0,
                h: inner.h + padding * 2.0,
            };

            let (frame, delta) = match &group.placement {
                None => (frame, (0.0, 0.0)),
                Some(Placement::At { x, y }) => (
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

/// Which group, if any, declares each entity.
fn ownership(diagram: &Diagram, index: &HashMap<&str, Entity>) -> HashMap<Entity, usize> {
    let mut owner = HashMap::new();
    for (g, group) in diagram.groups.iter().enumerate() {
        for member in &group.members {
            if let Some(entity) = index.get(member.as_str()) {
                owner.insert(*entity, g);
            }
        }
    }
    owner
}

/// The node an unplaced node flows from: the previous *node* declared in the
/// same container, so a group does not inherit the drift of the one before it.
fn siblings(diagram: &Diagram, owner: &HashMap<Entity, usize>) -> Vec<Option<usize>> {
    let mut previous: HashMap<Option<usize>, usize> = HashMap::new();
    let mut out = Vec::with_capacity(diagram.nodes.len());

    for i in 0..diagram.nodes.len() {
        let container = owner.get(&Entity::Node(i)).copied();
        out.push(previous.get(&container).copied());
        previous.insert(container, i);
    }

    out
}

/// The innermost ancestor group that carries a placement — the block this
/// entity will be translated with, if any.
fn placed_unit(diagram: &Diagram, owner: &HashMap<Entity, usize>, entity: Entity) -> Option<usize> {
    let mut cursor = owner.get(&entity).copied();
    let mut seen = Vec::new();
    while let Some(g) = cursor {
        if seen.contains(&g) {
            return None;
        }
        seen.push(g);
        if diagram.groups[g].placement.is_some() {
            return Some(g);
        }
        cursor = owner.get(&Entity::Group(g)).copied();
    }
    None
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
