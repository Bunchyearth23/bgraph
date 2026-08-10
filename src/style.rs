//! Style cascade: defaults ← implicit `node`/`edge` style ← classes ← inline block.
//!
//! Every field is optional during resolution so that overlaying a layer only
//! touches the properties it actually mentions.

use std::collections::HashMap;

use anyhow::{Result, bail};

use crate::ast::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    Rect,
    Stadium,
    Ellipse,
    Diamond,
    Hexagon,
    Cylinder,
}

impl Shape {
    fn parse(word: &str) -> Option<Shape> {
        Some(match word {
            "rect" | "box" | "rectangle" => Shape::Rect,
            "stadium" | "pill" | "rounded" => Shape::Stadium,
            "ellipse" | "circle" | "oval" => Shape::Ellipse,
            "diamond" | "rhombus" | "decision" => Shape::Diamond,
            "hexagon" | "hex" => Shape::Hexagon,
            "cylinder" | "database" | "db" => Shape::Cylinder,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Router {
    Straight,
    Ortho,
}

/// What a state is, in `type state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    State,
    Initial,
    Final,
    Choice,
}

/// What an element is in an architecture view. Drives shape and colour, so a
/// reader tells a person from a datastore before reading the label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Person,
    System,
    Container,
    Component,
    Database,
    Queue,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head {
    Solid,
    Open,
    Dot,
    None,
    // Crow's-foot cardinality. These are heads rather than a parallel system
    // because that is exactly what they are — a mark at one end of a line —
    // and it buys the whole marker pipeline, per-colour dedup included.
    /// One: a bar across the line.
    One,
    /// Many: the crow's foot.
    Many,
    /// Zero or one: a circle, then a bar.
    ZeroOne,
    /// Zero or many: a circle, then the foot.
    ZeroMany,
    /// One or many: a bar, then the foot.
    OneMany,
}

impl Head {
    /// Whether this head reads as a cardinality rather than a direction.
    /// Cardinality is drawn at both ends independently, and suppresses the
    /// arrowhead the edge would otherwise carry.
    pub fn is_cardinality(self) -> bool {
        matches!(
            self,
            Head::One | Head::Many | Head::ZeroOne | Head::ZeroMany | Head::OneMany
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct Style {
    pub fill: Option<String>,
    pub stroke: Option<String>,
    pub stroke_width: Option<f64>,
    pub dash: Option<String>,
    pub radius: Option<f64>,
    pub shape: Option<Shape>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub padding: Option<f64>,
    pub font_size: Option<f64>,
    pub font_family: Option<String>,
    pub font_color: Option<String>,
    pub font_weight: Option<String>,
    /// Manual nudge of the label, in user units. On an edge it also pins the
    /// label: a position the author declared is a decision, so the automatic
    /// avoidance leaves it alone.
    pub label_offset: Option<(f64, f64)>,
    pub head: Option<Head>,
    /// Cardinality at the tail of an edge — `er` only.
    pub from_card: Option<Head>,
    /// Cardinality at the head of an edge — `er` only.
    pub to_card: Option<Head>,
    pub router: Option<Router>,
    pub shadow: Option<bool>,
    /// Chart data, carried on the same layer stack as decoration so a class
    /// can supply it: `value` feeds `type pie`, `bits` feeds `type packet`.
    pub value: Option<f64>,
    pub bits: Option<(u32, u32)>,
    /// Quadrant coordinates, in the archetype's own range.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Timing data.
    pub wave: Option<String>,
    pub data: Option<Vec<String>>,
    /// A series, one number per category.
    pub points: Option<Vec<f64>>,
    /// State-machine semantics.
    pub kind: Option<StateKind>,
    pub guard: Option<String>,
    /// `type block` — what the element is.
    pub block_kind: Option<BlockKind>,
    /// `type block` — the technology line, drawn in brackets under the name.
    pub tech: Option<String>,
    /// `type block` — a sentence under the technology line.
    pub description: Option<String>,
    /// Scheduling: a start day and a length in days.
    pub start: Option<i64>,
    pub length: Option<f64>,
}

macro_rules! overlay {
    ($self:ident, $other:ident, $($field:ident),+ $(,)?) => {
        $(if $other.$field.is_some() { $self.$field = $other.$field.clone(); })+
    };
}

/// The crow's-foot vocabulary. Read at the end nearest the entity it
/// describes: `order -> line "contains" one many` is one order, many lines.
fn cardinality(word: &str) -> anyhow::Result<Head> {
    Ok(match word {
        "one" => Head::One,
        "many" => Head::Many,
        "zero-one" | "optional" => Head::ZeroOne,
        "zero-many" => Head::ZeroMany,
        "one-many" => Head::OneMany,
        other => bail!("unknown cardinality `{other}`"),
    })
}

impl Style {
    /// Values set in `other` win; everything else is left untouched.
    pub fn overlay(&mut self, other: &Style) {
        overlay!(
            self,
            other,
            fill,
            stroke,
            stroke_width,
            dash,
            radius,
            shape,
            width,
            height,
            padding,
            font_size,
            font_family,
            font_color,
            font_weight,
            label_offset,
            head,
            from_card,
            to_card,
            router,
            shadow,
            value,
            bits,
            x,
            y,
            wave,
            data,
            points,
            kind,
            block_kind,
            tech,
            description,
            guard,
            start,
            length,
        );
    }

    pub fn node_defaults() -> Style {
        Style {
            fill: Some("#ffffff".into()),
            stroke: Some("#1f2937".into()),
            stroke_width: Some(1.5),
            radius: Some(8.0),
            shape: Some(Shape::Rect),
            padding: Some(14.0),
            font_size: Some(14.0),
            font_family: Some("Inter, ui-sans-serif, system-ui, sans-serif".into()),
            font_color: Some("#111827".into()),
            font_weight: Some("500".into()),
            shadow: Some(false),
            ..Style::default()
        }
    }

    /// A frame, not a box: the fill is subtle and the border is what reads.
    pub fn group_defaults() -> Style {
        Style {
            fill: Some("#00000000".into()),
            stroke: Some("#94a3b8".into()),
            stroke_width: Some(1.0),
            dash: Some("dashed".into()),
            radius: Some(10.0),
            shape: Some(Shape::Rect),
            padding: Some(18.0),
            font_size: Some(12.0),
            font_family: Some("Inter, ui-sans-serif, system-ui, sans-serif".into()),
            font_color: Some("#64748b".into()),
            font_weight: Some("600".into()),
            ..Style::default()
        }
    }

    pub fn edge_defaults() -> Style {
        Style {
            stroke: Some("#64748b".into()),
            stroke_width: Some(1.5),
            font_size: Some(12.0),
            font_family: Some("Inter, ui-sans-serif, system-ui, sans-serif".into()),
            font_color: Some("#475569".into()),
            font_weight: Some("400".into()),
            head: Some(Head::Solid),
            router: Some(Router::Straight),
            ..Style::default()
        }
    }

    /// Dash patterns are named in the DSL and turned into SVG arrays here.
    pub fn dash_array(&self) -> Option<String> {
        match self.dash.as_deref() {
            None | Some("solid") | Some("none") => None,
            Some("dashed") => Some("7 5".into()),
            Some("dotted") => Some("1.5 4".into()),
            Some(custom) => Some(custom.replace(['|', '-'], " ")),
        }
    }
}

/// Reads one property into a style layer. Unknown names are a hard error so a
/// typo surfaces immediately instead of silently doing nothing.
fn apply_prop(style: &mut Style, prop: &Prop) -> Result<()> {
    let v = &prop.values;
    let first = v.first();

    match prop.name.as_str() {
        "fill" | "background" => style.fill = first.and_then(|x| x.as_paint()).map(String::from),
        "stroke" | "border" | "color" => {
            // `stroke: #38bdf8 2px dashed` — paint, then optional width, then pattern.
            for value in v {
                match value {
                    Value::Num(n) => style.stroke_width = Some(*n),
                    Value::Color(c) => style.stroke = Some(c.clone()),
                    Value::Word(w) if matches!(w.as_str(), "dashed" | "dotted" | "solid") => {
                        style.dash = Some(w.clone())
                    }
                    Value::Word(w) => style.stroke = Some(w.clone()),
                    Value::Str(s) => style.dash = Some(s.clone()),
                    // A date, a duration or an unbound hole says nothing
                    // about a stroke.
                    Value::Date { .. } | Value::Duration(_) | Value::Placeholder(_) => {}
                }
            }
        }
        "stroke-width" | "weight" => style.stroke_width = first.and_then(|x| x.as_num()),
        "dash" => {
            style.dash = Some(match v.as_slice() {
                [Value::Word(w)] => w.clone(),
                _ => v
                    .iter()
                    .filter_map(|x| x.as_num())
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            })
        }
        "radius" | "corner" => style.radius = first.and_then(|x| x.as_num()),
        "shape" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            match Shape::parse(word) {
                Some(shape) => style.shape = Some(shape),
                None => bail!("unknown shape `{word}`"),
            }
        }
        "size" => {
            style.width = v.first().and_then(|x| x.as_num());
            style.height = v.get(1).and_then(|x| x.as_num());
        }
        "width" => style.width = first.and_then(|x| x.as_num()),
        "height" => style.height = first.and_then(|x| x.as_num()),
        "padding" => style.padding = first.and_then(|x| x.as_num()),
        "font" => {
            // `font: 14 "Inter" #e2e8f0 bold` — order-free, read by value kind.
            for value in v {
                match value {
                    Value::Num(n) => style.font_size = Some(*n),
                    Value::Str(s) => style.font_family = Some(s.clone()),
                    Value::Color(c) => style.font_color = Some(c.clone()),
                    Value::Word(w) => style.font_weight = Some(w.clone()),
                    Value::Date { .. } | Value::Duration(_) | Value::Placeholder(_) => {}
                }
            }
        }
        "font-size" => style.font_size = first.and_then(|x| x.as_num()),
        "font-family" => style.font_family = first.and_then(|x| x.as_text()).map(String::from),
        "font-color" | "text-color" => {
            style.font_color = first.and_then(|x| x.as_paint()).map(String::from)
        }
        "font-weight" => style.font_weight = first.and_then(|x| x.as_text()).map(String::from),
        // `label-offset: 18` is a horizontal nudge; the vertical one is opt-in.
        "label-offset" | "text-offset" => {
            style.label_offset = Some((
                first.and_then(|x| x.as_num()).unwrap_or(0.0),
                v.get(1).and_then(|x| x.as_num()).unwrap_or(0.0),
            ))
        }
        "arrow" | "head" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            style.head = Some(match word {
                "solid" | "filled" | "triangle" => Head::Solid,
                "open" | "line" | "thin" => Head::Open,
                "dot" | "circle" => Head::Dot,
                "none" => Head::None,
                other => bail!("unknown arrow head `{other}`"),
            });
        }
        "from-card" | "to-card" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            let card = cardinality(word)?;
            if prop.name == "from-card" {
                style.from_card = Some(card);
            } else {
                style.to_card = Some(card);
            }
        }
        "router" | "route" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            style.router = Some(match word {
                "straight" | "direct" | "line" => Router::Straight,
                "ortho" | "orthogonal" | "elbow" => Router::Ortho,
                other => bail!("unknown router `{other}`"),
            });
        }
        "value" => style.value = first.and_then(|x| x.as_num()),
        "points" => style.points = Some(v.iter().filter_map(|x| x.as_num()).collect()),
        "kind" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            // One word, two genres: `check` has already refused the wrong
            // one for the declared archetype, so this only has to read it.
            match word {
                "initial" => style.kind = Some(StateKind::Initial),
                "final" => style.kind = Some(StateKind::Final),
                "choice" => style.kind = Some(StateKind::Choice),
                "state" => style.kind = Some(StateKind::State),
                "person" => style.block_kind = Some(BlockKind::Person),
                "system" => style.block_kind = Some(BlockKind::System),
                "container" => style.block_kind = Some(BlockKind::Container),
                "component" => style.block_kind = Some(BlockKind::Component),
                "database" => style.block_kind = Some(BlockKind::Database),
                "queue" => style.block_kind = Some(BlockKind::Queue),
                "external" => style.block_kind = Some(BlockKind::External),
                other => bail!("unknown kind `{other}`"),
            }
        }
        "tech" | "technology" => style.tech = first.and_then(|x| x.as_text()).map(String::from),
        "description" | "desc" => {
            style.description = first.and_then(|x| x.as_text()).map(String::from)
        }
        "guard" => style.guard = first.and_then(|x| x.as_text()).map(String::from),
        // A timeline event is a point, so it needs only the day — but the day
        // it sits on is the same field a gantt bar starts from.
        "date" => style.start = first.and_then(|x| x.as_date()),
        // `start: 2026-08-01 12d` or `start: 2026-08-01 2026-08-12` — read by
        // value kind, so either way of saying "how long" works.
        "start" => {
            style.start = first.and_then(|x| x.as_date());
            style.length = match (style.start, v.get(1)) {
                (_, Some(Value::Duration(days))) => Some(*days),
                (Some(from), Some(Value::Date { day, .. })) => Some((day - from) as f64),
                _ => style.length,
            };
        }
        "length" => style.length = first.and_then(|x| x.as_duration().or_else(|| x.as_num())),
        "wave" => style.wave = first.and_then(|v| v.as_text()).map(String::from),
        "data" => {
            style.data = Some(
                v.iter()
                    .filter_map(|x| x.as_text())
                    .map(String::from)
                    .collect(),
            )
        }
        "x" => style.x = first.and_then(|v| v.as_num()),
        "y" => style.y = first.and_then(|v| v.as_num()),
        "bits" => {
            let start = first.and_then(|x| x.as_num()).unwrap_or(0.0).max(0.0) as u32;
            let end = v
                .get(1)
                .and_then(|x| x.as_num())
                .map_or(start, |n| n.max(0.0) as u32);
            style.bits = Some((start.min(end), start.max(end)));
        }
        "shadow" => {
            style.shadow = Some(!matches!(
                first.and_then(|x| x.as_word()),
                Some("false" | "off")
            ))
        }
        other => bail!("unknown property `{other}`"),
    }

    Ok(())
}

pub fn layer_from_props(props: &[Prop]) -> Result<Style> {
    let mut style = Style::default();
    for prop in props {
        apply_prop(&mut style, prop)?;
    }
    Ok(style)
}

/// Named style blocks, already flattened through their `extends` chain.
pub struct Registry {
    resolved: HashMap<String, Style>,
}

impl Registry {
    pub fn build(decls: &[StyleDecl]) -> Result<Registry> {
        let by_name: HashMap<&str, &StyleDecl> =
            decls.iter().map(|d| (d.name.as_str(), d)).collect();

        let mut resolved = HashMap::new();
        for decl in decls {
            let mut chain = Vec::new();
            let mut cursor = Some(decl);
            while let Some(current) = cursor {
                if chain
                    .iter()
                    .any(|d: &&StyleDecl| d.name.as_str() == current.name.as_str())
                {
                    bail!(
                        "style `{}` extends itself in a cycle",
                        current.name.as_str()
                    );
                }
                chain.push(current);
                cursor = match current.extends.as_ref() {
                    Some(parent) => match by_name.get(parent.as_str()) {
                        Some(found) => Some(found),
                        None => bail!(
                            "style `{}` extends unknown `{}`",
                            current.name.as_str(),
                            parent.as_str()
                        ),
                    },
                    None => None,
                };
            }

            // Walk root-first so a child overrides its ancestors.
            let mut style = Style::default();
            for ancestor in chain.iter().rev() {
                style.overlay(&layer_from_props(&ancestor.props)?);
            }
            resolved.insert(decl.name.value.clone(), style);
        }

        Ok(Registry { resolved })
    }

