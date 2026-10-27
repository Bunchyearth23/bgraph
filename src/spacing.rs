//! External spacing shared by declared boxes and sequence frames.

use crate::{
    ast::{Diagram, RelDir, Value},
    props::ArchetypeDef,
    style::{Registry, Style},
};

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Margins {
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
    pub left: f64,
}

impl Margins {
    pub fn parse(values: &[Value]) -> Option<Self> {
        let numbers: Vec<_> = values.iter().map(Value::as_num).collect::<Option<_>>()?;
        if numbers.iter().any(|n| !n.is_finite() || *n < 0.0) {
            return None;
        }
        let (top, right, bottom, left) = match numbers.as_slice() {
            [all] => (*all, *all, *all, *all),
            [vertical, horizontal] => (*vertical, *horizontal, *vertical, *horizontal),
            [top, horizontal, bottom] => (*top, *horizontal, *bottom, *horizontal),
            [top, right, bottom, left] => (*top, *right, *bottom, *left),
            _ => return None,
        };
        Some(Self {
            top,
            right,
            bottom,
            left,
        })
    }

    pub fn of(style: &Style) -> Self {
        Self {
            top: style.margin_top.unwrap_or(0.0),
            right: style.margin_right.unwrap_or(0.0),
            bottom: style.margin_bottom.unwrap_or(0.0),
            left: style.margin_left.unwrap_or(0.0),
        }
    }

    pub fn gap_to(self, anchor: Self, direction: RelDir) -> f64 {
        match direction {
            RelDir::RightOf => anchor.right + self.left,
            RelDir::LeftOf => anchor.left + self.right,
            RelDir::Below => anchor.bottom + self.top,
            RelDir::Above => anchor.top + self.bottom,
        }
    }
}

pub(crate) fn is_margin(name: &str) -> bool {
    matches!(
        name,
        "margin" | "margin-top" | "margin-right" | "margin-bottom" | "margin-left"
    )
}

pub(crate) fn supported(def: &ArchetypeDef) -> bool {
    def.layout.accepts_layout_block() || def.name == "sequence"
}

pub(crate) fn entities(diagram: &Diagram) -> anyhow::Result<Vec<Margins>> {
    let registry = Registry::build(&diagram.styles)?;
    let def = crate::check::archetype_of(diagram);
    let mut out = Vec::with_capacity(diagram.nodes.len() + diagram.groups.len());
    for node in &diagram.nodes {
        let style = registry.resolve(
            Style::default(),
            def.style_word(node.keyword.as_str()),
            &node.classes,
            &node.props_for(def),
        )?;
        out.push(Margins::of(&style));
    }
    for group in &diagram.groups {
        let style = registry.resolve(Style::default(), "group", &group.classes, &group.props)?;
        out.push(Margins::of(&style));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Value;

    fn nums(values: &[f64]) -> Vec<Value> {
        values.iter().map(|n| Value::Num(*n)).collect()
    }

    #[test]
    fn the_shorthand_follows_the_css_side_order() {
        for (values, expected) in [
            (vec![5.0], [5.0, 5.0, 5.0, 5.0]),
            (vec![5.0, 7.0], [5.0, 7.0, 5.0, 7.0]),
            (vec![5.0, 7.0, 11.0], [5.0, 7.0, 11.0, 7.0]),
            (vec![5.0, 7.0, 11.0, 13.0], [5.0, 7.0, 11.0, 13.0]),
        ] {
            let margin = Margins::parse(&nums(&values)).expect("a valid margin");
            assert_eq!(
                [margin.top, margin.right, margin.bottom, margin.left],
                expected,
                "{values:?}"
            );
        }
    }

    #[test]
    fn a_margin_that_is_not_finite_and_nonnegative_is_refused() {
        for values in [
            vec![],
            nums(&[1.0, 2.0, 3.0, 4.0, 5.0]),
            nums(&[-1.0]),
            nums(&[f64::NAN]),
            nums(&[f64::INFINITY]),
            vec![Value::Str("auto".into())],
        ] {
            assert!(
                Margins::parse(&values).is_none(),
                "{values:?} must not parse as a margin"
            );
        }
    }

    #[test]
    fn adjacent_margins_add_on_the_sides_that_face_each_other() {
        let anchor = Margins {
            top: 1.0,
            right: 2.0,
            bottom: 3.0,
            left: 4.0,
        };
        let other = Margins {
            top: 5.0,
            right: 6.0,
            bottom: 7.0,
            left: 8.0,
        };
        assert_eq!(other.gap_to(anchor, RelDir::RightOf), 2.0 + 8.0);
        assert_eq!(other.gap_to(anchor, RelDir::LeftOf), 4.0 + 6.0);
        assert_eq!(other.gap_to(anchor, RelDir::Below), 3.0 + 5.0);
        assert_eq!(other.gap_to(anchor, RelDir::Above), 1.0 + 7.0);
    }

    /// The contract says a managed genre must not *offer* `margin` either, and
    /// the completion table and the validator read the same predicate — so a
    /// genre that rejects the property and suggests it would be one change
    /// away from a file that looks valid and is not.
    ///
    /// The rule is written out here rather than called, so that moving
    /// `supported` off the contract fails this test instead of passing it.
    #[test]
    fn margin_is_offered_only_where_it_is_supported() {
        for def in crate::props::ARCHETYPES {
            let expected = def.layout.accepts_layout_block() || def.name == "sequence";
            let offered = crate::props::item_table(def)
                .iter()
                .any(|prop| is_margin(prop.name));
            assert_eq!(
                offered, expected,
                "`margin` completion disagrees with the contract for `{}`",
                def.name
            );
            assert_eq!(
                supported(def),
                expected,
                "`margin` support disagrees with the contract for `{}`",
                def.name
            );
        }
    }
}
