//! `type bpmn` — the marks that make a process diagram BPMN rather than a
//! flowchart.
//!
//! The boxes, the diamonds and the rings are ordinary shapes the renderer
//! already draws, and the anchors place them exactly as in `free` — this module
//! is only what goes *inside* and *beside* them: the glyph in an event's ring,
//! the mark in a gateway's diamond, the marker in a task's corner, the caption
//! under a shape too small to hold one, and the boundary events that sit on a
//! task's edge.
//!
//! **Everything here is drawn from line segments.** The obvious alternative is
//! a Unicode character — `✉`, `⏱`, `✕` — and it fails for the same reason
//! `X-005` cannot be closed: an SVG names a font and the reader's machine
//! decides what it resolves to, so a glyph that is a clock here is a box
//! there. A polyline is the same shape on every machine, which for a notation
//! whose entire value is that the mark is unambiguous is not a detail.
//!
//! What is deliberately **not** here: a marker so fussy it stops reading at
//! sixteen pixels. BPMN's own spec draws a hand for a manual task and a gear
//! for a service one; at this size a gear is a smudge, so a service task gets a
//! ring of spokes that reads as machinery and a manual one gets a hand's
//! fingers. A mark a reader cannot identify is worse than a plain box, because
//! the plain box does not claim to say anything.

use crate::layout::{Anchor, Annotation, PlacedNode, Polyline, Rect};

/// Which of the four things a statement declared.
///
/// Read from the resolved properties, including the written form's builtin
/// defaults. `node` is the universal synonym for the primary task form.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Element {
    /// A box: something a person or a machine does.
    Task,
    /// A ring: something that happens.
    Event,
    /// A diamond: a branch or a join.
    Gateway,
    /// A page or a cylinder: what the process reads or writes.
    Data,
}

/// Which element a resolved style describes.
pub fn element_of(style: &crate::style::Style) -> Element {
    if style.stage.is_some() {
        Element::Event
    } else if style.gate.is_some() {
        Element::Gateway
    } else if style.artifact.is_some() {
        Element::Data
    } else {
        Element::Task
    }
}

/// An event's ring, across.
///
/// Fixed, because in BPMN it is: a bigger circle does not mean a bigger event,
/// and sizing one from its label would turn two lengths of text into two
/// importances. It is why an event's label is drawn underneath it.
pub const EVENT: f64 = 40.0;

/// A gateway's diamond, across. Fixed for the same reason as [`EVENT`].
pub const GATE: f64 = 48.0;

/// Resolve boundary relations before geometry; styles and inline properties share one cascade.
pub(crate) fn boundary_attachments(
    diagram: &crate::ast::Diagram,
) -> Vec<Option<crate::placement::BoundaryAttachment>> {
    let mut out = vec![None; diagram.nodes.len()];
    if !diagram
        .archetype
        .as_ref()
        .is_some_and(|kind| kind.as_str() == "bpmn")
    {
        return out;
    }
    let Some(def) = crate::props::ARCHETYPES
        .iter()
        .find(|def| def.name == "bpmn")
    else {
        return out;
    };
    let Ok(registry) = crate::style::Registry::build(&diagram.styles) else {
        return out;
    };
    let mut wanted = Vec::new();
    for (event, node) in diagram.nodes.iter().enumerate() {
        let props = node.props_for(def);
        let implicit = def.style_word(node.keyword.as_str());
        let Ok(style) = registry.resolve_node(crate::style::Style::node_defaults(), def, node)
        else {
            continue;
        };
        if style.stage.as_deref() != Some("boundary") {
            continue;
        }
        let Some(host) = style.attach.as_deref().and_then(|name| {
            diagram
                .nodes
                .iter()
                .position(|node| node.id.as_str() == name)
        }) else {
            continue;
        };
        let mut span = node.id.span;
        for name in registry
            .implicit_layers(implicit)
            .map(|(name, _)| name)
            .chain(node.classes.iter().map(|class| class.as_str()))
        {
            let mut lineage = Vec::new();
            let mut cursor = Some(name);
            while let Some(name) = cursor {
                let Some(declaration) = diagram
                    .styles
                    .iter()
                    .find(|declaration| declaration.name.as_str() == name)
                else {
                    break;
                };
                lineage.push(declaration);
                cursor = declaration.extends.as_ref().map(|name| name.as_str());
            }
            for declaration in lineage.into_iter().rev() {
                if let Some(prop) = declaration
                    .props
                    .iter()
                    .rev()
                    .find(|prop| prop.name == "attach")
                {
                    span = prop.span;
                }
            }
        }
        if let Some(prop) = props.iter().rev().find(|prop| prop.name == "attach") {
            span = prop.span;
        }
        wanted.push((event, host, span));
    }
    for (event, host, span) in &wanted {
        let count = wanted.iter().filter(|(_, other, _)| other == host).count();
        let order = wanted
            .iter()
            .take_while(|(other, _, _)| other != event)
            .filter(|(_, other, _)| other == host)
            .count();
        out[*event] = Some(crate::placement::BoundaryAttachment {
            host: *host,
            along: (order + 1) as f64 / (count + 1) as f64,
            span: *span,
        });
    }
    out
}

