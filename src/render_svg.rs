//! SVG emission: shapes, edge routing and text. The viewBox is derived from
//! the solved layout, so a diagram is never clipped by a hardcoded canvas.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::layout::{Layout, PlacedNode, Rect, text_width};
use crate::style::{Head, Router, Shape, Style};

pub fn render(layout: &Layout) -> String {
    let pad = layout.opts.padding;
    let (min_x, min_y, max_x, max_y) = bounds(layout);
    let width = (max_x - min_x) + pad * 2.0;
    let height = (max_y - min_y) + pad * 2.0;
    let dx = pad - min_x;
    let dy = pad - min_y;

    let mut body = String::new();
    let mut markers = BTreeMap::new();
    let mut needs_shadow = false;

    for edge in &layout.edges {
        let from = &layout.nodes[edge.from];
        let to = &layout.nodes[edge.to];
        let points = route(from, to, edge.style.router.unwrap_or(Router::Straight));
        if points.len() < 2 {
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

        let path: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
            .collect();
        let _ = write!(
            body,
            r#"<polyline points="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linejoin="round" stroke-linecap="round" opacity="{}"{}/>"#,
            path.join(" "),
            esc(stroke),
            n(edge.style.stroke_width.unwrap_or(1.5)),
            n(edge.style.opacity.unwrap_or(1.0)),
            attrs
        );

        if let Some(label) = &edge.label {
            let (lx, ly) = midpoint(&points);
            body.push_str(&edge_label(
                label,
                lx,
                ly,
                &edge.style,
                &layout.opts.background,
            ));
        }
    }

    for node in &layout.nodes {
        if node.style.shadow == Some(true) {
            needs_shadow = true;
        }
        body.push_str(&node_shape(node));
        body.push_str(&node_label(node));
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
            r#"<rect x="0" y="0" width="{}" height="{}" fill="{}"/>"#,
            n(width),
            n(height),
            esc(bg)
        );
    }
    let _ = write!(svg, r#"<g transform="translate({},{})">"#, n(dx), n(dy));
    svg.push_str(&body);
    svg.push_str("</g></svg>");
    svg
}

fn bounds(layout: &Layout) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    for node in &layout.nodes {
        min_x = min_x.min(node.rect.x);
        min_y = min_y.min(node.rect.y);
        max_x = max_x.max(node.rect.right());
        max_y = max_y.max(node.rect.bottom());
    }

    if layout.nodes.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    (min_x, min_y, max_x, max_y)
}

// ---------------------------------------------------------------- shapes

fn node_shape(node: &PlacedNode) -> String {
    let r = &node.rect;
    let s = &node.style;
    let fill = esc(s.fill.as_deref().unwrap_or("#ffffff"));
    let stroke = esc(s.stroke.as_deref().unwrap_or("#1f2937"));
    let sw = n(s.stroke_width.unwrap_or(1.5));
    let opacity = n(s.opacity.unwrap_or(1.0));
    let dash = s
        .dash_array()
        .map(|d| format!(r#" stroke-dasharray="{d}""#))
        .unwrap_or_default();
    let shadow = if s.shadow == Some(true) {
        r#" filter="url(#bg-shadow)""#
    } else {
        ""
    };
    let common = format!(
        r#" fill="{fill}" stroke="{stroke}" stroke-width="{sw}" opacity="{opacity}"{dash}{shadow}"#
    );

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

fn midpoint(points: &[(f64, f64)]) -> (f64, f64) {
    if points.len() == 2 {
        return (
            (points[0].0 + points[1].0) / 2.0,
            (points[0].1 + points[1].1) / 2.0,
        );
    }
    // For an elbow the middle segment is the one that reads as "the middle".
    let i = points.len() / 2 - 1;
    (
        (points[i].0 + points[i + 1].0) / 2.0,
        (points[i].1 + points[i + 1].1) / 2.0,
    )
}

// ---------------------------------------------------------------- text

fn node_label(node: &PlacedNode) -> String {
    let s = &node.style;
    let font_size = s.font_size.unwrap_or(14.0);
    let line_h = font_size * 1.3;
    let start = node.rect.cy() - (node.lines.len() as f64 - 1.0) * line_h / 2.0;

    let mut out = format!(
        r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}" fill="{}">"#,
        n(node.rect.cx()),
        n(start),
        esc(s.font_family.as_deref().unwrap_or("sans-serif")),
        n(font_size),
        esc(s.font_weight.as_deref().unwrap_or("500")),
        esc(s.font_color.as_deref().unwrap_or("#111827"))
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

fn edge_label(label: &str, x: f64, y: f64, style: &Style, background: &Option<String>) -> String {
    let font_size = style.font_size.unwrap_or(12.0);
    let lines: Vec<String> = label.split('\n').map(str::to_string).collect();
    let w = text_width(&lines, font_size) + 10.0;
    let h = lines.len() as f64 * font_size * 1.3 + 4.0;
    let plate = background.as_deref().unwrap_or("#ffffff");

    let mut out = format!(
        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="3" fill="{}"/>"#,
        n(x - w / 2.0),
        n(y - h / 2.0),
        n(w),
        n(h),
        esc(plate)
    );

    let line_h = font_size * 1.3;
    let start = y - (lines.len() as f64 - 1.0) * line_h / 2.0;
    let _ = write!(
        out,
        r#"<text x="{}" y="{}" text-anchor="middle" dominant-baseline="central" font-family="{}" font-size="{}" font-weight="{}" fill="{}">"#,
        n(x),
        n(start),
        esc(style.font_family.as_deref().unwrap_or("sans-serif")),
        n(font_size),
        esc(style.font_weight.as_deref().unwrap_or("400")),
        esc(style.font_color.as_deref().unwrap_or("#475569"))
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
    let color = esc(color);
    let inner = match head {
        Head::Solid => format!(r#"<path d="M0,0 L10,5 L0,10 z" fill="{color}"/>"#),
        Head::Open => format!(
            r#"<path d="M0,0 L10,5 L0,10" fill="none" stroke="{color}" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Head::Dot => format!(r#"<circle cx="5" cy="5" r="3.5" fill="{color}"/>"#),
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
