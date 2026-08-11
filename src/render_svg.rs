//! SVG emission: shapes, edge routing and text. The viewBox is derived from
//! the solved layout, so a diagram is never clipped by a hardcoded canvas.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::charts::annotation_bounds;
use crate::layout::{
    Anchor, Annotation, Chart, Guide, Layout, Pie, PlacedGroup, PlacedNode, Polyline, Rect,
    text_width,
};
use crate::style::{Head, Router, Shape, Style};

/// An edge after routing, with its label already placed.
/// An edge, once it knows where it runs and where its label sits.
///
/// Shared with the PDF writer for the same reason [`canvas`] is: routing,
/// endpoint clipping and label avoidance are the difference between a line
/// between two centres and a drawn edge, and a second implementation of them
/// would be a second answer.
pub struct Routed {
    /// The line as drawn, elbows included — not just the two endpoints.
    pub points: Vec<(f64, f64)>,
    /// The label and the rectangle it was given, once avoidance has run.
    pub label: Option<(String, Rect)>,
}

/// The page: how much to shift the geometry, and how big the surface is.
///
/// Shared with the PDF writer so the two outputs cannot disagree about where
/// the diagram sits — a second implementation of this would be a second place
/// for the viewBox to drift from the page box.
pub fn canvas(layout: &Layout) -> (f64, f64, f64, f64) {
    let routed = route_all(layout);
    let pad = layout.opts.padding;
    let (min_x, min_y, max_x, max_y) = bounds(layout, &routed);
    (
        pad - min_x,
        pad - min_y,
        (max_x - min_x) + pad * 2.0,
        (max_y - min_y) + pad * 2.0,
    )
}

