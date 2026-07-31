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
    pub graph: Vec<Prop>,
    pub graph_span: Option<Span>,
    pub styles: Vec<StyleDecl>,
    pub nodes: Vec<NodeDecl>,
    pub edges: Vec<EdgeDecl>,
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
    pub id: Spanned<String>,
    pub label: Option<String>,
    pub classes: Vec<Spanned<String>>,
    pub placement: Option<Placement>,
    pub props: Vec<Prop>,
    pub span: Span,
}

#[derive(Debug)]
pub struct EdgeDecl {
    pub from: Spanned<String>,
    pub to: Spanned<String>,
    pub arrow: Arrow,
    pub label: Option<String>,
    pub classes: Vec<Spanned<String>>,
    pub props: Vec<Prop>,
    pub span: Span,
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
