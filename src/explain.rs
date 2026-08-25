//! `bgraph explain diagram.bd api` — why that box is where it is.
//!
//! When a diagram looks wrong there was no way to ask why. `D-001` makes
//! placement the author's job, so a declared layout deserves a declared
//! explanation: every answer here is a *statement in the file* or a *default
//! that filled in for one*, and both are named.
//!
//! # Why this needs nothing from the layout
//!
//! The obvious design gives the solver a provenance trail. It is not needed:
//! the placement **is** declared, so the reason a box sits where it does is
//! readable from the source, and the only thing the layout is asked for is the
//! rectangle it arrived at. That keeps `layout.rs` untouched and keeps this
//! honest — an explanation derived from the same text the author wrote cannot
//! drift from it.
//!
//! Paint is different, because the cascade really does have layers. So the
//! cascade is **re-run one layer at a time** and each field is attributed to
//! the last layer that changed it.

use anyhow::{Result, bail};

use crate::ast::{Align, Diagram, Placement, RelDir};
use crate::layout::Layout;
use crate::placement::{EffectivePlacement, PlacementPlan};
use crate::style::{GraphOpts, Registry, Style};

/// Everything known about one box, as lines ready to print.
pub fn explain(diagram: &Diagram, layout: &Layout, id: &str) -> Result<String> {
    let Some((index, node)) = diagram
        .nodes
        .iter()
        .enumerate()
        .find(|(_, n)| n.id.as_str() == id)
    else {
        let known: Vec<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
        bail!(
            "no box called `{id}`{}",
            if known.is_empty() {
                String::new()
            } else {
                format!(" — this diagram has: {}", known.join(", "))
            }
        );
    };

    let opts = GraphOpts::from_props(&diagram.graph)?;
    let mut out = String::new();

    if let Some(placed) = layout.nodes.get(index) {
        out.push_str(&format!(
            "{id} — at {}, {} · {}×{}\n",
            round(placed.rect.x),
            round(placed.rect.y),
            round(placed.rect.w),
            round(placed.rect.h)
        ));
    } else {
        out.push_str(&format!("{id}\n"));
    }

    out.push_str("\nplacement\n");
    let placement = PlacementPlan::new(diagram);
    match placement.node(index, opts.flow) {
        EffectivePlacement::Declared(Placement::At { x, y }) => {
            out.push_str(&format!(
                "  at ({}, {})  — declared\n",
                round(*x),
                round(*y)
            ));
        }
        EffectivePlacement::Declared(Placement::Cell { col, row }) => {
            out.push_str(&format!(
                "  at (col {}, row {})  — declared\n",
                round(*col),
                round(*row)
            ));
            match opts.grid {
                Some((x, y)) => out.push_str(&format!(
                    "  grid {} × {}  — from `graph {{ grid }}`\n",
                    round(x),
                    round(y)
                )),
                // Worth saying loudly: a cell with no grid puts everything on
                // the origin, and that is a question nobody thinks to ask.
                None => out.push_str("  grid — NOT DECLARED, so a cell is one point\n"),
            }
        }
        EffectivePlacement::Declared(Placement::Rel {
            dir,
            anchor,
            gap,
            align,
        }) => {
            out.push_str(&format!(
                "  {} {}  — declared\n",
                match dir {
                    RelDir::RightOf => "right-of",
                    RelDir::LeftOf => "left-of",
                    RelDir::Below => "below",
                    RelDir::Above => "above",
                },
                anchor.as_str()
            ));
            match gap {
                Some(g) => out.push_str(&format!("  gap {}  — declared\n", round(*g))),
                None => out.push_str(&format!(
                    "  gap {}  — default, from `graph {{ gap }}`\n",
                    round(opts.gap)
                )),
            }
            match align {
                Some(a) => out.push_str(&format!(
                    "  align {}  — declared\n",
                    cross_axis_align_name(*a, *dir)
                )),
                None => out.push_str("  align center  — default\n"),
            }
        }
        EffectivePlacement::Flow { dir, anchor } => {
            // The rule an author most often forgets, and the one that most
            // often explains a surprise.
            out.push_str(&format!(
                "  none — flows {} from sibling `{}`\n",
                match dir {
                    RelDir::RightOf => "right",
                    RelDir::LeftOf => "left",
                    RelDir::Below => "down",
                    RelDir::Above => "up",
                },
                anchor.as_str()
            ));
        }
        EffectivePlacement::Series { dir, anchor, gap } => {
            out.push_str(&format!(
                "  {} {}  — from declared {}\n",
                match dir {
                    RelDir::RightOf => "right-of",
                    RelDir::LeftOf => "left-of",
                    RelDir::Below => "below",
                    RelDir::Above => "above",
                },
                anchor.as_str(),
                if dir == RelDir::RightOf {
                    "row"
                } else {
                    "column"
                },
            ));
            match gap {
                Some(g) => out.push_str(&format!("  gap {}  — declared\n", round(g))),
                None => out.push_str(&format!(
                    "  gap {}  — default, from `graph {{ gap }}`\n",
                    round(opts.gap)
                )),
            }
        }
        EffectivePlacement::Origin => {
            out.push_str("  none — the first box in its container, so it sits at its origin\n");
        }
    }

    // Constraints are the other thing that moves a box, and they are easy to
    // forget three screens away from the statement they act on.
    for constraint_ref in placement.constraints() {
        let constraint = constraint_ref.declaration();
        if constraint.members.iter().any(|m| m.as_str() == id) {
            let relation = match constraint.kind {
                crate::ast::ConstraintKind::AlignGroup => format!(
                    "align {} — aligns {}",
                    constraint_align_name(constraint),
                    constraint_axis_name(constraint.axis),
                ),
                crate::ast::ConstraintKind::SameWidth => "equal-width".to_string(),
                crate::ast::ConstraintKind::SameHeight => "equal-height".to_string(),
            };
            out.push_str(&format!(
                "  {}  — with {}\n",
                relation,
                constraint
                    .members
                    .iter()
                    .filter(|m| m.as_str() != id)
                    .map(|m| m.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    out.push_str("\npaint\n");
    out.push_str(&paint(diagram, node, id)?);
    Ok(out)
}

/// Which layer of the cascade settled each colour.
///
/// Re-run rather than recorded: the registry resolves in one pass, so this
/// walks the same layers again and attributes a field to the last one that
/// changed it. Doing it any other way would mean the resolver kept a trail
/// nothing else needs.
fn paint(diagram: &Diagram, node: &crate::ast::NodeDecl, id: &str) -> Result<String> {
    let registry = Registry::build(&diagram.styles)?;
    let def = crate::check::archetype_of(diagram);

    let mut current = Style::default();
    let mut source: Vec<(&'static str, String)> = Vec::new();
    let note = |current: &Style, before: &Style, from: String, out: &mut Vec<_>| {
        for (name, now, was) in [
            ("fill", &current.fill, &before.fill),
            ("stroke", &current.stroke, &before.stroke),
            ("font colour", &current.font_color, &before.font_color),
        ] {
            if now != was && now.is_some() {
                out.retain(|(field, _): &(&str, String)| *field != name);
                out.push((name, format!("{}  — from {from}", now.clone().unwrap())));
            }
        }
    };

    let layers: Vec<(String, Option<&Style>)> =
        std::iter::once((format!("`style {}`", def.item), registry.get(def.item)))
            .chain(
                node.classes
                    .iter()
                    .map(|c| (format!("`.{}`", c.as_str()), registry.get(c.as_str()))),
            )
            .collect();

    for (from, layer) in layers {
        let Some(layer) = layer else { continue };
        let before = current.clone();
        current.overlay(layer);
        note(&current, &before, from, &mut source);
    }

    let before = current.clone();
    current.overlay(&crate::style::layer_from_props(&node.props)?);
    note(
        &current,
        &before,
        format!("`{id}`'s own block"),
        &mut source,
    );

    if source.is_empty() {
        return Ok("  nothing declared — the renderer's own defaults\n".to_string());
    }
    Ok(source
        .iter()
        .map(|(field, what)| format!("  {field:<12}{what}\n"))
        .collect())
}

fn cross_axis_align_name(align: Align, dir: RelDir) -> &'static str {
    match dir {
        RelDir::RightOf | RelDir::LeftOf => match align {
            Align::Start => "top",
            Align::Center => "center",
            Align::End => "bottom",
        },
        RelDir::Below | RelDir::Above => match align {
            Align::Start => "left",
            Align::Center => "center",
            Align::End => "right",
        },
    }
}

fn constraint_align_name(constraint: &crate::ast::ConstraintDecl) -> &'static str {
    use crate::ast::ConstraintAxis;

    match constraint.axis {
        ConstraintAxis::Horizontal => match constraint.align {
            Align::Start => "left",
            Align::Center => "center",
            Align::End => "right",
        },
        ConstraintAxis::Vertical => match constraint.align {
            Align::Start => "top",
            Align::Center => "center",
            Align::End => "bottom",
        },
        // The parser constructs only `center` on this path. Keep the
        // explanation total if an AST is assembled by a library caller.
        ConstraintAxis::Inferred => match constraint.align {
            Align::Start => "start",
            Align::Center => "center",
            Align::End => "end",
        },
    }
}

fn constraint_axis_name(axis: crate::ast::ConstraintAxis) -> &'static str {
    match axis {
        crate::ast::ConstraintAxis::Horizontal => "x",
        crate::ast::ConstraintAxis::Vertical => "y",
        crate::ast::ConstraintAxis::Inferred => "an axis inferred by legacy v1",
    }
}

fn round(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v:.1}")
    }
}
