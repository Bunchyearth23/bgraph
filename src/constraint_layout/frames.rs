use super::*;
use kasuari::Expression;
use std::collections::BTreeMap;

pub(super) fn solve(
    diagram: &Diagram,
    plan: &PlacementPlan<'_>,
    sizes: &[(f64, f64)],
    intrinsic: &[(f64, f64)],
    overhang: &[(f64, f64, f64, f64)],
    opts: &GraphOpts,
) -> Result<SolvedGeometry> {
    let count = diagram.nodes.len() + diagram.groups.len();
    let mut order: Vec<_> = (0..count).collect();
    order.sort_by_key(|&index| entity_name(diagram, index));
    let mut variables: Vec<_> = order
        .into_iter()
        .map(|index| (index, Variables::new()))
        .collect();
    variables.sort_by_key(|(index, _)| *index);
    let variables: Vec<_> = variables
        .into_iter()
        .map(|(_, variables)| variables)
        .collect();
    let mut translations = vec![None; diagram.groups.len()];
    let mut order: Vec<_> = (0..diagram.groups.len()).collect();
    order.sort_by_key(|&group| diagram.groups[group].id.as_str());
    let mut extra_names = Vec::new();
    for group in order {
        if plan.declared_group_placement(group).is_some() {
            let x = Variable::new();
            let y = Variable::new();
            translations[group] = Some((x, y));
            extra_names.push((x, format!("{}.offset-x", diagram.groups[group].id.as_str())));
            extra_names.push((y, format!("{}.offset-y", diagram.groups[group].id.as_str())));
        }
    }
    let local_offset = |entity| {
        let mut x = Expression::from_constant(0.0);
        let mut y = Expression::from_constant(0.0);
        let mut owner = plan.owner().get(&entity).copied();
        while let Some(group) = owner {
            if let Some((tx, ty)) = translations[group] {
                x += tx;
                y += ty;
            }
            owner = plan.owner().get(&Entity::Group(group)).copied();
        }
        (x, y)
    };
    let grid_pitch = opts
        .grid_spec
        .map(|grid| grid.pitch(sizes))
        .unwrap_or((1.0, 1.0));
    let mut constraints = Vec::new();
    for (node, (&vars, &size)) in variables.iter().zip(sizes).enumerate() {
        lower_size(
            vars,
            node,
            diagram.nodes[node].span,
            intrinsic[node],
            size,
            &mut constraints,
        );
        if let Some(attachment) = plan.boundary(node) {
            lower_boundary_attachment(diagram, node, attachment, &variables, &mut constraints);
        }
        if matches!(plan.node(node, opts.flow), EffectivePlacement::Boundary(_)) {
            continue;
        }
        let entity = Entity::Node(node);
        let span = plan.placement_span(entity);
        let relative_origin =
            if matches!(plan.node(node, opts.flow), EffectivePlacement::Flow { .. }) {
                ConstraintOrigin::ImplicitFlow
            } else {
                ConstraintOrigin::RelativePlacement
            };
        let placement = match plan.node(node, opts.flow) {
            EffectivePlacement::Origin => None,
            EffectivePlacement::Boundary(_) => unreachable!("attachments were lowered above"),
            EffectivePlacement::Declared(placement) => Some(placement.clone()),
            EffectivePlacement::Flow { dir, anchor } => Some(Placement::Rel {
                dir,
                anchor: anchor.clone(),
                gap: None,
                align: None,
            }),
            EffectivePlacement::Series { dir, anchor, gap } => Some(Placement::Rel {
                dir,
                anchor: anchor.clone(),
                gap,
                align: None,
            }),
        };
        lower_entity_placement(
            diagram,
            plan,
            &variables,
            node,
            vars,
            placement.as_ref(),
            span,
            relative_origin,
            local_offset(entity),
            plan.v2_alignment_axes(node),
            grid_pitch,
            opts,
            &mut constraints,
        )?;
    }
    for (group, declaration) in diagram.groups.iter().enumerate() {
        let index = diagram.nodes.len() + group;
        let vars = variables[index];
        if let Some(placement) = plan.declared_group_placement(group) {
            lower_entity_placement(
                diagram,
                plan,
                &variables,
                index,
                vars,
                Some(placement),
                plan.placement_span(Entity::Group(group)),
                ConstraintOrigin::RelativePlacement,
                local_offset(Entity::Group(group)),
                [false; 2],
                grid_pitch,
                opts,
                &mut constraints,
            )?;
        }
        let padding = crate::layout::group_padding(diagram, group);
        let title = if declaration.label.is_some() {
            16.0
        } else {
            0.0
        };
        let named_width = declaration
            .label
            .as_deref()
            .map(|label| {
                crate::layout::text_width(&[label.to_owned()], 12.0) + padding * 2.0 + 10.0
            })
            .unwrap_or(0.0);
        let source = provenance(
            ConstraintOrigin::FrameBounds,
            Axis::Width,
            index,
            declaration.span,
        );
        push(
            &mut constraints,
            vars.w | GE(Strength::REQUIRED) | named_width,
            source,
        );
        push(
            &mut constraints,
            vars.h | GE(Strength::REQUIRED) | 0.0,
            source.with_axis(Axis::Height),
        );
        for member in &declaration.members {
            let Some(entity) = plan.index().get(member.as_str()).copied() else {
                continue;
            };
            let child_index = entity_index(entity, diagram);
            let child = variables[child_index];
            let (left, top, right, bottom) = match entity {
                Entity::Node(node) => overhang[node],
                Entity::Group(_) => (0.0, 0.0, 0.0, 0.0),
            };
            let source = provenance(
                ConstraintOrigin::FrameBounds,
                Axis::X,
                index,
                declaration.span,
            )
            .with_anchor(member.span)
            .with_anchor_index(child_index);
            push(
                &mut constraints,
                vars.x | LE(Strength::REQUIRED) | (child.x - left - padding),
                source,
            );
            push(
                &mut constraints,
                vars.y | LE(Strength::REQUIRED) | (child.y - top - padding - title),
                source.with_axis(Axis::Y),
            );
            push(
                &mut constraints,
                (vars.x + vars.w) | GE(Strength::REQUIRED) | (child.x + child.w + right + padding),
                source.with_axis(Axis::Width),
            );
            push(
                &mut constraints,
                (vars.y + vars.h) | GE(Strength::REQUIRED) | (child.y + child.h + bottom + padding),
                source.with_axis(Axis::Height),
            );
            let preference = provenance(
                ConstraintOrigin::FramePreference,
                Axis::X,
                index,
                declaration.span,
            )
            .with_anchor(member.span)
            .with_anchor_index(child_index);
            // Containment makes every residual nonnegative: these preferences select the tight left/top extrema.
            push(
                &mut constraints,
                vars.x | EQ(Strength::STRONG) | (child.x - left - padding),
                preference,
            );
            push(
                &mut constraints,
                vars.y | EQ(Strength::STRONG) | (child.y - top - padding - title),
                preference.with_axis(Axis::Y),
            );
        }
        push(
            &mut constraints,
            vars.w | EQ(Strength::WEAK) | 0.0,
            provenance(
                ConstraintOrigin::FramePreference,
                Axis::Width,
                index,
                declaration.span,
            ),
        );
        push(
            &mut constraints,
            vars.h | EQ(Strength::WEAK) | 0.0,
            provenance(
                ConstraintOrigin::FramePreference,
                Axis::Height,
                index,
                declaration.span,
            ),
        );
    }
    lower_size_equalities(plan, &variables, &mut constraints);
    for item in plan.constraints() {
        let declaration = item.declaration();
        if declaration.kind != ConstraintKind::AlignGroup {
            continue;
        }
        let members = plan.constraint_members(item);
        let Some((&lead, rest)) = members.split_first() else {
            continue;
        };
        for &node in rest {
            let source = provenance(
                ConstraintOrigin::AlignmentConstraint,
                Axis::X,
                node,
                declaration.span,
            )
            .with_anchor(declaration.members[0].span)
            .with_anchor_index(lead);
            match declaration.axis {
                ConstraintAxis::Horizontal => lower_horizontal_alignment(
                    variables[node],
                    variables[lead],
                    declaration.align,
                    source,
                    &mut constraints,
                ),
                ConstraintAxis::Vertical => lower_vertical_alignment(
                    variables[node],
                    variables[lead],
                    declaration.align,
                    source,
                    &mut constraints,
                ),
                ConstraintAxis::Inferred => {
                    return Err(anyhow!(
                        "an alignment without an axis reached frame lowering"
                    ));
                }
            }
        }
    }
    sort_constraints(&mut constraints, diagram);
    let mut values = BTreeMap::new();
    let mut first_failure = None;
    for component in constraint_components(constraints) {
        let (indices, constraints): (Vec<_>, Vec<_>) = component.into_iter().unzip();
        let mut solver = Solver::new();
        let mut failing = None;
        for (index, source) in constraints.iter().enumerate() {
            if !add_to_solver(&mut solver, source)? {
                failing = Some(index);
                break;
            }
        }
        if let Some(index) = failing {
            // Preserve the failure selected by the original global insertion order.
            if first_failure
                .as_ref()
                .is_none_or(|(earliest, _, _)| indices[index] < *earliest)
            {
                first_failure = Some((indices[index], constraints, index));
            }
            continue;
        }
        for source in &constraints {
            for term in &source.constraint.expr().terms {
                values
                    .entry(term.variable)
                    .or_insert_with(|| solver.get_value(term.variable));
            }
        }
    }
    if let Some((_, constraints, index)) = first_failure {
        return Err(conflict_error_with_names(
            explain_conflict(&constraints, index)?,
            diagram,
            &variables,
            &extra_names,
        ));
    }
    let value = |variable| values.get(&variable).copied().unwrap_or(0.0);
    let mut rects = variables
        .into_iter()
        .map(|vars| Rect {
            x: value(vars.x),
            y: value(vars.y),
            w: value(vars.w),
            h: value(vars.h),
        })
        .collect::<Vec<_>>();
    let groups = rects.split_off(diagram.nodes.len());
    Ok(SolvedGeometry {
        nodes: rects,
        groups,
    })
}