/// The glyphs, the rings and the captions, for a whole diagram.
///
/// Captions come in beside the nodes rather than out of them: an event and a
/// gateway are sized by the notation and not by their text, so the label cannot
/// go inside and is drawn underneath.
pub fn decorate(
    nodes: &[PlacedNode],
    captions: &[Option<String>],
) -> (Vec<Polyline>, Vec<Annotation>) {
    let mut lines = Vec::new();
    let mut text = Vec::new();

    for (i, node) in nodes.iter().enumerate() {
        let s = &node.style;
        let r = node.rect;
        let ink = s.stroke.clone().unwrap_or_else(|| "#1f2937".into());
        // The mark is drawn in the shape's own outline colour, so a class
        // that recolours a box recolours what is written inside it. Callers say
        // whether a shape is filled; only this closure knows with what.
        let mut draw = |points: Vec<(f64, f64)>, closed: bool, filled: bool| {
            lines.push(Polyline {
                points,
                closed,
                fill: filled.then(|| ink.clone()),
                stroke: Some(ink.clone()),
                width: 1.3,
                head: None,
                dash: None,
                // Inside a shape, so after it.
                over: true,
            });
        };

        match element_of(s) {
            Element::Event => {
                // An intermediate event is a *double* ring — the one piece of
                // BPMN notation that is a second outline rather than a mark,
                // so it is drawn here and not chosen as a shape.
                if s.stage.as_deref() == Some("intermediate")
                    || s.stage.as_deref() == Some("boundary")
                {
                    draw(circle(r.cx(), r.cy(), r.w / 2.0 - 4.0), true, false);
                }
                if let Some(t) = s.trigger.as_deref() {
                    trigger(&mut draw, t, r.cx(), r.cy(), 15.0);
                }
            }
            Element::Gateway => {
                if let Some(g) = s.gate.as_deref() {
                    gate(&mut draw, g, r.cx(), r.cy(), 15.0);
                }
            }
            Element::Data => {
                if s.artifact.as_deref() != Some("store") {
                    // The folded corner is what makes a rectangle a document.
                    let f = 12.0_f64.min(r.w / 3.0).min(r.h / 3.0);
                    draw(
                        vec![
                            (r.right() - f, r.y),
                            (r.right() - f, r.y + f),
                            (r.right(), r.y + f),
                        ],
                        false,
                        false,
                    );
                }
                if let Some(way) = s.artifact.as_deref()
                    && matches!(way, "input" | "output")
                {
                    // Which way the data went, as the small arrow BPMN puts in
                    // the corner: hollow for an input, filled for an output.
                    let (x, y) = (r.x + 8.0, r.y + 9.0);
                    draw(
                        vec![
                            (x, y + 3.0),
                            (x + 5.0, y + 3.0),
                            (x + 5.0, y + 6.0),
                            (x + 10.0, y),
                            (x + 5.0, y - 6.0),
                            (x + 5.0, y - 3.0),
                            (x, y - 3.0),
                        ],
                        true,
                        way == "output",
                    );
                }
            }
            Element::Task => {
                if let Some(k) = s.task_kind.as_deref() {
                    task_marker(&mut draw, k, r);
                }
            }
        }

        if let Some(caption) = captions.get(i).and_then(|c| c.clone()) {
            let size = s.font_size.unwrap_or(14.0) * 0.85;
            let bounds = crate::labels::caption_rect(r, &caption, s);
            for (line, part) in caption.split('\n').enumerate() {
                text.push(Annotation {
                    text: part.trim().to_string(),
                    x: bounds.cx(),
                    y: bounds.y + 3.0 + size * 0.6 + line as f64 * size * 1.3,
                    anchor: Anchor::Middle,
                    size,
                    color: s.font_color.clone().unwrap_or_else(|| "#1f2937".into()),
                    weight: "500".into(),
                    family: s.font_family.clone().unwrap_or_default(),
                    title: false,
                });
            }
        }
    }

    (lines, text)
}

