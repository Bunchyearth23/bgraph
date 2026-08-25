//! Cassowary lowering used while `W-153` runs the two solvers side by side.

use anyhow::{Result, anyhow};
use kasuari::WeightedRelation::*;
use kasuari::{Constraint, Solver, Strength, Variable};

use crate::ast::{Align, ConstraintKind, Diagram, Placement, RelDir, Span};
use crate::layout::Rect;
use crate::placement::{EffectivePlacement, Entity, PlacementPlan};
use crate::style::GraphOpts;

// The shadow exists to compare semantics while the lowering is incomplete,
// not to benchmark two engines on every hostile-input case. Large diagrams get
// their own performance gate before Kasuari becomes the source of geometry.
const MAX_SHADOW_NODES: usize = 512;

/// Which geometric quantity a lowered relation controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Width,
    Height,
}

/// The DSL construct from which a linear constraint was lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintOrigin {
    MeasuredSize,
    AbsolutePlacement,
    GridPlacement,
    RelativePlacement,
    ImplicitFlow,
    ContainerOrigin,
}

/// Source data retained with every constraint until the solver reports it.
///
/// The current shadow path only uses this for a precise failure message. The
/// same records will let the production solver compute a conflict set without
/// trying to reconstruct intent from rectangles after the fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConstraintProvenance {
    pub origin: ConstraintOrigin,
    pub axis: Axis,
    pub node: usize,
    pub anchor: Option<usize>,
    pub span: Span,
    pub related_span: Option<Span>,
}

#[derive(Clone, Debug)]
struct SourcedConstraint {
    constraint: Constraint,
    provenance: ConstraintProvenance,
}

#[derive(Clone, Copy)]
struct Variables {
    x: Variable,
    y: Variable,
    w: Variable,
    h: Variable,
}

impl Variables {
    fn new() -> Self {
        Self {
            x: Variable::new(),
            y: Variable::new(),
            w: Variable::new(),
            h: Variable::new(),
        }
    }
}

/// Shared lowering inputs keep the source-bearing relation functions narrow.
struct Lowering<'plan, 'diagram, 'out> {
    plan: &'plan PlacementPlan<'diagram>,
    variables: &'plan [Variables],
    default_gap: f64,
    grid: Option<(f64, f64)>,
    out: &'out mut Vec<SourcedConstraint>,
}