fn component_root(parents: &mut BTreeMap<Variable, Variable>, mut variable: Variable) -> Variable {
    let mut root = variable;
    while parents[&root] != root {
        root = parents[&root];
    }
    while variable != root {
        let next = parents[&variable];
        parents.insert(variable, root);
        variable = next;
    }
    root
}

fn constraint_components(
    constraints: Vec<SourcedConstraint>,
) -> Vec<Vec<(usize, SourcedConstraint)>> {
    let mut parents = BTreeMap::new();
    for source in &constraints {
        let mut lead = None;
        for term in &source.constraint.expr().terms {
            parents.entry(term.variable).or_insert(term.variable);
            let root = component_root(&mut parents, term.variable);
            if let Some(lead) = lead {
                let other = component_root(&mut parents, lead);
                parents.insert(root.max(other), root.min(other));
            } else {
                lead = Some(root);
            }
        }
    }
    let mut components: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for (index, source) in constraints.into_iter().enumerate() {
        // Constant constraints also go through a solver; an empty term list can be contradictory.
        let root = source
            .constraint
            .expr()
            .terms
            .first()
            .map(|term| component_root(&mut parents, term.variable));
        components.entry(root).or_default().push((index, source));
    }
    components.into_values().collect()
}

#[allow(clippy::too_many_arguments)]
fn lower_entity_placement(
    diagram: &Diagram,
    plan: &PlacementPlan<'_>,
    variables: &[Variables],
    index: usize,
    vars: Variables,
    placement: Option<&Placement>,
    span: Span,
    relative_origin: ConstraintOrigin,
    offset: (Expression, Expression),
    overridden_axes: [bool; 2],
    grid_pitch: (f64, f64),
    opts: &GraphOpts,
    constraints: &mut Vec<SourcedConstraint>,
) -> Result<()> {
    match placement {
        Some(Placement::Rel {
            dir,
            anchor,
            gap,
            align,
        }) => lower_relative(
            *dir,
            anchor.as_str(),
            *gap,
            RelativeAlignment {
                declared: *align,
                overridden_axes,
            },
            vars,
            provenance(relative_origin, Axis::X, index, span).with_anchor(anchor.span),
            &mut Lowering {
                diagram,
                plan,
                variables,
                default_gap: opts.gap,
                grid: opts.grid,
                grid_spec: opts.grid_spec,
                grid_pitch,
                out: constraints,
            },
        ),
        placement => {
            let (x, y, origin) = match placement {
                Some(Placement::At { x, y }) => (
                    if opts.grid_spec.is_some() {
                        x * grid_pitch.0
                    } else {
                        *x
                    },
                    if opts.grid_spec.is_some() {
                        y * grid_pitch.1
                    } else {
                        *y
                    },
                    ConstraintOrigin::AbsolutePlacement,
                ),
                Some(Placement::Cell { col, row }) => {
                    let pitch = opts.grid.unwrap_or(grid_pitch);
                    (
                        col * pitch.0,
                        row * pitch.1,
                        ConstraintOrigin::GridPlacement,
                    )
                }
                None => (0.0, 0.0, ConstraintOrigin::ContainerOrigin),
                Some(Placement::Rel { .. }) => unreachable!(),
            };
            push(
                constraints,
                (vars.x - offset.0) | EQ(Strength::REQUIRED) | x,
                provenance(origin, Axis::X, index, span),
            );
            push(
                constraints,
                (vars.y - offset.1) | EQ(Strength::REQUIRED) | y,
                provenance(origin, Axis::Y, index, span),
            );
            Ok(())
        }
    }
}
