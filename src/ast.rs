//! Untyped syntax tree: property values stay raw here and are interpreted in
//! `style.rs`, so new properties can be added without touching the parser.
//!
//! Every declaration carries byte spans back into the source — the language
//! server reports diagnostics, jumps to definitions and completes in context
//! from these.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn contains(&self, offset: usize) -> bool {
        (self.start..=self.end).contains(&offset)
    }
}

#[derive(Debug, Clone)]
pub struct Spanned<T> {
    pub value: T,
    pub span: Span,
}

impl Spanned<String> {
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

#[derive(Debug, Default)]
pub struct Diagram {
    /// Absent means `free` — the node-and-edge graph.
    pub archetype: Option<Spanned<String>>,
    pub archetype_props: Vec<Prop>,
    /// Redeclarations, kept so `check` can point at each one.
    pub extra_archetypes: Vec<Spanned<String>>,
    pub graph: Vec<Prop>,
    pub graph_span: Option<Span>,
    pub styles: Vec<StyleDecl>,
    pub nodes: Vec<NodeDecl>,
    pub edges: Vec<EdgeDecl>,
    /// Containers. Members live in the flat `nodes`/`groups` lists and are
    /// named here, so the identifier namespace stays single and global.
    pub groups: Vec<GroupDecl>,
}

#[derive(Debug)]
pub struct GroupDecl {
    pub id: Spanned<String>,
    pub label: Option<String>,
    pub classes: Vec<Spanned<String>>,
    /// Where the group goes. Its *size* always comes from its members; a
    /// placement translates the whole subtree once they are solved.
    pub placement: Option<Placement>,
    pub props: Vec<Prop>,
    /// Items and groups declared directly inside, in source order.
    pub members: Vec<Spanned<String>>,
    /// Edges declared directly inside, as indices into `Diagram::edges`.
    /// A sequence diagram's `loop` frame spans messages, not participants.
    pub edge_members: Vec<usize>,
    pub span: Span,
}

#[derive(Debug)]
pub struct StyleDecl {
    pub name: Spanned<String>,
    pub extends: Option<Spanned<String>>,
    pub props: Vec<Prop>,
    pub span: Span,
}

#[derive(Debug)]
pub struct NodeDecl {
    /// What the source called it: `node`, `slice`, `field`. Validated against
    /// the archetype by `check`.
    pub keyword: Spanned<String>,
    pub id: Spanned<String>,
    pub label: Option<String>,
    /// Values written after the label. `check` maps them onto the properties
    /// the archetype declares; `effective_props` performs the mapping.
    pub positional: Vec<Value>,
    pub positional_span: Span,
    pub classes: Vec<Spanned<String>>,
    pub placement: Option<Placement>,
    pub props: Vec<Prop>,
    /// Bare strings written in the block: a class's fields, a table's columns.
    /// Distinct from `GroupDecl::members`, which names other declarations.
    pub members: Vec<Spanned<String>>,
    pub span: Span,
}

impl NodeDecl {
    /// Inline properties, with the positional values folded in first — so an
    /// explicit `{ value: … }` still wins, and `check` warns about the clash.
    pub fn effective_props(&self, slots: &[crate::props::Slot]) -> Vec<Prop> {
        fold_positional(&self.positional, self.positional_span, slots, &self.props)
    }
}

/// Positional values fill the archetype's slots in order and are placed
/// before the inline block, so an explicit property still wins.
fn fold_positional(
    positional: &[Value],
    span: Span,
    slots: &[crate::props::Slot],
    inline: &[Prop],
) -> Vec<Prop> {
    if positional.is_empty() {
        return inline.to_vec();
    }

    let mut out = Vec::with_capacity(slots.len() + inline.len());
    let mut rest = positional;

    for slot in slots {
        let take = slot.arity.min(rest.len());
        if take == 0 {
            break;
        }
        let (values, tail) = rest.split_at(take);
        rest = tail;
        out.push(Prop {
            name: slot.prop.to_string(),
            name_span: span,
            values: values.to_vec(),
            values_span: span,
            span,
        });
    }

    out.extend(inline.iter().cloned());
    out
}

#[derive(Debug)]
pub struct EdgeDecl {
    pub from: Spanned<String>,
    pub to: Spanned<String>,
    pub arrow: Arrow,
    pub label: Option<String>,
    /// Values after the label, for archetypes that give edges slots.
    pub positional: Vec<Value>,
    pub positional_span: Span,
    pub classes: Vec<Spanned<String>>,
    pub props: Vec<Prop>,
    pub span: Span,
}

impl EdgeDecl {
    pub fn effective_props(&self, slots: &[crate::props::Slot]) -> Vec<Prop> {
        fold_positional(&self.positional, self.positional_span, slots, &self.props)
    }
}

#[derive(Debug, Clone)]
pub struct Prop {
    pub name: String,
    pub name_span: Span,
    pub values: Vec<Value>,
    /// Covers the values only — used to underline a bad value, not the key.
    pub values_span: Span,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Value {
    Num(f64),
    Str(String),
    Word(String),
    Color(String),
}

impl Value {
    pub fn as_num(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    /// A paint is either `#rrggbb` or a bare word (an SVG named color).
    pub fn as_paint(&self) -> Option<&str> {
        match self {
            Value::Color(c) => Some(c),
            Value::Word(w) => Some(w),
            _ => None,
        }
    }

    pub fn as_word(&self) -> Option<&str> {
        match self {
            Value::Word(w) => Some(w),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::Word(w) => Some(w),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Placement {
    At {
        x: f64,
        y: f64,
    },
    Rel {
        dir: RelDir,
        anchor: Spanned<String>,
        gap: Option<f64>,
        align: Option<Align>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelDir {
    RightOf,
    LeftOf,
    Below,
    Above,
}

/// Cross-axis alignment against the anchor: for a horizontal placement
/// Start/End mean top/bottom, for a vertical one they mean left/right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Start,
    Center,
    End,
}

/// Line ends and stroke pattern implied by the arrow token itself.
#[derive(Debug, Clone, Copy)]
pub struct Arrow {
    pub head_start: bool,
    pub head_end: bool,
    pub dash: Option<&'static str>,
}

impl Arrow {
    pub fn parse(token: &str) -> Self {
        let dash = match token {
            "-->" => Some("dashed"),
            "..>" | ".." => Some("dotted"),
            _ => None,
        };
        Arrow {
            head_start: token.starts_with('<'),
            head_end: token.ends_with('>'),
            dash,
        }
    }
}
