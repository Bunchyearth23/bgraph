use anyhow::{Context, Result, bail};
use pest::Parser;
use pest::iterators::Pair;
use pest_derive::Parser as DeriveParser;

use crate::ast::*;

#[derive(DeriveParser)]
#[grammar = "syntax.pest"]
pub struct DiagramParser;

/// A parse failure with the byte range it points at, so the language server
/// can underline it. The CLI just prints `message`.
#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

pub fn parse_diagram(input: &str) -> Result<Diagram> {
    parse(input).map_err(anyhow::Error::from)
}

pub fn parse(input: &str) -> std::result::Result<Diagram, ParseError> {
    let parsed = DiagramParser::parse(Rule::file, input).map_err(|err| ParseError {
        message: format!("syntax error: {}", expectation(&err)),
        span: error_span(&err, input),
    })?;

    let file = parsed
        .into_iter()
        .next()
        .expect("the file rule always yields one pair");

    build(file).map_err(|err| ParseError {
        message: format!("{err:#}"),
        span: Span { start: 0, end: 0 },
    })
}

fn expectation(err: &pest::error::Error<Rule>) -> String {
    match &err.variant {
        pest::error::ErrorVariant::ParsingError { positives, .. } if !positives.is_empty() => {
            let names: Vec<String> = positives.iter().map(|r| friendly(*r)).collect();
            format!("expected {}", names.join(" or "))
        }
        pest::error::ErrorVariant::ParsingError { .. } => "unexpected input".into(),
        pest::error::ErrorVariant::CustomError { message } => message.clone(),
    }
}

fn friendly(rule: Rule) -> String {
    match rule {
        Rule::ident => "an identifier",
        Rule::string => "a quoted label",
        Rule::number => "a number",
        Rule::color => "a color like #38bdf8",
        Rule::block => "a `{ … }` block",
        Rule::prop => "a property",
        Rule::arrow => "an arrow (`->`, `--`, `<->`, …)",
        Rule::point => "a point like (40, 60)",
        Rule::align_kw => "left, right, center, top or bottom",
        Rule::rel_dir => "right-of, left-of, above or below",
        other => return format!("{other:?}"),
    }
    .to_string()
}

fn error_span(err: &pest::error::Error<Rule>, input: &str) -> Span {
    let (start, end) = match err.location {
        pest::error::InputLocation::Pos(p) => (p, p + 1),
        pest::error::InputLocation::Span((s, e)) => (s, e),
    };
    Span {
        start: start.min(input.len()),
        end: end.min(input.len()),
    }
}

fn build(file: Pair<Rule>) -> Result<Diagram> {
    let mut diagram = Diagram::default();

    for pair in file.into_inner() {
        match pair.as_rule() {
            Rule::type_decl => {
                let mut parts = pair.into_inner();
                let name = parts.next().expect("a type declaration names an archetype");
                // A second `type` line is reported by `check`, not silently
                // dropped, so keep the first and let validation speak.
                if diagram.archetype.is_none() {
                    diagram.archetype = Some(spanned(&name));
                    if let Some(block) = parts.next() {
                        diagram.archetype_props = parse_block(block)?;
                    }
                } else {
                    diagram.extra_archetypes.push(spanned(&name));
                }
            }
            Rule::graph_decl => {
                diagram.graph_span = Some(span_of(&pair));
                let block = pair.into_inner().next().expect("graph block");
                diagram.graph.extend(parse_block(block)?);
            }
            Rule::style_decl => diagram.styles.push(parse_style(pair)?),
            Rule::node_decl => diagram.nodes.push(parse_node(pair)?),
            Rule::edge_decl => diagram.edges.push(parse_edge(pair)?),
            Rule::EOI => {}
            other => bail!("unexpected rule {other:?}"),
        }
    }

    Ok(diagram)
}

fn span_of(pair: &Pair<Rule>) -> Span {
    let s = pair.as_span();
    Span {
        start: s.start(),
        end: s.end(),
    }
}

fn spanned(pair: &Pair<Rule>) -> Spanned<String> {
    Spanned {
        value: pair.as_str().to_string(),
        span: span_of(pair),
    }
}

fn parse_style(pair: Pair<Rule>) -> Result<StyleDecl> {
    let span = span_of(&pair);
    let mut name = None;
    let mut extends = None;
    let mut props = Vec::new();

    for part in pair.into_inner() {
        match part.as_rule() {
            // Two bare idents in a row means `style child extends parent`.
            Rule::ident if name.is_none() => name = Some(spanned(&part)),
            Rule::ident => extends = Some(spanned(&part)),
            Rule::block => props = parse_block(part)?,
            other => bail!("unexpected rule {other:?} in style declaration"),
        }
    }

    Ok(StyleDecl {
        name: name.expect("a style declaration always names the style"),
        extends,
        props,
        span,
    })
}

