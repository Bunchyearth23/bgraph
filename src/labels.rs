//! Text footprints shared by caption placement and edge-label avoidance.

use crate::layout::{Anchor, Annotation, Rect, text_width};

pub(crate) fn annotation_rect(text: &Annotation) -> Rect {
    let w = text_width(std::slice::from_ref(&text.text), text.size);
    let x = match text.anchor {
        Anchor::Start => text.x,
        Anchor::Middle => text.x - w / 2.0,
        Anchor::End => text.x - w,
    };
    Rect {
        x: x - 3.0,
        y: text.y - text.size * 0.6 - 3.0,
        w: w + 6.0,
        h: text.size * 1.2 + 6.0,
    }
}

pub(crate) fn overlap(a: Rect, b: Rect) -> f64 {
    (a.right().min(b.right()) - a.x.max(b.x)).max(0.0)
        * (a.bottom().min(b.bottom()) - a.y.max(b.y)).max(0.0)
}

pub(crate) fn group_title_rect(group: &crate::layout::PlacedGroup) -> Option<Rect> {
    if group.lines.iter().all(String::is_empty) {
        return None;
    }
    let size = group.style.font_size.unwrap_or(12.0);
    let (dx, dy) = group.style.label_offset.unwrap_or_default();
    Some(Rect {
        x: group.rect.x + 9.0 + dx,
        y: group.rect.y + size * 0.58 + dy,
        w: text_width(&[group.lines.join(" ")], size) + 6.0,
        h: size * 1.4,
    })
}

pub(crate) fn node_spacing(
    node: &crate::layout::PlacedNode,
    caption: Option<&str>,
    bpmn: bool,
) -> crate::spacing::Margins {
    let mut spacing = crate::spacing::Margins::of(&node.style);
    #[cfg(feature = "bpmn")]
    if bpmn
        && node.lines.is_empty()
        && let Some(caption) = caption
    {
        let ext = caption_overhang(node.rect, caption, &node.style);
        spacing.left += ext.left;
        spacing.top += ext.top;
        spacing.right += ext.right;
        spacing.bottom += ext.bottom;
    }
    #[cfg(not(feature = "bpmn"))]
    let _ = (&mut spacing, caption, bpmn);
    spacing
}

#[cfg(feature = "bpmn")]
pub(crate) fn caption_rect(rect: Rect, text: &str, style: &crate::style::Style) -> Rect {
    let size = style.font_size.unwrap_or(14.0) * 0.85;
    let lines: Vec<_> = text
        .split('\n')
        .map(|line| line.trim().to_string())
        .collect();
    let w = text_width(&lines, size) + 6.0;
    let (dx, dy) = style.label_offset.unwrap_or_default();
    let mut caption = Rect {
        x: rect.cx() - w / 2.0 + dx,
        y: rect.bottom() + 6.0 + size * 0.4 - 3.0 + dy,
        w,
        h: (lines.len().saturating_sub(1) as f64 * 1.3 + 1.2) * size + 6.0,
    };
    match style.label_position.as_deref() {
        Some("top") => caption.y = rect.y - 6.0 - caption.h + dy,
        Some("right") => {
            caption.x = rect.right() + 6.0 + dx;
            caption.y = rect.cy() - caption.h / 2.0 + dy;
        }
        Some("left") => {
            caption.x = rect.x - 6.0 - caption.w + dx;
            caption.y = rect.cy() - caption.h / 2.0 + dy;
        }
        _ => {}
    }
    caption
}

#[cfg(feature = "bpmn")]
pub(crate) fn caption_overhang(
    rect: Rect,
    text: &str,
    style: &crate::style::Style,
) -> crate::spacing::Margins {
    let label = caption_rect(rect, text, style);
    crate::spacing::Margins {
        left: (rect.x - label.x).max(0.0),
        top: (rect.y - label.y).max(0.0),
        right: (label.right() - rect.right()).max(0.0),
        bottom: (label.bottom() - rect.bottom()).max(0.0),
    }
}

