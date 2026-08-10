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
    /// Whether this is an ordinary frame or an activation bar. `activate`'s
    /// identifier names an **existing** item rather than declaring one, which
    /// is the whole reason the two words are distinguishable here (`D-024`).
    pub activates: bool,
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

    /// The same, resolved against the archetype — which is nearly always what
    /// a caller wants, because the positional shape follows the **word the
    /// author wrote** (`D-023`), not the archetype's primary form. Taking the
    /// def rather than a slot list is what stops a caller reaching for
    /// `def.slots` and silently misreading a second form.
    pub fn props_for(&self, def: &crate::props::ArchetypeDef) -> Vec<Prop> {
        self.effective_props(def.slots_for(self.keyword.as_str()))
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
    /// A calendar day, kept as written and as a day number so a span can be
    /// measured without pulling in a date library.
    Date {
        text: String,
        day: i64,
    },
    /// A length of time, in days. `6h` is a quarter of one.
    Duration(f64),
    /// `{{name}}` — bound by the library at render time. It reaches `check`
    /// so a template can be edited, and suppresses the checks that depend on
    /// a value nobody has supplied yet.
    Placeholder(String),
}

/// Days since 1970-01-01, by Howard Hinnant's civil-calendar algorithm. Fewer
/// than twenty lines, and it saves a dependency for the one thing we need.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The inverse, for writing a tick label back out.
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
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

    pub fn as_date(&self) -> Option<i64> {
        match self {
            Value::Date { day, .. } => Some(*day),
            _ => None,
        }
    }

    pub fn as_duration(&self) -> Option<f64> {
        match self {
            Value::Duration(days) => Some(*days),
            _ => None,
        }
    }

    /// True of a value nobody has bound yet.
    pub fn is_hole(&self) -> bool {
        matches!(self, Value::Placeholder(_))
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
            Value::Date { text, .. } => Some(text),
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