/// The whole diagram as one SVG document.
pub fn render(layout: &Layout) -> String {
    // Routing comes first: a self-loop reaches above its node, and a label
    // may be nudged clear of a neighbouring line, so neither the canvas nor
    // the label placement can be decided before the geometry exists.
    let routed = route_all(layout);

    let (dx, dy, width, height) = canvas(layout);

    let mut body = String::new();
    let mut markers = BTreeMap::new();
    let mut needs_shadow = false;

    // Guides sit under everything, frames under what they enclose.
    for guide in &layout.guides {
        body.push_str(&guide_line(guide));
    }
    for shape in layout.polylines.iter().filter(|p| !p.over) {
        if let Some(head) = shape.head
            && head != Head::None
        {
            let colour = shape.stroke.as_deref().unwrap_or("#64748b");
            markers
                .entry(marker_id(head, colour))
                .or_insert((head, colour.to_string()));
        }
        body.push_str(&free_polyline(shape));
    }
    for group in &layout.groups {
        body.push_str(&described(
            &group_frame(group, &layout.opts.background),
            group.style.description.as_deref(),
        ));
    }

    for (edge, route) in layout.edges.iter().zip(&routed) {
        if route.points.len() < 2 {
            continue;
        }

        let head = edge.style.head.unwrap_or(Head::Solid);
        let stroke = edge.style.stroke.as_deref().unwrap_or("#64748b");
        let mut attrs = String::new();

        // Each end is decided on its own: a declared cardinality replaces the
        // arrowhead there, because a crow's foot already says which way the
        // relation reads and two marks at one end would fight.
        let end_head = edge
            .style
            .to_card
            .or_else(|| edge.arrow.head_end.then_some(head));
        let start_head = edge
            .style
            .from_card
            .or_else(|| edge.arrow.head_start.then_some(head));

        for (which, head) in [("end", end_head), ("start", start_head)] {
            let Some(head) = head.filter(|h| *h != Head::None) else {
                continue;
            };
            let id = marker_id(head, stroke);
            markers
                .entry(id.clone())
                .or_insert((head, stroke.to_string()));
            let _ = write!(attrs, r#" marker-{which}="url(#{id})""#);
        }
        if let Some(dash) = edge.style.dash_array() {
            let _ = write!(attrs, r#" stroke-dasharray="{dash}""#);
        }

        let path: Vec<String> = route
            .points
            .iter()
            .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
            .collect();
        let _ = write!(
            body,
            r#"<polyline points="{}" fill="none"{} stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"{}/>"#,
            path.join(" "),
            paint("stroke", stroke),
            n(edge.style.stroke_width.unwrap_or(1.5)),
            attrs
        );
    }

    // Labels last among the edge layer, so no line is drawn over a plate.
    for (edge, route) in layout.edges.iter().zip(&routed) {
        if let Some((text, rect)) = &route.label {
            body.push_str(&edge_label(
                text,
                rect,
                &edge.style,
                &layout.opts.background,
            ));
        }
    }

    if let Chart::Pie(pie) = &layout.chart {
        body.push_str(&pie_slices(pie));
    }

    for node in &layout.nodes {
        if node.style.shadow == Some(true) {
            needs_shadow = true;
        }
        let shape = if layout.opts.sketch {
            sketch_shape(node)
        } else {
            node_shape(node)
        };
        body.push_str(&described(&shape, node.style.description.as_deref()));
        body.push_str(&node_label(node));
    }

    // A mark that lives *inside* a shape goes on after it. There is exactly one
    // kind — the glyph in a BPMN ring, diamond or corner — and under the box it
    // would simply not be there.
    for shape in layout.polylines.iter().filter(|p| p.over) {
        body.push_str(&free_polyline(shape));
    }

    for annotation in &layout.annotations {
        body.push_str(&annotation_text(annotation));
    }

    let mut defs = String::new();
    // The font, when one was handed in. An `@font-face` in `<defs><style>` is
    // what makes an SVG's text *exact* everywhere rather than approximately
    // right — it is the practical half of `X-005`, which is that this crate
    // measures text with an advance table and a viewer may not have the font
    // that table describes. Carrying the font removes the "may not".
    if let Some((family, bytes)) = &layout.opts.embed_font {
        let _ = write!(
            defs,
            r#"<style>@font-face{{font-family:"{}";src:url("data:font/ttf;base64,{}");}}</style>"#,
            esc(family),
            base64(bytes)
        );
    }
    for (id, (head, color)) in &markers {
        defs.push_str(&marker_def(id, *head, color, &layout.opts.background));
    }
    if needs_shadow {
        defs.push_str(
            r#"<filter id="bg-shadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="2" stdDeviation="3" flood-opacity="0.18"/></filter>"#,
        );
    }

    // `role="img"` plus a label is what makes an SVG readable to a screen
    // reader at all: without them it is announced as a group of anonymous
    // shapes, or skipped. The label is what the file already says — the
    // diagram's own description, or failing that its title — rather than
    // something to be written twice.
    let label = layout
        .opts
        .description
        .as_deref()
        .or(layout.opts.title.as_deref())
        .map(|text| format!(r#" role="img" aria-label="{}""#, esc(text)))
        .unwrap_or_default();

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}"{label}>"#,
        w = n(width),
        h = n(height)
    );
    if !defs.is_empty() {
        let _ = write!(svg, "<defs>{defs}</defs>");
    }
    if let Some(bg) = &layout.opts.background {
        let _ = write!(
            svg,
            r#"<rect x="0" y="0" width="{}" height="{}"{}/>"#,
            n(width),
            n(height),
            paint("fill", bg)
        );
    }
    let _ = write!(svg, r#"<g transform="translate({},{})">"#, n(dx), n(dy));
    svg.push_str(&body);
    svg.push_str("</g></svg>");
    svg
}

/// Route every edge and place every edge label.
///
/// Public because three writers need the same answer — SVG, PDF and the JSON
/// export — and a second implementation would be a second diagram.
pub fn route_all(layout: &Layout) -> Vec<Routed> {
    let mut routed: Vec<Routed> = layout
        .edges
        .iter()
        .map(|edge| Routed {
            points: if edge.from == edge.to {
                self_loop(&layout.nodes[edge.from])
            } else {
                // Every box but the two being joined is an obstacle. A group
                // frame is not: a box inside one can only be reached by
                // crossing it, so treating a frame as solid would refuse the
                // routes that are correct.
                let obstacles: Vec<crate::layout::Rect> = layout
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !crate::layout::endpoint_of(layout, edge, *i))
                    .map(|(_, n)| n.rect)
                    .collect();
                route(
                    &layout.nodes[edge.from],
                    &layout.nodes[edge.to],
                    edge.style.router.unwrap_or(Router::Straight),
                    &obstacles,
                )
            },
            label: None,
        })
        .collect();

    // Placed greedily in source order, each one avoiding what is already down.
    let mut taken: Vec<Rect> = Vec::new();
    for i in 0..routed.len() {
        let Some(text) = layout.edges[i].label.clone() else {
            continue;
        };
        let rect = place_label(&text, i, &routed, layout, &taken, &layout.edges[i].style);
        taken.push(rect);
        routed[i].label = Some((text, rect));
    }

    routed
}

/// A label sits on the midpoint of its own edge by default, and slides along
/// the perpendicular when that would put it on top of something else.
fn place_label(
    text: &str,
    index: usize,
    routed: &[Routed],
    layout: &Layout,
    taken: &[Rect],
    style: &Style,
) -> Rect {
    let (anchor, (dx, dy)) = label_anchor(&routed[index].points);
    let (w, h) = label_size(text, style);

    // A declared offset is a decision. Honour it exactly — measured from the
    // default anchor, not from wherever the solver would have run to, so the
    // same source always puts the label in the same place.
    if let Some((ox, oy)) = style.label_offset {
        return Rect {
            x: anchor.0 + ox - w / 2.0,
            y: anchor.1 + oy - h / 2.0,
            w,
            h,
        };
    }

    // Perpendicular to the segment the label sits on.
    let len = (dx * dx + dy * dy).sqrt().max(1e-6);
    let (px, py) = (-dy / len, dx / len);
    let step = h * 0.85;

    let mut best = (
        f64::MAX,
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
    );

    for k in [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0] {
        let cx = anchor.0 + px * step * k;
        let cy = anchor.1 + py * step * k;
        let rect = Rect {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
        };

        // Distance is a real cost, not a tie-break: a label three steps from
        // its own edge reads as belonging to something else, which is worse
        // than crossing one line.
        let cost = label_cost(&rect, index, routed, layout, taken) + k.abs() * 0.4;
        if cost < best.0 {
            best = (cost, rect);
        }
        if cost < 0.001 {
            break;
        }
    }

    best.1
}

/// How much a candidate box overlaps things the reader needs to see. The
/// edge's own line is excluded — a label sits on its own route by design.
fn label_cost(
    rect: &Rect,
    index: usize,
    routed: &[Routed],
    layout: &Layout,
    taken: &[Rect],
) -> f64 {
    let mut cost = 0.0;

    for (i, other) in routed.iter().enumerate() {
        if i == index {
            continue;
        }
        for pair in other.points.windows(2) {
            if segment_hits(pair[0], pair[1], rect) {
                cost += 1.0;
            }
        }
    }

    // Proportional, not flat: clipping a node by five pixels is nothing like
    // sitting on top of it, and charging both the same exiles the label.
    for node in &layout.nodes {
        cost += 2.0 * overlap_fraction(rect, &node.rect);
    }
    // A frame is a boundary the reader uses; a label astride one is confusing
    // about what it belongs to.
    for group in &layout.groups {
        for pair in [
            [
                (group.rect.x, group.rect.y),
                (group.rect.right(), group.rect.y),
            ],
            [
                (group.rect.right(), group.rect.y),
                (group.rect.right(), group.rect.bottom()),
            ],
            [
                (group.rect.right(), group.rect.bottom()),
                (group.rect.x, group.rect.bottom()),
            ],
            [
                (group.rect.x, group.rect.bottom()),
                (group.rect.x, group.rect.y),
            ],
        ] {
            if segment_hits(pair[0], pair[1], rect) {
                cost += 1.0;
            }
        }
    }
    for placed in taken {
        cost += 2.0 * overlap_fraction(rect, placed);
    }

    cost
}

fn label_size(text: &str, style: &Style) -> (f64, f64) {
    let font_size = style.font_size.unwrap_or(12.0);
    let lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    (
        text_width(&lines, font_size) + 10.0,
        lines.len() as f64 * font_size * 1.3 + 4.0,
    )
}

/// The point a label hangs off, plus the direction of the segment it is on.
fn label_anchor(points: &[(f64, f64)]) -> ((f64, f64), (f64, f64)) {
    if points.len() < 2 {
        return ((0.0, 0.0), (1.0, 0.0));
    }
    // For an elbow, the middle segment is the one that reads as "the middle".
    let i = if points.len() == 2 {
        0
    } else {
        points.len() / 2 - 1
    };
    let (a, b) = (points[i], points[i + 1]);
    (
        ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0),
        (b.0 - a.0, b.1 - a.1),
    )
}

/// How much of `a` lies inside `b`, from 0 to 1.
fn overlap_fraction(a: &Rect, b: &Rect) -> f64 {
    let w = (a.right().min(b.right()) - a.x.max(b.x)).max(0.0);
    let h = (a.bottom().min(b.bottom()) - a.y.max(b.y)).max(0.0);
    let area = a.w * a.h;
    if area <= 0.0 { 0.0 } else { (w * h) / area }
}

fn segment_hits(p: (f64, f64), q: (f64, f64), r: &Rect) -> bool {
    let inside =
        |(x, y): (f64, f64)| (r.x..=r.right()).contains(&x) && (r.y..=r.bottom()).contains(&y);
    if inside(p) || inside(q) {
        return true;
    }

    let corners = [
        (r.x, r.y),
        (r.right(), r.y),
        (r.right(), r.bottom()),
        (r.x, r.bottom()),
    ];
    (0..4).any(|i| segments_cross(p, q, corners[i], corners[(i + 1) % 4]))
}

fn segments_cross(p1: (f64, f64), p2: (f64, f64), p3: (f64, f64), p4: (f64, f64)) -> bool {
    let d = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    let (d1, d2) = (d(p3, p4, p1), d(p3, p4, p2));
    let (d3, d4) = (d(p1, p2, p3), d(p1, p2, p4));

    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// Where a node's label ends up once it has been moved — `None` while it sits
/// in its shape, which the bounds already cover.
fn moved_node_label(node: &PlacedNode) -> Option<Rect> {
    let (dx, dy) = node.style.label_offset?;
    if node.lines.iter().all(|l| l.is_empty()) {
        return None;
    }
    let font_size = node.style.font_size.unwrap_or(14.0);
    let (w, h) = (
        text_width(&node.lines, font_size),
        node.lines.len() as f64 * font_size * 1.3,
    );
    Some(Rect {
        x: node.rect.cx() + dx - w / 2.0,
        y: node.rect.cy() + dy - h / 2.0,
        w,
        h,
    })
}

/// The same for a group's title, which hangs off the top-left of its frame.
fn moved_group_label(group: &PlacedGroup) -> Option<Rect> {
    let (dx, dy) = group.style.label_offset?;
    if group.lines.iter().all(|l| l.is_empty()) {
        return None;
    }
    let size = group.style.font_size.unwrap_or(12.0);
    Some(Rect {
        x: group.rect.x + 10.0 + dx,
        y: group.rect.y + size / 2.0 + dy,
        w: text_width(&[group.lines.join(" ")], size),
        h: size,
    })
}

fn bounds(layout: &Layout, routed: &[Routed]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    let mut boxes: Vec<Rect> = layout.nodes.iter().map(|n| n.rect).collect();
    boxes.extend(layout.groups.iter().map(|g| g.rect));
    for p in &layout.polylines {
        for &(x, y) in &p.points {
            boxes.push(Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            });
        }
    }
    for g in &layout.guides {
        for (x, y) in [g.from, g.to] {
            boxes.push(Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            });
        }
    }
    boxes.extend(layout.annotations.iter().map(annotation_bounds));
    boxes.extend(routed.iter().filter_map(|r| r.label.as_ref().map(|l| l.1)));
    // A label the author moved can leave the shape it belongs to entirely.
    boxes.extend(layout.nodes.iter().filter_map(moved_node_label));
    boxes.extend(layout.groups.iter().filter_map(moved_group_label));

    // Edge geometry can leave the nodes' box entirely — a self-loop does.
    for route in routed {
        for &(x, y) in &route.points {
            boxes.push(Rect {
                x,
                y,
                w: 0.0,
                h: 0.0,
            });
        }
    }

    if let Chart::Pie(pie) = &layout.chart {
        boxes.push(Rect {
            x: pie.cx - pie.radius,
            y: pie.cy - pie.radius,
            w: pie.radius * 2.0,
            h: pie.radius * 2.0,
        });
    }

    if boxes.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }

    for r in &boxes {
        min_x = min_x.min(r.x);
        min_y = min_y.min(r.y);
        max_x = max_x.max(r.right());
        max_y = max_y.max(r.bottom());
    }
    (min_x, min_y, max_x, max_y)
}

