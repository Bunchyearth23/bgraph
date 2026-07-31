//! SVG emission: shapes, edge routing and text. The viewBox is derived from
//! the solved layout, so a diagram is never clipped by a hardcoded canvas.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::charts::annotation_bounds;
use crate::layout::{Anchor, Annotation, Chart, Layout, Pie, PlacedNode, Rect, text_width};
use crate::style::{Head, Router, Shape, Style};

/// An edge after routing, with its label already placed.
struct Routed {
    points: Vec<(f64, f64)>,
    label: Option<(String, Rect)>,
}

pub fn render(layout: &Layout) -> String {
    // Routing comes first: a self-loop reaches above its node, and a label
    // may be nudged clear of a neighbouring line, so neither the canvas nor
    // the label placement can be decided before the geometry exists.
    let routed = route_all(layout);

    let pad = layout.opts.padding;
    let (min_x, min_y, max_x, max_y) = bounds(layout, &routed);
    let width = (max_x - min_x) + pad * 2.0;
    let height = (max_y - min_y) + pad * 2.0;
    let dx = pad - min_x;
    let dy = pad - min_y;

    let mut body = String::new();
    let mut markers = BTreeMap::new();
    let mut needs_shadow = false;

    for (edge, route) in layout.edges.iter().zip(&routed) {
        if route.points.len() < 2 {
            continue;
        }

        let head = edge.style.head.unwrap_or(Head::Solid);
        let stroke = edge.style.stroke.as_deref().unwrap_or("#64748b");
        let mut attrs = String::new();

        if edge.arrow.head_end && head != Head::None {
            let id = marker_id(head, stroke);
            markers
                .entry(id.clone())
                .or_insert((head, stroke.to_string()));
            let _ = write!(attrs, r#" marker-end="url(#{id})""#);
        }
        if edge.arrow.head_start && head != Head::None {
            let id = marker_id(head, stroke);
            markers
                .entry(id.clone())
                .or_insert((head, stroke.to_string()));
            let _ = write!(attrs, r#" marker-start="url(#{id})""#);
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
        body.push_str(&node_shape(node));
        body.push_str(&node_label(node));
    }

    for annotation in &layout.annotations {
        body.push_str(&annotation_text(annotation));
    }

    let mut defs = String::new();
    for (id, (head, color)) in &markers {
        defs.push_str(&marker_def(id, *head, color));
    }
    if needs_shadow {
        defs.push_str(
            r#"<filter id="bg-shadow" x="-30%" y="-30%" width="160%" height="160%"><feDropShadow dx="0" dy="2" stdDeviation="3" flood-opacity="0.18"/></filter>"#,
        );
    }

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="0 0 {w} {h}">"#,
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

fn route_all(layout: &Layout) -> Vec<Routed> {
    let mut routed: Vec<Routed> = layout
        .edges
        .iter()
        .map(|edge| Routed {
            points: if edge.from == edge.to {
                self_loop(&layout.nodes[edge.from])
            } else {
                route(
                    &layout.nodes[edge.from],
                    &layout.nodes[edge.to],
                    edge.style.router.unwrap_or(Router::Straight),
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

        let cost = label_cost(&rect, index, routed, layout, taken) + k.abs() * 0.01;
        if cost < best.0 {
            best = (cost, rect);
        }
        if cost < 0.011 {
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

    for node in &layout.nodes {
        if overlaps(rect, &node.rect) {
            cost += 2.0;
        }
    }
    for placed in taken {
        if overlaps(rect, placed) {
            cost += 2.0;
        }
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

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
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

fn bounds(layout: &Layout, routed: &[Routed]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    let mut boxes: Vec<Rect> = layout.nodes.iter().map(|n| n.rect).collect();
    boxes.extend(layout.annotations.iter().map(annotation_bounds));
    boxes.extend(routed.iter().filter_map(|r| r.label.as_ref().map(|l| l.1)));

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

// ---------------------------------------------------------------- shapes

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

fn route(from: &PlacedNode, to: &PlacedNode, router: Router) -> Vec<(f64, f64)> {
    let a = &from.rect;
    let b = &to.rect;

    match router {
        Router::Straight => {
            let start = boundary(from, (b.cx(), b.cy()));
            let end = boundary(to, (a.cx(), a.cy()));
            vec![start, end]
        }
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
    let start = node.rect.cy() - (node.lines.len() as f64 - 1.0) * line_h / 2.0;

    let mut out = format!(
        r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}"{}>"#,
        n(node.rect.cx()),
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
            n(node.rect.cx()),
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
    };
    let slug: String = color
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("h-{kind}-{slug}")
}

fn marker_def(id: &str, head: Head, color: &str) -> String {
    let fill = paint("fill", color);
    let stroke = paint("stroke", color);
    let inner = match head {
        Head::Solid => format!(r#"<path d="M0,0 L10,5 L0,10 z"{fill}/>"#),
        Head::Open => format!(
            r#"<path d="M0,0 L10,5 L0,10" fill="none"{stroke} stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Head::Dot => format!(r#"<circle cx="5" cy="5" r="3.5"{fill}/>"#),
        Head::None => String::new(),
    };
    let ref_x = if head == Head::Dot { 5 } else { 10 };

    format!(
        r#"<marker id="{id}" markerUnits="userSpaceOnUse" markerWidth="11" markerHeight="11" refX="{ref_x}" refY="5" orient="auto-start-reverse">{inner}</marker>"#
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
