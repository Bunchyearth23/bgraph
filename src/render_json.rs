//! The solved geometry as JSON, for a program that wants the picture without
//! learning the language.
//!
//! Extends the embedding promise past "give me an SVG": a document generator
//! that needs to place a caption under a box, a test that asserts two things
//! do not overlap, an editor drawing its own handles — all of them need the
//! rectangles, not the markup.
//!
//! # What is promised, and what is not
//!
//! `D-018` keeps [`crate::layout`] deliberately outside the semver surface,
//! because pinning it would mean the diagrams could never be improved. This
//! export inherits exactly half of that, and the split is the useful part:
//!
//! - **The shape is stable.** Field names, nesting, types. Code written against
//!   this schema keeps compiling and keeps reading the right fields.
//! - **The values are not.** A coordinate moves whenever the router, the
//!   advance table or a default changes — the same way the SVG moves, and for
//!   the same reasons. A consumer that pins a number is pinning a drawing
//!   decision, and it will break.
//!
//! `version` is in the document so a consumer can tell when the first half of
//! that has changed.
//!
//! # Coordinates
//!
//! Canvas space, the same numbers the SVG carries — the layout's own
//! coordinates translated by the padding offset, so a rectangle here is the
//! rectangle in the file beside it. Emitting the untranslated form would hand
//! every consumer the same off-by-a-padding bug to discover on their own.

use serde_json::{Value, json};

use crate::layout::{Annotation, Layout, Rect};

/// The schema's version. Bumped when a field is renamed, moved or removed —
/// never for a coordinate that moved, which is expected and is not a change to
/// the shape.
pub const VERSION: u32 = 1;

/// The whole layout as a JSON document.
pub fn render(layout: &Layout) -> String {
    to_value(layout).to_string()
}

/// The same, indented, for a person reading it in a terminal.
pub fn render_pretty(layout: &Layout) -> String {
    serde_json::to_string_pretty(&to_value(layout)).unwrap_or_else(|_| render(layout))
}

fn to_value(layout: &Layout) -> Value {
    let routed = crate::render_svg::route_all(layout);
    let (dx, dy, width, height) = crate::render_svg::canvas(layout);
    let at = |r: &Rect| rect(r, dx, dy);

    json!({
        "version": VERSION,
        "canvas": { "width": width, "height": height },
        "background": layout.opts.background,
        "title": layout.opts.title,
        "description": layout.opts.description,
        "groups": layout.groups.iter().map(|g| json!({
            "id": g.id,
            "label": g.lines,
            "description": g.style.description,
            "rect": at(&g.rect),
        })).collect::<Vec<_>>(),
        "nodes": layout.nodes.iter().map(|n| json!({
            // Empty for a box the author never wrote — a legend swatch, a
            // compartment rule. Reported rather than omitted, so a consumer
            // sees the whole drawing and can decide what is addressable.
            "id": n.id,
            "label": n.lines,
            "rect": at(&n.rect),
            // What the box *is*, when the file said. The same sentence the
            // SVG carries as a `<title>` — a consumer building an index or a
            // caption should not have to parse the picture for it.
            "description": n.style.description,
            "fill": n.style.fill,
            "stroke": n.style.stroke,
        })).collect::<Vec<_>>(),
        "edges": layout.edges.iter().zip(&routed).map(|(e, r)| json!({
            "from": layout.nodes.get(e.from).map(|n| n.id.clone()),
            "to": layout.nodes.get(e.to).map(|n| n.id.clone()),
            "label": e.label,
            // The route as drawn, elbows and all — not the two endpoints. An
            // orthogonal edge that turns twice is three segments, and a
            // consumer drawing a straight line between the ends would be
            // drawing a different diagram.
            "points": r.points.iter().map(|(x, y)| json!([x + dx, y + dy])).collect::<Vec<_>>(),
            "label_rect": r.label.as_ref().map(|(_, rect)| at(rect)),
        })).collect::<Vec<_>>(),
        "label_conflicts": label_conflicts(layout, &routed),
        "annotations": layout.annotations.iter().map(|a| text(a, dx, dy)).collect::<Vec<_>>(),
        "guides": layout.guides.iter().map(|g| json!({
            "from": [g.from.0 + dx, g.from.1 + dy],
            "to": [g.to.0 + dx, g.to.1 + dy],
        })).collect::<Vec<_>>(),
    })
}

/// Residual conflicts are data, not a second validator: the renderer has
/// already picked the least-cost deterministic arrangement, and an editor can
/// now show the remaining trade-off or offer `label-offset` to its author.
fn label_conflicts(layout: &Layout, routed: &[crate::render_svg::Routed]) -> Vec<Value> {
    let mut out = Vec::new();
    for (edge, route) in routed.iter().enumerate() {
        let Some((_, label)) = &route.label else {
            continue;
        };
        for placed in &layout.nodes {
            if overlaps(label, &placed.rect) {
                out.push(json!({ "kind": "label-node", "edge": edge, "node": placed.id }));
            }
        }
        for (other, other_route) in routed.iter().enumerate().skip(edge + 1) {
            if let Some((_, other_label)) = &other_route.label
                && overlaps(label, other_label)
            {
                out.push(json!({ "kind": "label-label", "edge": edge, "other": other }));
            }
        }
    }
    out
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() && a.right() > b.x && a.y < b.bottom() && a.bottom() > b.y
}

fn rect(r: &Rect, dx: f64, dy: f64) -> Value {
    json!({ "x": r.x + dx, "y": r.y + dy, "w": r.w, "h": r.h })
}

fn text(a: &Annotation, dx: f64, dy: f64) -> Value {
    json!({
        "text": a.text,
        "x": a.x + dx,
        "y": a.y + dy,
        "size": a.size,
        "color": a.color,
    })
}
