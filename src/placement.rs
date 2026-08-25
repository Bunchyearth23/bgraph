//! Normalized view of the placement declarations shared by every front end.
//!
//! The parser keeps the source shape. This module answers the structural
//! questions that used to be reimplemented by the validator, solver,
//! `explain` and `layout`: which names are entities, which container owns an
//! entity, and which sibling supplies an omitted placement.

use std::collections::HashMap;

use crate::ast::{
    ConstraintAxis, ConstraintDecl, ConstraintKind, Diagram, LayoutStmt, Placement, RelDir, Spanned,
};

/// A named entity that can participate in placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entity {
    Node(usize),
    Group(usize),
}

/// A legacy or scoped-v2 relation, lowered through one consumer interface.
#[derive(Clone, Copy)]
pub struct ConstraintRef<'a> {
    declaration: &'a ConstraintDecl,
    v2: bool,
}

impl<'a> ConstraintRef<'a> {
    pub fn declaration(self) -> &'a ConstraintDecl {
        self.declaration
    }

    pub fn is_v2(self) -> bool {
        self.v2
    }
}

/// The effective placement of a node, including the implicit flow rule.
pub enum EffectivePlacement<'a> {
    /// A placement written on the declaration.
    Declared(&'a Placement),
    /// No placement was written; flow from the previous sibling.
    Flow {
        dir: RelDir,
        anchor: &'a Spanned<String>,
    },
    /// The first node in a container starts at that container's origin.
    Origin,
}

/// Structural placement facts derived once from a diagram.
pub struct PlacementPlan<'a> {
    diagram: &'a Diagram,
    index: HashMap<&'a str, Entity>,
    owner: HashMap<Entity, usize>,
    previous_sibling: Vec<Option<usize>>,
    layout_placement: Vec<Option<&'a Placement>>,
    layout_group_placement: Vec<Option<&'a Placement>>,
}

impl<'a> PlacementPlan<'a> {
    pub fn new(diagram: &'a Diagram) -> Self {
        let mut index = HashMap::new();
        for (i, node) in diagram.nodes.iter().enumerate() {
            index.insert(node.id.as_str(), Entity::Node(i));
        }
        for (i, group) in diagram.groups.iter().enumerate() {
            index.insert(group.id.as_str(), Entity::Group(i));
        }

        let mut owner = HashMap::new();
        for (g, group) in diagram.groups.iter().enumerate() {
            for member in &group.members {
                if let Some(entity) = index.get(member.as_str()) {
                    owner.insert(*entity, g);
                }
            }
        }

        let mut previous: HashMap<Option<usize>, usize> = HashMap::new();
        let mut previous_sibling = Vec::with_capacity(diagram.nodes.len());
        for i in 0..diagram.nodes.len() {
            let container = owner.get(&Entity::Node(i)).copied();
            previous_sibling.push(previous.get(&container).copied());
            previous.insert(container, i);
        }

        let mut layout_placement = vec![None; diagram.nodes.len()];
        let mut layout_group_placement = vec![None; diagram.groups.len()];
        for layout in &diagram.layouts {
            for statement in &layout.statements {
                if let LayoutStmt::Place {
                    target, placement, ..
                } = statement
                {
                    match index.get(target.as_str()).copied() {
                        Some(Entity::Node(node)) => layout_placement[node] = Some(placement),
                        Some(Entity::Group(group)) => {
                            layout_group_placement[group] = Some(placement)
                        }
                        None => {}
                    }
                }
            }
        }

        Self {
            diagram,
            index,
            owner,
            previous_sibling,
            layout_placement,
            layout_group_placement,
        }
    }

