//! Normalized view of the placement declarations shared by every front end.
//!
//! The parser keeps the source shape. This module answers the structural
//! questions that used to be reimplemented by the validator, solver,
//! `explain` and `layout`: which names are entities, which container owns an
//! entity, and which sibling supplies an omitted placement.

use std::collections::HashMap;

use crate::ast::{Diagram, Placement, RelDir, Spanned};

/// A named entity that can participate in placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Entity {
    Node(usize),
    Group(usize),
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

        Self {
            diagram,
            index,
            owner,
            previous_sibling,
        }
    }

    pub fn index(&self) -> &HashMap<&'a str, Entity> {
        &self.index
    }

    pub fn owner(&self) -> &HashMap<Entity, usize> {
        &self.owner
    }

    pub fn implicit_anchor(&self, node: usize) -> Option<usize> {
        self.previous_sibling[node]
    }

    pub fn node(&self, node: usize, flow: RelDir) -> EffectivePlacement<'a> {
        if let Some(placement) = self.diagram.nodes[node].placement.as_ref() {
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
            if self.diagram.groups[group].placement.is_some() {
                return Some(group);
            }
            cursor = self.owner.get(&Entity::Group(group)).copied();
        }
        None
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
}
