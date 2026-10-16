//! Deterministic simultaneous Cassowary placement for nodes and content-sized frames.

use anyhow::{Result, anyhow};
use kasuari::WeightedRelation::*;
use kasuari::{AddConstraintError, Constraint, StaticSolver as Solver, Strength, Variable};

use crate::ast::{Align, ConstraintAxis, ConstraintKind, Diagram, Placement, RelDir, Span};
use crate::layout::Rect;
use crate::placement::{EffectivePlacement, Entity, PlacementPlan};
use crate::style::GraphOpts;

#[cfg(test)]
mod completion_tests;
mod conflict;
mod frames;

pub struct SolvedGeometry {
    pub nodes: Vec<Rect>,
    pub groups: Vec<Rect>,
}

/// Exact source relations are shared by nodes and content-sized frames.
pub fn solve_declared(
    diagram: &Diagram,
    plan: &PlacementPlan<'_>,
    sizes: &[(f64, f64)],
    intrinsic: &[(f64, f64)],
    overhang: &[(f64, f64, f64, f64)],
    opts: &GraphOpts,
) -> Result<SolvedGeometry> {
    frames::solve(diagram, plan, sizes, intrinsic, overhang, opts)
}

/// Which geometric quantity a lowered relation controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Axis {
    X,
    Y,
    Width,
    Height,
}

/// The DSL construct from which a linear constraint was lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConstraintOrigin {
    MeasuredSize,
    AbsolutePlacement,
    GridPlacement,
    RelativePlacement,
    BoundaryAttachment,
    ImplicitFlow,
    ContainerOrigin,
    AlignmentConstraint,
    SizeConstraint,
    FrameBounds,
    FramePreference,
}

/// Source data retained with every constraint until the solver reports it.
///
/// Conflict extraction keeps these records instead of reconstructing intent
/// from the final rectangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConstraintProvenance {
    pub origin: ConstraintOrigin,
    pub axis: Axis,
    pub node: usize,
    pub anchor: Option<usize>,
    pub span: Span,
    pub related_span: Option<Span>,
    pub relation: &'static str,
    pub value: f64,
}

#[derive(Clone, Debug)]
struct SourcedConstraint {
    constraint: Constraint,
    provenance: ConstraintProvenance,
}

/// False means a required relation is unsatisfiable; engine failures stay errors.
fn add_to_solver(solver: &mut Solver, sourced: &SourcedConstraint) -> Result<bool> {
    classify_add_result(solver.add_constraint(sourced.constraint.clone()))
}

fn classify_add_result(result: std::result::Result<(), AddConstraintError>) -> Result<bool> {
    match result {
        // Kasuari compares constraint identity, so this handle is already in the system.
        Ok(()) | Err(AddConstraintError::DuplicateConstraint) => Ok(true),
        Err(AddConstraintError::UnsatisfiableConstraint) => Ok(false),
        Err(error @ AddConstraintError::InternalSolverError(_)) => Err(error.into()),
    }
}

#[cfg(test)]
fn satisfiable(constraints: &[SourcedConstraint]) -> Result<bool> {
    let mut solver = Solver::new();
    for sourced in constraints {
        if !add_to_solver(&mut solver, sourced)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn explain_conflict(all: &[SourcedConstraint], failing: usize) -> Result<Vec<SourcedConstraint>> {
    conflict::minimum(all, failing)
}

/// A required relation in a minimum-cardinality conflict, with its source and equation.
#[derive(Debug, Clone)]
pub struct ConflictConstraint {
    pub source: ConstraintProvenance,
    pub equation: String,
}

/// Source-bearing solver failure shared by every diagnostic surface.
#[derive(Debug, Clone)]
pub struct ConstraintConflict {
    pub constraints: Vec<ConflictConstraint>,
}

impl std::fmt::Display for ConstraintConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "required layout constraints conflict: ")?;
        for (index, constraint) in self.constraints.iter().enumerate() {
            if index > 0 {
                write!(f, "; ")?;
            }
            write!(
                f,
                "{} (bytes {}..{})",
                constraint.equation, constraint.source.span.start, constraint.source.span.end
            )?;
        }
        write!(
            f,
            ". Change an `at`, alignment or gap so these required relations agree"
        )
    }
}