/// Solve the v1 node relations that already have an exact Cassowary lowering.
///
/// `None` means the diagram still uses a construct the shadow path has not
/// migrated. It is intentionally not approximated: a partial comparison that
/// silently drops a group or an alignment would certify the wrong engine.
pub fn solve_nodes(
    diagram: &Diagram,
    plan: &PlacementPlan<'_>,
    sizes: &[(f64, f64)],
    opts: &GraphOpts,
) -> Result<Option<Vec<Rect>>> {
    if diagram.nodes.len() > MAX_SHADOW_NODES
        || !diagram.groups.is_empty()
        || diagram
            .constraints
            .iter()
            .any(|constraint| constraint.kind == ConstraintKind::AlignGroup)
    {
        return Ok(None);
    }

    let variables: Vec<Variables> = (0..diagram.nodes.len()).map(|_| Variables::new()).collect();
    let mut constraints: Vec<SourcedConstraint> = Vec::with_capacity(diagram.nodes.len() * 4);

    for (i, ((w, h), vars)) in sizes.iter().zip(&variables).enumerate() {
        let node_span = diagram.nodes[i].span;
        push(
            &mut constraints,
            vars.w | EQ(Strength::REQUIRED) | *w,
            provenance(ConstraintOrigin::MeasuredSize, Axis::Width, i, node_span),
        );
        push(
            &mut constraints,
            vars.h | EQ(Strength::REQUIRED) | *h,
            provenance(ConstraintOrigin::MeasuredSize, Axis::Height, i, node_span),
        );

        match plan.node(i, opts.flow) {
            EffectivePlacement::Origin => {
                push(
                    &mut constraints,
                    vars.x | EQ(Strength::REQUIRED) | 0.0,
                    provenance(ConstraintOrigin::ContainerOrigin, Axis::X, i, node_span),
                );
                push(
                    &mut constraints,
                    vars.y | EQ(Strength::REQUIRED) | 0.0,
                    provenance(ConstraintOrigin::ContainerOrigin, Axis::Y, i, node_span),
                );
            }
            EffectivePlacement::Declared(placement) => {
                lower_placement(
                    placement,
                    i,
                    node_span,
                    *vars,
                    &mut Lowering {
                        plan,
                        variables: &variables,
                        default_gap: opts.gap,
                        grid: opts.grid,
                        out: &mut constraints,
                    },
                )?;
            }
            EffectivePlacement::Flow { dir, anchor } => {
                lower_relative(
                    dir,
                    anchor.as_str(),
                    None,
                    None,
                    *vars,
                    provenance(ConstraintOrigin::ImplicitFlow, Axis::X, i, node_span)
                        .with_anchor(anchor.span),
                    &mut Lowering {
                        plan,
                        variables: &variables,
                        default_gap: opts.gap,
                        grid: opts.grid,
                        out: &mut constraints,
                    },
                )?;
            }
        }
    }

    let mut solver = Solver::new();
    for sourced in constraints {
        solver.add_constraint(sourced.constraint).map_err(|error| {
            let source = sourced.provenance;
            anyhow!(
                "Kasuari shadow rejected a {:?} {:?} relation at bytes {}..{}: {error}",
                source.origin,
                source.axis,
                source.span.start,
                source.span.end,
            )
        })?;
    }

    Ok(Some(
        variables
            .iter()
            .map(|vars| Rect {
                x: solver.get_value(vars.x),
                y: solver.get_value(vars.y),
                w: solver.get_value(vars.w),
                h: solver.get_value(vars.h),
            })
            .collect(),
    ))
}

fn lower_placement(
    placement: &Placement,
    node: usize,
    node_span: Span,
    vars: Variables,
    lowering: &mut Lowering<'_, '_, '_>,
) -> Result<()> {
    let origin = match placement {
        Placement::Cell { .. } => ConstraintOrigin::GridPlacement,
        Placement::At { .. } => ConstraintOrigin::AbsolutePlacement,
        Placement::Rel { .. } => ConstraintOrigin::RelativePlacement,
    };
    match placement.resolved(lowering.grid) {
        Placement::At { x, y } => {
            push(
                lowering.out,
                vars.x | EQ(Strength::REQUIRED) | x,
                provenance(origin, Axis::X, node, node_span),
            );
            push(
                lowering.out,
                vars.y | EQ(Strength::REQUIRED) | y,
                provenance(origin, Axis::Y, node, node_span),
            );
        }
        Placement::Rel {
            dir,
            anchor,
            gap,
            align,
        } => lower_relative(
            dir,
            anchor.as_str(),
            gap,
            align,
            vars,
            provenance(origin, Axis::X, node, node_span).with_anchor(anchor.span),
            lowering,
        )?,
        // `Placement::resolved` above removes every cell.
        Placement::Cell { .. } => {
            return Err(anyhow!("a grid cell reached the Cassowary lowering"));
        }
    }
    Ok(())
}

