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
            Rule::item_decl => diagram.nodes.push(parse_item(pair)?),
            Rule::group_decl => parse_group(pair, &mut diagram)?,
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

/// Flattens a group: its members are appended to the diagram's own lists and
/// named on the group, so every identifier stays in one global namespace.
fn parse_group(pair: Pair<Rule>, diagram: &mut Diagram) -> Result<()> {
    let span = span_of(&pair);
    let mut id = None;
    let mut label = None;
    let mut classes = Vec::new();
    let mut placement = None;
    let mut props = Vec::new();
    let mut members = Vec::new();
    let mut edge_members = Vec::new();

    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::ident if id.is_none() => id = Some(spanned(&part)),
            Rule::string if label.is_none() => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::abs_pos | Rule::rel_pos => placement = Some(parse_placement(part)?),
            Rule::group_block => {
                for inner in part.into_inner() {
                    match inner.as_rule() {
                        Rule::prop => props.extend(parse_prop(inner)?),
                        Rule::item_decl => {
                            let node = parse_item(inner)?;
                            members.push(node.id.clone());
                            diagram.nodes.push(node);
                        }
                        Rule::group_decl => {
                            let name = group_name(&inner)?;
                            members.push(name);
                            parse_group(inner, diagram)?;
                        }
                        Rule::edge_decl => {
                            edge_members.push(diagram.edges.len());
                            diagram.edges.push(parse_edge(inner)?);
                        }
                        Rule::style_decl => diagram.styles.push(parse_style(inner)?),
                        other => bail!("`{other:?}` cannot be nested in a group"),
                    }
                }
            }
            other => bail!("unexpected rule {other:?} in group declaration"),
        }
    }

    diagram.groups.push(GroupDecl {
        id: id.expect("a group declaration always names the group"),
        label,
        classes,
        placement,
        props,
        members,
        edge_members,
        span,
    });

    Ok(())
}

fn group_name(pair: &Pair<Rule>) -> Result<Spanned<String>> {
    pair.clone()
        .into_inner()
        .find(|p| p.as_rule() == Rule::ident)
        .map(|p| spanned(&p))
        .ok_or_else(|| anyhow::anyhow!("a group declaration always names the group"))
}

fn parse_item(pair: Pair<Rule>) -> Result<NodeDecl> {
    let span = span_of(&pair);
    let mut keyword = None;
    let mut id = None;
    let mut label = None;
    let mut positional = Vec::new();
    let mut positional_span: Option<Span> = None;
    let mut classes = Vec::new();
    let mut placement = None;
    let mut props = Vec::new();

    for part in pair.into_inner() {
        let part_span = span_of(&part);
        match part.as_rule() {
            // The first two identifiers are the keyword and the name; a third
            // is a bare-word positional value.
            Rule::ident if keyword.is_none() => keyword = Some(spanned(&part)),
            Rule::ident if id.is_none() => id = Some(spanned(&part)),
            // Likewise the first string is the label, by construction.
            Rule::string if label.is_none() => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::abs_pos | Rule::rel_pos => placement = Some(parse_placement(part)?),
            Rule::block => props = parse_block(part)?,
            Rule::ident | Rule::string | Rule::number | Rule::color => {
                let start = positional_span.map_or(part_span.start, |s: Span| s.start);
                positional_span = Some(Span {
                    start,
                    end: part_span.end,
                });
                positional.push(parse_value(part)?);
            }
            other => bail!("unexpected rule {other:?} in item declaration"),
        }
    }

    Ok(NodeDecl {
        keyword: keyword.expect("an item declaration always leads with a keyword"),
        id: id.expect("an item declaration always names the item"),
        label,
        positional,
        positional_span: positional_span.unwrap_or(span),
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
    let mut positional = Vec::new();
    let mut positional_span: Option<Span> = None;
    let mut classes = Vec::new();
    let mut props = Vec::new();

    for part in pair.into_inner() {
        let part_span = span_of(&part);
        match part.as_rule() {
            // The first two identifiers are the endpoints; a third is a
            // bare-word positional value.
            Rule::ident if endpoints.len() < 2 => endpoints.push(spanned(&part)),
            Rule::arrow => arrow = Arrow::parse(part.as_str()),
            Rule::string if label.is_none() => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::block => props = parse_block(part)?,
            Rule::ident | Rule::string | Rule::number | Rule::color => {
                let start = positional_span.map_or(part_span.start, |s: Span| s.start);
                positional_span = Some(Span {
                    start,
                    end: part_span.end,
                });
                positional.push(parse_value(part)?);
            }
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
        positional,
        positional_span: positional_span.unwrap_or(span),
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
        props.extend(parse_prop(prop)?);
    }
    Ok(props)
}

fn parse_prop(prop: Pair<Rule>) -> Result<Vec<Prop>> {
    let mut props = Vec::new();
    {
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

            values.push(parse_value(value)?);
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

fn parse_value(pair: Pair<Rule>) -> Result<Value> {
    Ok(match pair.as_rule() {
        Rule::number => Value::Num(number(pair)?),
        Rule::color => Value::Color(pair.as_str().to_string()),
        Rule::string => Value::Str(unescape(pair)),
        Rule::ident => Value::Word(pair.as_str().to_string()),
        other => bail!("unexpected rule {other:?} as a value"),
    })
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