impl std::error::Error for ConstraintConflict {}

fn conflict_error(
    conflict: Vec<SourcedConstraint>,
    diagram: &Diagram,
    variables: &[Variables],
) -> anyhow::Error {
    conflict_error_with_names(conflict, diagram, variables, &[])
}

fn conflict_error_with_names(
    conflict: Vec<SourcedConstraint>,
    diagram: &Diagram,
    variables: &[Variables],
    extra: &[(Variable, String)],
) -> anyhow::Error {
    let constraints = conflict
        .into_iter()
        .map(|sourced| {
            let mut terms: Vec<(String, f64)> = sourced
                .constraint
                .expr()
                .terms
                .iter()
                .filter_map(|term| {
                    variables
                        .iter()
                        .enumerate()
                        .find_map(|(index, vars)| {
                            let axis = [
                                (vars.x, "x"),
                                (vars.y, "y"),
                                (vars.w, "width"),
                                (vars.h, "height"),
                            ]
                            .into_iter()
                            .find_map(|(variable, axis)| {
                                (variable == term.variable).then_some(axis)
                            })?;
                            let name = if index < diagram.nodes.len() {
                                diagram.nodes[index].id.as_str()
                            } else {
                                diagram.groups[index - diagram.nodes.len()].id.as_str()
                            };
                            Some((format!("{name}.{axis}"), term.coefficient))
                        })
                        .or_else(|| {
                            extra
                                .iter()
                                .find(|(variable, _)| *variable == term.variable)
                                .map(|(_, name)| (name.clone(), term.coefficient))
                        })
                })
                .collect();
            terms.sort_by(|a, b| a.0.cmp(&b.0));
            let expression = terms
                .into_iter()
                .map(|(name, coefficient)| {
                    if coefficient == 1.0 {
                        name
                    } else {
                        format!("{coefficient}*{name}")
                    }
                })
                .collect::<Vec<_>>()
                .join(" + ");
            ConflictConstraint {
                source: sourced.provenance,
                equation: format!(
                    "{expression} {} {}",
                    sourced.provenance.relation, sourced.provenance.value
                ),
            }
        })
        .collect();
    ConstraintConflict { constraints }.into()
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
    diagram: &'diagram Diagram,
    plan: &'plan PlacementPlan<'diagram>,
    variables: &'plan [Variables],
    default_gap: f64,
    grid_pitch: (f64, f64),
    out: &'out mut Vec<SourcedConstraint>,
}

/// The ordinary cross-axis alignment, and whether a later v2 relation owns
/// that coordinate instead.
#[derive(Clone, Copy)]
struct RelativeAlignment {
    declared: Option<Align>,
    overridden_axes: [bool; 2],
}

fn entity_index(entity: Entity, diagram: &Diagram) -> usize {
    match entity {
        Entity::Node(i) => i,
        Entity::Group(i) => diagram.nodes.len() + i,
    }
}

fn lower_boundary_attachment(
    diagram: &Diagram,
    node: usize,
    boundary: crate::placement::BoundaryAttachment,
    variables: &[Variables],
    out: &mut Vec<SourcedConstraint>,
) {
    let event = variables[node];
    let host = variables[boundary.host];
    let source = provenance(
        ConstraintOrigin::BoundaryAttachment,
        Axis::X,
        node,
        boundary.span,
    )
    .with_anchor(diagram.nodes[boundary.host].id.span)
    .with_anchor_index(boundary.host);
    push(
        out,
        (event.x + event.w * 0.5) | EQ(Strength::REQUIRED) | (host.x + host.w * boundary.along),
        source,
    );
    push(
        out,
        (event.y + event.h * 0.5) | EQ(Strength::REQUIRED) | (host.y + host.h),
        source.with_axis(Axis::Y),
    );
}

