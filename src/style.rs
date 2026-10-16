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
    /// A bow between the two ends. Sampled into the polyline rather than
    /// emitted as a Bezier, so the PDF writer, the JSON export, the bounds and
    /// the label solver all keep working on the one shape they already know
    /// (`W-125`) — the same choice the self-loop made.
    Curved,
}

/// A head's name as written. Shared by `head`, `from-head` and `to-head`, so
/// the three cannot drift into accepting different words.
fn head_word(word: &str) -> Result<Head> {
    Ok(match word {
        "solid" | "filled" => Head::Solid,
        "open" | "line" | "thin" => Head::Open,
        "dot" | "circle" => Head::Dot,
        "none" => Head::None,
        // UML. `triangle` used to be a synonym for the solid arrow, which was
        // a word spent on a shape the language already had; it is the hollow
        // generalisation triangle now, which is a shape it did not.
        "triangle" | "generalisation" | "extends" => Head::Hollow,
        "diamond" | "aggregation" => Head::Diamond,
        "composition" | "filled-diamond" => Head::Composed,
        other => bail!("unknown arrow head `{other}`"),
    })
}

/// Which side of a box a line attaches to.
///
/// Declared, unlike the route between two of them — `D-033`. Nothing here says
/// *where* on the side: several lines sharing one spread along it, which is a
/// rule and not a decision, so it stays clear of `D-001`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Top,
    Bottom,
    Left,
    Right,
}

/// What a state is, in `type state`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateKind {
    State,
    Initial,
    Final,
    Choice,
}

