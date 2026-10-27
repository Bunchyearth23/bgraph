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
            // A declared offset is the author's own placement and always wins,
            // zero included. A declared *side* says which side the caption is
            // on, not which point of that side, so such a caption may still
            // step along it to clear a flow.
            if pinned[i] && node.style.label_offset.is_some() {
                continue;
            }
            let side = node.style.label_position.as_deref();
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
            // A boundary event hangs half over the edge of the task it is
            // attached to, so "beside the event, bottom-aligned" puts the
            // caption's top a hair *above* that task's edge and the caption
            // is then reported as overlapping it. The host's edge is the
            // anchor that clears the box.
            if let Some(host) = (node.style.stage.as_deref() == Some("boundary"))
                .then_some(node.style.attach.as_deref())
                .flatten()
                .and_then(|host| layout.nodes.iter().find(|n| n.id == host))
            {
                for x in [r.right() + gap, r.x - gap - base.w] {
                    candidates.push(Rect {
                        y: host.rect.bottom(),
                        x,
                        ..base
                    });
                }
            }
            // A caption pinned to a side keeps that side: only the point along
            // it is the engine's to choose.
            if let Some(side) = side {
                candidates.retain(|candidate| on_side(*candidate, r, side));
            }
            let cost = |candidate: Rect| {
                let mut collisions = 0.0;
                // The part of the cost that is a *flow* rather than a box: a
                // caption drawn over a wire is a defect, while a caption whose
                // padded box grazes a box is a warning.
                let mut wire = 0.0;
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
                            wire += 1.0;
                        }
                    }
                    if let Some((_, rect)) = &route.label {
                        let hit = overlap(candidate, *rect) / (candidate.w * candidate.h).max(1.0);
                        collisions += hit;
                        wire += hit;
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
                (collisions, wire, distance)
            };
            let mut best = (cost(current), current);
            for candidate in candidates {
                let score = cost(candidate);
                if (score.0, score.2) < (best.0.0, best.0.2) {
                    best = (score, candidate);
                }
            }
            // A caption drawn over a flow is a defect, not a warning, so it
            // gets a bounded chance to step along its own side until the
            // wire is clear. The side is the author's and does not move.
            if best.0.1 > 0.0
                && let Some(side) = side
            {
                for candidate in slide_positions(base, side) {
                    let score = cost(candidate);
                    if (score.0, score.2) < (best.0.0, best.0.2) {
                        best = (score, candidate);
                    }
                }
            }
            rects[i] = Some(best.1);
            offsets[i] = (best.1.x - base.x, best.1.y - base.y);
        }
    }
    offsets
}

/// Whether a caption still sits on the side its author pinned it to.
#[cfg(feature = "bpmn")]
fn on_side(candidate: Rect, shape: Rect, side: &str) -> bool {
    match side {
        "top" => candidate.bottom() <= shape.y,
        "bottom" => candidate.y >= shape.bottom(),
        "left" => candidate.right() <= shape.x,
        "right" => candidate.x >= shape.right(),
        _ => true,
    }
}

/// The places a caption may try when it is drawn over a flow: the same
/// place, stepped along the axis that keeps it on the side it is on.
///
/// The step is half the caption's own depth plus its clearance, so one step
/// lifts the text clear of a wire that runs through its middle.
#[cfg(feature = "bpmn")]
fn slide_positions(base: Rect, side: &str) -> Vec<Rect> {
    let across = matches!(side, "top" | "bottom");
    let step = if across { base.w / 2.0 } else { base.h / 2.0 } + 3.0;
    (1..=6)
        .flat_map(|k| {
            let d = step * k as f64;
            [
                if across {
                    Rect {
                        x: base.x - d,
                        ..base
                    }
                } else {
                    Rect {
                        y: base.y - d,
                        ..base
                    }
                },
                if across {
                    Rect {
                        x: base.x + d,
                        ..base
                    }
                } else {
                    Rect {
                        y: base.y + d,
                        ..base
                    }
                },
            ]
        })
        .collect()
}

#[cfg(all(test, feature = "bpmn"))]
mod tests {
    use crate::layout::Rect;

    /// The caption of a declaration, as the engine measures it: the padded
    /// box the validator compares against the drawing.
    fn caption_of(analysis: &crate::SourceAnalysis, id: &str) -> (Rect, String) {
        let declared = analysis
            .diagram
            .as_ref()
            .and_then(|diagram| diagram.nodes.iter().find(|node| node.id.as_str() == id))
            .expect("the declaration");
        let text = declared.label.as_deref().expect("a caption").to_string();
        let placed = analysis
            .layout
            .as_ref()
            .and_then(|layout| layout.nodes.iter().find(|node| node.id == id))
            .expect("the placed node");
        (
            crate::labels::caption_rect(placed.rect, &text, &placed.style),
            text,
        )
    }