fn parse_node(pair: Pair<Rule>) -> Result<NodeDecl> {
    let span = span_of(&pair);
    let mut id = None;
    let mut label = None;
    let mut classes = Vec::new();
    let mut placement = None;
    let mut props = Vec::new();

    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::ident => id = Some(spanned(&part)),
            Rule::string => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::abs_pos | Rule::rel_pos => placement = Some(parse_placement(part)?),
            Rule::block => props = parse_block(part)?,
            other => bail!("unexpected rule {other:?} in node declaration"),
        }
    }

    Ok(NodeDecl {
        id: id.expect("a node declaration always names the node"),
        label,
        classes,
        placement,
        props,
        span,
    })
}

fn parse_edge(pair: Pair<Rule>) -> Result<EdgeDecl> {
    let span = span_of(&pair);
    let mut endpoints = Vec::new();
    let mut arrow = Arrow::parse("->");
    let mut label = None;
    let mut classes = Vec::new();
    let mut props = Vec::new();

    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::ident => endpoints.push(spanned(&part)),
            Rule::arrow => arrow = Arrow::parse(part.as_str()),
            Rule::string => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::block => props = parse_block(part)?,
            other => bail!("unexpected rule {other:?} in edge declaration"),
        }
    }

    let to = endpoints.pop().expect("an edge always has two endpoints");
    let from = endpoints.pop().expect("an edge always has two endpoints");

    Ok(EdgeDecl {
        from,
        to,
        arrow,
        label,
        classes,
        props,
        span,
    })
}

fn parse_placement(pair: Pair<Rule>) -> Result<Placement> {
    match pair.as_rule() {
        Rule::abs_pos => {
            let point = pair.into_inner().next().expect("at() always has a point");
            let mut coords = point.into_inner();
            let x = number(coords.next().expect("point x"))?;
            let y = number(coords.next().expect("point y"))?;
            Ok(Placement::At { x, y })
        }
        Rule::rel_pos => {
            let mut dir = RelDir::RightOf;
            let mut anchor = None;
            let mut gap = None;
            let mut align = None;

            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::rel_dir => {
                        dir = match part.as_str() {
                            "right-of" => RelDir::RightOf,
                            "left-of" => RelDir::LeftOf,
                            "below" => RelDir::Below,
                            "above" => RelDir::Above,
                            other => bail!("unknown direction `{other}`"),
                        }
                    }
                    Rule::ident => anchor = Some(spanned(&part)),
                    Rule::gap => {
                        let n = part.into_inner().next().expect("gap value");
                        gap = Some(number(n)?);
                    }
                    Rule::align => {
                        let kw = part.into_inner().next().expect("align keyword");
                        align = Some(match kw.as_str() {
                            "left" | "top" => Align::Start,
                            "right" | "bottom" => Align::End,
                            _ => Align::Center,
                        });
                    }
                    other => bail!("unexpected rule {other:?} in placement"),
                }
            }

            Ok(Placement::Rel {
                dir,
                anchor: anchor.expect("a relative placement always names an anchor"),
                gap,
                align,
            })
        }
        other => bail!("unexpected rule {other:?} as placement"),
    }
}

fn parse_block(pair: Pair<Rule>) -> Result<Vec<Prop>> {
    let mut props = Vec::new();

    for prop in pair.into_inner() {
        let span = span_of(&prop);
        let mut parts = prop.into_inner();
        let key = parts.next().expect("property name");
        let name = key.as_str().to_string();
        let name_span = span_of(&key);

        let mut values = Vec::new();
        let mut values_span = Span {
            start: name_span.end,
            end: span.end,
        };
        let mut first_value = true;

        for value in parts {
            let vspan = span_of(&value);
            if first_value {
                values_span.start = vspan.start;
                first_value = false;
            }
            values_span.end = vspan.end;

            values.push(match value.as_rule() {
                Rule::number => Value::Num(number(value)?),
                Rule::color => Value::Color(value.as_str().to_string()),
                Rule::string => Value::Str(unescape(value)),
                Rule::ident => Value::Word(value.as_str().to_string()),
                other => bail!("unexpected rule {other:?} as property value"),
            });
        }

        props.push(Prop {
            name,
            name_span,
            values,
            values_span,
            span,
        });
    }

    Ok(props)
}

fn class_name(pair: Pair<Rule>) -> Spanned<String> {
    let inner = pair
        .into_inner()
        .next()
        .expect("a class reference always names a style");
    spanned(&inner)
}

/// `12`, `12px` and `50%` all yield the bare number; units are advisory.
fn number(pair: Pair<Rule>) -> Result<f64> {
    let raw = pair.as_str();
    let trimmed = raw.trim_end_matches("px").trim_end_matches('%');
    trimmed
        .parse()
        .with_context(|| format!("`{raw}` is not a number"))
}

fn unescape(pair: Pair<Rule>) -> String {
    let raw = pair.into_inner().next().map(|p| p.as_str()).unwrap_or("");
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();

    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }

    out
}
