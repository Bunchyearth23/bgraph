//! SVG emission: shapes, edge routing and text. The viewBox is derived from
//! the solved layout, so a diagram is never clipped by a hardcoded canvas.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::charts::annotation_bounds;
use crate::layout::{
    Anchor, Annotation, Chart, Guide, Layout, Pie, PlacedGroup, PlacedNode, Polyline, Rect,
    text_width,
};
use crate::style::{GraphOpts, Head, Router, Shape, Style};

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
    for group in &layout.groups {
        body.push_str(&described(
            &group_frame(group, &layout.opts),
            group.style.description.as_deref(),
        ));
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

    for (edge, route) in layout.edges.iter().zip(&routed) {
        if route.points.len() < 2 {
            continue;
        }

        let head = edge.style.head.unwrap_or(Head::Solid);
        let stroke = edge.style.stroke.as_deref().unwrap_or("#64748b");
        let mut marker_attrs = String::new();
        let mut stroke_attrs = String::new();

        // Each end is decided on its own: a declared cardinality replaces the
        // arrowhead there, because a crow's foot already says which way the
        // relation reads and two marks at one end would fight.
        let end_head = edge
            .style
            .to_head
            .or_else(|| edge.arrow.head_end.then_some(head));
        let start_head = edge
            .style
            .from_head
            .or_else(|| edge.arrow.head_start.then_some(head));

        for (which, head) in [("end", end_head), ("start", start_head)] {
            let Some(head) = head.filter(|h| *h != Head::None) else {
                continue;
            };
            let id = marker_id(head, stroke);
            markers
                .entry(id.clone())
                .or_insert((head, stroke.to_string()));
            let _ = write!(marker_attrs, r#" marker-{which}="url(#{id})""#);
        }
        let linecap = if end_head.is_some() || start_head.is_some() {
            "butt"
        } else {
            "round"
        };
        if let Some(dash) = edge.style.dash_array() {
            let _ = write!(stroke_attrs, r#" stroke-dasharray="{dash}""#);
        }

        let mut points = route.points.clone();
        stabilise_marker_tangent(&mut points);
        let marker_path: Vec<String> = points
            .iter()
            .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
            .collect();
        let mut line_points = points.clone();
        trim_marker_stems(
            &mut line_points,
            marker_stem(start_head),
            marker_stem(end_head),
        );
        let path: Vec<String> = line_points
            .iter()
            .map(|(x, y)| format!("{},{}", n(*x), n(*y)))
            .collect();
        if !marker_attrs.is_empty() {
            let _ = write!(
                body,
                r#"<polyline points="{}" fill="none" stroke="none"{marker_attrs}/>"#,
                marker_path.join(" "),
            );
        }
        let _ = write!(
            body,
            r#"<polyline points="{}" fill="none"{} stroke-width="{}" stroke-linejoin="round" stroke-linecap="{}"{}/>"#,
            path.join(" "),
            paint("stroke", stroke),
            n(edge.style.stroke_width.unwrap_or(1.5)),
            linecap,
            stroke_attrs
        );
    }

    // A crossing is not an invisible topological fact: without a bridge the
    // reader cannot tell which link continues. Draw the marker after all edge
    // strokes so it remains visible regardless of declaration order.
    body.push_str(&crossing_marks(layout, &routed));

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
    // Embedding supplies a portable CSS font face; layout advances remain estimates.
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

fn crossing_marks(layout: &Layout, routed: &[Routed]) -> String {
    let mut out = String::new();
    let background = layout.opts.background.as_deref().unwrap_or("#ffffff");
    let mut seen = Vec::new();
    for (i, first) in routed.iter().enumerate() {
        for (j, second) in routed.iter().enumerate().skip(i + 1) {
            for &(a, b) in &segments(&first.points) {
                for &(c, d) in &segments(&second.points) {
                    let Some((x, y, dx, dy)) = crossing(a, b, c, d) else {
                        continue;
                    };
                    if seen
                        .iter()
                        .any(|(px, py): &(f64, f64)| (px - x).abs() < 1e-6 && (py - y).abs() < 1e-6)
                    {
                        continue;
                    }
                    seen.push((x, y));
                    let stroke = layout.edges[j].style.stroke.as_deref().unwrap_or("#64748b");
                    if background != "none" && background != "transparent" {
                        let _ = write!(
                            out,
                            r#"<circle cx="{}" cy="{}" r="6"{} />"#,
                            n(x),
                            n(y),
                            paint("fill", background)
                        );
                    }
                    let length = (dx * dx + dy * dy).sqrt();
                    let (ux, uy) = (dx / length, dy / length);
                    let (px, py) = (-uy, ux);
                    let start = (x - ux * 6.0, y - uy * 6.0);
                    let end = (x + ux * 6.0, y + uy * 6.0);
                    let control = (x + px * 6.0, y + py * 6.0);
                    let d = format!("M{} {}", n(start.0), n(start.1));
                    let q = format!(
                        "{} {} {} {}",
                        n(control.0),
                        n(control.1),
                        n(end.0),
                        n(end.1)
                    );
                    let _ = write!(
                        out,
                        r#"<path d="{} Q {}" fill="none"{} stroke-width="{}" stroke-linecap="round"/>"#,
                        d,
                        q,
                        paint("stroke", stroke),
                        n(layout.edges[j].style.stroke_width.unwrap_or(1.5)),
                    );
                }
            }
        }
    }
    out
}

fn segments(points: &[(f64, f64)]) -> Vec<((f64, f64), (f64, f64))> {
    points.windows(2).map(|pair| (pair[0], pair[1])).collect()
}

/// Return an interior crossing and the direction of the later segment.
/// Endpoints and collinear overlap are intentionally excluded. This covers
/// straight, curved and orthogonal routes alike; a crossing marker is a visual
/// property of two links, not of one router.
fn crossing(
    a: (f64, f64),
    b: (f64, f64),
    c: (f64, f64),
    d: (f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    let (rx, ry) = (b.0 - a.0, b.1 - a.1);
    let (sx, sy) = (d.0 - c.0, d.1 - c.1);
    let denominator = rx * sy - ry * sx;
    if denominator.abs() < 1e-6 {
        return None;
    }
    let (qx, qy) = (c.0 - a.0, c.1 - a.1);
    let t = (qx * sy - qy * sx) / denominator;
    let u = (qx * ry - qy * rx) / denominator;
    if t <= 1e-6 || t >= 1.0 - 1e-6 || u <= 1e-6 || u >= 1.0 - 1e-6 {
        return None;
    }
    Some((a.0 + t * rx, a.1 + t * ry, sx, sy))
}

/// Which edges run between the same pair of boxes, and where each sits among
/// them: `None` for a line that has the pair to itself.
///
/// Direction is ignored — `a -> b` and `b -> a` are the same corridor, and two
/// lines drawn on top of each other are two lines a reader sees as one however
/// they were written (`X-018`, narrowed).
fn parallels(layout: &Layout) -> Vec<Option<(usize, usize)>> {
    let mut groups: Vec<((usize, usize), Vec<usize>)> = Vec::new();
    for (e, edge) in layout.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        let pair = (edge.from.min(edge.to), edge.from.max(edge.to));
        match groups.iter_mut().find(|(p, _)| *p == pair) {
            Some((_, ids)) => ids.push(e),
            None => groups.push((pair, vec![e])),
        }
    }

    let mut out = vec![None; layout.edges.len()];
    for (_, ids) in groups.iter().filter(|(_, ids)| ids.len() > 1) {
        for (k, e) in ids.iter().enumerate() {
            out[*e] = Some((k, ids.len()));
        }
    }
    out
}

/// Where each edge attaches, at each end: `None` where the router still picks.
///
/// Two passes, because the second needs the first finished. **Which side** is
/// declared by `from` / `to`, or failing that derived the way the elbow router
/// always derived it — the dominant direction between the two centres. **Where
/// on that side** is then a share: every line arriving at one side of one box
/// takes its own slice of it, in declaration order, so two heads can no longer
/// land on the same three pixels. `lane` reorders them by hand.
///
/// Spreading decides no order and moves no box, so it stays clear of `D-001`;
/// it only stops the engine from putting several answers in one place.
fn attachments(layout: &Layout) -> Vec<[Option<(f64, f64)>; 2]> {
    use crate::style::Side;

    /// One end of one edge: which edge, and whether this is its tail.
    type End = (usize, bool);
    /// Everything wanting one side of one box, in one direction.
    type Bucket = (usize, Side, bool, u8, Vec<End>);

    // Which side each end of each edge uses, declared or derived.
    let side_of = |edge: &crate::layout::PlacedEdge, at_from: bool| -> Option<Side> {
        let declared = if at_from {
            edge.style.from_side
        } else {
            edge.style.to_side
        };
        if declared.is_some() {
            return declared;
        }
        let node = &layout.nodes[if at_from { edge.from } else { edge.to }];
        if node.style.stage.as_deref() == Some("boundary") && node.style.attach.is_some() {
            return Some(Side::Bottom);
        }
        let (a, b) = (layout.nodes[edge.from].rect, layout.nodes[edge.to].rect);
        let (dx, dy) = (b.cx() - a.cx(), b.cy() - a.cy());
        let horizontal = if edge.style.flow.as_deref() == Some("association") {
            dx.abs() / node.rect.w.max(1.0) >= dy.abs() / node.rect.h.max(1.0)
        } else {
            dx.abs() >= dy.abs()
        };
        // The same rule the elbow used: whichever axis dominates decides.
        Some(match (horizontal, at_from) {
            (true, true) => {
                if dx >= 0.0 {
                    Side::Right
                } else {
                    Side::Left
                }
            }
            (true, false) => {
                if dx >= 0.0 {
                    Side::Left
                } else {
                    Side::Right
                }
            }
            (false, true) => {
                if dy >= 0.0 {
                    Side::Bottom
                } else {
                    Side::Top
                }
            }
            (false, false) => {
                if dy >= 0.0 {
                    Side::Top
                } else {
                    Side::Bottom
                }
            }
        })
    };

    // Every (box, side, direction) and the ends that want it, in declaration
    // order. **Direction is part of the key**, and that is what keeps the rule
    // from being a nuisance: a line arriving at a side and a line leaving it
    // are not in each other's way — a head and a tail at one point read
    // perfectly well — so separating them only bends two lines that were
    // straight. Two heads at one point is the thing worth fixing, and two
    // heads are two arrivals.
    let mut buckets: Vec<Bucket> = Vec::new();
    for (e, edge) in layout.edges.iter().enumerate() {
        if edge.from == edge.to {
            continue;
        }
        for at_from in [true, false] {
            let node = if at_from { edge.from } else { edge.to };
            let Some(side) = side_of(edge, at_from) else {
                continue;
            };
            let kind = match edge.style.flow.as_deref() {
                Some("message") => 1,
                Some("association") => 2,
                _ => 0,
            };
            match buckets.iter_mut().find(|(n, s, out, flow, _)| {
                *n == node && *s == side && *out == at_from && *flow == kind
            }) {
                Some((_, _, _, _, ends)) => ends.push((e, at_from)),
                None => buckets.push((node, side, at_from, kind, vec![(e, at_from)])),
            }
        }
    }

    let mut out = vec![[None, None]; layout.edges.len()];
    for (node, side, _, _, ends) in buckets {
        let blocked: Vec<_> = if side == Side::Bottom {
            layout
                .nodes
                .iter()
                .filter(|event| {
                    event.style.stage.as_deref() == Some("boundary")
                        && event.style.attach.as_deref() == Some(layout.nodes[node].id.as_str())
                })
                .map(|event| event.rect)
                .collect()
        } else {
            Vec::new()
        };
        // One line on a side keeps the middle, which is what every drawing did
        // before this existed — so a diagram with no crowding does not move.
        if ends.len() < 2
            && !declares(&layout.edges[ends[0].0], ends[0].1)
            && layout.edges[ends[0].0].style.router != Some(Router::Ortho)
            && blocked.is_empty()
        {
            continue;
        }
        let r = layout.nodes[node].rect;
        let n = ends
            .iter()
            .filter_map(|(e, at_from)| {
                let edge = &layout.edges[*e];
                if *at_from {
                    edge.style.from_port.map(|(_, total)| total)
                } else {
                    edge.style.to_port.map(|(_, total)| total)
                }
            })
            .next()
            .unwrap_or(ends.len() as f64)
            .max(1.0);
        for (k, (e, at_from)) in ends.iter().enumerate() {
            // `lane` counts from 1 and is clamped rather than refused: a lane
            // past the end is a diagram that still has to draw.
            let edge = &layout.edges[*e];
            let port = if *at_from {
                edge.style.from_port
            } else {
                edge.style.to_port
            };
            let slot = port
                .map(|(index, _)| (index - 1.0).clamp(0.0, n - 1.0))
                .or_else(|| edge.style.lane.map(|l| (l - 1.0).clamp(0.0, n - 1.0)))
                .unwrap_or(k as f64);
            let along = (slot + 1.0) / (n + 1.0);
            let mut on_rect = match side {
                Side::Top => (r.x + r.w * along, r.y),
                Side::Bottom => (r.x + r.w * along, r.bottom()),
                Side::Left => (r.x, r.y + r.h * along),
                Side::Right => (r.right(), r.y + r.h * along),
            };
            if port.is_none() && !blocked.is_empty() {
                let free = |x: f64| {
                    blocked
                        .iter()
                        .all(|event| x < event.x - 5.0 || x > event.right() + 5.0)
                };
                if !free(on_rect.0) {
                    let mut candidates: Vec<_> = blocked
                        .iter()
                        .flat_map(|event| [event.x - 6.0, event.right() + 6.0])
                        .filter(|x| *x >= r.x + 6.0 && *x <= r.right() - 6.0 && free(*x))
                        .collect();
                    candidates.sort_by(|a, b| {
                        (a - on_rect.0)
                            .abs()
                            .total_cmp(&(b - on_rect.0).abs())
                            .then(a.total_cmp(b))
                    });
                    if let Some(x) = candidates.first() {
                        on_rect.0 = *x;
                    }
                }
            }
            // Onto the **outline**, not the bounding box. A hexagon's widest
            // point is at mid-height, so a port two thirds of the way down its
            // right-hand side sits outside the shape entirely and the line
            // starts in mid-air — which is exactly what `type network` drew the
            // first time this ran. `boundary` follows the ray from the centre,
            // so the offset survives and the point lands on the drawn edge.
            out[*e][usize::from(!at_from)] = Some(boundary(&layout.nodes[node], on_rect));
        }
    }
    // Keep an aligned straight message straight when a boundary event moves
    // its departure port. Indexed ports remain the author's explicit choice.
    for (i, edge) in layout.edges.iter().enumerate() {
        if edge.style.router != Some(Router::Straight)
            || edge.style.from_port.is_some()
            || edge.style.to_port.is_some()
            || edge.style.flow.as_deref() != Some("message")
        {
            continue;
        }
        let a = &layout.nodes[edge.from];
        let b = &layout.nodes[edge.to];
        if (a.rect.cx() - b.rect.cx()).abs() < 0.01 {
            let x = out[i][0].or(out[i][1]).map(|p| p.0);
            if let Some(x) = x {
                let ay = if b.rect.cy() > a.rect.cy() {
                    a.rect.bottom()
                } else {
                    a.rect.y
                };
                let by = if b.rect.cy() > a.rect.cy() {
                    b.rect.y
                } else {
                    b.rect.bottom()
                };
                let blocked = layout.nodes.iter().any(|event| {
                    event.style.stage.as_deref() == Some("boundary")
                        && [a.id.as_str(), b.id.as_str()]
                            .contains(&event.style.attach.as_deref().unwrap_or(""))
                        && x >= event.rect.x - 5.0
                        && x <= event.rect.right() + 5.0
                        && (ay == a.rect.bottom() || by == b.rect.bottom())
                });
                if !blocked
                    && x > a.rect.x
                    && x < a.rect.right()
                    && x > b.rect.x
                    && x < b.rect.right()
                {
                    out[i] = [Some(boundary(a, (x, ay))), Some(boundary(b, (x, by)))];
                }
            }
        }
    }
    out
}

/// Whether this end of this edge names its own side.
fn declares(edge: &crate::layout::PlacedEdge, at_from: bool) -> bool {
    if at_from {
        edge.style.from_side.is_some()
    } else {
        edge.style.to_side.is_some()
    }
}

/// Route every edge and place every edge label.
///
/// Public because three writers need the same answer — SVG, PDF and the JSON
/// export — and a second implementation would be a second diagram.
pub fn route_all(layout: &Layout) -> Vec<Routed> {
    let ports = attachments(layout);
    let siblings = parallels(layout);

    // What earlier lines have already run along. Accumulated in declaration
    // order, which is what makes the answer the same every run — and what makes
    // it the author's to change, since reordering two statements reorders who
    // gets the direct channel (`X-018`).
    let mut used: Vec<crate::ortho::Used> = Vec::new();

    let mut routed: Vec<Routed> = layout
        .edges
        .iter()
        .enumerate()
        .map(|(e, edge)| Routed {
            points: if edge.from == edge.to {
                self_loop(&layout.nodes[edge.from])
            } else {
                // Every box but the two being joined is an obstacle. A group
                // frame is not: a box inside one can only be reached by
                // crossing it, so treating a frame as solid would refuse the
                // routes that are correct.
                let mut obstacles: Vec<crate::layout::Rect> = layout
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !crate::layout::endpoint_of(layout, edge, *i))
                    .map(|(_, n)| n.rect)
                    .collect();
                obstacles.extend(
                    layout
                        .annotations
                        .iter()
                        .filter(|text| !text.title)
                        .map(crate::labels::annotation_rect),
                );
                obstacles.extend(
                    layout
                        .groups
                        .iter()
                        .filter_map(crate::labels::group_title_rect),
                );
                let line = if edge.style.router == Some(Router::Ortho) {
                    orthogonal(layout, edge, ports[e], &obstacles, &used)
                } else {
                    let line = route(
                        &layout.nodes[edge.from],
                        &layout.nodes[edge.to],
                        edge.style.router.unwrap_or(Router::Straight),
                        &obstacles,
                        ports[e],
                        &used,
                    );
                    through(line, &edge.via, &obstacles, edge.style.router, &used)
                };
                // Two lines between the same pair lie on top of each other
                // when both are straight. Each takes its own bow, alternating
                // sides so a pair separates rather than both leaning one way.
                let line = match siblings[e] {
                    Some(_) if edge.style.router == Some(Router::Ortho) => line,
                    Some((k, n)) => {
                        // The sign follows the *pair*, not the direction the
                        // edge was written in. A bow is perpendicular to the
                        // line, so `b -> a` flips the perpendicular too, and
                        // the two ended up leaning the same way — which is
                        // exactly the overlap this exists to remove.
                        let way = if edge.from <= edge.to { 1.0 } else { -1.0 };
                        let spread = 0.12 * (k as f64 - (n as f64 - 1.0) / 2.0) * way;
                        bow(line[0], *line.last().expect("an end"), spread)
                    }
                    _ => line,
                };
                // Only an orthogonal run is worth remembering: a straight line
                // is a diagonal and two of them meeting is a crossing, which
                // reads fine, and a curve already bows clear of its twin.
                if edge.style.router == Some(Router::Ortho) {
                    for pair in line.windows(2) {
                        used.push((pair[0], pair[1]));
                    }
                }
                round_corners(&line, edge.style.bend.unwrap_or(0.0))
            },
            label: None,
        })
        .collect();

    // Seed in source order, then improve against the complete set. The seed
    // preserves the old deterministic answer when every candidate ties; the
    // two coordinate-descent passes let an early label step aside once a later
    // one proves the original choice was globally expensive.
    let mut taken: Vec<Rect> = Vec::new();
    for i in 0..routed.len() {
        let Some(text) = layout.edges[i]
            .label
            .clone()
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let rect = place_label(&text, i, &routed, layout, &taken, &layout.edges[i].style);
        taken.push(rect);
        routed[i].label = Some((text, rect));
    }

    // Coordinate descent is deliberately bounded: it improves the global
    // arrangement without making rendering time depend on convergence. Four
    // passes are enough for a label to move aside and let the next one settle
    // without turning a pathological document into an unbounded search.
    for _ in 0..4 {
        for i in 0..routed.len() {
            if layout.edges[i].style.label_offset.is_some() {
                continue;
            }
            let Some((text, _)) = routed[i].label.clone() else {
                continue;
            };
            let taken: Vec<Rect> = routed
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != i)
                .filter_map(|(_, route)| route.label.as_ref().map(|(_, rect)| *rect))
                .collect();
            let rect = place_label(&text, i, &routed, layout, &taken, &layout.edges[i].style);
            routed[i].label = Some((text, rect));
        }
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
        (true, f64::MAX),
        Rect {
            x: 0.0,
            y: 0.0,
            w,
            h,
        },
    );

    for (along, k) in [0.0, 0.2, -0.2, 0.35, -0.35].into_iter().flat_map(|along| {
        [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0]
            .into_iter()
            .map(move |k| (along, k))
    }) {
        let cx = anchor.0 + dx * along + px * step * k;
        let cy = anchor.1 + dy * along + py * step * k;
        let rect = Rect {
            x: cx - w / 2.0,
            y: cy - h / 2.0,
            w,
            h,
        };

        // Prefer a clear candidate; keep the search near its own segment so
        // the reader can still tell which edge the label belongs to.
        let collision = label_cost(&rect, index, routed, layout, taken);
        let distance = k.abs() * 0.4 + along.abs() * len / step * 0.4;
        let cost = (collision > 1e-9, collision + distance);
        if cost < best.0 {
            best = (cost, rect);
        }
        if cost == (false, 0.0) {
            break;
        }
    }

    // A label is never left over a node (`X-034`). The midpoint can sit on a
    // short elbow jog pressed against a box — spread edges land one there —
    // where every nearby candidate clips the box or crosses a frame, and the
    // least-bad answer is still an overpainted label. The room is usually
    // elsewhere on the same route, so before settling, walk the edge's other
    // segments and take the cheapest candidate that clears every box. The
    // search stays on the route itself: a label that cannot escape *there* is
    // reported by the check, not teleported off its line.
    if layout
        .nodes
        .iter()
        .any(|node| overlap_fraction(&best.1, &node.rect) > 1e-9)
    {
        let mut escaped: Option<((bool, bool, f64), Rect)> = None;
        for pair in routed[index].points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (sx, sy) = (b.0 - a.0, b.1 - a.1);
            let slen = (sx * sx + sy * sy).sqrt();
            if slen < 1e-6 {
                continue;
            }
            let (spx, spy) = (-sy / slen, sx / slen);
            let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            for (along, k) in [0.0, 0.2, -0.2, 0.35, -0.35].into_iter().flat_map(|along| {
                [0.0, 1.0, -1.0, 2.0, -2.0, 3.0, -3.0]
                    .into_iter()
                    .map(move |k| (along, k))
            }) {
                let cx = mid.0 + sx * along + spx * step * k;
                let cy = mid.1 + sy * along + spy * step * k;
                let rect = Rect {
                    x: cx - w / 2.0,
                    y: cy - h / 2.0,
                    w,
                    h,
                };
                if layout
                    .nodes
                    .iter()
                    .any(|node| overlap_fraction(&rect, &node.rect) > 1e-9)
                {
                    continue;
                }
                let collision = label_cost(&rect, index, routed, layout, taken);
                let distance = (crate::math::square(cx - anchor.0)
                    + crate::math::square(cy - anchor.1))
                .sqrt()
                    / step
                    * 0.4;
                // A candidate on the wire itself reads as belonging to the
                // edge; one beside a far segment does not, however near.
                let cost = (collision > 1e-9, k != 0.0, collision + distance);
                if escaped.is_none_or(|(prev, _)| cost < prev) {
                    escaped = Some((cost, rect));
                }
            }
        }
        if let Some((_, rect)) = escaped {
            return rect;
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
        if let Some(label) = moved_node_label(node) {
            cost += 4.0 * overlap_fraction(rect, &label);
        }
    }
    for annotation in &layout.annotations {
        cost += 4.0 * overlap_fraction(rect, &crate::labels::annotation_rect(annotation));
    }
    // A frame is a boundary the reader uses; a label astride one is confusing
    // about what it belongs to.
    for group in &layout.groups {
        if let Some(title) = crate::labels::group_title_rect(group) {
            cost += 4.0 * overlap_fraction(rect, &title);
        }
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
    let area = a.w * a.h;
    if area <= 0.0 {
        0.0
    } else {
        crate::labels::overlap(*a, *b) / area
    }
}

pub(crate) fn segment_hits(p: (f64, f64), q: (f64, f64), r: &Rect) -> bool {
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
pub(crate) fn moved_node_label(node: &PlacedNode) -> Option<Rect> {
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
pub(crate) fn moved_group_label(group: &PlacedGroup, opts: &GraphOpts) -> Option<Rect> {
    if group.lines.iter().all(|l| l.is_empty()) {
        return None;
    }
    let size = group.style.font_size.unwrap_or(12.0);
    let measured = embedded_width(
        opts,
        group.style.font_family.as_deref().unwrap_or("sans-serif"),
        &group.lines.join(" "),
        size,
    );
    let (dx, dy) = group.style.label_offset.or(measured.map(|_| (0.0, 0.0)))?;
    if let Some(width) = measured {
        return Some(Rect {
            x: group.rect.x + 9.0 + dx,
            y: group.rect.y + size * (1.3 - 0.72) + dy,
            w: width + 6.0,
            h: size * 1.4,
        });
    }
    Some(Rect {
        x: group.rect.x + 10.0 + dx,
        y: group.rect.y + size / 2.0 + dy,
        w: text_width(&[group.lines.join(" ")], size),
        h: size,
    })
}

fn embedded_width(opts: &GraphOpts, family: &str, text: &str, size: f64) -> Option<f64> {
    let (embedded, bytes) = opts.embed_font.as_ref()?;
    let primary = family.split(',').next()?.trim().trim_matches(['\'', '"']);
    if !primary.eq_ignore_ascii_case(embedded) {
        return None;
    }
    crate::subset::text_advance(bytes, text, size)
}

fn bounds(layout: &Layout, routed: &[Routed]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;

    let outer = |rect: Rect, style: &Style| {
        let margin = crate::spacing::Margins::of(style);
        Rect {
            x: rect.x - margin.left,
            y: rect.y - margin.top,
            w: rect.w + margin.left + margin.right,
            h: rect.h + margin.top + margin.bottom,
        }
    };
    let mut boxes: Vec<Rect> = layout
        .nodes
        .iter()
        .map(|n| outer(n.rect, &n.style))
        .collect();
    boxes.extend(layout.groups.iter().map(|g| outer(g.rect, &g.style)));
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
    boxes.extend(layout.annotations.iter().map(|annotation| {
        let mut rect = annotation_bounds(annotation);
        if annotation.title
            && let Some(width) = embedded_width(
                &layout.opts,
                &annotation.family,
                &annotation.text,
                annotation.size,
            )
        {
            rect.x = match annotation.anchor {
                Anchor::Start => annotation.x,
                Anchor::Middle => annotation.x - width / 2.0,
                Anchor::End => annotation.x - width,
            };
            rect.w = width;
        }
        rect
    }));
    boxes.extend(routed.iter().filter_map(|r| r.label.as_ref().map(|l| l.1)));
    // A label the author moved can leave the shape it belongs to entirely.
    boxes.extend(layout.nodes.iter().filter_map(moved_node_label));
    boxes.extend(
        layout
            .groups
            .iter()
            .filter_map(|group| moved_group_label(group, &layout.opts)),
    );

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
    let mut points: Vec<(f64, f64)> = p.points.clone();
    if p.head.is_some() {
        stabilise_marker_tangent(&mut points);
    }

    let points: Vec<String> = points
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

    let linecap = if p.head.is_some() { "butt" } else { "round" };
    format!(
        r#"<{} points="{}"{}{} stroke-width="{}" stroke-linejoin="round" stroke-linecap="{linecap}"{attrs}/>"#,
        if p.closed { "polygon" } else { "polyline" },
        points.join(" "),
        paint("fill", p.fill.as_deref().unwrap_or("none")),
        paint("stroke", p.stroke.as_deref().unwrap_or("none")),
        n(p.width)
    )
}

/// A frame and its name, set into the top-left of the border.
fn group_frame(group: &PlacedGroup, opts: &GraphOpts) -> String {
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
        if let Some(fill) = opts.background.as_deref().filter(|b| *b != "none") {
            let w = embedded_width(
                opts,
                s.font_family.as_deref().unwrap_or("sans-serif"),
                &text,
                size,
            )
            .unwrap_or_else(|| text_width(std::slice::from_ref(&text), size));
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
    // A group, rather than reopening the shape's own tag to hold the title.
    //
    // Reopening it was the first answer and it was **wrong for any shape drawn
    // from more than one element**: it took the tag name from the first `<` and
    // the closing `/>` from the last, so a cylinder — a `<path>` for the body
    // and an `<ellipse>` for the cap — came out as
    // `<ellipse …><title>…</title></path>`. A browser is lenient enough to draw
    // that, which is why the preview and the golden test never saw it; resvg is
    // not, so `--window` and `--png` failed on any diagram with a described
    // cylinder. A `<g>` is valid whatever the shape is made of, and it takes
    // the tag-sniffing out entirely.
    format!("<g><title>{}</title>{shape}</g>", esc(text))
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
            let ry = cylinder_cap(r);
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

#[allow(clippy::too_many_arguments)]
fn orthogonal(
    layout: &Layout,
    edge: &crate::layout::PlacedEdge,
    ports: [Option<(f64, f64)>; 2],
    obstacles: &[Rect],
    used: &[crate::ortho::Used],
) -> Vec<(f64, f64)> {
    let from = &layout.nodes[edge.from];
    let to = &layout.nodes[edge.to];
    let fallback = route(from, to, Router::Straight, &[], ports, &[]);
    let start = ports[0].unwrap_or(fallback[0]);
    let end = ports[1].unwrap_or(fallback[1]);
    let outward = |node: &PlacedNode, p: (f64, f64)| {
        if node.style.stage.as_deref() == Some("boundary") && node.style.attach.is_some() {
            return (0.0, 16.0);
        }
        let r = node.rect;
        let dx = (p.0 - r.cx()) / r.w.max(1.0);
        let dy = (p.1 - r.cy()) / r.h.max(1.0);
        if dx.abs() >= dy.abs() {
            (dx.signum() * 16.0, 0.0)
        } else {
            (0.0, dy.signum() * 16.0)
        }
    };
    let a = outward(from, start);
    let b = outward(to, end);
    let a = (start.0 + a.0, start.1 + a.1);
    let b = (end.0 + b.0, end.1 + b.1);
    let mut obstacles = obstacles.to_vec();
    obstacles.extend([from.rect, to.rect]);
    // The host is solid for a boundary flow once its port has left the event.
    for node in [from, to] {
        if let Some(host) = node
            .style
            .attach
            .as_deref()
            .and_then(|id| layout.nodes.iter().find(|n| n.id == id))
        {
            obstacles.push(host.rect);
        }
    }
    let mut out = vec![start, a];
    let mut at = a;
    for stop in edge.via.iter().chain(std::iter::once(&b)) {
        let leg = crate::ortho::route_points(at, *stop, &obstacles, used)
            .unwrap_or_else(|| vec![at, (stop.0, at.1), *stop]);
        out.extend(leg.into_iter().skip(1));
        at = *stop;
    }
    out.push(end);
    straighten(out)
}

fn route(
    from: &PlacedNode,
    to: &PlacedNode,
    router: Router,
    obstacles: &[crate::layout::Rect],
    // Where each end attaches, when it was declared or shared with another
    // line (`D-033`). `None` leaves that end exactly where it always was, so a
    // diagram nobody has crowded draws the same as it did.
    ports: [Option<(f64, f64)>; 2],
    used: &[crate::ortho::Used],
) -> Vec<(f64, f64)> {
    let a = &from.rect;
    let b = &to.rect;

    match router {
        Router::Straight => {
            // An attachment aims the ray: the line still leaves the shape's
            // real outline, so a diamond or an ellipse is clipped as before —
            // it is the *direction* the port decides, not the clipping.
            let start =
                ports[0].unwrap_or_else(|| boundary(from, ports[1].unwrap_or((b.cx(), b.cy()))));
            let end = ports[1].unwrap_or_else(|| boundary(to, start));
            vec![start, end]
        }
        // A bow between the two ends, sampled. Two lines between the same pair
        // are the case it exists for: straight, they lie on top of each other.
        Router::Curved => {
            let start =
                ports[0].unwrap_or_else(|| boundary(from, ports[1].unwrap_or((b.cx(), b.cy()))));
            let end = ports[1].unwrap_or_else(|| boundary(to, start));
            bow(start, end, 0.0)
        }
        // `X-004`. The solver goes around the boxes; the elbow below is what is
        // left when there is no way round, such as a box wholly enclosed.
        // Keeping the old line rather
        // than dropping the edge is deliberate: `check::after_layout` then says
        // the line crosses something, which is a drawing that admits it is
        // wrong, where a missing edge is one that lies.
        Router::Ortho if let Some(path) = crate::ortho::route(*a, *b, obstacles, used) => {
            // The solver runs centre to centre and is clipped to the two
            // outlines; a declared attachment replaces the clipped end with the
            // point that was asked for, and a short stub joins the two so the
            // line still leaves the box square rather than at an angle.
            attach(path, ports, *a, *b)
        }
        Router::Ortho => {
            let dx = b.cx() - a.cx();
            let dy = b.cy() - a.cy();

            let elbow = if dx.abs() >= dy.abs() {
                let sx = if dx >= 0.0 { a.right() } else { a.x };
                let ex = if dx >= 0.0 { b.x } else { b.right() };
                let mid = (sx + ex) / 2.0;
                vec![(sx, a.cy()), (mid, a.cy()), (mid, b.cy()), (ex, b.cy())]
            } else {
                let sy = if dy >= 0.0 { a.bottom() } else { a.y };
                let ey = if dy >= 0.0 { b.y } else { b.bottom() };
                let mid = (sy + ey) / 2.0;
                vec![(a.cx(), sy), (a.cx(), mid), (b.cx(), mid), (b.cx(), ey)]
            };
            // The fallback honours an attachment too: which line you get when
            // the solver declines is not a reason to lose what you declared.
            attach(elbow, ports, *a, *b)
        }
    }
}

/// Move a routed path's ends onto their declared attachment points.
///
/// The solver runs centre to centre and clips to the two outlines, which is the
/// right answer when nobody has said otherwise. When somebody has, the clipped
/// end is replaced and a **stub** is inserted from the port to the first point
/// the path already had inside the box — so the line still leaves square
/// through the side that was named, rather than cutting to it at an angle.
fn attach(
    mut path: Vec<(f64, f64)>,
    ports: [Option<(f64, f64)>; 2],
    a: Rect,
    b: Rect,
) -> Vec<(f64, f64)> {
    if path.len() < 2 {
        return path;
    }
    if let Some(p) = ports[0] {
        let next = path[1];
        path[0] = p;
        // A right angle out of the side: horizontal off a left or right edge,
        // vertical off a top or bottom one.
        let stub = if (p.0 - a.x).abs() < 0.01 || (p.0 - a.right()).abs() < 0.01 {
            (next.0, p.1)
        } else {
            (p.0, next.1)
        };
        if long_enough(p, stub) {
            path.insert(1, stub);
        }
    }
    if let Some(p) = ports[1] {
        let last = path.len() - 1;
        let prev = path[last - 1];
        path[last] = p;
        let stub = if (p.0 - b.x).abs() < 0.01 || (p.0 - b.right()).abs() < 0.01 {
            (prev.0, p.1)
        } else {
            (p.0, prev.1)
        };
        if long_enough(p, stub) {
            path.insert(last, stub);
        }
    }
    path
}

/// Bend a routed line through the places it was told to go (`D-034`).
///
/// **The waypoints are a passage, not a preference**: the line goes through
/// each one, in the order written. What survives between them is the
/// obstacle avoidance — every leg is routed by the same solver `W-105` added,
/// so a line told to pass left of a box still goes around whatever stands
/// between it and there. That is the whole reason a waypoint is not simply a
/// list of points to join up: joining them up would trade the guarantee this
/// project spent a chantier acquiring for the control it spent this one on,
/// and there is no reason to pay twice.
///
/// A waypoint inside a box is not repaired here. `check::after_layout` reports
/// it, which is the same bargain `X-004` struck: a drawing that admits it is
/// wrong beats one that quietly moves what the author wrote.
fn through(
    line: Vec<(f64, f64)>,
    via: &[(f64, f64)],
    obstacles: &[Rect],
    router: Option<Router>,
    used: &[crate::ortho::Used],
) -> Vec<(f64, f64)> {
    if via.is_empty() || line.len() < 2 {
        return line;
    }
    let start = line[0];
    let end = *line.last().expect("a routed line has an end");

    let mut out = vec![start];
    let mut at = start;
    for stop in via.iter().chain(std::iter::once(&end)) {
        // A leg between two bare points has no boxes of its own to leave, so
        // the solver is given two degenerate rectangles: it routes from one to
        // the other and avoids everything else, which is exactly the question.
        let leg = if router == Some(Router::Ortho) {
            crate::ortho::route(dot(at), dot(*stop), obstacles, used)
                .unwrap_or_else(|| vec![at, (stop.0, at.1), *stop])
        } else {
            vec![at, *stop]
        };
        out.extend(leg.into_iter().skip(1));
        at = *stop;
    }
    straighten(out)
}

/// A quadratic bow from `a` to `b`, sampled as a polyline.
///
/// `lift` is how far the middle is pushed off the straight line, as a fraction
/// of its length; zero takes a gentle default. Sampled rather than emitted as a
/// curve so the bounds, the label solver, the PDF writer and the JSON export
/// all keep working on the polyline they already understand — the same choice
/// `self_loop` made for the same reason.
fn bow(a: (f64, f64), b: (f64, f64), lift: f64) -> Vec<(f64, f64)> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = crate::math::hypot(dx, dy).max(1e-6);
    let amount = if lift.abs() < 1e-9 { 0.18 } else { lift };
    // Perpendicular, so the bow is always across the line rather than along it.
    let control = (
        (a.0 + b.0) / 2.0 - dy / len * len * amount,
        (a.1 + b.1) / 2.0 + dx / len * len * amount,
    );

    const STEPS: usize = 16;
    (0..=STEPS)
        .map(|i| {
            let t = i as f64 / STEPS as f64;
            let u = 1.0 - t;
            (
                u * u * a.0 + 2.0 * u * t * control.0 + t * t * b.0,
                u * u * a.1 + 2.0 * u * t * control.1 + t * t * b.1,
            )
        })
        .collect()
}

/// Round every corner of a path by `radius`, sampled.
///
/// Each turn becomes a short arc between two points cut back along the two
/// segments meeting there. The cut is never more than half the shorter segment,
/// so a tight elbow rounds a little and a long one rounds fully — a radius that
/// ate a whole segment would turn a route into a different route.
fn round_corners(path: &[(f64, f64)], radius: f64) -> Vec<(f64, f64)> {
    if radius <= 0.0 || path.len() < 3 {
        return path.to_vec();
    }
    let mut out = vec![path[0]];
    for i in 1..path.len() - 1 {
        let (p, c, n) = (path[i - 1], path[i], path[i + 1]);
        let back = (crate::math::hypot(c.0 - p.0, c.1 - p.1) / 2.0).min(radius);
        let fwd = (crate::math::hypot(n.0 - c.0, n.1 - c.1) / 2.0).min(radius);
        let r = back.min(fwd);
        if r < 0.5 {
            out.push(c);
            continue;
        }
        let towards = |from: (f64, f64), to: (f64, f64), d: f64| {
            let len = crate::math::hypot(to.0 - from.0, to.1 - from.1).max(1e-6);
            (
                from.0 + (to.0 - from.0) / len * d,
                from.1 + (to.1 - from.1) / len * d,
            )
        };
        let enter = towards(c, p, r);
        let leave = towards(c, n, r);

        const STEPS: usize = 6;
        for k in 0..=STEPS {
            let t = k as f64 / STEPS as f64;
            let u = 1.0 - t;
            out.push((
                u * u * enter.0 + 2.0 * u * t * c.0 + t * t * leave.0,
                u * u * enter.1 + 2.0 * u * t * c.1 + t * t * leave.1,
            ));
        }
    }
    out.push(*path.last().expect("a path has an end"));
    out
}

/// A point as a rectangle of no size, for a solver that speaks in boxes.
fn dot((x, y): (f64, f64)) -> Rect {
    Rect {
        x,
        y,
        w: 0.0,
        h: 0.0,
    }
}

/// Drop the points that say nothing: a repeat, or a corner that does not turn.
fn straighten(points: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(points.len());
    for p in points {
        if out.last().is_some_and(|q| same(*q, p)) {
            continue;
        }
        if out.len() >= 2 {
            let (a, b) = (out[out.len() - 2], out[out.len() - 1]);
            // Collinear **and going the same way**. Three points sharing an
            // ordinate are not on one straight run if the middle one is a
            // turn-back: `(80,200) (240,200) (220,200)` is a line that
            // overshoots and returns, and dropping the middle point silently
            // deleted the very waypoint it had been sent to (`W-126`).
            let between = |a: f64, b: f64, c: f64| (a - b) * (c - b) <= 0.0;
            let straight = ((a.0 - b.0).abs() < 0.01
                && (b.0 - p.0).abs() < 0.01
                && between(a.1, b.1, p.1))
                || ((a.1 - b.1).abs() < 0.01 && (b.1 - p.1).abs() < 0.01 && between(a.0, b.0, p.0));
            if straight {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

fn same(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
}

/// Whether a stub is worth having. A one-pixel elbow is not a right angle, it
/// is a kink — and the label solver anchors on whichever segment it finds, so a
/// degenerate one puts the text on the box the line has just left.
fn long_enough(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() + (a.1 - b.1).abs() > 6.0
}

/// SVG markers read the tangent of the first and last segments. If either end
/// is effectively a zero-length stub, the arrow can aim away from the route.
///
/// This keeps those terminal segments long enough to be numerically stable,
/// without altering routing decisions elsewhere.
fn stabilise_marker_tangent(points: &mut [(f64, f64)]) {
    if points.len() < 3 {
        return;
    }

    const MIN_LEN: f64 = 1.5;

    let i = points.len() - 1;
    let (a, b) = (points[i - 2], points[i - 1]);
    let end = points[i];
    let end_len = (crate::math::square(end.0 - b.0) + crate::math::square(end.1 - b.1)).sqrt();
    if end_len > 0.0 && end_len < MIN_LEN {
        let (px, py) = (b.0 - a.0, b.1 - a.1);
        let prev_len = (px * px + py * py).sqrt();
        if prev_len > 0.0 {
            let ux = px / prev_len;
            let uy = py / prev_len;
            points[i - 1] = (end.0 - ux * MIN_LEN, end.1 - uy * MIN_LEN);
        }
    }

    let (start, b, c) = (points[0], points[1], points[2]);
    let start_len =
        (crate::math::square(b.0 - start.0) + crate::math::square(b.1 - start.1)).sqrt();
    if start_len > 0.0 && start_len < MIN_LEN {
        let (nx, ny) = (c.0 - b.0, c.1 - b.1);
        let next_len = (nx * nx + ny * ny).sqrt();
        if next_len > 0.0 {
            let ux = nx / next_len;
            let uy = ny / next_len;
            points[1] = (start.0 + ux * MIN_LEN, start.1 + uy * MIN_LEN);
        }
    }
}

/// Length occupied behind the marker tip. The visible stroke ends here while
/// a second, invisible polyline anchors the SVG marker at the real endpoint.
fn marker_stem(head: Option<Head>) -> Option<f64> {
    const OVERLAP: f64 = 1.5;

    match head.filter(|head| *head != Head::None) {
        // Let the visible stroke run a little way under the marker base. This
        // avoids a hairline seam from SVG anti-aliasing without reaching the tip.
        Some(Head::Solid | Head::Open | Head::Hollow) => Some(11.0 - OVERLAP),
        _ => None,
    }
}

fn trim_marker_stems(points: &mut [(f64, f64)], start: Option<f64>, end: Option<f64>) {
    if points.len() < 2 {
        return;
    }

    if let Some(amount) = end {
        let last = points.len() - 1;
        let (x0, y0) = points[last - 1];
        let (x1, y1) = points[last];
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = crate::math::hypot(dx, dy);
        if len > amount {
            points[last] = (x1 - dx * amount / len, y1 - dy * amount / len);
        }
    }

    if let Some(amount) = start {
        let (x0, y0) = points[0];
        let (x1, y1) = points[1];
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = crate::math::hypot(dx, dy);
        if len > amount {
            points[0] = (x0 + dx * amount / len, y0 + dy * amount / len);
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
            (
                cx + rx * crate::math::cos(t),
                top - ry * crate::math::sin(t),
            )
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
        let t = 1.0 / (crate::math::square(dx / rx) + crate::math::square(dy / ry)).sqrt();
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

/// Half-height of the ellipse a cylinder is capped with.
///
/// Shared with the label, which has to sit clear of it: `shape_allowance` adds
/// the cap's room to the box's *height*, and centring then splits that room
/// evenly while the cap only eats from the top. Two copies of this expression
/// would be two chances for the text and the drawing to disagree (`W-109`).
pub(crate) fn cylinder_cap(r: &Rect) -> f64 {
    (r.h * 0.16).min(14.0)
}

fn node_label(node: &PlacedNode) -> String {
    if node.lines.iter().all(|l| l.is_empty()) {
        return String::new();
    }
    let s = &node.style;
    let font_size = s.font_size.unwrap_or(14.0);
    let line_h = font_size * 1.3;
    let (ox, oy) = s.label_offset.unwrap_or((0.0, 0.0));
    let cx = node.rect.cx() + ox;
    // A cylinder's readable area starts below its cap, so its text sits half a
    // cap lower than the box's own middle.
    let sink = if s.shape == Some(Shape::Cylinder) {
        cylinder_cap(&node.rect) / 2.0
    } else {
        0.0
    };
    let start = node.rect.cy() + oy + sink - (node.lines.len() as f64 - 1.0) * line_h / 2.0;

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
        Head::Hollow => "hollow",
        Head::Diamond => "diamond",
        Head::Composed => "composed",
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
        // Keep the arrow tip at the marker origin. This makes the line end
        // and the visible tip the same point in every orientation.
        Head::Solid => format!(r#"<path d="M-11,0 L0,5.5 L-11,11 z"{fill}/>"#),
        Head::Open => format!(
            r#"<path d="M-11,0 L0,5.5 L-11,11" fill="none"{stroke} stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round"/>"#
        ),
        Head::Dot => format!(r#"<circle cx="5.5" cy="5.5" r="3.5"{fill}/>"#),
        Head::None => String::new(),
        // UML. The two hollow ones are filled with the **surface** rather than
        // left transparent: the line runs under the marker, and a see-through
        // triangle reads as a triangle with a line through it (`W-125`).
        Head::Hollow => format!(
            r#"<path d="M0,0 L11,5.5 L0,11 z"{surface}{stroke} stroke-width="1.4" stroke-linejoin="round"/>"#,
            surface = paint("fill", background.as_deref().unwrap_or("#ffffff"))
        ),
        Head::Diamond | Head::Composed => {
            let paint_in = if head == Head::Composed {
                paint("fill", color)
            } else {
                paint("fill", background.as_deref().unwrap_or("#ffffff"))
            };
            format!(
                r#"<path d="M0,5.5 L8,1.6 L16,5.5 L8,9.4 z"{paint_in}{stroke} stroke-width="1.3" stroke-linejoin="miter"/>"#
            )
        }
        _ => unreachable!("cardinality is drawn by `cardinality_def`"),
    };
    let (ref_x, width, view_box) = match head {
        Head::Dot => (5, 11, "0 0 11 11"),
        Head::Hollow => (11, 12, "0 0 12 11"),
        Head::Diamond | Head::Composed => (16, 17, "0 0 17 11"),
        // Leave one unit of viewport past x=0: SVG clips marker content at its
        // viewport edge, which otherwise turns the arrow tip into a flat end.
        Head::Solid | Head::Open => (0, 13, "-12 0 13 11"),
        _ => unreachable!("handled above"),
    };

    format!(
        r#"<marker id="{id}" markerUnits="userSpaceOnUse" markerWidth="{width}" markerHeight="11" viewBox="{view_box}" refX="{ref_x}" refY="5.5" orient="auto-start-reverse">{inner}</marker>"#
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
        r#"<marker id="{id}" markerUnits="userSpaceOnUse" markerWidth="19" markerHeight="18" viewBox="0 0 19 18" refX="18" refY="9" orient="auto-start-reverse">{inner}</marker>"#
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
    (cx + r * crate::math::sin(a), cy - r * crate::math::cos(a))
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
