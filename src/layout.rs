//! Anchor solver: turns declarative placement into absolute rectangles.
//!
//! Nodes are resolved by repeated passes — a node whose anchor is not placed
//! yet simply waits for the next round. If a full pass places nothing, the
//! remaining nodes form an anchor cycle.

use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::ast::*;
use crate::style::{GraphOpts, Registry, Shape, Style};

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

pub struct Layout {
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

/// Rough advance width for a proportional sans-serif — enough to size a box
/// around its label without shipping font metrics.
pub fn text_width(lines: &[String], font_size: f64) -> f64 {
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    longest as f64 * font_size * 0.56
}

fn natural_size(lines: &[String], style: &Style) -> (f64, f64) {
    let font_size = style.font_size.unwrap_or(14.0);
    let padding = style.padding.unwrap_or(14.0);

    let mut w = text_width(lines, font_size) + padding * 2.0;
    let mut h = lines.len().max(1) as f64 * font_size * LINE_HEIGHT + padding * 2.0;

    // Round shapes need extra room: their corners cut into the text box.
    match style.shape.unwrap_or(Shape::Rect) {
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

    (
        style.width.unwrap_or(w.max(MIN_WIDTH)),
        style.height.unwrap_or(h.max(MIN_HEIGHT)),
    )
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
        "pie" => return crate::charts::pie(diagram, opts, &registry),
        "packet" => return crate::charts::packet(diagram, opts, &registry),
        _ => {}
    }

    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        index.insert(node.id.as_str(), i);
    }

    // Pass 1 — resolve styles and intrinsic sizes; positions come later.
    let mut sizes = Vec::with_capacity(diagram.nodes.len());
    let mut styles = Vec::with_capacity(diagram.nodes.len());
    let mut labels = Vec::with_capacity(diagram.nodes.len());

    let item = crate::check::archetype_of(diagram);
    for node in &diagram.nodes {
        let style = registry
            .resolve(
                Style::node_defaults(),
                item.item,
                &node.classes,
                &node.effective_props(item.slots),
            )
            .map_err(|e| e.context(format!("in node `{}`", node.id.as_str())))?;

        let lines: Vec<String> = node
            .label
            .clone()
            .unwrap_or_else(|| node.id.value.clone())
            .split('\n')
            .map(|l| l.trim().to_string())
            .collect();

        sizes.push(natural_size(&lines, &style));
        styles.push(style);
        labels.push(lines);
    }

    // Pass 2 — place, retrying until every anchor is known.
    let mut placed: Vec<Option<Rect>> = vec![None; diagram.nodes.len()];
    let mut remaining: Vec<usize> = (0..diagram.nodes.len()).collect();

    while !remaining.is_empty() {
        let before = remaining.len();
        let mut deferred = Vec::new();

        for &i in &remaining {
            let node = &diagram.nodes[i];
            let (w, h) = sizes[i];

            // No explicit placement: flow from the previous node in source order.
            let placement = node.placement.clone().unwrap_or_else(|| match i {
                0 => Placement::At { x: 0.0, y: 0.0 },
                _ => Placement::Rel {
                    dir: opts.flow,
                    anchor: diagram.nodes[i - 1].id.clone(),
                    gap: None,
                    align: None,
                },
            });

            let rect = match placement {
                Placement::At { x, y } => Rect { x, y, w, h },
                Placement::Rel {
                    dir,
                    ref anchor,
                    gap,
                    align,
                } => {
                    let a = index[anchor.as_str()];
                    let Some(anchor_rect) = placed[a] else {
                        deferred.push(i);
                        continue;
                    };
                    offset_from(&anchor_rect, dir, gap.unwrap_or(opts.gap), align, w, h)
                }
            };

            placed[i] = Some(rect);
        }

        if deferred.len() == before {
            let names: Vec<&str> = deferred
                .iter()
                .map(|&i| diagram.nodes[i].id.as_str())
                .collect();
            bail!("anchor cycle between: {}", names.join(", "));
        }
        remaining = deferred;
    }

    let nodes: Vec<PlacedNode> = diagram
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
        let (from, to) = (index[edge.from.as_str()], index[edge.to.as_str()]);

        let mut style = registry
            .resolve(Style::edge_defaults(), "edge", &edge.classes, &edge.props)
            .map_err(|e| {
                e.context(format!(
                    "in edge `{} -> {}`",
                    edge.from.as_str(),
                    edge.to.as_str()
                ))
            })?;

        // A dashed/dotted arrow token sets the pattern unless a style already did.
        if let Some(dash) = edge.arrow.dash
            && style.dash.is_none()
        {
            style.dash = Some(dash.to_string());
        }

        edges.push(PlacedEdge {
            from,
            to,
            label: edge.label.clone(),
            arrow: edge.arrow,
            style,
        });
    }

    Ok(Layout {
        nodes,
        edges,
        opts,
        chart: Chart::Free,
        annotations: Vec::new(),
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
