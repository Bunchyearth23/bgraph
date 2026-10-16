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