/// What a box on a topology is. Drives the shape only — a topology's colour
/// belongs to the author, since a network diagram's palette usually means
/// something already (environment, tenancy, trust zone).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetRole {
    Server,
    Switch,
    Router,
    Firewall,
    Storage,
    Client,
    Cloud,
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
    // UML's three, which `type class` had no way to draw at all until `W-125`
    // — a class diagram without a generalisation is a class diagram missing
    // the one relation the notation exists for.
    /// Generalisation: a hollow triangle, pointing at the parent.
    Hollow,
    /// Aggregation: a hollow diamond, at the whole.
    Diamond,
    /// Composition: the same diamond filled, at the whole.
    Composed,
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
    pub margin_top: Option<f64>,
    pub margin_right: Option<f64>,
    pub margin_bottom: Option<f64>,
    pub margin_left: Option<f64>,
    pub font_size: Option<f64>,
    pub font_family: Option<String>,
    pub font_color: Option<String>,
    pub font_weight: Option<String>,
    /// Manual nudge of the label, in user units. On an edge it also pins the
    /// label: a position the author declared is a decision, so the automatic
    /// avoidance leaves it alone.
    pub label_offset: Option<(f64, f64)>,
    pub label_position: Option<String>,
    pub head: Option<Head>,
    /// Cardinality at the tail of an edge — `er` only.
    pub from_head: Option<Head>,
    /// Cardinality at the head of an edge — `er` only.
    pub to_head: Option<Head>,
    pub router: Option<Router>,
    /// How far to round an `ortho` elbow. A style on the router rather than a
    /// second router, which is how it was asked for.
    pub bend: Option<f64>,
    pub shadow: Option<bool>,
    /// Chart data, carried on the same layer stack as decoration so a class
    /// can supply it: `value` feeds `type pie`, `bits` feeds `type packet`.
    pub value: Option<f64>,
    pub bits: Option<(u32, u32)>,
    /// The same range one order of magnitude up, for `type bytefield`. A
    /// separate field because a value cannot tell you which unit it was
    /// written in, and only the archetype knows.
    pub bytes: Option<(u32, u32)>,
    /// Quadrant coordinates, in the archetype's own range.
    pub x: Option<f64>,
    pub y: Option<f64>,
    /// Timing data.
    pub wave: Option<String>,
    pub data: Option<Vec<String>>,
    /// A series, one number per category.
    pub points: Option<Vec<f64>>,
    /// Coordinates, read two at a time — the shape `xy`'s `scatter` keyword
    /// carries (`D-023`). Kept as a flat list because that is how it is
    /// written; `check` is what refuses an odd count.
    pub pairs: Option<Vec<f64>>,
    /// State-machine semantics.
    pub kind: Option<StateKind>,
    pub guard: Option<String>,
    /// `type block` — what the element is.
    pub block_kind: Option<BlockKind>,
    /// `type block` — the technology line, drawn in brackets under the name.
    pub tech: Option<String>,
    /// The text this class shows in a `legend`. Its presence is what opts a
    /// class into one: a legend derived from every class in the file would
    /// list the ones that only round a corner, and one derived from the
    /// colours in use would have nothing to call them.
    pub legend: Option<String>,
    /// What this is, in a sentence.
    ///
    /// One property, one meaning. `type block` draws it under the technology
    /// line because that genre has room for it; **every** genre emits it as an
    /// SVG `<title>`, which is a tooltip to a reader with a mouse and a label
    /// to one with a screen reader. Inventing a second word for the second use
    /// would have been two names for one idea, which is what `D-015` is for.
    ///
    /// It lives on `Style` rather than on the placed box because `Style` is
    /// the one thing already resolved per item and carried to the renderer —
    /// a second value would have to be threaded through all thirty places a
    /// placed box is built.
    pub description: Option<String>,
    /// `type requirements` — how likely it is to go unmet, and how it will be
    /// shown to be met. Kept as words: both are drawn, neither is computed.
    pub risk: Option<String>,
    pub verify: Option<String>,
    /// `type requirements` — what a relation asserts, read tail to head.
    pub link: Option<String>,
    /// `type gitgraph` — which rail a commit sits on, and any tag beside it.
    pub branch: Option<String>,
    pub tag: Option<String>,
    /// `type harness` — the pin a wire lands on at each end.
    pub wire: Option<(u32, u32)>,
    /// `type venn` — how many things are in a set, and how many two share.
    pub set_size: Option<f64>,
    pub shared: Option<f64>,
    /// Which side of each box the line attaches to, and where it sits among
    /// the lines sharing that side (`D-033`). `None` is `auto`: the router
    /// picks, which is right nearly always and is the reason this is opt-in.
    pub from_side: Option<Side>,
    pub to_side: Option<Side>,
    /// Explicit point on the selected side: (index, total), both one-based.
    pub from_port: Option<(f64, f64)>,
    pub to_port: Option<(f64, f64)>,
    pub lane: Option<f64>,
    /// `type bpmn` — which element this is, and what it carries.
    ///
    /// Kept as the written word rather than six more enums, the way `risk`,
    /// `verify` and `link` are: `check` has already closed each set against
    /// the archetype's own table, so a second closed set here would be the
    /// same list written twice and one more place for it to fall behind.
    pub task_kind: Option<String>,
    pub stage: Option<String>,
    pub trigger: Option<String>,
    pub gate: Option<String>,
    pub artifact: Option<String>,
    pub flow: Option<String>,
    /// The task a `boundary` event sits on. Placement, not decoration.
    pub attach: Option<String>,
    /// `type network` — what the box is, and its address line.
    pub role: Option<NetRole>,
    pub addr: Option<String>,
    /// `type network` — what a link runs at.
    pub speed: Option<String>,
    /// `type rack` — the inclusive span of rack units a device occupies,
    /// counted from 1 at the bottom.
    pub u: Option<(u32, u32)>,
    /// `type journey` — one score per actor, in the order `actors` names them.
    pub scores: Option<Vec<f64>>,
    /// `type kanban` — a column's work-in-progress limit. On the cascade like
    /// any other datum, so a `.class` can carry a board's house limits.
    pub limit: Option<f64>,
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