/// A circle as a polyline. Sampled, like the self-loop, so it goes through the
/// same stroke and dash machinery as everything else.
fn circle(cx: f64, cy: f64, r: f64) -> Vec<(f64, f64)> {
    const STEPS: usize = 32;
    (0..STEPS)
        .map(|i| {
            let t = std::f64::consts::TAU * i as f64 / STEPS as f64;
            (cx + r * crate::math::cos(t), cy + r * crate::math::sin(t))
        })
        .collect()
}

/// The mark inside an event's ring.
fn trigger<F>(draw: &mut F, kind: &str, cx: f64, cy: f64, d: f64)
where
    F: FnMut(Vec<(f64, f64)>, bool, bool),
{
    let (w, h) = (d / 2.0, d / 2.6);
    match kind {
        "message" => {
            // An envelope, and the flap: without the flap it is a rectangle.
            draw(
                vec![
                    (cx - w, cy - h),
                    (cx + w, cy - h),
                    (cx + w, cy + h),
                    (cx - w, cy + h),
                ],
                true,
                false,
            );
            draw(
                vec![(cx - w, cy - h), (cx, cy + 1.0), (cx + w, cy - h)],
                false,
                false,
            );
        }
        "timer" => {
            draw(circle(cx, cy, d / 2.0), true, false);
            // Hands at ten past eleven, which is where a clock face is drawn
            // when it is drawn as a symbol rather than read as a time.
            draw(vec![(cx, cy), (cx, cy - d / 2.6)], false, false);
            draw(vec![(cx, cy), (cx + d / 3.4, cy + d / 6.0)], false, false);
        }
        "error" => draw(
            vec![
                (cx - w, cy + h),
                (cx - w / 4.0, cy - h),
                (cx + w / 6.0, cy + h / 3.0),
                (cx + w, cy - h),
            ],
            false,
            false,
        ),
        "signal" => draw(
            vec![(cx, cy - d / 2.0), (cx + w, cy + h), (cx - w, cy + h)],
            true,
            false,
        ),
        "escalation" => draw(
            vec![
                (cx, cy - d / 2.0),
                (cx + w, cy + h),
                (cx, cy),
                (cx - w, cy + h),
            ],
            true,
            false,
        ),
        "conditional" => {
            draw(
                vec![
                    (cx - w, cy - h),
                    (cx + w, cy - h),
                    (cx + w, cy + h),
                    (cx - w, cy + h),
                ],
                true,
                false,
            );
            for k in 0..3 {
                let y = cy - h + h * 0.55 * (k as f64 + 1.0);
                draw(vec![(cx - w * 0.6, y), (cx + w * 0.6, y)], false, false);
            }
        }
        // A terminate event is a solid disc: the process stops, and the mark
        // says so by being the only filled one in the set.
        "terminate" => draw(circle(cx, cy, d / 2.4), true, true),
        _ => {}
    }
}

/// The mark inside a gateway's diamond.
fn gate<F>(draw: &mut F, kind: &str, cx: f64, cy: f64, d: f64)
where
    F: FnMut(Vec<(f64, f64)>, bool, bool),
{
    let a = d / 2.6;
    match kind {
        "exclusive" => {
            draw(vec![(cx - a, cy - a), (cx + a, cy + a)], false, false);
            draw(vec![(cx - a, cy + a), (cx + a, cy - a)], false, false);
        }
        "parallel" => {
            draw(vec![(cx, cy - d / 2.0), (cx, cy + d / 2.0)], false, false);
            draw(vec![(cx - d / 2.0, cy), (cx + d / 2.0, cy)], false, false);
        }
        "inclusive" => draw(circle(cx, cy, d / 2.4), true, false),
        // An event-based gateway is a ring around the pentagon that means
        // "whichever happens first"; the double ring is the part a reader uses
        // to tell it from an inclusive one.
        "event" => {
            draw(circle(cx, cy, d / 2.0), true, false);
            draw(circle(cx, cy, d / 2.9), true, false);
        }
        "complex" => {
            draw(vec![(cx, cy - d / 2.0), (cx, cy + d / 2.0)], false, false);
            draw(vec![(cx - d / 2.0, cy), (cx + d / 2.0, cy)], false, false);
            draw(vec![(cx - a, cy - a), (cx + a, cy + a)], false, false);
            draw(vec![(cx - a, cy + a), (cx + a, cy - a)], false, false);
        }
        _ => {}
    }
}

