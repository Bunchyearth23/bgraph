//! Two versions of a diagram, as one picture.
//!
//! A diagram in review shows a diff of its *source*, which nobody can read as
//! a picture. What a reviewer wants to know is what appeared, what left and
//! what moved — and all three are visible at a glance if you draw them.
//!
//! Matching is by identifier, which the language already gives every box, so
//! this is a **set difference on the solved geometry** rather than an image
//! comparison. It stays vector, it stays deterministic, and it needs no
//! rasteriser.
//!
//! What is drawn:
//!
//! | | |
//! |---|---|
//! | added | in full colour, in the new layout |
//! | removed | where it used to be, struck through in red |
//! | moved | in the new place, with a ghost of the old one |
//! | unchanged | muted, so the changes are what the eye lands on |

use crate::layout::{Anchor, Annotation, Chart, Layout, PlacedNode, Polyline, Rect};
use crate::style::Style;

const ADDED: &str = "#16a34a";
const REMOVED: &str = "#dc2626";
const MOVED: &str = "#d97706";
const UNCHANGED: &str = "#94a3b8";

/// The two layouts as one, ready to render.
pub fn compare(before: &Layout, after: &Layout) -> Layout {
    let mut out = Layout {
        guides: Vec::new(),
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        // The *new* diagram's surface and padding: a diff is a view of what
        // the drawing has become, not a third document with settings of its
        // own.
        opts: after.opts.clone(),
        chart: Chart::Free,
        annotations: Vec::new(),
    };

    let mut removed = 0usize;
    let find = |layout: &Layout, id: &str| {
        layout
            .nodes
            .iter()
            .find(|n| !n.id.is_empty() && n.id == id)
            .map(|n| n.rect)
    };

    for node in &after.nodes {
        let was = if node.id.is_empty() {
            None
        } else {
            find(before, &node.id)
        };
        match was {
            None => out.nodes.push(tinted(node, ADDED, false)),
            Some(old) if moved(old, node.rect) => {
                // A ghost where it was, the box where it is. The pair is what
                // says "moved" — either alone reads as removed or added.
                out.nodes.push(ghost(old, MOVED));
                out.nodes.push(tinted(node, MOVED, false));
            }
            Some(_) => out.nodes.push(tinted(node, UNCHANGED, false)),
        }
    }

    // What is gone, in a row beneath the drawing rather than where it stood.
    //
    // Drawing it in place is the obvious idea and it does not survive
    // contact: something new usually takes the space a removed box left, and
    // the two then sit on the same rectangle with their labels written over
    // each other — the first attempt here produced a box reading "Qaeuhee".
    // A gutter never collides, and where a box *used to be* is rarely the
    // question; a box that merely moved is already shown by its ghost.
    let baseline = out
        .nodes
        .iter()
        .map(|n| n.rect.bottom())
        .fold(f64::MIN, f64::max);
    let left = out.nodes.iter().map(|n| n.rect.x).fold(f64::MAX, f64::min);
    let mut cursor = if left.is_finite() { left } else { 0.0 };

    for node in &before.nodes {
        if node.id.is_empty() || find(after, &node.id).is_some() {
            continue;
        }
        let rect = Rect {
            x: cursor,
            y: baseline + 56.0,
            w: node.rect.w,
            h: node.rect.h,
        };
        cursor += node.rect.w + 18.0;
        out.nodes.push(PlacedNode {
            rect,
            ..tinted(node, REMOVED, true)
        });
        out.polylines.push(strike(rect, REMOVED));
        removed += 1;
    }

    if removed > 0 {
        out.annotations.push(Annotation {
            text: "gone:".into(),
            x: if left.is_finite() { left } else { 0.0 },
            y: baseline + 40.0,
            anchor: Anchor::Start,
            size: 12.0,
            color: REMOVED.into(),
            weight: "600".into(),
            family: String::new(),
        });
    }

    // Frames follow their contents; nothing about them is matched separately,
    // because a frame is sized by what it holds and would report a change
    // that belongs to a box inside it.
    for group in &after.groups {
        out.groups.push(crate::layout::PlacedGroup {
            id: group.id.clone(),
            lines: group.lines.clone(),
            rect: group.rect,
            style: Style {
                fill: None,
                stroke: Some(UNCHANGED.into()),
                stroke_width: Some(1.0),
                dash: Some("dashed".into()),
                ..Style::default()
            },
        });
    }

    out.annotations.push(key(after));
    out
}

/// Whether a box has moved far enough to be worth saying so.
///
/// Half a pixel, not zero: a box whose neighbour gained a character shifts by
/// a fraction, and reporting that as a move would make every diff say
/// everything moved.
fn moved(a: Rect, b: Rect) -> bool {
    (a.x - b.x).abs() > 0.5 || (a.y - b.y).abs() > 0.5
}

fn tinted(node: &PlacedNode, colour: &str, faded: bool) -> PlacedNode {
    PlacedNode {
        id: node.id.clone(),
        lines: node.lines.clone(),
        rect: node.rect,
        style: Style {
            // `faded` is the removed case: a pale fill and a dashed border,
            // in the gutter under the drawing.
            fill: Some(tint(colour)),
            stroke: Some(colour.into()),
            stroke_width: Some(1.4),
            dash: if faded { Some("dashed".into()) } else { None },
            font_color: Some(colour.into()),
            font_size: node.style.font_size,
            font_family: node.style.font_family.clone(),
            radius: node.style.radius,
            ..Style::default()
        },
    }
}

/// An outline with nothing in it: where a box used to be.
fn ghost(rect: Rect, colour: &str) -> PlacedNode {
    PlacedNode {
        id: String::new(),
        lines: Vec::new(),
        rect,
        style: Style {
            fill: Some("none".into()),
            stroke: Some(colour.into()),
            stroke_width: Some(1.0),
            dash: Some("dashed".into()),
            ..Style::default()
        },
    }
}

fn strike(rect: Rect, colour: &str) -> Polyline {
    Polyline {
        points: vec![(rect.x, rect.y), (rect.right(), rect.bottom())],
        closed: false,
        fill: None,
        stroke: Some(colour.into()),
        width: 1.2,
        head: None,
        dash: None,
        over: false,
    }
}

/// A pale version of a colour, for a fill under text of the same hue.
fn tint(colour: &str) -> String {
    match colour {
        ADDED => "#dcfce7".into(),
        REMOVED => "#fee2e2".into(),
        MOVED => "#fef3c7".into(),
        _ => "#f1f5f9".into(),
    }
}

/// One line saying what the colours mean, above the drawing.
fn key(after: &Layout) -> Annotation {
    let top = after
        .nodes
        .iter()
        .map(|n| n.rect.y)
        .fold(f64::MAX, f64::min);
    let left = after
        .nodes
        .iter()
        .map(|n| n.rect.x)
        .fold(f64::MAX, f64::min);
    Annotation {
        text: "green added · red removed · amber moved".into(),
        x: if left.is_finite() { left } else { 0.0 },
        y: if top.is_finite() { top - 22.0 } else { 0.0 },
        anchor: Anchor::Start,
        size: 12.0,
        color: UNCHANGED.into(),
        weight: "500".into(),
        family: String::new(),
    }
}