    fn rect_of(analysis: &crate::SourceAnalysis, id: &str) -> Rect {
        analysis
            .layout
            .as_ref()
            .and_then(|layout| layout.nodes.iter().find(|node| node.id == id))
            .expect("the placed node")
            .rect
    }

    fn reports(analysis: &crate::SourceAnalysis, id: &str) -> bool {
        analysis
            .diagnostics
            .iter()
            .any(|d| d.message.contains(&format!("the caption of `{id}`")))
    }

    /// A boundary event hangs half over the edge of the task it is attached
    /// to, so the caption beside it has to be anchored on the *task's* edge.
    const BOUNDARY: &str = r#"
type bpmn { title: "boundary" }
style task { size: 176 74 }
group w "Worker" {
  task t "Lancer" service
  task r "Relance" service
  task top "Acquitter" send
  layout { place t at (0, 120)
    place r at (210, 240)
    place top at (0, 0) }
}
group m "Modules" {
  task c "Capture" service
  layout { place c at (0, 0) }
}
event e "Mort" boundary error { attach: t }
layout { place w at (0, 0)
  place m below w gap 60 align left }
t -> top
e -> r
t -> c message
"#;

    #[test]
    fn a_boundary_event_caption_clears_the_task_it_hangs_on() {
        let analysis = crate::analyze_source(BOUNDARY);
        assert!(
            !reports(&analysis, "e"),
            "the caption of a boundary event must not overlap the task it hangs on: {:?}",
            analysis.diagnostics
        );
        let (caption, _) = caption_of(&analysis, "e");
        let task = rect_of(&analysis, "t");
        assert!(
            caption.y >= task.bottom() - 1e-7,
            "the caption {caption:?} reaches into the task {task:?}"
        );
        assert!(
            crate::labels::overlap(caption, task) <= 1e-7,
            "the caption {caption:?} still covers the task {task:?}"
        );
    }

    #[test]
    fn a_pinned_boundary_caption_still_reports_its_collision() {
        let source = BOUNDARY.replace("{ attach: t }", "{ attach: t label-offset: 0 0 }");
        let analysis = crate::analyze_source(&source);
        assert!(
            reports(&analysis, "e"),
            "a caption the author pinned must still be reported: {:?}",
            analysis.diagnostics
        );
    }

    /// A caption pinned to a side may step along that side, but the side is
    /// the author's and does not move.
    const SIDE: &str = r#"
type bpmn
style event { label-position: right }
event got "Message reçu" start message
task next "Suite"
layout {
  place got at (0, 0)
  place next right-of got gap 150
}
got -> next
"#;

    #[test]
    fn a_caption_pinned_to_a_side_steps_along_it_to_clear_the_flow() {
        let analysis = crate::analyze_source(SIDE);
        assert!(
            !reports(&analysis, "got"),
            "a caption drawn over a flow must be reported, and then stepped clear: {:?}",
            analysis.diagnostics
        );
        let (caption, _) = caption_of(&analysis, "got");
        let event = rect_of(&analysis, "got");
        assert!(
            caption.x >= event.right(),
            "the caption left the side its author pinned: {caption:?} against {event:?}"
        );
        let layout = analysis.layout.as_ref().expect("the layout");
        for route in crate::render_svg::route_all(layout) {
            for pair in route.points.windows(2) {
                assert!(
                    !crate::render_svg::segment_hits(pair[0], pair[1], &caption),
                    "the caption {caption:?} is still drawn over a flow"
                );
            }
        }
    }

    #[test]
    fn a_pinned_caption_offset_wins_over_the_step_along_the_side() {
        let source = SIDE.replace(
            "event got \"Message reçu\" start message",
            "event got \"Message reçu\" start message { label-offset: 0 0 }",
        );
        let analysis = crate::analyze_source(&source);
        let (caption, _) = caption_of(&analysis, "got");
        let event = rect_of(&analysis, "got");
        assert!(
            (caption.x - (event.right() + 6.0)).abs() < 1e-7
                && (caption.cy() - event.cy()).abs() < 1e-7,
            "a declared offset must not be overridden: {caption:?} against {event:?}"
        );
        assert!(
            reports(&analysis, "got"),
            "a pinned caption that still covers a flow must be reported: {:?}",
            analysis.diagnostics
        );
    }
}