fn guide_line(g: &Guide) -> String {
    let dash = g
        .dash
        .as_deref()
        .map(|d| format!(r#" stroke-dasharray="{}""#, esc(d)))
        .unwrap_or_default();
    format!(
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}"{} stroke-width="{}" stroke-linecap="round"{dash}/>"#,
        n(g.from.0),
        n(g.from.1),
        n(g.to.0),
        n(g.to.1),
        paint("stroke", &g.color),
        n(g.width)
    )
}

fn free_polyline(p: &Polyline) -> String {
    let points: Vec<String> = p
        .points
        .iter()
        .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
        .collect();

    let mut attrs = String::new();
    if let Some(head) = p.head
        && head != Head::None
    {
        let id = marker_id(head, p.stroke.as_deref().unwrap_or("#64748b"));
        let _ = write!(attrs, r#" marker-end="url(#{id})""#);
    }
    if let Some(dash) = &p.dash {
        let _ = write!(attrs, r#" stroke-dasharray="{}""#, esc(dash));
    }

    format!(
        r#"<{} points="{}"{}{} stroke-width="{}" stroke-linejoin="round" stroke-linecap="round"{attrs}/>"#,
        if p.closed { "polygon" } else { "polyline" },
        points.join(" "),
        paint("fill", p.fill.as_deref().unwrap_or("none")),
        paint("stroke", p.stroke.as_deref().unwrap_or("none")),
        n(p.width)
    )
}

/// A frame and its name, set into the top-left of the border.
fn group_frame(group: &PlacedGroup, background: &Option<String>) -> String {
    let s = &group.style;
    let r = &group.rect;

    let dash = s
        .dash_array()
        .map(|d| format!(r#" stroke-dasharray="{d}""#))
        .unwrap_or_default();

    let mut out = format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}"{}{} stroke-width="{}"{dash}/>"#,
        n(r.x),
        n(r.y),
        n(r.w),
        n(r.h),
        n(s.radius.unwrap_or(10.0)),
        paint("fill", s.fill.as_deref().unwrap_or("none")),
        paint("stroke", s.stroke.as_deref().unwrap_or("#94a3b8")),
        n(s.stroke_width.unwrap_or(1.0))
    );

    if group.lines.iter().any(|l| !l.is_empty()) {
        let size = s.font_size.unwrap_or(12.0);
        let (ox, oy) = s.label_offset.unwrap_or((0.0, 0.0));
        let text = group.lines.join(" ");

        // A plate of the background, as edge labels get. A frame's title can
        // sit over a lifeline or a gridline it has no way to avoid, and this
        // is what already makes every other label readable when it does.
        if let Some(fill) = background.as_deref().filter(|b| *b != "none") {
            let w = text_width(std::slice::from_ref(&text), size);
            let _ = write!(
                out,
                r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" />"#,
                n(r.x + 12.0 + ox - 3.0),
                n(r.y + size * 1.3 + oy - size * 0.72),
                n(w + 6.0),
                n(size * 1.4),
                esc(fill)
            );
        }

        let _ = write!(
            out,
            r#"<text x="{}" y="{}" text-anchor="start" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}"{}>{}</text>"#,
            n(r.x + 12.0 + ox),
            // In the band the layout reserves for it, clear of both the
            // frame's own border and the first thing inside.
            n(r.y + size * 1.3 + oy),
            esc(s.font_family.as_deref().unwrap_or("sans-serif")),
            n(size),
            esc(s.font_weight.as_deref().unwrap_or("600")),
            paint("fill", s.font_color.as_deref().unwrap_or("#64748b")),
            esc(&text)
        );
    }

    out
}

// ---------------------------------------------------------------- shapes

/// A shape with its description attached, when it has one.
///
/// `<title>` as a child of the shape rather than a wrapping group: it is valid
/// on any SVG element, it is what a browser shows as a tooltip, and it is what
/// a screen reader reads — so one line serves the sighted reader with a mouse
/// and the reader without sight, which is the whole reason `I-040` was widened
/// past accessibility.
fn described(shape: &str, description: Option<&str>) -> String {
    let Some(text) = description.filter(|t| !t.is_empty()) else {
        return shape.to_string();
    };
    // The shapes are written self-closing, so the tag is reopened to hold the
    // title. Done here rather than in each of the eight shape branches, which
    // would be eight places to forget.
    let Some(stripped) = shape.strip_suffix("/>") else {
        return shape.to_string();
    };
    let tag = stripped
        .split_once(|c: char| c.is_whitespace())
        .map(|(head, _)| head.trim_start_matches('<'))
        .unwrap_or_default();
    if tag.is_empty() {
        return shape.to_string();
    }
    format!("{stripped}><title>{}</title></{tag}>", esc(text))
}

/// A deterministic offset in `[-amount, amount]`, from a shape's own numbers.
///
/// **Not random**, and it cannot be: the golden test asserts the output is
/// byte-identical run to run, and more to the point a diagram that redrew
/// itself differently every save would be unreviewable. The wobble is an
/// FNV-1a hash of the geometry, so the same box always deforms the same way —
/// and two boxes in different places deform differently, which is what makes
/// it look drawn rather than stamped.
fn wobble(seed: &[f64], salt: u64, amount: f64) -> f64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325 ^ salt;
    for value in seed {
        // Through the bit pattern of the rounded value: two runs must hash
        // the same, and a float printed and reparsed does not always.
        for byte in ((value * 64.0).round() as i64).to_le_bytes() {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(0x100_0000_01b3);
        }
    }
    let unit = (hash >> 11) as f64 / (1u64 << 53) as f64;
    (unit * 2.0 - 1.0) * amount
}

/// A node in sketch mode. Only the rectangular shapes are redrawn: a circle
/// or a diamond drawn freehand stops being recognisable as one, and a shape
/// whose meaning is its outline is not one to wobble.
fn sketch_shape(node: &PlacedNode) -> String {
    let s = &node.style;
    match s.shape.unwrap_or(Shape::Rect) {
        Shape::Rect | Shape::Stadium => {
            let fill = paint("fill", s.fill.as_deref().unwrap_or("#ffffff"));
            let stroke = paint("stroke", s.stroke.as_deref().unwrap_or("#1f2937"));
            let sw = n(s.stroke_width.unwrap_or(1.5));
            let dash = s
                .dash_array()
                .map(|d| format!(r#" stroke-dasharray="{d}""#))
                .unwrap_or_default();
            sketched_box(
                &node.rect,
                &format!(r#"{fill}{stroke} stroke-width="{sw}"{dash}"#),
            )
        }
        _ => node_shape(node),
    }
}

/// A rectangle drawn as if by hand: each corner nudged, each side bowed.
fn sketched_box(r: &Rect, common: &str) -> String {
    let a = 2.4;
    let p = |x: f64, y: f64, salt: u64| {
        (
            x + wobble(&[r.x, r.y, r.w, r.h, x], salt, a),
            y + wobble(&[r.x, r.y, r.w, r.h, y], salt ^ 0x9e37, a),
        )
    };
    let (x0, y0) = p(r.x, r.y, 1);
    let (x1, y1) = p(r.right(), r.y, 2);
    let (x2, y2) = p(r.right(), r.bottom(), 3);
    let (x3, y3) = p(r.x, r.bottom(), 4);

    // A quadratic through the middle of each side, pulled off the straight
    // line — a hand does not draw a chord, it draws a slight bow.
    let mid = |ax: f64, ay: f64, bx: f64, by: f64, salt: u64| {
        (
            (ax + bx) / 2.0 + wobble(&[ax, ay, bx, by], salt, a * 1.6),
            (ay + by) / 2.0 + wobble(&[bx, by, ax, ay], salt, a * 1.6),
        )
    };
    let (m0x, m0y) = mid(x0, y0, x1, y1, 11);
    let (m1x, m1y) = mid(x1, y1, x2, y2, 12);
    let (m2x, m2y) = mid(x2, y2, x3, y3, 13);
    let (m3x, m3y) = mid(x3, y3, x0, y0, 14);

    format!(
        r#"<path d="M {} {} Q {} {} {} {} Q {} {} {} {} Q {} {} {} {} Q {} {} {} {} Z"{common} stroke-linejoin="round"/>"#,
        n(x0),
        n(y0),
        n(m0x),
        n(m0y),
        n(x1),
        n(y1),
        n(m1x),
        n(m1y),
        n(x2),
        n(y2),
        n(m2x),
        n(m2y),
        n(x3),
        n(y3),
        n(m3x),
        n(m3y),
        n(x0),
        n(y0),
    )
}

fn node_shape(node: &PlacedNode) -> String {
    let r = &node.rect;
    let s = &node.style;
    let fill = paint("fill", s.fill.as_deref().unwrap_or("#ffffff"));
    let stroke = paint("stroke", s.stroke.as_deref().unwrap_or("#1f2937"));
    let sw = n(s.stroke_width.unwrap_or(1.5));
    let dash = s
        .dash_array()
        .map(|d| format!(r#" stroke-dasharray="{d}""#))
        .unwrap_or_default();
    let shadow = if s.shadow == Some(true) {
        r#" filter="url(#bg-shadow)""#
    } else {
        ""
    };
    let common = format!(r#"{fill}{stroke} stroke-width="{sw}"{dash}{shadow}"#);

    match s.shape.unwrap_or(Shape::Rect) {
        Shape::Rect => format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}"{common}/>"#,
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            n(s.radius.unwrap_or(8.0))
        ),
        Shape::Stadium => format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}"{common}/>"#,
            n(r.x),
            n(r.y),
            n(r.w),
            n(r.h),
            n(r.h / 2.0)
        ),
        Shape::Ellipse => format!(
            r#"<ellipse cx="{}" cy="{}" rx="{}" ry="{}"{common}/>"#,
            n(r.cx()),
            n(r.cy()),
            n(r.w / 2.0),
            n(r.h / 2.0)
        ),
        Shape::Diamond | Shape::Hexagon => format!(
            r#"<polygon points="{}"{common}/>"#,
            outline(r, s.shape.unwrap_or(Shape::Rect))
                .iter()
                .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
                .collect::<Vec<_>>()
                .join(" ")
        ),
        Shape::Cylinder => {
            let ry = (r.h * 0.16).min(14.0);
            let rx = r.w / 2.0;
            let body = format!(
                "M {} {} L {} {} A {} {} 0 0 0 {} {} L {} {} Z",
                n(r.x),
                n(r.y + ry),
                n(r.x),
                n(r.bottom() - ry),
                n(rx),
                n(ry),
                n(r.right()),
                n(r.bottom() - ry),
                n(r.right()),
                n(r.y + ry)
            );
            format!(
                r#"<path d="{body}"{common}/><ellipse cx="{}" cy="{}" rx="{}" ry="{}"{common}/>"#,
                n(r.cx()),
                n(r.y + ry),
                n(rx),
                n(ry)
            )
        }
    }
}

/// Polygon approximation of a shape, used both for drawing and for clipping
/// edge endpoints against the outline.
fn outline(r: &Rect, shape: Shape) -> Vec<(f64, f64)> {
    match shape {
        Shape::Diamond => vec![
            (r.cx(), r.y),
            (r.right(), r.cy()),
            (r.cx(), r.bottom()),
            (r.x, r.cy()),
        ],
        Shape::Hexagon => {
            let inset = (r.w * 0.18).min(r.h / 2.0);
            vec![
                (r.x + inset, r.y),
                (r.right() - inset, r.y),
                (r.right(), r.cy()),
                (r.right() - inset, r.bottom()),
                (r.x + inset, r.bottom()),
                (r.x, r.cy()),
            ]
        }
        _ => vec![
            (r.x, r.y),
            (r.right(), r.y),
            (r.right(), r.bottom()),
            (r.x, r.bottom()),
        ],
    }
}

// ---------------------------------------------------------------- edges

fn route(
    from: &PlacedNode,
    to: &PlacedNode,
    router: Router,
    obstacles: &[crate::layout::Rect],
) -> Vec<(f64, f64)> {
    let a = &from.rect;
    let b = &to.rect;

    match router {
        Router::Straight => {
            let start = boundary(from, (b.cx(), b.cy()));
            let end = boundary(to, (a.cx(), a.cy()));
            vec![start, end]
        }
        // `X-004`. The solver goes around the boxes; the elbow below is what is
        // left when there is no way round — a box wholly enclosed, or a diagram
        // too large for the grid to pay for itself. Keeping the old line rather
        // than dropping the edge is deliberate: `check::after_layout` then says
        // the line crosses something, which is a drawing that admits it is
        // wrong, where a missing edge is one that lies.
        Router::Ortho if let Some(path) = crate::ortho::route(*a, *b, obstacles) => path,
        Router::Ortho => {
            let dx = b.cx() - a.cx();
            let dy = b.cy() - a.cy();

            if dx.abs() >= dy.abs() {
                let sx = if dx >= 0.0 { a.right() } else { a.x };
                let ex = if dx >= 0.0 { b.x } else { b.right() };
                let mid = (sx + ex) / 2.0;
                vec![(sx, a.cy()), (mid, a.cy()), (mid, b.cy()), (ex, b.cy())]
            } else {
                let sy = if dy >= 0.0 { a.bottom() } else { a.y };
                let ey = if dy >= 0.0 { b.y } else { b.bottom() };
                let mid = (sy + ey) / 2.0;
                vec![(a.cx(), sy), (a.cx(), mid), (b.cx(), mid), (b.cx(), ey)]
            }
        }
    }
}

/// An edge from a node to itself: a loop over its top edge. Sampled as a
/// polyline so it inherits the dash, opacity and arrow-head machinery every
/// other edge already uses.
fn self_loop(node: &PlacedNode) -> Vec<(f64, f64)> {
    let r = &node.rect;
    let rx = (r.w * 0.22).max(18.0);
    let ry = (r.h * 0.70).max(26.0);
    let (cx, top) = (r.cx(), r.y);

    // t from π to 0: leaves the top edge on the left, arcs over, and comes
    // back down on the right — so the arrow head points into the node.
    const STEPS: usize = 24;
    (0..=STEPS)
        .map(|i| {
            let t = std::f64::consts::PI * (1.0 - i as f64 / STEPS as f64);
            (cx + rx * t.cos(), top - ry * t.sin())
        })
        .collect()
}

/// Where the ray from the node's centre toward `target` leaves the shape.
fn boundary(node: &PlacedNode, target: (f64, f64)) -> (f64, f64) {
    let r = &node.rect;
    let (cx, cy) = (r.cx(), r.cy());
    let (dx, dy) = (target.0 - cx, target.1 - cy);

    if dx.abs() < f64::EPSILON && dy.abs() < f64::EPSILON {
        return (cx, cy);
    }

    let shape = node.style.shape.unwrap_or(Shape::Rect);
    if shape == Shape::Ellipse {
        let (rx, ry) = (r.w / 2.0, r.h / 2.0);
        let t = 1.0 / ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt();
        return (cx + dx * t, cy + dy * t);
    }

    let poly = outline(r, shape);
    let mut best = f64::MAX;

    for i in 0..poly.len() {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % poly.len()];
        let (ex, ey) = (x2 - x1, y2 - y1);

        // centre + t*d  ==  p1 + u*e , keep the nearest hit with 0 <= u <= 1.
        let denom = dx * ey - dy * ex;
        if denom.abs() < 1e-9 {
            continue;
        }
        let t = ((x1 - cx) * ey - (y1 - cy) * ex) / denom;
        let u = ((x1 - cx) * dy - (y1 - cy) * dx) / denom;
        if t > 0.0 && (0.0..=1.0).contains(&u) && t < best {
            best = t;
        }
    }

    if best == f64::MAX {
        return (cx, cy);
    }
    (cx + dx * best, cy + dy * best)
}

// ---------------------------------------------------------------- text

fn node_label(node: &PlacedNode) -> String {
    if node.lines.iter().all(|l| l.is_empty()) {
        return String::new();
    }
    let s = &node.style;
    let font_size = s.font_size.unwrap_or(14.0);
    let line_h = font_size * 1.3;
    let (ox, oy) = s.label_offset.unwrap_or((0.0, 0.0));
    let cx = node.rect.cx() + ox;
    let start = node.rect.cy() + oy - (node.lines.len() as f64 - 1.0) * line_h / 2.0;

    let mut out = format!(
        r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}"{}>"#,
        n(cx),
        n(start),
        esc(s.font_family.as_deref().unwrap_or("sans-serif")),
        n(font_size),
        esc(s.font_weight.as_deref().unwrap_or("500")),
        paint("fill", s.font_color.as_deref().unwrap_or("#111827"))
    );

    for (i, line) in node.lines.iter().enumerate() {
        let _ = write!(
            out,
            r#"<tspan x="{}" dy="{}">{}</tspan>"#,
            n(cx),
            if i == 0 { "0".to_string() } else { n(line_h) },
            esc(line)
        );
    }

    out.push_str("</text>");
    out
}

fn edge_label(label: &str, rect: &Rect, style: &Style, background: &Option<String>) -> String {
    let font_size = style.font_size.unwrap_or(12.0);
    let lines: Vec<String> = label.split('\n').map(str::to_string).collect();
    let (x, y) = (rect.cx(), rect.cy());
    let plate = background.as_deref().unwrap_or("#ffffff");

    let mut out = format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3"{}/>"#,
        n(rect.x),
        n(rect.y),
        n(rect.w),
        n(rect.h),
        paint("fill", plate)
    );

    let line_h = font_size * 1.3;
    let start = y - (lines.len() as f64 - 1.0) * line_h / 2.0;
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}"{}>"#,
        n(x),
        n(start),
        esc(style.font_family.as_deref().unwrap_or("sans-serif")),
        n(font_size),
        esc(style.font_weight.as_deref().unwrap_or("400")),
        paint("fill", style.font_color.as_deref().unwrap_or("#475569"))
    );
    for (i, line) in lines.iter().enumerate() {
        let _ = write!(
            out,
            r#"<tspan x="{}" dy="{}">{}</tspan>"#,
            n(x),
            if i == 0 { "0".to_string() } else { n(line_h) },
            esc(line)
        );
    }
    out.push_str("</text>");
    out
}

