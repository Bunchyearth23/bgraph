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
    /// Other files this one pulls in. Resolved by [`crate::compose`] before
    /// anything is checked, so by the time `check` runs there is one diagram
    /// and no such thing as a file boundary (`D-029`).
    pub includes: Vec<IncludeDecl>,
    /// Fragments declared once and placed many times, and the placings.
    /// Expanded by [`crate::compose`] alongside `include`, because a component
    /// *is* an include of a file you did not have to make (`D-030`).
    pub components: Vec<ComponentDecl>,
    pub uses: Vec<UseDecl>,
    /// Adjustments applied to the solved layout, in declaration order.
    pub constraints: Vec<ConstraintDecl>,
    /// Scoped v2 placement declarations. They are kept separate from the
    /// entities they position so the source has one canonical geometry block.
    pub layouts: Vec<LayoutDecl>,
    /// Free text placed by the ordinary anchors, belonging to no box.
    pub notes: Vec<NoteDecl>,
    /// A key to the style classes in use. At most one is meaningful; a second
    /// is reported rather than drawn twice.
    pub legends: Vec<NoteDecl>,
}

/// `note "Figures are March" below api gap 20`.
///
/// It declares **no identifier**, and that is the design rather than an
/// omission: a note is not a thing the diagram can point at, so there is
/// nothing for an edge to land on and no way to write one that tries.
#[derive(Debug)]
pub struct NoteDecl {
    pub text: String,
    pub classes: Vec<Spanned<String>>,
    pub placement: Option<Placement>,
    pub props: Vec<Prop>,
    pub span: Span,
}

/// `align-group left a b c`, `same-width a b c`.
#[derive(Debug)]
pub struct ConstraintDecl {
    pub kind: ConstraintKind,
    /// Which edge to line up on, for `align-group`. Defaults to `center`,
    /// matching what `align` on a placement defaults to.
    pub align: Align,
    /// Axis selected by the spelling in a legacy `align-group` declaration.
    /// `center`/`middle` did not say which one, so only that legacy spelling
    /// retains the geometry-based fallback until the scoped `layout` DSL
    /// replaces it.
    pub axis: ConstraintAxis,
    pub members: Vec<Spanned<String>>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintKind {
    /// One cross-axis coordinate for the set.
    AlignGroup,
    SameWidth,
    SameHeight,
}

/// The coordinate an `align-group` relation controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintAxis {
    Horizontal,
    Vertical,
    /// Legacy `center` / `middle`, whose axis was not represented in v1.
    Inferred,
}

/// A `layout { … }` block, at the file root or owned by one group.
#[derive(Debug)]
pub struct LayoutDecl {
    /// `None` is the file-level block; a group block is local to that group.
    pub scope: Option<Spanned<String>>,
    pub statements: Vec<LayoutStmt>,
    pub span: Span,
}

/// A geometric relation in the canonical v2 layout grammar.
#[derive(Debug)]
pub enum LayoutStmt {
    /// `place api right-of gateway gap 24 align top`.
    Place {
        target: Spanned<String>,
        placement: Placement,
        span: Span,
    },
}

/// `component service { … }` — the body kept as source text, because that is
/// what the template engine fills and what the prefixing pass then parses.
#[derive(Debug)]
pub struct ComponentDecl {
    pub name: Spanned<String>,
    /// The block's contents, braces excluded, exactly as written.
    pub body: String,
    pub span: Span,
}

/// `use service as web { label: "Web" }`.
#[derive(Debug)]
pub struct UseDecl {
    pub component: Spanned<String>,
    /// The namespace this instance's names arrive under — always present,
    /// because two instances of one component would otherwise declare the
    /// same identifiers twice.
    pub prefix: Spanned<String>,
    /// Where the instance goes. It lands on the frame the expansion wraps
    /// the instance in, so an instance is placed and anchored exactly as any
    /// other group is — rather than needing a placement vocabulary of its own.
    pub placement: Option<Placement>,
    pub bindings: Vec<Prop>,
    pub span: Span,
}

/// `include "shared.bds"`, or `include "core.bd" as core`.
#[derive(Debug)]
pub struct IncludeDecl {
    /// As written, resolved relative to the file that wrote it.
    pub path: Spanned<String>,
    /// The namespace its names arrive under. Without one, every name must be
    /// unique across the whole set, and a clash is an error naming both files.
    pub prefix: Option<Spanned<String>>,
    pub span: Span,
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

/// The pin a member line declares: `- 1: +12V` is pin 1.
///
/// Shared, because `check` proves a wire lands on a pin that exists and the
/// layout then draws it there — two readings of the same line, and two
/// parsers would be two chances to disagree about what a pin is called.
pub fn pin_number(member: &str) -> Option<u32> {
    member
        .trim_start_matches(['-', '+'])
        .trim()
        .split([':', ' '])
        .next()
        .and_then(|n| n.parse().ok())
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
    /// Places the line must go through, in the order written (`D-034`).
    ///
    /// A `Placement` and not a point, because a waypoint has to survive a
    /// neighbour being inserted the same way a box does — so `left-of c gap 20`
    /// is the useful form and a bare coordinate is the escape hatch. Resolved
    /// in the layout, which is the only place that knows where `c` ended up.
    pub via: Vec<Placement>,
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
    /// `at (col 3, row 2)` — the same absolute placement in grid units,
    /// multiplied by the step `graph { grid }` declares. Resolved in the
    /// layout rather than the parser, because the parser does not know the
    /// step (`D-031`).
    Cell {
        col: f64,
        row: f64,
    },
    Rel {
        dir: RelDir,
        anchor: Spanned<String>,
        gap: Option<f64>,
        align: Option<Align>,
    },
}

impl Placement {
    /// A grid cell as the absolute position it stands for.
    ///
    /// The one definition of what a cell means, so the two places the layout
    /// reads a placement cannot disagree. Without a declared `grid` a cell
    /// falls back to a step of one, which places everything on top of the
    /// origin — visibly wrong rather than silently ignored, and `check` says
    /// so before it is ever drawn.
    pub fn resolved(&self, grid: Option<(f64, f64)>) -> Placement {
        match self {
            Placement::Cell { col, row } => {
                let (sx, sy) = grid.unwrap_or((1.0, 1.0));
                Placement::At {
                    x: col * sx,
                    y: row * sy,
                }
            }
            other => other.clone(),
        }
    }
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
