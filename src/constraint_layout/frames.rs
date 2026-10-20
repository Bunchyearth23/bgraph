use super::*;
use kasuari::Expression;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn solve(
    diagram: &Diagram,
    plan: &PlacementPlan<'_>,
    sizes: &[(f64, f64)],
    intrinsic: &[(f64, f64)],
    overhang: &[(f64, f64, f64, f64)],
    opts: &GraphOpts,
) -> Result<SolvedGeometry> {
    let margins = crate::spacing::entities(diagram)?;
    let bpmn = crate::check::archetype_of(diagram).name == "bpmn";
    let mut spacing = margins.clone();
    #[cfg(feature = "bpmn")]
    if crate::check::archetype_of(diagram).name == "bpmn" {
        for (space, &(left, top, right, bottom)) in spacing.iter_mut().zip(overhang) {
            space.left += left;
            space.top += top;
            space.right += right;
            space.bottom += bottom;
        }
    }
    #[cfg(not(feature = "bpmn"))]
    let _ = &mut spacing;
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
        if bpmn || plan.declared_group_placement(group).is_some() {
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
    let registry = crate::style::Registry::build(&diagram.styles)?;
    let group_styles: Vec<_> = diagram
        .groups
        .iter()
        .map(|group| {
            registry.resolve(
                crate::style::Style::group_defaults(),
                "group",
                &group.classes,
                &group.props,
            )
        })
        .collect::<Result<_>>()?;
    if bpmn {
        for (group, translation) in translations.iter().enumerate() {
            let Some((tx, ty)) = translation else {
                continue;
            };
            let vars = variables[diagram.nodes.len() + group];
            let offset = local_offset(Entity::Group(group));
            let padding = crate::layout::group_padding(diagram, group);
            let title = if diagram.groups[group].label.is_some() {
                16.0
            } else {
                0.0
            };
            let source = provenance(
                ConstraintOrigin::ContainerOrigin,
                Axis::X,
                diagram.nodes.len() + group,
                diagram.groups[group].span,
            );
            for (origin, inset, source) in [
                (*tx + offset.0, vars.x + padding, source),
                (
                    *ty + offset.1,
                    vars.y + padding + title,
                    source.with_axis(Axis::Y),
                ),
            ] {
                // Keep positive local offsets; negative extents can move the origin inward.
                push(
                    &mut constraints,
                    origin.clone() | GE(Strength::REQUIRED) | inset.clone(),
                    source,
                );
                push(
                    &mut constraints,
                    origin | EQ(Strength::STRONG) | inset,
                    source,
                );
            }
        }
    }
    for (node, (&vars, &size)) in variables.iter().zip(sizes).enumerate() {
        let flexible = [ConstraintKind::SameWidth, ConstraintKind::SameHeight].map(|kind| {
            plan.constraints().any(|item| {
                item.declaration().kind == kind
                    && item
                        .declaration()
                        .members
                        .iter()
                        .any(|member| member.as_str() == diagram.nodes[node].id.as_str())
                    && item.declaration().members.iter().any(|member| {
                        matches!(plan.index().get(member.as_str()), Some(Entity::Group(_)))
                    })
            })
        });
        lower_size(
            vars,
            node,
            diagram.nodes[node].span,
            intrinsic[node],
            size,
            flexible,
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
            &spacing,
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
                &spacing,
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
        for (value, variable, axis) in [
            (group_styles[group].width, vars.w, Axis::Width),
            (group_styles[group].height, vars.h, Axis::Height),
        ] {
            if let Some(value) = value {
                push(
                    &mut constraints,
                    variable | EQ(Strength::REQUIRED) | value,
                    provenance(
                        ConstraintOrigin::SizeConstraint,
                        axis,
                        index,
                        declaration.span,
                    ),
                );
            }
        }
        if let Some(pool) = plan.lane_pool(group) {
            let parent = variables[diagram.nodes.len() + pool];
            let padding = crate::layout::group_padding(diagram, pool);
            let margin = margins[index];
            push(
                &mut constraints,
                (vars.x + vars.w + padding + margin.right)
                    | EQ(Strength::REQUIRED)
                    | (parent.x + parent.w),
                source,
            );
        }
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
                Entity::Node(_) if bpmn => (0.0, 0.0, 0.0, 0.0),
                Entity::Node(node) => overhang[node],
                Entity::Group(_) => (0.0, 0.0, 0.0, 0.0),
            };
            let margin = margins[child_index];
            let (left, top, right, bottom) = (
                left + if bpmn { 0.0 } else { margin.left },
                top + if bpmn { 0.0 } else { margin.top },
                right + margin.right,
                bottom + margin.bottom,
            );
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
        let members = plan.constraint_indices(item);
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
    let ranks = dependency_ranks(&constraints, diagram);
    let mut values = BTreeMap::new();
    let mut best_conflict = None;
    for component in constraint_components(constraints) {
        let (indices, constraints): (Vec<_>, Vec<_>) = component.into_iter().unzip();
        let mut solver = Solver::new();
        // Keep the model in canonical source order for conflict ranking. Only insertion is
        // scheduled by dependencies so a long chain does not repeatedly merge solved prefixes.
        let mut execution: Vec<_> = (0..constraints.len()).collect();
        execution.sort_by_key(|&index| {
            let source = &constraints[index];
            (
                match source.provenance.origin {
                    ConstraintOrigin::ContainerOrigin => ConstraintOrigin::AbsolutePlacement,
                    origin => origin,
                },
                source.provenance.axis,
                ranks[source.provenance.node],
                index,
            )
        });
        let mut failed = false;
        for index in execution {
            if !add_to_solver(&mut solver, &constraints[index])? {
                failed = true;
                break;
            }
        }
        if failed {
            // A failure in scheduled order need not be a failure of the same canonical prefix.
            let conflict = explain_conflict(&constraints, constraints.len() - 1)?;
            let mut global_indices: Vec<_> = conflict
                .iter()
                .map(|source| {
                    let local = constraints
                        .iter()
                        .position(|candidate| candidate.constraint == source.constraint)
                        .expect("a conflict retains original constraint handles");
                    indices[local]
                })
                .collect();
            global_indices.sort_unstable();
            let rank = (conflict.len(), global_indices);
            if best_conflict.as_ref().is_none_or(|(best, _)| rank < *best) {
                best_conflict = Some((rank, conflict));
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
    if let Some((_, conflict)) = best_conflict {
        return Err(if extra_names.is_empty() {
            conflict_error(conflict, diagram, &variables)
        } else {
            conflict_error_with_names(conflict, diagram, &variables, &extra_names)
        });
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

fn dependency_ranks(constraints: &[SourcedConstraint], diagram: &Diagram) -> Vec<usize> {
    let count = diagram.nodes.len() + diagram.groups.len();
    let mut children = vec![BTreeSet::new(); count];
    let mut incoming = vec![0usize; count];
    for source in constraints {
        if !matches!(
            source.provenance.origin,
            ConstraintOrigin::RelativePlacement
                | ConstraintOrigin::BoundaryAttachment
                | ConstraintOrigin::ImplicitFlow
                | ConstraintOrigin::FrameBounds
        ) {
            continue;
        }
        if let Some(anchor) = source.provenance.anchor {
            let node = source.provenance.node;
            if anchor != node && children[anchor].insert(node) {
                incoming[node] += 1;
            }
        }
    }
    let mut remaining: BTreeSet<_> = (0..count)
        .map(|index| (entity_name(diagram, index), index))
        .collect();
    let mut ready: BTreeSet<_> = remaining
        .iter()
        .copied()
        .filter(|(_, index)| incoming[*index] == 0)
        .collect();
    let mut ranks = vec![usize::MAX; count];
    for rank in 0..count {
        // A cycle introduces no artificial origin: break only the scheduling tie by name.
        let (_, node) = ready
            .pop_first()
            .unwrap_or_else(|| *remaining.first().unwrap());
        remaining.remove(&(entity_name(diagram, node), node));
        ranks[node] = rank;
        for &child in &children[node] {
            if ranks[child] == usize::MAX {
                incoming[child] -= 1;
                if incoming[child] == 0 {
                    ready.insert((entity_name(diagram, child), child));
                }
            }
        }
    }
    ranks
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
    margins: &[crate::spacing::Margins],
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
                margins,
                default_gap: opts.gap,
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