    pub fn get(&self, name: &str) -> Option<&Style> {
        self.resolved.get(name)
    }

    /// defaults ← implicit theme ← listed classes ← inline props.
    ///
    /// The implicit theme is the archetype's own word (`style slice { … }`),
    /// with `node` staying a universal alias — applied first, so the specific
    /// name wins where both are declared.
    pub fn resolve(
        &self,
        base: Style,
        implicit: &str,
        classes: &[Spanned<String>],
        inline: &[Prop],
    ) -> Result<Style> {
        let mut style = base;

        if implicit != "node"
            && let Some(found) = self.get("node")
        {
            style.overlay(found);
        }
        if let Some(found) = self.get(implicit) {
            style.overlay(found);
        }
        for class in classes {
            match self.get(class.as_str()) {
                Some(found) => style.overlay(found),
                None => bail!("unknown style `.{}`", class.as_str()),
            }
        }
        style.overlay(&layer_from_props(inline)?);

        Ok(style)
    }
}

/// Diagram-level settings that are not per-element styling.
#[derive(Debug, Clone)]
pub struct GraphOpts {
    pub padding: f64,
    pub gap: f64,
    pub background: Option<String>,
    pub flow: RelDir,
}

impl Default for GraphOpts {
    fn default() -> Self {
        GraphOpts {
            padding: 32.0,
            gap: 64.0,
            // Explicit, so the preview and the exported file agree on what
            // the surface is. `background: none` opts back out.
            background: Some("#ffffff".into()),
            flow: RelDir::RightOf,
        }
    }
}

impl GraphOpts {
    pub fn from_props(props: &[Prop]) -> Result<GraphOpts> {
        let mut opts = GraphOpts::default();

        for prop in props {
            let first = prop.values.first();
            match prop.name.as_str() {
                "padding" => opts.padding = first.and_then(|x| x.as_num()).unwrap_or(opts.padding),
                "gap" => opts.gap = first.and_then(|x| x.as_num()).unwrap_or(opts.gap),
                "background" | "bg" => {
                    opts.background = match first.and_then(|x| x.as_paint()) {
                        Some("none" | "transparent") => None,
                        other => other.map(String::from),
                    }
                }
                "flow" | "direction" => {
                    let word = first.and_then(|x| x.as_word()).unwrap_or_default();
                    opts.flow = match word {
                        "right" | "LR" => RelDir::RightOf,
                        "left" | "RL" => RelDir::LeftOf,
                        "down" | "TB" => RelDir::Below,
                        "up" | "BT" => RelDir::Above,
                        other => bail!("unknown flow `{other}`"),
                    };
                }
                other => bail!("unknown graph setting `{other}`"),
            }
        }

        Ok(opts)
    }
}