fn lower_size(
    vars: Variables,
    node: usize,
    span: Span,
    intrinsic: (f64, f64),
    resolved: (f64, f64),
    out: &mut Vec<SourcedConstraint>,
) {
    for (variable, axis, natural, final_size) in [
        (vars.w, Axis::Width, intrinsic.0, resolved.0),
        (vars.h, Axis::Height, intrinsic.1, resolved.1),
    ] {
        push(
            out,
            if natural == final_size {
                variable | EQ(Strength::REQUIRED) | natural
            } else {
                variable | GE(Strength::REQUIRED) | natural
            },
            provenance(ConstraintOrigin::MeasuredSize, axis, node, span),
        );
    }
}

fn lower_size_equalities(
    plan: &PlacementPlan<'_>,
    variables: &[Variables],
    out: &mut Vec<SourcedConstraint>,
) {
    for item in plan.constraints() {
        let declaration = item.declaration();
        let axis = match declaration.kind {
            ConstraintKind::SameWidth => Axis::Width,
            ConstraintKind::SameHeight => Axis::Height,
            ConstraintKind::AlignGroup => continue,
        };
        let members = plan.constraint_members(item);
        let Some((&lead, rest)) = members.split_first() else {
            continue;
        };
        for &node in rest {
            let (variable, target) = if axis == Axis::Width {
                (variables[node].w, variables[lead].w)
            } else {
                (variables[node].h, variables[lead].h)
            };
            push(
                out,
                variable | EQ(Strength::REQUIRED) | target,
                provenance(
                    ConstraintOrigin::SizeConstraint,
                    axis,
                    node,
                    declaration.span,
                )
                .with_anchor(declaration.members[0].span)
                .with_anchor_index(lead),
            );
        }
    }
}