/// The marker in a task's top-left corner, and the `+` a collapsed subprocess
/// takes at the bottom instead.
fn task_marker<F>(draw: &mut F, kind: &str, r: Rect)
where
    F: FnMut(Vec<(f64, f64)>, bool, bool),
{
    if kind == "subprocess" {
        // Bottom centre, in a box, which is how a reader tells it from a
        // parallel gateway's plain cross.
        let (cx, cy, s) = (r.cx(), r.bottom() - 10.0, 5.0);
        draw(
            vec![
                (cx - s, cy - s),
                (cx + s, cy - s),
                (cx + s, cy + s),
                (cx - s, cy + s),
            ],
            true,
            false,
        );
        draw(vec![(cx, cy - s * 0.6), (cx, cy + s * 0.6)], false, false);
        draw(vec![(cx - s * 0.6, cy), (cx + s * 0.6, cy)], false, false);
        return;
    }

    let (x, y) = (r.x + 7.0, r.y + 7.0);
    let (w, h) = (11.0, 11.0);
    match kind {
        "user" => {
            // A head and a pair of shoulders. Two strokes, and it still reads
            // as a person, which a five-stroke figure does not at this size.
            draw(circle(x + w / 2.0, y + h * 0.28, h * 0.26), true, false);
            draw(
                vec![
                    (x, y + h),
                    (x, y + h * 0.75),
                    (x + w * 0.3, y + h * 0.55),
                    (x + w * 0.7, y + h * 0.55),
                    (x + w, y + h * 0.75),
                    (x + w, y + h),
                ],
                false,
                false,
            );
        }
        "service" => {
            // A gear at eleven pixels is a smudge, so this is a ring with
            // spokes: it reads as machinery, which is what the marker is for.
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            draw(circle(cx, cy, h * 0.28), true, false);
            for k in 0..6 {
                let t = std::f64::consts::TAU * k as f64 / 6.0;
                draw(
                    vec![
                        (
                            cx + h * 0.30 * crate::math::cos(t),
                            cy + h * 0.30 * crate::math::sin(t),
                        ),
                        (
                            cx + h * 0.48 * crate::math::cos(t),
                            cy + h * 0.48 * crate::math::sin(t),
                        ),
                    ],
                    false,
                    false,
                );
            }
        }
        "script" => {
            draw(
                vec![
                    (x + w * 0.2, y),
                    (x + w * 0.8, y),
                    (x + w * 0.8, y + h),
                    (x + w * 0.2, y + h),
                ],
                true,
                false,
            );
            for k in 0..3 {
                let ly = y + h * (0.25 + 0.25 * k as f64);
                draw(vec![(x + w * 0.34, ly), (x + w * 0.66, ly)], false, false);
            }
        }
        "manual" => {
            // A hand: a palm and three fingers. Enough to be a hand, few
            // enough strokes to stay one.
            draw(
                vec![
                    (x, y + h * 0.55),
                    (x + w * 0.45, y + h * 0.55),
                    (x + w * 0.45, y + h),
                ],
                false,
                false,
            );
            for k in 0..3 {
                let fx = x + w * (0.45 + 0.18 * k as f64);
                draw(vec![(fx, y + h * 0.55), (fx, y + h * 0.15)], false, false);
            }
        }
        "send" | "receive" => {
            // BPMN fills the whole envelope for a send and leaves it open for
            // a receive. At eleven pixels a filled envelope is a black smudge
            // with no envelope left in it, so the *flap* carries the fill
            // instead: the outline stays an envelope either way, and the two
            // are still told apart at a glance.
            draw(
                vec![
                    (x, y + h * 0.2),
                    (x + w, y + h * 0.2),
                    (x + w, y + h * 0.8),
                    (x, y + h * 0.8),
                ],
                true,
                false,
            );
            draw(
                vec![
                    (x, y + h * 0.2),
                    (x + w / 2.0, y + h * 0.55),
                    (x + w, y + h * 0.2),
                ],
                kind == "send",
                kind == "send",
            );
        }
        "rule" => {
            draw(
                vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
                true,
                false,
            );
            draw(vec![(x, y + h * 0.3), (x + w, y + h * 0.3)], false, false);
            draw(vec![(x, y + h * 0.65), (x + w, y + h * 0.65)], false, false);
        }
        _ => {}
    }
}