fn lower_relative(
    dir: RelDir,
    anchor: &str,
    gap: Option<f64>,
    align: Option<Align>,
    vars: Variables,
    source: ConstraintProvenance,
    lowering: &mut Lowering<'_, '_, '_>,
) -> Result<()> {
    let Some(Entity::Node(anchor)) = lowering.plan.index().get(anchor).copied() else {
        return Err(anyhow!(
            "the Cassowary shadow cannot resolve node anchor `{anchor}`"
        ));
    };
    let target = lowering.variables[anchor];
    let gap = gap.unwrap_or(lowering.default_gap);
    let align = align.unwrap_or(Align::Center);

    match dir {
        RelDir::RightOf => {
            push(
                lowering.out,
                vars.x | EQ(Strength::REQUIRED) | (target.x + target.w + gap),
                source.with_axis(Axis::X).with_anchor_index(anchor),
            );
            lower_vertical_alignment(
                vars,
                target,
                align,
                source.with_anchor_index(anchor),
                lowering.out,
            );
        }
        RelDir::LeftOf => {
            push(
                lowering.out,
                (vars.x + vars.w + gap) | EQ(Strength::REQUIRED) | target.x,
                source.with_axis(Axis::X).with_anchor_index(anchor),
            );
            lower_vertical_alignment(
                vars,
                target,
                align,
                source.with_anchor_index(anchor),
                lowering.out,
            );
        }
        RelDir::Below => {
            push(
                lowering.out,
                vars.y | EQ(Strength::REQUIRED) | (target.y + target.h + gap),
                source.with_axis(Axis::Y).with_anchor_index(anchor),
            );
            lower_horizontal_alignment(
                vars,
                target,
                align,
                source.with_anchor_index(anchor),
                lowering.out,
            );
        }
        RelDir::Above => {
            push(
                lowering.out,
                (vars.y + vars.h + gap) | EQ(Strength::REQUIRED) | target.y,
                source.with_axis(Axis::Y).with_anchor_index(anchor),
            );
            lower_horizontal_alignment(
                vars,
                target,
                align,
                source.with_anchor_index(anchor),
                lowering.out,
            );
        }
    }
    Ok(())
}

fn lower_vertical_alignment(
    vars: Variables,
    target: Variables,
    align: Align,
    source: ConstraintProvenance,
    out: &mut Vec<SourcedConstraint>,
) {
    push(
        out,
        match align {
            Align::Start => vars.y | EQ(Strength::REQUIRED) | target.y,
            Align::Center => {
                (vars.y + vars.h * 0.5) | EQ(Strength::REQUIRED) | (target.y + target.h * 0.5)
            }
            Align::End => (vars.y + vars.h) | EQ(Strength::REQUIRED) | (target.y + target.h),
        },
        source.with_axis(Axis::Y),
    );
}

fn lower_horizontal_alignment(
    vars: Variables,
    target: Variables,
    align: Align,
    source: ConstraintProvenance,
    out: &mut Vec<SourcedConstraint>,
) {
    push(
        out,
        match align {
            Align::Start => vars.x | EQ(Strength::REQUIRED) | target.x,
            Align::Center => {
                (vars.x + vars.w * 0.5) | EQ(Strength::REQUIRED) | (target.x + target.w * 0.5)
            }
            Align::End => (vars.x + vars.w) | EQ(Strength::REQUIRED) | (target.x + target.w),
        },
        source.with_axis(Axis::X),
    );
}

fn provenance(
    origin: ConstraintOrigin,
    axis: Axis,
    node: usize,
    span: Span,
) -> ConstraintProvenance {
    ConstraintProvenance {
        origin,
        axis,
        node,
        anchor: None,
        span,
        related_span: None,
    }
}

impl ConstraintProvenance {
    fn with_axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    fn with_anchor(mut self, related_span: Span) -> Self {
        self.related_span = Some(related_span);
        self
    }

    fn with_anchor_index(mut self, anchor: usize) -> Self {
        self.anchor = Some(anchor);
        self
    }
}

fn push(
    out: &mut Vec<SourcedConstraint>,
    constraint: Constraint,
    provenance: ConstraintProvenance,
) {
    out.push(SourcedConstraint {
        constraint,
        provenance,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provenance_keeps_the_declared_relation_and_its_anchor() {
        let source = provenance(
            ConstraintOrigin::RelativePlacement,
            Axis::X,
            2,
            Span { start: 40, end: 63 },
        )
        .with_anchor(Span { start: 57, end: 60 })
        .with_anchor_index(1);

        assert_eq!(source.origin, ConstraintOrigin::RelativePlacement);
        assert_eq!(source.axis, Axis::X);
        assert_eq!(source.node, 2);
        assert_eq!(source.anchor, Some(1));
        assert_eq!(source.span, Span { start: 40, end: 63 });
        assert_eq!(source.related_span, Some(Span { start: 57, end: 60 }));
    }
}