    pub fn index(&self) -> &HashMap<&'a str, Entity> {
        &self.index
    }

    /// Constraints resolve their member names through the same namespace as
    /// every other placement relation. Validation reports non-node members;
    /// consumers can therefore skip them without inventing a second error.
    pub fn constraint_members(&self, constraint: ConstraintRef<'a>) -> Vec<usize> {
        constraint
            .declaration()
            .members
            .iter()
            .filter_map(|member| match self.index.get(member.as_str()).copied() {
                Some(Entity::Node(node)) => Some(node),
                Some(Entity::Group(_)) | None => None,
            })
            .collect()
    }

    pub fn constraints(&self) -> impl Iterator<Item = ConstraintRef<'a>> + '_ {
        self.diagram
            .constraints
            .iter()
            .map(|declaration| ConstraintRef {
                declaration,
                v2: false,
            })
            .chain(self.diagram.layouts.iter().flat_map(|layout| {
                layout
                    .statements
                    .iter()
                    .filter_map(|statement| match statement {
                        LayoutStmt::Constraint { constraint } => Some(ConstraintRef {
                            declaration: constraint,
                            v2: true,
                        }),
                        LayoutStmt::Place { .. } => None,
                    })
            }))
    }

    /// A v2 alignment is applied after ordinary placement, so it owns that
    /// cross-axis coordinate. The Cassowary shadow uses this to avoid keeping
    /// the temporary placement alignment as a competing required equation.
    pub fn v2_alignment_axis(&self, node: usize) -> Option<ConstraintAxis> {
        self.constraints()
            .filter(|constraint| {
                constraint.is_v2()
                    && constraint.declaration().kind == ConstraintKind::AlignGroup
                    && constraint
                        .declaration()
                        .members
                        .iter()
                        .skip(1)
                        .any(|member| {
                            matches!(
                                self.index.get(member.as_str()),
                                Some(Entity::Node(index)) if *index == node
                            )
                        })
            })
            .map(|constraint| constraint.declaration().axis)
            .last()
    }

    pub fn owner(&self) -> &HashMap<Entity, usize> {
        &self.owner
    }

    pub fn implicit_anchor(&self, node: usize) -> Option<usize> {
        self.previous_sibling[node]
    }

    pub fn declared_placement(&self, node: usize) -> Option<&'a Placement> {
        self.layout_placement[node].or(self.diagram.nodes[node].placement.as_ref())
    }

    pub fn declared_group_placement(&self, group: usize) -> Option<&'a Placement> {
        self.layout_group_placement[group].or(self.diagram.groups[group].placement.as_ref())
    }

    pub fn node(&self, node: usize, flow: RelDir) -> EffectivePlacement<'a> {
        if let Some(placement) = self.declared_placement(node) {
            EffectivePlacement::Declared(placement)
        } else if let Some(anchor) = self.implicit_anchor(node) {
            EffectivePlacement::Flow {
                dir: flow,
                anchor: &self.diagram.nodes[anchor].id,
            }
        } else {
            EffectivePlacement::Origin
        }
    }

    /// The innermost ancestor group whose placement will translate `entity`.
    pub fn placed_unit(&self, entity: Entity) -> Option<usize> {
        let mut cursor = self.owner.get(&entity).copied();
        let mut seen = Vec::new();
        while let Some(group) = cursor {
            if seen.contains(&group) {
                return None;
            }
            seen.push(group);
            if self.declared_group_placement(group).is_some() {
                return Some(group);
            }
            cursor = self.owner.get(&Entity::Group(group)).copied();
        }
        None
    }

    /// Entities whose coordinates or final position can never become ready.
    ///
    /// This mirrors the solver's two states without doing geometry: a member
    /// of a placed group can have coordinates while still waiting for the
    /// group translation. Keeping that distinction is what catches a frame
    /// placed relative to one of its own descendants.
    pub fn unresolved(&self) -> Vec<Entity> {
        let mut node_coordinates = vec![false; self.diagram.nodes.len()];
        let mut group_coordinates = vec![false; self.diagram.groups.len()];
        let mut node_settled = vec![false; self.diagram.nodes.len()];
        let mut group_settled = vec![false; self.diagram.groups.len()];

        let has_coordinates = |entity: Entity, nodes: &[bool], groups: &[bool]| match entity {
            Entity::Node(i) => nodes[i],
            Entity::Group(i) => groups[i],
        };
        let is_settled = |entity: Entity, nodes: &[bool], groups: &[bool]| match entity {
            Entity::Node(i) => nodes[i],
            Entity::Group(i) => groups[i],
        };

        loop {
            let mut progress = false;

            for i in 0..self.diagram.nodes.len() {
                if node_coordinates[i] {
                    continue;
                }
                let entity = Entity::Node(i);
                let dependency = match self.node(i, RelDir::RightOf) {
                    EffectivePlacement::Declared(Placement::Rel { anchor, .. }) => {
                        self.index.get(anchor.as_str()).copied()
                    }
                    EffectivePlacement::Flow { anchor, .. } => {
                        self.index.get(anchor.as_str()).copied()
                    }
                    EffectivePlacement::Declared(Placement::At { .. } | Placement::Cell { .. })
                    | EffectivePlacement::Origin => None,
                };
                let ready = dependency.is_none_or(|target| {
                    // Self references and unknown names are diagnosed by the
                    // name validator and must not masquerade as a second error.
                    target == entity
                        || if self.placed_unit(entity).is_some()
                            && self.placed_unit(entity) == self.placed_unit(target)
                        {
                            has_coordinates(target, &node_coordinates, &group_coordinates)
                        } else {
                            is_settled(target, &node_settled, &group_settled)
                        }
                });
                if ready {
                    node_coordinates[i] = true;
                    node_settled[i] = self.placed_unit(entity).is_none();
                    progress = true;
                }
            }

            for g in 0..self.diagram.groups.len() {
                if group_coordinates[g] {
                    continue;
                }
                let group = &self.diagram.groups[g];
                let members_ready = group
                    .members
                    .iter()
                    .filter_map(|member| self.index.get(member.as_str()).copied())
                    .all(|member| has_coordinates(member, &node_coordinates, &group_coordinates));
                if !members_ready {
                    continue;
                }

                let entity = Entity::Group(g);
                let anchor_ready = match self.declared_group_placement(g) {
                    Some(Placement::Rel { anchor, .. }) => self
                        .index
                        .get(anchor.as_str())
                        .copied()
                        .is_none_or(|target| {
                            target == entity || is_settled(target, &node_settled, &group_settled)
                        }),
                    Some(Placement::At { .. } | Placement::Cell { .. }) | None => true,
                };
                if !anchor_ready {
                    continue;
                }

                group_coordinates[g] = true;
                group_settled[g] = true;
                self.settle_subtree(g, &mut node_settled, &mut group_settled);
                progress = true;
            }

            let done = node_coordinates.iter().all(|ready| *ready)
                && group_coordinates.iter().all(|ready| *ready)
                && node_settled.iter().all(|ready| *ready)
                && group_settled.iter().all(|ready| *ready);
            if done {
                return Vec::new();
            }
            if !progress {
                return (0..self.diagram.nodes.len())
                    .filter(|i| !node_coordinates[*i] || !node_settled[*i])
                    .map(Entity::Node)
                    .chain(
                        (0..self.diagram.groups.len())
                            .filter(|g| !group_coordinates[*g] || !group_settled[*g])
                            .map(Entity::Group),
                    )
                    .collect();
            }
        }
    }

    fn settle_subtree(&self, group: usize, node_settled: &mut [bool], group_settled: &mut [bool]) {
        for member in &self.diagram.groups[group].members {
            match self.index.get(member.as_str()) {
                Some(Entity::Node(i)) => node_settled[*i] = true,
                Some(Entity::Group(g)) => {
                    group_settled[*g] = true;
                    self.settle_subtree(*g, node_settled, group_settled);
                }
                None => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implicit_flow_is_scoped_to_the_container() {
        let diagram = crate::parser::parse_diagram(
            "node outside\ngroup g {\n node first\n node second\n}\nnode after",
        )
        .unwrap();
        let plan = PlacementPlan::new(&diagram);

        assert_eq!(plan.implicit_anchor(0), None);
        assert_eq!(plan.implicit_anchor(1), None);
        assert_eq!(plan.implicit_anchor(2), Some(1));
        assert_eq!(plan.implicit_anchor(3), Some(0));
    }

    #[test]
    fn the_innermost_placed_ancestor_is_the_translation_unit() {
        let diagram = crate::parser::parse_diagram(
            "group outer at (10, 10) {\n group inner {\n node a\n }\n}",
        )
        .unwrap();
        let plan = PlacementPlan::new(&diagram);

        assert_eq!(plan.placed_unit(Entity::Node(0)), Some(1));
        assert_eq!(plan.placed_unit(Entity::Group(0)), Some(1));
    }

    #[test]
    fn a_group_waiting_for_its_own_member_is_unresolved() {
        let diagram = crate::parser::parse_diagram("group g right-of a {\n node a\n}").unwrap();
        let plan = PlacementPlan::new(&diagram);

        assert_eq!(plan.unresolved(), [Entity::Node(0), Entity::Group(0)]);
    }

    #[test]
    fn constraints_share_the_placement_namespace() {
        let diagram =
            crate::parser::parse_diagram("group g { node a }\nnode b\nsame-width a g missing b")
                .unwrap();
        let plan = PlacementPlan::new(&diagram);

        assert_eq!(
            plan.constraint_members(plan.constraints().next().unwrap()),
            [0, 1]
        );
    }
}