// ---------------------------------------------------------------- markers

fn marker_id(head: Head, color: &str) -> String {
    let kind = match head {
        Head::Solid => "solid",
        Head::Open => "open",
        Head::Dot => "dot",
        Head::None => "none",
        Head::One => "one",
        Head::Many => "many",
        Head::ZeroOne => "zeroone",
        Head::ZeroMany => "zeromany",
        Head::OneMany => "onemany",
    };
    let slug: String = color
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("h-{kind}-{slug}")
}

fn marker_def(id: &str, head: Head, color: &str, background: &Option<String>) -> String {
    let fill = paint("fill", color);
    let stroke = paint("stroke", color);

    if head.is_cardinality() {
        return cardinality_def(id, head, color, background);
    }

    let inner = match head {
        Head::Solid => format!(r#"<path d="M0,0 L10,5 L0,10 z"{fill}/>"#),
        Head::Open => format!(
            r#"<path d="M0,0 L10,5 L0,10" fill="none"{stroke} stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Head::Dot => format!(r#"<circle cx="5" cy="5" r="3.5"{fill}/>"#),
        Head::None => String::new(),
        _ => unreachable!("cardinality is drawn by `cardinality_def`"),
    };
    let ref_x = if head == Head::Dot { 5 } else { 10 };

    format!(
        r#"<marker id="{id}" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" refX="{ref_x}" refY="5" orient="auto-start-reverse">{inner}</marker>"#
    )
}

/// Crow's foot, drawn in a wider box so two symbols fit.
///
/// The marker box runs from x=0 (out along the line) to x=18 (against the
/// entity), which is where `refX` pins it. Notation reads inward: the symbol
/// nearest the entity is the cardinality, the one behind it the optionality.
fn cardinality_def(id: &str, head: Head, color: &str, background: &Option<String>) -> String {
    let stroke = paint("stroke", color);
    let line = |d: String| {
        format!(r#"<path d="{d}" fill="none"{stroke} stroke-width="1.6" stroke-linecap="round"/>"#)
    };
    // Three prongs meeting at a vertex back down the line.
    let foot = |from: f64| line(format!("M{from},9 L18,1 M{from},9 L18,9 M{from},9 L18,17"));
    let bar = |at: f64| line(format!("M{at},1 L{at},17"));
    // Filled with the surface, not left transparent: the relation line runs
    // under the marker, and a see-through ring reads as a crossed-out circle
    // rather than as "zero".
    let surface = paint("fill", background.as_deref().unwrap_or("#ffffff"));
    let ring = |cx: f64| {
        format!(r#"<circle cx="{cx}" cy="9" r="3.6"{surface}{stroke} stroke-width="1.6"/>"#)
    };

    let inner = match head {
        Head::One => bar(12.0),
        Head::Many => foot(8.0),
        Head::ZeroOne => format!("{}{}", ring(5.0), bar(13.0)),
        Head::ZeroMany => format!("{}{}", ring(4.0), foot(9.0)),
        Head::OneMany => format!("{}{}", bar(5.0), foot(9.0)),
        _ => unreachable!("not a cardinality"),
    };

    format!(
        r#"<marker id="{id}" markerUnits="userSpaceOnUse" markerWidth="19" markerHeight="18" refX="18" refY="9" orient="auto-start-reverse">{inner}</marker>"#
    )
}

// ---------------------------------------------------------------- helpers

/// Trims float noise so the SVG stays readable and diffable.
fn n(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() {
        format!("{}", r as i64)
    } else {
        format!("{r}")
    }
}

/// A color and its SVG attribute pair.
///
/// `#rrggbbaa` is CSS Color 4 and modern renderers honour it, but several
/// SVG-to-PDF paths and older viewers silently drop the alpha. Emitting the
/// opacity as its own attribute renders identically everywhere.
fn paint(name: &str, color: &str) -> String {
    let (rgb, alpha) = split_alpha(color);
    match alpha {
        Some(a) => format!(r#" {name}="{}" {name}-opacity="{}""#, esc(&rgb), n(a)),
        None => format!(r#" {name}="{}""#, esc(&rgb)),
    }
}

/// Splits `#rgba` / `#rrggbbaa` into the opaque color and its alpha. A named
/// color, or one with no alpha channel, is fully opaque.
pub fn split_alpha(color: &str) -> (String, Option<f64>) {
    let Some(hex) = color.strip_prefix('#') else {
        return (color.to_string(), None);
    };
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return (color.to_string(), None);
    }

    let expand = |s: &str| -> String { s.chars().flat_map(|c| [c, c]).collect() };

    let (rgb, alpha) = match hex.len() {
        4 => (expand(&hex[0..3]), Some(expand(&hex[3..4]))),
        8 => (hex[0..6].to_string(), Some(hex[6..8].to_string())),
        3 => (expand(hex), None),
        _ => (hex.to_string(), None),
    };

    let alpha = alpha
        .and_then(|a| u8::from_str_radix(&a, 16).ok())
        .map(|a| f64::from(a) / 255.0)
        // Fully opaque is the default; saying so would be noise.
        .filter(|a| *a < 1.0);

    (format!("#{rgb}"), alpha)
}

/// Base64, written out rather than pulled in.
///
/// Twenty lines against a dependency, in a crate whose whole shape is that
/// the SVG writer, the PDF writer and the HTTP preview are all hand-rolled.
/// There is no decoder here because nothing in this crate reads one back.
#[cfg(test)]
pub(crate) fn base64_for_test(bytes: &[u8]) -> String {
    base64(bytes)
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(ALPHABET[(n >> 18 & 63) as usize] as char);
        out.push(ALPHABET[(n >> 12 & 63) as usize] as char);
        // The tail is padded rather than truncated: a decoder is entitled to
        // a multiple of four, and a browser that gets three characters simply
        // drops the font with no message.
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------- charts

/// One path per slice, stroked in the surface color so adjacent fills keep a
/// 2px gap instead of bleeding into each other.
fn pie_slices(pie: &Pie) -> String {
    let mut out = String::new();

    for slice in &pie.slices {
        let sweep = slice.end - slice.start;
        if sweep <= 0.0 {
            continue;
        }

        // A slice covering the whole circle has no arc endpoints to draw
        // between; two half-circles do the job.
        let d = if sweep >= 359.99 {
            full_ring(pie)
        } else {
            wedge(pie, slice.start, slice.end)
        };

        let _ = write!(
            out,
            r#"<path d="{d}"{}{} stroke-width="2" stroke-linejoin="round"/>"#,
            paint("fill", &slice.fill),
            paint("stroke", &pie.gap_color)
        );
    }

    out
}

/// `0` is twelve o'clock and angles grow clockwise, which is how people read
/// a pie — not how SVG measures them.
fn on_circle(cx: f64, cy: f64, r: f64, degrees: f64) -> (f64, f64) {
    let a = degrees.to_radians();
    (cx + r * a.sin(), cy - r * a.cos())
}

fn wedge(pie: &Pie, start: f64, end: f64) -> String {
    let large = usize::from(end - start > 180.0);
    let (ox1, oy1) = on_circle(pie.cx, pie.cy, pie.radius, start);
    let (ox2, oy2) = on_circle(pie.cx, pie.cy, pie.radius, end);

    if pie.inner <= 0.0 {
        return format!(
            "M {} {} L {} {} A {} {} 0 {large} 1 {} {} Z",
            n(pie.cx),
            n(pie.cy),
            n(ox1),
            n(oy1),
            n(pie.radius),
            n(pie.radius),
            n(ox2),
            n(oy2)
        );
    }

    let (ix2, iy2) = on_circle(pie.cx, pie.cy, pie.inner, end);
    let (ix1, iy1) = on_circle(pie.cx, pie.cy, pie.inner, start);

    format!(
        "M {} {} A {} {} 0 {large} 1 {} {} L {} {} A {} {} 0 {large} 0 {} {} Z",
        n(ox1),
        n(oy1),
        n(pie.radius),
        n(pie.radius),
        n(ox2),
        n(oy2),
        n(ix2),
        n(iy2),
        n(pie.inner),
        n(pie.inner),
        n(ix1),
        n(iy1)
    )
}

fn full_ring(pie: &Pie) -> String {
    let ring = |r: f64, sweep: usize| {
        format!(
            "M {} {} A {r2} {r2} 0 1 {sweep} {} {} A {r2} {r2} 0 1 {sweep} {} {} Z",
            n(pie.cx - r),
            n(pie.cy),
            n(pie.cx + r),
            n(pie.cy),
            n(pie.cx - r),
            n(pie.cy),
            r2 = n(r)
        )
    };

    match pie.inner > 0.0 {
        // Opposite sweeps punch the hole with the even-odd default.
        true => format!("{} {}", ring(pie.radius, 1), ring(pie.inner, 0)),
        false => ring(pie.radius, 1),
    }
}

fn annotation_text(a: &Annotation) -> String {
    format!(
        r#"<text x="{}" y="{}" text-anchor="{}" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}"{}>{}</text>"#,
        n(a.x),
        n(a.y),
        match a.anchor {
            Anchor::Start => "start",
            Anchor::Middle => "middle",
            Anchor::End => "end",
        },
        esc(&a.family),
        n(a.size),
        esc(&a.weight),
        paint("fill", &a.color),
        esc(&a.text)
    )
}