#[cfg(feature = "bpmn")]
pub(crate) fn place_captions(
    layout: &crate::layout::Layout,
    captions: &[Option<String>],
    pinned: &[bool],
    caption_annotations: std::ops::Range<usize>,
) -> Vec<(f64, f64)> {
    let routed = crate::render_svg::route_all(layout);
    let mut offsets: Vec<_> = layout
        .nodes
        .iter()
        .take(captions.len())
        .map(|node| node.style.label_offset.unwrap_or_default())
        .collect();
    let mut rects: Vec<_> = layout
        .nodes
        .iter()
        .zip(captions)
        .map(|(node, text)| {
            text.as_ref()
                .map(|text| caption_rect(node.rect, text, &node.style))
        })
        .collect();
    // Bounded refinement keeps dense drawings deterministic and lets an early
    // caption step aside after a later caption has found its own place.
    for _ in 0..3 {
        for (i, node) in layout.nodes.iter().take(captions.len()).enumerate() {
            let Some(current) = rects[i] else {
                continue;
            };
            if pinned[i] {
                continue;
            }
            let mut base = current;
            base.x -= offsets[i].0;
            base.y -= offsets[i].1;
            let r = node.rect;
            let gap = 6.0;
            let mut candidates = vec![base];
            for y in [base.y, r.y - gap - base.h] {
                for x in [r.cx() - base.w / 2.0, r.cx() + gap, r.cx() - gap - base.w] {
                    candidates.push(Rect { x, y, ..base });
                }
            }
            for x in [r.right() + gap, r.x - gap - base.w] {
                for y in [r.cy() - base.h / 2.0, r.y, r.bottom() - base.h] {
                    candidates.push(Rect { x, y, ..base });
                }
            }
            let cost = |candidate: Rect| {
                let mut collisions = 0.0;
                for other in &layout.nodes {
                    collisions +=
                        overlap(candidate, other.rect) / (candidate.w * candidate.h).max(1.0);
                }
                for (j, rect) in rects.iter().enumerate() {
                    if i != j
                        && let Some(rect) = rect
                    {
                        collisions +=
                            overlap(candidate, *rect) / (candidate.w * candidate.h).max(1.0);
                    }
                }
                // A route already bends around its caption; also reserve its
                // natural departure/arrival so text cannot cause that detour.
                for edge in &layout.edges {
                    let (other, side) = if edge.from == i {
                        (edge.to, edge.style.from_side)
                    } else if edge.to == i {
                        (edge.from, edge.style.to_side)
                    } else {
                        continue;
                    };
                    let target = layout.nodes[other].rect;
                    let (dx, dy) = (target.cx() - r.cx(), target.cy() - r.cy());
                    use crate::style::Side;
                    let side = side.unwrap_or_else(|| {
                        if node.style.stage.as_deref() == Some("boundary") {
                            Side::Bottom
                        } else if dx.abs() >= dy.abs() {
                            if dx >= 0.0 { Side::Right } else { Side::Left }
                        } else if dy >= 0.0 {
                            Side::Bottom
                        } else {
                            Side::Top
                        }
                    });
                    let reach = 48.0 + candidate.w.max(candidate.h);
                    let tip = match side {
                        Side::Top => (r.cx(), r.y - reach),
                        Side::Bottom => (r.cx(), r.bottom() + reach),
                        Side::Left => (r.x - reach, r.cy()),
                        Side::Right => (r.right() + reach, r.cy()),
                    };
                    if crate::render_svg::segment_hits((r.cx(), r.cy()), tip, &candidate) {
                        collisions += 2.0;
                    }
                }
                for route in &routed {
                    for pair in route.points.windows(2) {
                        if crate::render_svg::segment_hits(pair[0], pair[1], &candidate) {
                            collisions += 1.0;
                        }
                    }
                    if let Some((_, rect)) = &route.label {
                        collisions +=
                            overlap(candidate, *rect) / (candidate.w * candidate.h).max(1.0);
                    }
                }
                for text in layout.annotations[..caption_annotations.start]
                    .iter()
                    .chain(&layout.annotations[caption_annotations.end..])
                {
                    collisions += overlap(candidate, annotation_rect(text))
                        / (candidate.w * candidate.h).max(1.0);
                }
                for group in &layout.groups {
                    let r = group.rect;
                    for (a, b) in [
                        ((r.x, r.y), (r.right(), r.y)),
                        ((r.right(), r.y), (r.right(), r.bottom())),
                        ((r.right(), r.bottom()), (r.x, r.bottom())),
                        ((r.x, r.bottom()), (r.x, r.y)),
                    ] {
                        if crate::render_svg::segment_hits(a, b, &candidate) {
                            collisions += 1.0;
                        }
                    }
                    if let Some(title) = group_title_rect(group) {
                        collisions +=
                            overlap(candidate, title) / (candidate.w * candidate.h).max(1.0);
                    }
                }
                let distance = (candidate.x - base.x).abs() + (candidate.y - base.y).abs();
                (collisions, distance)
            };
            let mut best = (cost(current), current);
            for candidate in candidates {
                let score = cost(candidate);
                if score < best.0 {
                    best = (score, candidate);
                }
            }
            rects[i] = Some(best.1);
            offsets[i] = (best.1.x - base.x, best.1.y - base.y);
        }
    }
    offsets
}
