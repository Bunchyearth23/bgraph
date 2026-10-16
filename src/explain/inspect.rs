//! Source relations shared by the inspector, navigation and hover.

use crate::ast::{ConstraintAxis, ConstraintKind, Diagram, LayoutStmt, Placement, RelDir, Span};
use crate::layout::{Layout, Rect};
use crate::placement::{EffectivePlacement, Entity, PlacementPlan};
use crate::style::GraphOpts;

pub(crate) struct Relation {
    pub span: Span,
    pub reference: Span,
    pub axes: &'static str,
    pub peers: Vec<String>,
    pub controls: bool,
}

/// Every structural mention of an entity in layout, with controlling statements first.
pub(crate) fn relations(diagram: &Diagram, id: &str) -> Vec<Relation> {
    let mut out = Vec::new();
    for layout in &diagram.layouts {
        for statement in &layout.statements {
            match statement {
                LayoutStmt::Place {
                    target,
                    placement,
                    span,
                } => {
                    let anchor = match placement {
                        Placement::Rel { anchor, .. } => Some(anchor),
                        _ => None,
                    };
                    let controls = target.as_str() == id;
                    let reference = if controls {
                        Some(target.span)
                    } else {
                        anchor
                            .filter(|anchor| anchor.as_str() == id)
                            .map(|anchor| anchor.span)
                    };
                    if let Some(reference) = reference {
                        out.push(Relation {
                            span: *span,
                            reference,
                            axes: "X/Y",
                            controls,
                            peers: if controls {
                                anchor
                                    .map(|anchor| vec![anchor.value.clone()])
                                    .unwrap_or_default()
                            } else {
                                vec![target.value.clone()]
                            },
                        });
                    }
                }
                LayoutStmt::Series {
                    members, dir, span, ..
                } => {
                    if let Some(member) = members.iter().find(|member| member.as_str() == id) {
                        out.push(Relation {
                            span: *span,
                            reference: member.span,
                            axes: if matches!(dir, RelDir::RightOf | RelDir::LeftOf) {
                                "X"
                            } else {
                                "Y"
                            },
                            peers: members
                                .iter()
                                .filter(|member| member.as_str() != id)
                                .map(|member| member.value.clone())
                                .collect(),
                            controls: true,
                        });
                    }
                }
                LayoutStmt::Constraint { constraint } => {
                    if let Some(member) = constraint
                        .members
                        .iter()
                        .find(|member| member.as_str() == id)
                    {
                        out.push(Relation {
                            span: constraint.span,
                            reference: member.span,
                            axes: match constraint.kind {
                                ConstraintKind::SameWidth => "width",
                                ConstraintKind::SameHeight => "height",
                                ConstraintKind::AlignGroup => match constraint.axis {
                                    ConstraintAxis::Horizontal => "X",
                                    ConstraintAxis::Vertical => "Y",
                                    ConstraintAxis::Inferred => "X/Y",
                                },
                            },
                            peers: constraint
                                .members
                                .iter()
                                .filter(|member| member.as_str() != id)
                                .map(|member| member.value.clone())
                                .collect(),
                            controls: true,
                        });
                    }
                }
            }
        }
    }
    out.sort_by_key(|relation| (!relation.controls, relation.span.start));
    out
}

pub(crate) struct Inspection {
    pub id: String,
    pub kind: &'static str,
    pub declaration: Span,
    pub span: Span,
    pub parent: Option<String>,
    pub bounds: Option<Rect>,
    pub anchor: Option<String>,
    pub reason: String,
    pub relations: Vec<Relation>,
}

pub(crate) fn entities(diagram: &Diagram, layout: Option<&Layout>) -> Vec<Inspection> {
    let plan = PlacementPlan::new(diagram);
    let opts = GraphOpts::from_props(&diagram.graph).unwrap_or_default();
    let mut out = Vec::new();
    for (index, node) in diagram.nodes.iter().enumerate() {
        let (anchor, fallback) = match plan.node(index, opts.flow) {
            EffectivePlacement::Boundary(boundary) => (
                Some(diagram.nodes[boundary.host].id.value.clone()),
                "Required boundary attachment",
            ),
            EffectivePlacement::Declared(Placement::Rel { anchor, .. }) => {
                (Some(anchor.value.clone()), "Declared relative placement")
            }
            EffectivePlacement::Declared(_) => (None, "Declared absolute or grid placement"),
            EffectivePlacement::Flow { anchor, .. } => (
                Some(anchor.value.clone()),
                "Implicit flow from the previous sibling",
            ),
            EffectivePlacement::Series { anchor, .. } => {
                (Some(anchor.value.clone()), "Declared row or column")
            }
            EffectivePlacement::Origin => (None, "First item at its container origin"),
        };
        let parent = plan
            .owner()
            .get(&Entity::Node(index))
            .map(|parent| diagram.groups[*parent].id.value.clone());
        let bounds = layout.and_then(|layout| {
            layout
                .nodes
                .iter()
                .find(|placed| placed.id == node.id.value)
                .map(|placed| placed.rect)
        });
        let managed = crate::check::archetype_of(diagram).layout_capability()
            == crate::props::LayoutCapability::ArchetypeManaged;
        let reason = if managed {
            format!(
                "Geometry is managed by type {}.\n\npaint\n{}",
                crate::check::archetype_of(diagram).name,
                super::paint(diagram, node, node.id.as_str())
                    .unwrap_or_else(|error| error.to_string())
            )
        } else {
            layout
                .and_then(|layout| super::explain(diagram, layout, node.id.as_str()).ok())
                .unwrap_or_else(|| fallback.to_string())
        };
        let mut related = relations(diagram, node.id.as_str());
        if let Some(boundary) = plan.boundary(index) {
            related.push(Relation {
                span: boundary.span,
                reference: boundary.span,
                axes: "X/Y",
                peers: vec![diagram.nodes[boundary.host].id.value.clone()],
                controls: true,
            });
        }
        out.push(Inspection {
            id: node.id.value.clone(),
            kind: "node",
            declaration: node.id.span,
            span: node.span,
            parent,
            bounds,
            anchor,
            reason,
            relations: related,
        });
    }
    for (index, group) in diagram.groups.iter().enumerate() {
        let parent = plan
            .owner()
            .get(&Entity::Group(index))
            .map(|parent| diagram.groups[*parent].id.value.clone());
        let bounds = layout.and_then(|layout| {
            layout
                .groups
                .iter()
                .find(|placed| placed.id == group.id.value)
                .map(|placed| placed.rect)
        });
        let anchor = match plan.declared_group_placement(index) {
            Some(Placement::Rel { anchor, .. }) => Some(anchor.value.clone()),
            _ => None,
        };
        out.push(Inspection {
            id: group.id.value.clone(), kind: "group", declaration: group.id.span,
            span: group.span, parent, bounds, anchor,
            reason: layout.and_then(|layout| super::explain(diagram, layout, group.id.as_str()).ok()).unwrap_or_else(|| "Frame bounds enclose the children, title and padding; follow the declared source relations.".to_string()),
            relations: relations(diagram, group.id.as_str()),
        });
    }
    out
}