fn lower_relative(
    dir: RelDir,
    anchor: &str,
    gap: Option<f64>,
    alignment: RelativeAlignment,
    vars: Variables,
    source: ConstraintProvenance,
    lowering: &mut Lowering<'_, '_, '_>,
) -> Result<()> {
    let Some(anchor) = lowering.plan.index().get(anchor).copied() else {
        return Err(anyhow!(
            "the Cassowary shadow cannot resolve node anchor `{anchor}`"
        ));
    };
    let anchor = entity_index(anchor, lowering.diagram);
    let target = lowering.variables[anchor];
    let gap = gap.unwrap_or(lowering.default_gap);
    let gap_x = if matches!(dir, RelDir::RightOf | RelDir::LeftOf) {
        gap * lowering.grid_pitch.0
    } else {
        gap * lowering.grid_pitch.1
    };
    let align = alignment.declared.unwrap_or(Align::Center);

    match dir {
        RelDir::RightOf => {
            push(
                lowering.out,
                vars.x | EQ(Strength::REQUIRED) | (target.x + target.w + gap_x),
                source.with_axis(Axis::X).with_anchor_index(anchor),
            );
            if alignment.declared.is_some() || !alignment.overridden_axes[1] {
                lower_vertical_alignment(
                    vars,
                    target,
                    align,
                    source.with_anchor_index(anchor),
                    lowering.out,
                );
            }
        }
        RelDir::LeftOf => {
            push(
                lowering.out,
                (vars.x + vars.w + gap_x) | EQ(Strength::REQUIRED) | target.x,
                source.with_axis(Axis::X).with_anchor_index(anchor),
            );
            if alignment.declared.is_some() || !alignment.overridden_axes[1] {
                lower_vertical_alignment(
                    vars,
                    target,
                    align,
                    source.with_anchor_index(anchor),
                    lowering.out,
                );
            }
        }
        RelDir::Below => {
            push(
                lowering.out,
                vars.y | EQ(Strength::REQUIRED) | (target.y + target.h + gap_x),
                source.with_axis(Axis::Y).with_anchor_index(anchor),
            );
            if alignment.declared.is_some() || !alignment.overridden_axes[0] {
                lower_horizontal_alignment(
                    vars,
                    target,
                    align,
                    source.with_anchor_index(anchor),
                    lowering.out,
                );
            }
        }
        RelDir::Above => {
            push(
                lowering.out,
                (vars.y + vars.h + gap_x) | EQ(Strength::REQUIRED) | target.y,
                source.with_axis(Axis::Y).with_anchor_index(anchor),
            );
            if alignment.declared.is_some() || !alignment.overridden_axes[0] {
                lower_horizontal_alignment(
                    vars,
                    target,
                    align,
                    source.with_anchor_index(anchor),
                    lowering.out,
                );
            }
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
        relation: "?",
        value: 0.0,
    }
}

fn relation_and_value(constraint: &Constraint) -> (&'static str, f64) {
    (
        match constraint.op() {
            kasuari::RelationalOperator::LessOrEqual => "<=",
            kasuari::RelationalOperator::Equal => "==",
            kasuari::RelationalOperator::GreaterOrEqual => ">=",
        },
        if constraint.expr().constant == 0.0 {
            0.0
        } else {
            -constraint.expr().constant
        },
    )
}

fn entity_name(diagram: &Diagram, index: usize) -> &str {
    if index < diagram.nodes.len() {
        diagram.nodes[index].id.as_str()
    } else {
        diagram.groups[index - diagram.nodes.len()].id.as_str()
    }
}

fn sort_constraints(constraints: &mut [SourcedConstraint], diagram: &Diagram) {
    constraints.sort_by_key(|sourced| {
        (
            sourced.provenance.origin,
            sourced.provenance.axis,
            entity_name(diagram, sourced.provenance.node),
            sourced
                .provenance
                .anchor
                .map(|anchor| entity_name(diagram, anchor)),
            sourced.provenance.span.start,
            sourced.provenance.span.end,
        )
    });
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
    let (relation, value) = relation_and_value(&constraint);
    out.push(SourcedConstraint {
        constraint,
        provenance: ConstraintProvenance {
            relation,
            value,
            ..provenance
        },
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_reduction_keeps_only_an_irreducible_contradiction() {
        let x = Variable::new();
        let unrelated = Variable::new();
        let mut constraints = Vec::new();
        for (index, constraint) in [
            unrelated | EQ(Strength::REQUIRED) | 12.0,
            x | EQ(Strength::REQUIRED) | 0.0,
            x | EQ(Strength::REQUIRED) | 40.0,
        ]
        .into_iter()
        .enumerate()
        {
            push(
                &mut constraints,
                constraint,
                provenance(
                    ConstraintOrigin::AbsolutePlacement,
                    Axis::X,
                    index,
                    Span {
                        start: index,
                        end: index + 1,
                    },
                ),
            );
        }
        let conflict = explain_conflict(&constraints, 2).unwrap();
        assert_eq!(conflict.len(), 2);
        assert!(!satisfiable(&conflict).unwrap());
        for index in 0..conflict.len() {
            let mut reduced = conflict.clone();
            reduced.remove(index);
            assert!(satisfiable(&reduced).unwrap());
        }
        assert_eq!(conflict[0].provenance.span.start, 1);
        assert_eq!(conflict[1].provenance.value, 40.0);
    }

    #[test]
    fn contradictory_absolute_alignment_reports_each_actual_layout_statement() {
        let source =
            "node a\nnode b\nlayout {\n place a at (0, 0)\n place b at (0, 80)\n align top a b\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let error = crate::layout::solve(&diagram).err().unwrap();
        let conflict = error.downcast_ref::<ConstraintConflict>().unwrap();
        assert_eq!(conflict.constraints.len(), 3, "{error}");
        let diagnostics = crate::check::layout_error(&diagram, &error);
        assert_eq!(diagnostics.len(), 3);
        for diagnostic in diagnostics {
            let statement = &source[diagnostic.span.start..diagnostic.span.end];
            assert!(
                statement.starts_with("place ") || statement.starts_with("align "),
                "{statement}"
            );
            assert!(
                diagnostic.message.contains("a.y")
                    && diagnostic.message.contains("b.y")
                    && diagnostic.message.contains("80"),
                "{}",
                diagnostic.message
            );
        }
    }

    #[test]
    fn declared_geometry_is_stable_when_declarations_and_layout_are_permuted() {
        let declarations = [
            "node a { size: 20 20 }",
            "node b { size: 40 30 }",
            "node c { size: 10 50 }",
        ];
        let statements = [
            "place a at (10, 20)",
            "place b right-of a gap 7",
            "place c below b gap 9",
            "align left a c",
        ];
        let mut expected = None;
        for node_order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            for statement_order in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]] {
                let source = format!(
                    "{}\nlayout {{\n{}\n}}",
                    node_order.map(|i| declarations[i]).join("\n"),
                    statement_order.map(|i| statements[i]).join("\n")
                );
                let diagram = crate::parser::parse(&source).unwrap();
                let layout = crate::layout::solve(&diagram).unwrap();
                let mut actual: Vec<_> = layout
                    .nodes
                    .into_iter()
                    .map(|node| {
                        (
                            node.id,
                            [node.rect.x, node.rect.y, node.rect.w, node.rect.h],
                        )
                    })
                    .collect();
                actual.sort_by(|a, b| a.0.cmp(&b.0));
                if let Some(expected) = &expected {
                    assert_eq!(&actual, expected);
                } else {
                    expected = Some(actual);
                }
            }
        }
    }

    #[test]
    fn simultaneous_alignments_override_both_implicit_cross_axes() {
        let source = "node a { size: 20 20 }\nnode b { size: 20 20 }\nnode c { size: 20 20 }\nlayout {\n place a at (0, 0)\n place b at (52, 100)\n place c right-of a gap 32\n align top b c\n align left b c\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        assert_eq!(layout.nodes[2].rect.x, 52.0);
        assert_eq!(layout.nodes[2].rect.y, 100.0);
    }

    #[test]
    fn explicit_relative_alignment_cannot_be_replaced_by_another_alignment() {
        let source = "node a { size: 20 20 }\nnode b { size: 20 20 }\nnode c { size: 20 20 }\nlayout {\n place a at (0, 0)\n place b at (52, 100)\n place c right-of a gap 32 align top\n align top b c\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let error = crate::layout::solve(&diagram).err().unwrap();
        assert!(
            error.downcast_ref::<ConstraintConflict>().is_some(),
            "{error}"
        );
    }

    #[test]
    fn simultaneous_frame_alignment_propagates_to_its_dependents() {
        let source = "node lead { size: 20 20 }\ngroup g {\n padding: 0\n node a { size: 20 20 }\n node b { size: 20 20 }\n node c { size: 20 20 }\n node extent { size: 20 20 }\n}\nlayout {\n place lead at (0, 100)\n place a at (0, 0)\n place b right-of a gap 30\n place c below b gap 10\n place extent at (0, 300)\n align top lead b\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        let diagnostics = crate::check::after_layout(&diagram, &layout);
        assert_eq!(layout.nodes[3].rect.y, 130.0);
        assert!(
            !diagnostics
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn nested_placed_frames_keep_their_local_absolute_origins() {
        let source = "group outer {\n padding: 10\n node a { size: 20 20 }\n group inner {\n padding: 5\n node b { size: 20 20 }\n layout { place b at (100, 0) }\n }\n layout {\n place a at (0, 0)\n place inner at (50, 40)\n }\n}\nlayout { place outer at (200, 100) }";
        let diagram = crate::parser::parse(source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        assert_eq!(
            (layout.nodes[0].rect.x, layout.nodes[0].rect.y),
            (210.0, 110.0)
        );
        assert_eq!(
            (layout.nodes[1].rect.x, layout.nodes[1].rect.y),
            (265.0, 155.0)
        );
        let outer = layout
            .groups
            .iter()
            .find(|group| group.id == "outer")
            .unwrap()
            .rect;
        let inner = layout
            .groups
            .iter()
            .find(|group| group.id == "inner")
            .unwrap()
            .rect;
        assert_eq!(
            (outer.x, outer.y, outer.w, outer.h),
            (200.0, 100.0, 100.0, 90.0)
        );
        assert_eq!(
            (inner.x, inner.y, inner.w, inner.h),
            (260.0, 150.0, 30.0, 30.0)
        );
    }

    #[test]
    fn frame_preferences_cannot_enlarge_a_frame_to_hide_a_source_contradiction() {
        let source = "node anchor { size: 20 20 }\ngroup g {\n padding: 0\n node a { size: 20 20 }\n node b { size: 20 20 }\n layout {\n place a at (0, 0)\n place b at (100, 0)\n }\n}\nlayout {\n place anchor at (50, 0)\n place g at (0, 0)\n align left anchor a\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let error = crate::layout::solve(&diagram).err().unwrap();
        let diagnostics = crate::check::layout_error(&diagram, &error);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .contains("cannot retain the exact bounds"),
            "{error}"
        );
        assert_eq!(diagnostics[0].span, diagram.groups[0].span);
    }

    #[test]
    fn translated_frame_members_can_anchor_an_external_node() {
        let source = "group g {\n padding: 10\n node a { size: 40 30 }\n layout { place a at (100, 50) }\n}\nnode b { size: 20 20 }\nlayout {\n place g at (200, 100)\n place b right-of a gap 25 align bottom\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        assert_eq!(
            (layout.nodes[0].rect.x, layout.nodes[0].rect.y),
            (210.0, 110.0)
        );
        assert_eq!(
            (layout.nodes[1].rect.x, layout.nodes[1].rect.y),
            (275.0, 120.0)
        );
        assert!(
            !crate::check::after_layout(&diagram, &layout)
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error)
        );
    }

    #[test]
    fn size_conflicts_include_the_actual_transitive_equal_width_chain() {
        let source = "node a { size: 20 20 }\nnode b { size: 20 20 }\nnode c { size: 60 20 }\nnode d { size: 20 20 }\nlayout {\n place a at (0, 0)\n place b below a\n place c below b\n place d at (20, 0)\n equal-width a b\n equal-width b c\n align right a d\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let error = crate::layout::solve(&diagram).err().unwrap();
        let conflict = error.downcast_ref::<ConstraintConflict>().unwrap();
        let spans: Vec<_> = conflict
            .constraints
            .iter()
            .filter(|constraint| constraint.source.origin == ConstraintOrigin::SizeConstraint)
            .map(|constraint| &source[constraint.source.span.start..constraint.source.span.end])
            .collect();
        assert!(spans.contains(&"equal-width a b"), "{error}");
        assert!(spans.contains(&"equal-width b c"), "{error}");
    }

    #[test]
    fn size_conflicts_include_the_actual_transitive_equal_height_chain() {
        let source = "node a { size: 20 20 }\nnode b { size: 20 20 }\nnode c { size: 20 60 }\nnode d { size: 20 20 }\nlayout {\n place a at (0, 0)\n place b right-of a\n place c right-of b\n place d at (0, 20)\n equal-height a b\n equal-height b c\n align bottom a d\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let error = crate::layout::solve(&diagram).err().unwrap();
        let conflict = error.downcast_ref::<ConstraintConflict>().unwrap();
        let spans: Vec<_> = conflict
            .constraints
            .iter()
            .filter(|constraint| constraint.source.origin == ConstraintOrigin::SizeConstraint)
            .map(|constraint| &source[constraint.source.span.start..constraint.source.span.end])
            .collect();
        assert!(spans.contains(&"equal-height a b"), "{error}");
        assert!(spans.contains(&"equal-height b c"), "{error}");
    }

    #[test]
    #[cfg(feature = "state")]
    fn frame_bounds_include_a_title_and_a_glyph_caption() {
        let source = "type state\ngroup g \"State machine title\" {\n padding: 10\n state start \"Caption outside the initial glyph\" { kind: initial }\n}\nlayout { place g at (100, 80) }";
        let diagram = crate::parser::parse(source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        let frame = layout.groups[0].rect;
        let node = layout.nodes[0].rect;
        assert_eq!((frame.x, frame.y), (100.0, 80.0));
        assert!(
            frame.w > node.w + 20.0,
            "caption or title clipped: {frame:?}"
        );
        assert!(frame.h > node.h + 36.0, "caption clipped: {frame:?}");
        assert!(
            !crate::check::after_layout(&diagram, &layout)
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error)
        );
    }

    #[test]
    fn large_graphs_reject_a_contradictory_required_alignment() {
        let mut source = String::from("node a\nnode b\n");
        for index in 2..=512 {
            source.push_str(&format!("node n{index}\n"));
        }
        source.push_str("layout {\n place a at (0, 0)\n place b at (0, 80)\n align top a b\n}");
        let diagram = crate::parser::parse(&source).unwrap();
        let error = crate::layout::solve(&diagram).err().unwrap();
        let diagnostics = crate::check::layout_error(&diagram, &error);
        assert!(
            diagnostics
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error
                    && diag
                        .message
                        .contains("required layout constraints conflict")),
            "{diagnostics:?}"
        );
    }

    #[test]
    fn large_graphs_retain_nested_frame_local_absolute_origins() {
        let mut source = String::from(
            "group outer {\n padding: 10\n group inner {\n padding: 5\n node a { size: 20 20 }\n layout { place a at (100, 0) }\n }\n layout { place inner at (50, 40) }\n}\nlayout { place outer at (200, 100) }\n",
        );
        for index in 0..512 {
            source.push_str(&format!("node filler{index} {{ size: 1 1 }}\n"));
        }
        let diagram = crate::parser::parse(&source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        assert_eq!(
            (layout.nodes[0].rect.x, layout.nodes[0].rect.y),
            (215.0, 115.0)
        );
        let inner = layout
            .groups
            .iter()
            .find(|group| group.id == "inner")
            .unwrap()
            .rect;
        assert_eq!((inner.x, inner.y), (210.0, 110.0));
        assert!(
            !crate::check::after_layout(&diagram, &layout)
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error)
        );
    }

    #[test]
    fn frame_absolute_placement_uses_the_same_bounded_grid_as_nodes() {
        let source = "graph { grid: 4 3 20 30 }\nnode anchor { size: 100 60 }\ngroup g {\n padding: 0\n node a { size: 100 60 }\n}\nlayout {\n place anchor at (0, 0)\n place g at (2, 1)\n}";
        let diagram = crate::parser::parse(source).unwrap();
        let layout = crate::layout::solve(&diagram).unwrap();
        assert_eq!(layout.groups[0].rect.x, 240.0);
        assert_eq!(layout.groups[0].rect.y, 90.0);
        assert!(
            !crate::check::after_layout(&diagram, &layout)
                .iter()
                .any(|diag| diag.severity == crate::check::Severity::Error)
        );
    }

    #[test]
    fn frame_relative_gaps_use_the_bounded_grid_axis_pitch() {
        for (relation, expected) in [("right-of", (340.0, 0.0)), ("below", (0.0, 240.0))] {
            let source = format!(
                "graph {{ grid: 4 3 20 30 }}\nnode anchor {{ size: 100 60 }}\ngroup g {{\n padding: 0\n node a {{ size: 100 60 }}\n}}\nlayout {{\n place anchor at (0, 0)\n place g {relation} anchor gap 2\n}}"
            );
            let diagram = crate::parser::parse(&source).unwrap();
            let layout = crate::layout::solve(&diagram).unwrap();
            assert_eq!(
                (layout.groups[0].rect.x, layout.groups[0].rect.y),
                expected,
                "{relation}"
            );
            assert!(
                !crate::check::after_layout(&diagram, &layout)
                    .iter()
                    .any(|diag| diag.severity == crate::check::Severity::Error)
            );
        }
    }

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
