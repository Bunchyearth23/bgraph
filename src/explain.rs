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
//! the last layer that declared it.

use anyhow::{Result, bail};

use crate::ast::{Align, Diagram, Placement, RelDir};
use crate::layout::Layout;
use crate::placement::{EffectivePlacement, PlacementPlan};
use crate::style::{GraphOpts, Registry, Style};

pub(crate) mod inspect;

/// Everything known about one box, as lines ready to print.
pub fn explain(diagram: &Diagram, layout: &Layout, id: &str) -> Result<String> {
    if let Some(index) = diagram
        .groups
        .iter()
        .position(|group| group.id.as_str() == id)
    {
        return explain_group(diagram, layout, index);
    }
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

    if let Some(placed) = layout.nodes.iter().find(|placed| placed.id == id) {
        out.push_str(&format!(
            "{id} — at {}, {} · {}×{}\n",
            round(placed.rect.x),
            round(placed.rect.y),
            round(placed.rect.w),
            round(placed.rect.h)
        ));
        out.push_str(&margin_details(&placed.style));
    } else {
        out.push_str(&format!("{id}\n"));
    }

    out.push_str("\nplacement\n");
    let placement = PlacementPlan::new(diagram);
    if let Some(parent) = placement
        .owner()
        .get(&crate::placement::Entity::Node(index))
    {
        out.push_str(&format!(
            "  parent `{}` — coordinates are local before the frame translation\n",
            diagram.groups[*parent].id.as_str()
        ));
    }
    match placement.node(index, opts.flow) {
        EffectivePlacement::Boundary(boundary) => {
            out.push_str(&format!(
                "  boundary on `{}` — declared by `attach`; center at {} of its bottom edge\n",
                diagram.nodes[boundary.host].id.as_str(),
                round(boundary.along),
            ));
        }
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
            if let Some(spec) = opts.grid_spec {
                out.push_str(&format!(
                    "  grid {} × {}, gaps {} × {} — from `graph {{ grid }}`\n",
                    spec.columns,
                    spec.rows,
                    round(spec.gap_col),
                    round(spec.gap_row)
                ));
            } else {
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
                None => {
                    let axes = placement.v2_alignment_axes(index);
                    let cross_axis = match dir {
                        RelDir::RightOf | RelDir::LeftOf => 1,
                        _ => 0,
                    };
                    if axes[cross_axis] {
                        out.push_str("  cross-axis alignment comes from the explicit layout alignment below\n");
                    } else {
                        out.push_str("  align center  — default\n");
                    }
                }
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

/// Which layer of the cascade settled each colour and notation property.
///
/// Re-run rather than recorded: the registry resolves in one pass, so this
/// walks the same layers again and attributes a field to the last one that
/// declares it. Doing it any other way would mean the resolver kept a trail
/// nothing else needs.
fn paint(diagram: &Diagram, node: &crate::ast::NodeDecl, id: &str) -> Result<String> {
    let registry = Registry::build(&diagram.styles)?;
    let def = crate::check::archetype_of(diagram);

    let mut source: Vec<(&'static str, String)> = Vec::new();
    let note = |layer: &Style, from: String, out: &mut Vec<_>| {
        for (name, value) in [
            ("fill", &layer.fill),
            ("stroke", &layer.stroke),
            ("font colour", &layer.font_color),
            ("kind", &layer.task_kind),
            ("stage", &layer.stage),
            ("trigger", &layer.trigger),
            ("gate", &layer.gate),
            ("artifact", &layer.artifact),
        ] {
            if let Some(value) = value {
                out.retain(|(field, _): &(&str, String)| *field != name);
                out.push((name, format!("{value}  — from {from}")));
            }
        }
    };

    note(
        &crate::style::item_defaults(Style::default(), def, node.keyword.as_str()),
        format!(
            "the `{}` form's builtin default",
            def.style_word(node.keyword.as_str())
        ),
        &mut source,
    );
    let item_word = def.style_word(node.keyword.as_str());
    for (name, layer) in registry.implicit_layers(item_word) {
        note(layer, format!("`style {name}`"), &mut source);
    }
    for class in &node.classes {
        if let Some(layer) = registry.get(class.as_str()) {
            note(layer, format!("`.{}`", class.as_str()), &mut source);
        }
    }

    note(
        &crate::style::layer_from_props(&node.props_for(def))?,
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

fn margin_details(style: &Style) -> String {
    let margin = crate::spacing::Margins::of(style);
    if [margin.top, margin.right, margin.bottom, margin.left]
        .iter()
        .all(|value| *value == 0.0)
    {
        return String::new();
    }
    format!(
        "  external margin (top/right/bottom/left): {} {} {} {}\n",
        round(margin.top),
        round(margin.right),
        round(margin.bottom),
        round(margin.left)
    )
}

fn explain_group(diagram: &Diagram, layout: &Layout, index: usize) -> Result<String> {
    let group = &diagram.groups[index];
    let plan = PlacementPlan::new(diagram);
    let mut out = format!("{} — frame", group.id.as_str());
    if let Some(placed) = layout
        .groups
        .iter()
        .find(|placed| placed.id == group.id.value)
    {
        let rect = placed.rect;
        out.push_str(&format!(
            " at {}, {} · {}×{}",
            round(rect.x),
            round(rect.y),
            round(rect.w),
            round(rect.h)
        ));
    }
    out.push_str("\n\nplacement\n  bounds enclose the children, title and padding\n");
    if let Some(parent) = plan.owner().get(&crate::placement::Entity::Group(index)) {
        out.push_str(&format!(
            "  parent `{}` — declared coordinates are local before the parent translation\n",
            diagram.groups[*parent].id.as_str()
        ));
    }
    match plan.declared_group_placement(index) {
        Some(Placement::At { x, y }) => out.push_str(&format!(
            "  at ({}, {}) — declared; translates this frame and its descendants\n",
            round(*x),
            round(*y)
        )),
        Some(Placement::Cell { col, row }) => out.push_str(&format!(
            "  grid cell ({}, {}) — declared; translates this frame and its descendants\n",
            round(*col),
            round(*row)
        )),
        Some(Placement::Rel {
            dir,
            anchor,
            gap,
            align,
        }) => {
            let opts = GraphOpts::from_props(&diagram.graph)?;
            let direction = match dir {
                RelDir::RightOf => "right-of",
                RelDir::LeftOf => "left-of",
                RelDir::Above => "above",
                RelDir::Below => "below",
            };
            out.push_str(&format!(
                "  {direction} `{}` — declared; translates this frame and its descendants\n",
                anchor.as_str()
            ));
            out.push_str(&format!(
                "  gap {} — {}\n",
                round(gap.unwrap_or(opts.gap)),
                if gap.is_some() {
                    "declared"
                } else {
                    "from graph gap"
                }
            ));
            out.push_str(&format!(
                "  align {} — {}\n",
                cross_axis_align_name(align.unwrap_or(Align::Center), *dir),
                if align.is_some() {
                    "declared"
                } else {
                    "default"
                }
            ));
        }
        None => out.push_str("  no declared frame translation; bounds follow the children\n"),
    }
    if crate::check::archetype_of(diagram).name == "bpmn" {
        out.push_str("  local origin starts after padding and title; negative child extents move it inward\n");
    }
    for relation in inspect::relations(diagram, group.id.as_str()) {
        if !relation.controls || !relation.peers.is_empty() {
            out.push_str(&format!(
                "  {} relation with {}\n",
                relation.axes,
                relation.peers.join(", ")
            ));
        }
    }
    Ok(out)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "bpmn")]
    fn paint_attributes_equal_overrides_to_the_last_declaring_layer() {
        let diagram = crate::parser::parse(
            "type bpmn\n\
             style node { fill: #123456 font-color: #234567 }\n\
             style event { fill: #123456 stroke: #345678 }\n\
             style emphasis { stroke: #345678 }\n\
             event e \"Event\" start plain .emphasis { fill: #123456 }\n",
        )
        .unwrap();
        let paint = paint(&diagram, &diagram.nodes[0], "e").unwrap();
        assert!(paint.contains("#123456  — from `e`'s own block"), "{paint}");
        assert!(paint.contains("#345678  — from `.emphasis`"), "{paint}");
        assert!(paint.contains("#234567  — from `style node`"), "{paint}");
    }

    #[test]
    #[cfg(feature = "xy")]
    fn alternate_chart_words_use_the_same_theme_in_render_and_explain() {
        for word in ["series", "scatter", "node"] {
            let target = if word == "node" { "series" } else { word };
            let source = format!(
                "type xy\nstyle node {{ font-color: #234567 }}\n\
                 style {target} {{ fill: #123456 }}\n{word} a \"A\" 0 10 1 20\n"
            );
            let diagram = crate::parser::parse(&source).unwrap();
            let layout = crate::layout::solve(&diagram).unwrap();
            let svg = crate::render_svg::render(&layout);
            assert!(
                svg.contains("#123456"),
                "{word}: the theme reaches the drawing"
            );
            let paint = paint(&diagram, &diagram.nodes[0], "a").unwrap();
            assert!(paint.contains(&format!("from `style {target}`")), "{paint}");
            assert!(paint.contains("from `style node`"), "{paint}");
        }
    }
    #[test]
    fn frame_explanation_names_parent_bounds_and_declared_translation() {
        let diagram =
            crate::parser::parse(include_str!("../tests/runtime/frame-nested.bd")).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        let explanation = explain(&diagram, &layout, "inner").unwrap();
        assert!(
            explanation.contains("inner — frame at 260, 150 · 30×30"),
            "{explanation}"
        );
        assert!(explanation.contains("parent `outer`"));
        assert!(explanation.contains("at (50, 40) — declared"));
        assert!(explanation.contains("translates this frame and its descendants"));
    }

    #[test]
    fn explicit_alignment_owns_the_relative_cross_axis_in_the_explanation() {
        let diagram = crate::parser::parse("node a\nnode b\nlayout {\n place a at (0, 0)\n place b right-of a\n align top a b\n}\n").unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        let explanation = explain(&diagram, &layout, "b").unwrap();
        assert!(
            explanation.contains("cross-axis alignment comes from the explicit layout"),
            "{explanation}"
        );
        assert!(!explanation.contains("align center  — default"));
    }
}
