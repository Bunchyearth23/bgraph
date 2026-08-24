//! Cassowary lowering used while `W-153` runs the two solvers side by side.

use anyhow::{Result, anyhow};
use kasuari::WeightedRelation::*;
use kasuari::{Constraint, Solver, Strength, Variable};

use crate::ast::{Align, ConstraintKind, Diagram, Placement, RelDir};
use crate::layout::Rect;
use crate::placement::{EffectivePlacement, Entity, PlacementPlan};
use crate::style::GraphOpts;

// The shadow exists to compare semantics while the lowering is incomplete,
// not to benchmark two engines on every hostile-input case. Large diagrams get
// their own performance gate before Kasuari becomes the source of geometry.
const MAX_SHADOW_NODES: usize = 512;

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
    let mut constraints: Vec<Constraint> = Vec::with_capacity(diagram.nodes.len() * 4);

    for (i, ((w, h), vars)) in sizes.iter().zip(&variables).enumerate() {
        constraints.push(vars.w | EQ(Strength::REQUIRED) | *w);
        constraints.push(vars.h | EQ(Strength::REQUIRED) | *h);

        match plan.node(i, opts.flow) {
            EffectivePlacement::Origin => {
                constraints.push(vars.x | EQ(Strength::REQUIRED) | 0.0);
                constraints.push(vars.y | EQ(Strength::REQUIRED) | 0.0);
            }
            EffectivePlacement::Declared(placement) => {
                lower_placement(
                    placement.resolved(opts.grid),
                    *vars,
                    plan,
                    &variables,
                    opts.gap,
                    &mut constraints,
                )?;
            }
            EffectivePlacement::Flow { dir, anchor } => {
                lower_relative(
                    dir,
                    anchor.as_str(),
                    None,
                    None,
                    *vars,
                    plan,
                    &variables,
                    opts.gap,
                    &mut constraints,
                )?;
            }
        }
    }

    let mut solver = Solver::new();
    solver
        .add_constraints(constraints)
        .map_err(|error| anyhow!("Kasuari shadow rejected the v1 relations: {error}"))?;

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
    placement: Placement,
    vars: Variables,
    plan: &PlacementPlan<'_>,
    variables: &[Variables],
    default_gap: f64,
    out: &mut Vec<Constraint>,
) -> Result<()> {
    match placement {
        Placement::At { x, y } => {
            out.push(vars.x | EQ(Strength::REQUIRED) | x);
            out.push(vars.y | EQ(Strength::REQUIRED) | y);
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
            plan,
            variables,
            default_gap,
            out,
        )?,
        // `Placement::resolved` above removes every cell.
        Placement::Cell { .. } => {
            return Err(anyhow!("a grid cell reached the Cassowary lowering"));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn lower_relative(
    dir: RelDir,
    anchor: &str,
    gap: Option<f64>,
    align: Option<Align>,
    vars: Variables,
    plan: &PlacementPlan<'_>,
    variables: &[Variables],
    default_gap: f64,
    out: &mut Vec<Constraint>,
) -> Result<()> {
    let Some(Entity::Node(anchor)) = plan.index().get(anchor).copied() else {
        return Err(anyhow!(
            "the Cassowary shadow cannot resolve node anchor `{anchor}`"
        ));
    };
    let target = variables[anchor];
    let gap = gap.unwrap_or(default_gap);
    let align = align.unwrap_or(Align::Center);

    match dir {
        RelDir::RightOf => {
            out.push(vars.x | EQ(Strength::REQUIRED) | (target.x + target.w + gap));
            lower_vertical_alignment(vars, target, align, out);
        }
        RelDir::LeftOf => {
            out.push((vars.x + vars.w + gap) | EQ(Strength::REQUIRED) | target.x);
            lower_vertical_alignment(vars, target, align, out);
        }
        RelDir::Below => {
            out.push(vars.y | EQ(Strength::REQUIRED) | (target.y + target.h + gap));
            lower_horizontal_alignment(vars, target, align, out);
        }
        RelDir::Above => {
            out.push((vars.y + vars.h + gap) | EQ(Strength::REQUIRED) | target.y);
            lower_horizontal_alignment(vars, target, align, out);
        }
    }
    Ok(())
}

fn lower_vertical_alignment(
    vars: Variables,
    target: Variables,
    align: Align,
    out: &mut Vec<Constraint>,
) {
    out.push(match align {
        Align::Start => vars.y | EQ(Strength::REQUIRED) | target.y,
        Align::Center => {
            (vars.y + vars.h * 0.5) | EQ(Strength::REQUIRED) | (target.y + target.h * 0.5)
        }
        Align::End => (vars.y + vars.h) | EQ(Strength::REQUIRED) | (target.y + target.h),
    });
}

fn lower_horizontal_alignment(
    vars: Variables,
    target: Variables,
    align: Align,
    out: &mut Vec<Constraint>,
) {
    out.push(match align {
        Align::Start => vars.x | EQ(Strength::REQUIRED) | target.x,
        Align::Center => {
            (vars.x + vars.w * 0.5) | EQ(Strength::REQUIRED) | (target.x + target.w * 0.5)
        }
        Align::End => (vars.x + vars.w) | EQ(Strength::REQUIRED) | (target.x + target.w),
    });
}