/// Read the optional numeric suffix on `from`/`to` (`side index of total`).
fn indexed_port(prop: &Prop) -> Option<(f64, f64)> {
    let nums: Vec<f64> = prop.values.iter().filter_map(|v| v.as_num()).collect();
    match nums.as_slice() {
        [index, total, ..] => Some((*index, *total)),
        _ => None,
    }
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
            margin_top,
            margin_right,
            margin_bottom,
            margin_left,
            font_size,
            font_family,
            font_color,
            font_weight,
            label_offset,
            label_position,
            head,
            from_head,
            to_head,
            router,
            bend,
            shadow,
            value,
            bits,
            bytes,
            x,
            y,
            wave,
            data,
            points,
            pairs,
            kind,
            block_kind,
            tech,
            description,
            legend,
            risk,
            verify,
            link,
            branch,
            tag,
            wire,
            set_size,
            shared,
            from_side,
            to_side,
            from_port,
            to_port,
            lane,
            task_kind,
            stage,
            trigger,
            gate,
            artifact,
            flow,
            attach,
            role,
            addr,
            speed,
            u,
            scores,
            limit,
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
        "legend" => style.legend = first.and_then(|x| x.as_text()).map(String::from),
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
        "label-position" => {
            style.label_position = first.and_then(|x| x.as_word()).map(String::from)
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
        "margin" => {
            if let Some(margin) = crate::spacing::Margins::parse(v) {
                style.margin_top = Some(margin.top);
                style.margin_right = Some(margin.right);
                style.margin_bottom = Some(margin.bottom);
                style.margin_left = Some(margin.left);
            }
        }
        "margin-top" => style.margin_top = first.and_then(|x| x.as_num()),
        "margin-right" => style.margin_right = first.and_then(|x| x.as_num()),
        "margin-bottom" => style.margin_bottom = first.and_then(|x| x.as_num()),
        "margin-left" => style.margin_left = first.and_then(|x| x.as_num()),
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
            style.head = Some(head_word(word)?);
        }
        // A mark at one named end, whatever kind. `from-card` and `to-card`
        // are the ER spelling of the same idea and stay as aliases: UML's
        // diamond belongs at the *whole*, which is nearly always the tail, and
        // there was no way to put a mark there except a cardinality.
        "from-head" | "to-head" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            let head = head_word(word)?;
            if prop.name == "from-head" {
                style.from_head = Some(head);
            } else {
                style.to_head = Some(head);
            }
        }
        "from-card" | "to-card" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            let card = cardinality(word)?;
            if prop.name == "from-card" {
                style.from_head = Some(card);
            } else {
                style.to_head = Some(card);
            }
        }
        "router" | "route" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            style.router = Some(match word {
                "straight" | "direct" | "line" => Router::Straight,
                "ortho" | "orthogonal" | "elbow" => Router::Ortho,
                "curved" | "curve" => Router::Curved,
                other => bail!("unknown router `{other}`"),
            });
        }
        "bend" => style.bend = first.and_then(|x| x.as_num()),
        "value" => style.value = first.and_then(|x| x.as_num()),
        "points" => style.points = Some(v.iter().filter_map(|x| x.as_num()).collect()),
        "pairs" => style.pairs = Some(v.iter().filter_map(|x| x.as_num()).collect()),
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
                // A BPMN task's kind. Three genres now answer to one word, and
                // `check` is what keeps them apart — a `person` in a process
                // and a `service` in a state machine are both refused before
                // they reach here, naming the genre that does read them.
                "plain" | "user" | "service" | "script" | "manual" | "send" | "receive"
                | "rule" | "subprocess" => style.task_kind = Some(word.to_string()),
                other => bail!("unknown kind `{other}`"),
            }
        }
        "from" | "to" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or("auto");
            let side = match word {
                "auto" => None,
                "top" => Some(Side::Top),
                "bottom" => Some(Side::Bottom),
                "left" => Some(Side::Left),
                "right" => Some(Side::Right),
                other => bail!("unknown side `{other}`"),
            };
            if prop.name == "from" {
                style.from_side = side;
                style.from_port = indexed_port(prop);
            } else {
                style.to_side = side;
                style.to_port = indexed_port(prop);
            }
        }
        "lane" => style.lane = first.and_then(|x| x.as_num()),
        "stage" => style.stage = first.and_then(|x| x.as_word()).map(String::from),
        "trigger" => style.trigger = first.and_then(|x| x.as_word()).map(String::from),
        "gate" => style.gate = first.and_then(|x| x.as_word()).map(String::from),
        "artifact" => style.artifact = first.and_then(|x| x.as_word()).map(String::from),
        "flow" => style.flow = first.and_then(|x| x.as_word()).map(String::from),
        "attach" => style.attach = first.and_then(|x| x.as_word()).map(String::from),
        "tech" | "technology" => style.tech = first.and_then(|x| x.as_text()).map(String::from),
        // Three words `check` has already closed the set of; this only reads
        // them, and keeps them as written so the renderer can print them.
        "risk" => style.risk = first.and_then(|x| x.as_word()).map(String::from),
        "verify" => style.verify = first.and_then(|x| x.as_word()).map(String::from),
        "link" => style.link = first.and_then(|x| x.as_word()).map(String::from),
        "limit" => style.limit = first.and_then(|x| x.as_num()),
        "scores" => style.scores = Some(v.iter().filter_map(|x| x.as_num()).collect()),
        "branch" => style.branch = first.and_then(|x| x.as_text()).map(String::from),
        "tag" => style.tag = first.and_then(|x| x.as_text()).map(String::from),
        "wire" => {
            let a = first.and_then(|x| x.as_num()).unwrap_or(0.0).max(0.0) as u32;
            let b = v
                .get(1)
                .and_then(|x| x.as_num())
                .map_or(a, |n| n.max(0.0) as u32);
            style.wire = Some((a, b));
        }
        "count" => style.set_size = first.and_then(|x| x.as_num()),
        "shared" => style.shared = first.and_then(|x| x.as_num()),
        "role" => {
            let word = first.and_then(|x| x.as_word()).unwrap_or_default();
            style.role = Some(match word {
                "server" => NetRole::Server,
                "switch" => NetRole::Switch,
                "router" => NetRole::Router,
                "firewall" => NetRole::Firewall,
                "storage" => NetRole::Storage,
                "client" => NetRole::Client,
                "cloud" => NetRole::Cloud,
                other => bail!("unknown role `{other}`"),
            });
        }
        "addr" => style.addr = first.and_then(|x| x.as_text()).map(String::from),
        "speed" => style.speed = first.and_then(|x| x.as_text()).map(String::from),
        "u" => {
            let a = first.and_then(|x| x.as_num()).unwrap_or(1.0).max(0.0) as u32;
            let b = v
                .get(1)
                .and_then(|x| x.as_num())
                .map_or(a, |n| n.max(0.0) as u32);
            style.u = Some((a.min(b), a.max(b)));
        }
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
        // Same shape, two units. A single value is a range of one, which is
        // what makes `bits: 8` and `bytes: 4` mean one cell rather than none.
        "bits" | "bytes" => {
            let start = first.and_then(|x| x.as_num()).unwrap_or(0.0).max(0.0) as u32;
            let end = v
                .get(1)
                .and_then(|x| x.as_num())
                .map_or(start, |n| n.max(0.0) as u32);
            let range = Some((start.min(end), start.max(end)));
            if prop.name == "bits" {
                style.bits = range;
            } else {
                style.bytes = range;
            }
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

/// Builtin notation supplied by the written form, underneath author styles.
/// The genre guard matters: `event` also belongs to timeline.
pub fn item_defaults(mut base: Style, def: &crate::props::ArchetypeDef, written: &str) -> Style {
    if def.name == "bpmn" {
        match def.style_word(written) {
            "task" => {
                base.task_kind.get_or_insert_with(|| "plain".into());
            }
            "event" => {
                base.stage.get_or_insert_with(|| "intermediate".into());
                base.trigger.get_or_insert_with(|| "plain".into());
            }
            "gateway" => {
                base.gate.get_or_insert_with(|| "exclusive".into());
            }
            "data" => {
                base.artifact.get_or_insert_with(|| "object".into());
            }
            _ => {}
        }
    }
    base
}

/// Named style blocks, already flattened through their `extends` chain.
pub struct Registry {
    resolved: HashMap<String, Style>,
}

impl Registry {
    /// Resolve one declaration through builtin form defaults and the common cascade.
    pub fn resolve_node(
        &self,
        base: Style,
        def: &crate::props::ArchetypeDef,
        node: &NodeDecl,
    ) -> Result<Style> {
        self.resolve(
            item_defaults(base, def, node.keyword.as_str()),
            def.style_word(node.keyword.as_str()),
            &node.classes,
            &node.props_for(def),
        )
    }

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

    /// The common node theme followed by the declaration's own theme.
    pub(crate) fn implicit_layers<'a>(
        &'a self,
        implicit: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a Style)> {
        ["node", implicit]
            .into_iter()
            .enumerate()
            .filter(move |(i, _)| *i != 0 || implicit != "node")
            .filter_map(|(_, name)| self.get(name).map(|style| (name, style)))
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

        for (_, found) in self.implicit_layers(implicit) {
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
    /// A heading for any genre. The chart archetypes carry a `title` of their
    /// own in their settings and that one wins; this is what every other
    /// genre — `free`, `class`, `er`, `harness` — had no way to say at all.
    pub title: Option<String>,
    /// Where the heading sits across the drawing. `None` is `left`.
    pub title_align: Option<crate::layout::Anchor>,
    /// What the diagram is, for a screen reader. Becomes `aria-label` on the
    /// SVG root alongside `role="img"`.
    pub description: Option<String>,
    /// Column and row step for `at (col 3, row 2)`, when one is declared.
    pub grid: Option<(f64, f64)>,
    /// A bounded grid: columns, rows, and the gaps between tracks.
    pub grid_spec: Option<GridSpec>,
    /// A font to carry inside the SVG: the family it should be called, and
    /// its bytes. Set from the command line rather than from the file,
    /// because it is a choice about the *output* — the same diagram is
    /// embedded for a hand-off and left light for a repository.
    pub embed_font: Option<(String, std::sync::Arc<Vec<u8>>)>,
    /// Draw as if by hand. A sketched architecture says "this is a proposal"
    /// where a crisp one says "this is decided", which is information rather
    /// than ornament.
    pub sketch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridSpec {
    pub columns: usize,
    pub rows: usize,
    pub gap_col: f64,
    pub gap_row: f64,
}

impl GridSpec {
    /// Fixed track pitch derived from the largest item in each axis. This is
    /// deterministic and keeps every declared cell large enough for a node.
    pub fn pitch(self, sizes: &[(f64, f64)]) -> (f64, f64) {
        let width = sizes.iter().map(|(w, _)| *w).fold(0.0, f64::max);
        let height = sizes.iter().map(|(_, h)| *h).fold(0.0, f64::max);
        (width + self.gap_col, height + self.gap_row)
    }
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
            title: None,
            title_align: None,
            description: None,
            grid: None,
            grid_spec: None,
            embed_font: None,
            sketch: false,
        }
    }
}

impl GraphOpts {
    pub fn from_props(props: &[Prop]) -> Result<GraphOpts> {
        let mut opts = GraphOpts::default();

        for prop in props {
            let first = prop.values.first();
            match prop.name.as_str() {
                "title" => opts.title = first.and_then(|x| x.as_text()).map(String::from),
                "title-align" => {
                    opts.title_align = match first.and_then(|x| x.as_word()).unwrap_or("left") {
                        "left" => None,
                        "center" | "centre" | "middle" => Some(crate::layout::Anchor::Middle),
                        "right" => Some(crate::layout::Anchor::End),
                        other => bail!("unknown title alignment `{other}`"),
                    }
                }
                "description" | "desc" => {
                    opts.description = first.and_then(|x| x.as_text()).map(String::from)
                }
                "sketch" => {
                    opts.sketch = !matches!(
                        first.and_then(|x| x.as_word()),
                        Some("off" | "false" | "none")
                    )
                }
                "grid" => {
                    let numbers: Vec<f64> = prop.values.iter().filter_map(|v| v.as_num()).collect();
                    if let [columns, rows, gap_col, gap_row] = numbers.as_slice() {
                        if *columns < 1.0
                            || *rows < 1.0
                            || columns.fract() != 0.0
                            || rows.fract() != 0.0
                            || *gap_col < 0.0
                            || *gap_row < 0.0
                        {
                            bail!("`grid` needs positive integer dimensions and non-negative gaps");
                        }
                        opts.grid_spec = Some(GridSpec {
                            columns: *columns as usize,
                            rows: *rows as usize,
                            gap_col: *gap_col,
                            gap_row: *gap_row,
                        });
                        opts.grid = None;
                    } else {
                        opts.grid = match numbers.as_slice() {
                            // One number is a square cell, which is the common
                            // case and reads better than writing it twice.
                            [step] => Some((*step, *step)),
                            [x, y, ..] => Some((*x, *y)),
                            [] => {
                                bail!("`grid` needs a step: `grid: 160 90`, or one number for both")
                            }
                        };
                        opts.grid_spec = None;
                    }
                }
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
