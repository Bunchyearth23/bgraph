//! Source text to a [`Diagram`].
//!
//! The grammar in `syntax.pest` is authoritative; the tree-sitter grammar the
//! editor extension ships is a mirror of it, and a test holds the two to the
//! same corpus.

use crate::ast::*;
use anyhow::{Context, Result, bail};
use pest::Parser;
use pest::iterators::Pair;

// The generated grammar, and the `Rule` enum that comes with it. Private: the
// shape of a pest parse tree is not something a caller should be able to
// depend on, and nothing outside this module reads it.
#[allow(missing_docs)]
mod grammar {
    use pest_derive::Parser as DeriveParser;

    #[derive(DeriveParser)]
    #[grammar = "syntax.pest"]
    pub struct DiagramParser;
}

use grammar::{DiagramParser, Rule};

/// A parse failure with the byte range it points at, so the language server
/// can underline it. The CLI just prints `message`.
#[derive(Debug)]
pub struct ParseError {
    /// What is wrong, phrased for the person writing the file.
    pub message: String,
    /// Byte range in the source. Zero-length when the failure is structural
    /// rather than positional.
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parses source text, discarding the span an error carries.
///
/// This is the form the CLI wants. Editors want [`parse`], which keeps the
/// range so it can be underlined.
pub fn parse_diagram(input: &str) -> Result<Diagram> {
    parse(input).map_err(anyhow::Error::from)
}

/// How deeply groups may nest.
///
/// This is not a taste judgement about diagrams — it is a guard on the host
/// process. `pest` parses by recursive descent, so nesting depth is stack
/// depth, and a few thousand braces abort the program with `SIGABRT` before
/// any Rust code of ours runs. A library that a document generator embeds must
/// not be able to kill its caller with a text file, and an abort cannot even be
/// caught. So the depth is counted lexically, first, and refused cleanly.
///
/// Sixty-four is roughly an order of magnitude below where the stack gives out
/// and two orders above anything a person draws.
pub const MAX_NESTING: usize = 64;

/// Parses source text, keeping the span an error points at.
pub fn parse(input: &str) -> std::result::Result<Diagram, ParseError> {
    check_nesting(input)?;

    let parsed = DiagramParser::parse(Rule::file, input).map_err(|err| ParseError {
        message: format!("syntax error: {}", expectation(&err)),
        span: error_span(&err, input),
    })?;

    let file = parsed
        .into_iter()
        .next()
        .expect("the file rule always yields one pair");

    build(file, input).map_err(|err| ParseError {
        message: format!("{err:#}"),
        span: Span { start: 0, end: 0 },
    })
}

/// Counts brace depth without parsing, skipping the three places a brace is
/// not structure: string literals, comments, and `{{holes}}`.
///
/// Deliberately its own scanner rather than a grammar rule — pest would have to
/// recurse to find the depth, which is the very thing being guarded against.
fn check_nesting(input: &str) -> std::result::Result<(), ParseError> {
    let mut chars = input.char_indices().peekable();
    let mut depth = 0usize;

    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                while let Some((_, c)) = chars.next() {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' | '\n' => break,
                        _ => {}
                    }
                }
            }
            '/' if chars.peek().map(|&(_, c)| c) == Some('/') => {
                for (_, c) in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' if chars.peek().map(|&(_, c)| c) == Some('{') => {
                // A placeholder, not two blocks. Skip to its close, or give up
                // on it — an unterminated `{{` is pest's error to report.
                chars.next();
                let mut prev = None;
                for (_, c) in chars.by_ref() {
                    if prev == Some('}') && c == '}' {
                        break;
                    }
                    prev = Some(c);
                }
            }
            '{' => {
                depth += 1;
                if depth > MAX_NESTING {
                    return Err(ParseError {
                        message: format!(
                            "nested more than {MAX_NESTING} levels deep — \
                             refused before it overflows the stack"
                        ),
                        span: Span {
                            start: i,
                            end: i + 1,
                        },
                    });
                }
            }
            '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }

    Ok(())
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

fn build(file: Pair<Rule>, input: &str) -> Result<Diagram> {
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
            Rule::include_decl => diagram.includes.push(parse_include(pair)?),
            Rule::constraint_decl => diagram.constraints.push(parse_constraint(pair)?),
            Rule::note_decl => diagram.notes.push(parse_note(pair)?),
            Rule::legend_decl => diagram.legends.push(parse_note(pair)?),
            Rule::component_decl => diagram.components.push(parse_component(pair, input)?),
            Rule::use_decl => diagram.uses.push(parse_use(pair)?),
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

/// Whether this pair stands where a *name* goes — an identifier, or a hole
/// that will become one. `check` is what refuses a hole where a real name is
/// needed to answer a question.
fn is_name(rule: Rule) -> bool {
    matches!(rule, Rule::ident | Rule::placeholder)
}

fn spanned(pair: &Pair<Rule>) -> Spanned<String> {
    Spanned {
        value: pair.as_str().to_string(),
        span: span_of(pair),
    }
}

/// A `note` or a `legend`. They differ in what is drawn, not in how they are
/// written — both are placed by the ordinary anchors, both take classes, and
/// neither declares a name.
fn parse_note(pair: Pair<Rule>) -> Result<crate::ast::NoteDecl> {
    let span = span_of(&pair);
    let mut text = String::new();
    let mut classes = Vec::new();
    let mut placement = None;
    let mut props = Vec::new();

    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::string => text = unescape(part),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::block => props = parse_block(part)?,
            _ => placement = Some(parse_placement(part)?),
        }
    }

    Ok(crate::ast::NoteDecl {
        text,
        classes,
        placement,
        props,
        span,
    })
}

fn parse_constraint(pair: Pair<Rule>) -> Result<crate::ast::ConstraintDecl> {
    use crate::ast::{Align, ConstraintKind};
    let span = span_of(&pair);
    let mut kind = ConstraintKind::AlignGroup;
    let mut align = Align::Center;
    let mut members = Vec::new();

    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::constraint_kw => {
                kind = match part.as_str() {
                    "align-group" => ConstraintKind::AlignGroup,
                    "same-width" => ConstraintKind::SameWidth,
                    "same-height" => ConstraintKind::SameHeight,
                    other => bail!("unknown constraint `{other}`"),
                }
            }
            Rule::align_kw => {
                align = match part.as_str() {
                    "left" | "top" => Align::Start,
                    "right" | "bottom" => Align::End,
                    _ => Align::Center,
                }
            }
            _ => members.push(spanned(&part)),
        }
    }

    if members.len() < 2 {
        bail!("a constraint needs at least two names — one box is already aligned with itself");
    }
    Ok(crate::ast::ConstraintDecl {
        kind,
        align,
        members,
        span,
    })
}

fn parse_component(pair: Pair<Rule>, source: &str) -> Result<crate::ast::ComponentDecl> {
    let span = span_of(&pair);
    let mut inner = pair.into_inner();
    let name = inner
        .next()
        .map(|p| spanned(&p))
        .context("a component needs a name")?;
    let block = inner.next().context("a component needs a block")?;
    let block_span = span_of(&block);

    // The body is kept as *text* rather than as statements: it is filled by
    // the template engine before it is parsed, so a hole may stand anywhere,
    // including in places a parsed form could not hold one.
    let body = source
        .get(block_span.start + 1..block_span.end.saturating_sub(1))
        .unwrap_or_default()
        .to_string();

    Ok(crate::ast::ComponentDecl { name, body, span })
}

fn parse_use(pair: Pair<Rule>) -> Result<crate::ast::UseDecl> {
    let span = span_of(&pair);
    let mut inner = pair.into_inner();
    let component = inner
        .next()
        .map(|p| spanned(&p))
        .context("`use` needs a component")?;
    let prefix = inner
        .next()
        .map(|p| spanned(&p))
        .context("`use` needs `as <name>`")?;
    let mut placement = None;
    let mut bindings = Vec::new();
    for rest in inner {
        match rest.as_rule() {
            Rule::block => bindings = parse_block(rest)?,
            _ => placement = Some(parse_placement(rest)?),
        }
    }
    Ok(crate::ast::UseDecl {
        component,
        prefix,
        placement,
        bindings,
        span,
    })
}

fn parse_include(pair: Pair<Rule>) -> Result<crate::ast::IncludeDecl> {
    let span = span_of(&pair);
    let mut inner = pair.into_inner();

    let path = inner
        .next()
        .context("an include needs a path")
        .map(|p| Spanned {
            span: span_of(&p),
            value: unescape(p),
        })?;

    let prefix = inner.next().map(|p| spanned(&p));

    Ok(crate::ast::IncludeDecl { path, prefix, span })
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
    let mut activates = false;

    for part in pair.into_inner() {
        match part.as_rule() {
            Rule::group_kw => activates = part.as_str() == "activate",
            r if is_name(r) && id.is_none() => id = Some(spanned(&part)),
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
                            // An `activate` block names a participant that is
                            // already declared; listing it as a member would
                            // put the same id in two places.
                            if !is_activation(&inner) {
                                members.push(group_name(&inner)?);
                            }
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
        activates,
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

fn is_activation(pair: &Pair<Rule>) -> bool {
    pair.clone()
        .into_inner()
        .any(|p| p.as_rule() == Rule::group_kw && p.as_str() == "activate")
}

fn group_name(pair: &Pair<Rule>) -> Result<Spanned<String>> {
    pair.clone()
        .into_inner()
        .find(|p| is_name(p.as_rule()))
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
    let mut members = Vec::new();

    for part in pair.into_inner() {
        let part_span = span_of(&part);
        match part.as_rule() {
            // The first two identifiers are the keyword and the name; a third
            // is a bare-word positional value.
            Rule::ident if keyword.is_none() => keyword = Some(spanned(&part)),
            r if is_name(r) && id.is_none() => id = Some(spanned(&part)),
            // Likewise the first string is the label, by construction.
            Rule::string if label.is_none() => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::abs_pos | Rule::rel_pos => placement = Some(parse_placement(part)?),
            Rule::block => (props, members) = parse_body(part)?,
            Rule::ident
            | Rule::string
            | Rule::number
            | Rule::color
            | Rule::date
            | Rule::duration
            | Rule::placeholder => {
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
        members,
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
    let mut via = Vec::new();

    for part in pair.into_inner() {
        let part_span = span_of(&part);
        match part.as_rule() {
            // `via (300, 120) (left-of c gap 20)` — the places the line goes
            // through, each parenthesised so the list needs no separator.
            Rule::via_list => {
                for stop in part.into_inner() {
                    via.push(parse_waypoint(stop)?);
                }
            }
            // The first two identifiers are the endpoints; a third is a
            // bare-word positional value.
            r if is_name(r) && endpoints.len() < 2 => endpoints.push(spanned(&part)),
            Rule::arrow => arrow = Arrow::parse(part.as_str()),
            Rule::string if label.is_none() => label = Some(unescape(part)),
            Rule::class_ref => classes.push(class_name(part)),
            Rule::block => props = parse_block(part)?,
            Rule::ident
            | Rule::string
            | Rule::number
            | Rule::color
            | Rule::date
            | Rule::duration
            | Rule::placeholder => {
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
        via,
        span,
    })
}

/// One stop on a line's route: a point, a grid cell, or a place beside a box.
///
/// The three are written the same way — parenthesised — so an author moving a
/// waypoint from a coordinate to something relative changes what is inside the
/// brackets and nothing else.
fn parse_waypoint(pair: Pair<Rule>) -> Result<Placement> {
    match pair.as_rule() {
        Rule::point => {
            let mut coords = pair.into_inner();
            let x = number(coords.next().expect("point x"))?;
            let y = number(coords.next().expect("point y"))?;
            Ok(Placement::At { x, y })
        }
        Rule::cell => {
            let mut coords = pair.into_inner();
            let col = number(coords.next().expect("cell col"))?;
            let row = number(coords.next().expect("cell row"))?;
            Ok(Placement::Cell { col, row })
        }
        Rule::rel_point => {
            let inner = pair.into_inner().next().expect("a relative waypoint");
            parse_placement(inner)
        }
        other => bail!("unexpected rule {other:?} in a waypoint"),
    }
}

fn parse_placement(pair: Pair<Rule>) -> Result<Placement> {
    match pair.as_rule() {
        Rule::abs_pos => {
            let point = pair.into_inner().next().expect("at() always has a point");
            let grid = point.as_rule() == Rule::cell;
            let mut coords = point.into_inner();
            let x = number(coords.next().expect("point x"))?;
            let y = number(coords.next().expect("point y"))?;
            Ok(if grid {
                Placement::Cell { col: x, row: y }
            } else {
                Placement::At { x, y }
            })
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
                    r if is_name(r) => anchor = Some(spanned(&part)),
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
    Ok(parse_body(pair)?.0)
}

/// A block holds properties and, for archetypes that read them, members.
fn parse_body(pair: Pair<Rule>) -> Result<(Vec<Prop>, Vec<Spanned<String>>)> {
    let mut props = Vec::new();
    let mut members = Vec::new();

    for entry in pair.into_inner() {
        match entry.as_rule() {
            Rule::member => {
                // The marker is kept: `-` and `+` are the visibility a reader
                // of UML expects to see.
                let span = span_of(&entry);
                let text = entry.as_str().trim_end().to_string();
                members.push(Spanned { value: text, span });
            }
            _ => props.extend(parse_prop(entry)?),
        }
    }

    Ok((props, members))
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
        Rule::date => {
            let text = pair.as_str().to_string();
            let part = |a: usize, b: usize| -> i64 { text[a..b].parse().unwrap_or(0) };
            let day = crate::ast::days_from_civil(part(0, 4), part(5, 7), part(8, 10));
            Value::Date { text, day }
        }
        Rule::duration => {
            let raw = pair.as_str();
            let cut = raw
                .find(|c: char| c.is_ascii_alphabetic())
                .unwrap_or(raw.len());
            let amount: f64 = raw[..cut].parse().unwrap_or(0.0);
            let days = match &raw[cut..] {
                "h" => amount / 24.0,
                "w" => amount * 7.0,
                "mo" => amount * 30.0,
                _ => amount,
            };
            Value::Duration(days)
        }
        Rule::placeholder => {
            let raw = pair.as_str();
            Value::Placeholder(raw[2..raw.len() - 2].trim().to_string())
        }
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

// ------------------------------------------------------- error recovery

/// Parse, and keep going past the first mistake.
///
/// A statement ends at a newline, so where to resume is not a guess — which is
/// what makes this worth doing here rather than in the grammar. Without it, a
/// missing separator on line 3 costs the diagnostics *and* the semantic
/// highlighting of the fifty lines after it, which is precisely the moment
/// both were wanted.
///
/// Returns whatever could be understood alongside every problem found. The
/// diagram is `None` only when nothing at all could be salvaged.
///
/// The sound path is untouched: a file with no mistake is parsed once, exactly
/// as [`parse`] does, and pays nothing for this.
pub fn parse_recovering(input: &str) -> (Option<Diagram>, Vec<ParseError>) {
    match parse(input) {
        Ok(diagram) => return (Some(diagram), Vec::new()),
        Err(first) => {
            // An unbalanced brace is not a statement-shaped mistake: every
            // boundary after it is a guess, so the one honest answer is the
            // error itself.
            if check_nesting(input).is_err() {
                return (None, vec![first]);
            }
        }
    }

    let mut errors = Vec::new();
    // Spaces rather than deletion, so every span in what is left still points
    // where the reader's cursor is.
    let mut repaired: Vec<u8> = input.as_bytes().to_vec();

    for (start, chunk) in statements(input) {
        let Err(err) = parse(chunk) else {
            continue;
        };
        errors.push(ParseError {
            message: err.message,
            span: Span {
                start: start + err.span.start,
                end: start + err.span.end.max(err.span.start),
            },
        });
        for byte in &mut repaired[start..start + chunk.len()] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    }

    if errors.is_empty() {
        // Every statement parses on its own but the file does not: the
        // mistake is *between* two of them rather than inside one, and
        // blanking a statement at random would be a worse answer than simply
        // reporting what the whole-file parse said.
        return (None, parse(input).err().into_iter().collect());
    }

    let salvaged = String::from_utf8(repaired)
        .ok()
        .and_then(|text| parse(&text).ok());
    (salvaged, errors)
}

/// The top-level statements, each with its byte offset.
///
/// A statement runs to the newline that returns brace depth to zero, so a
/// block spanning ten lines is one statement rather than ten. The scanning
/// rules are [`check_nesting`]'s: a brace inside a string, a comment or a
/// `{{hole}}` is not structure.
fn statements(input: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut chars = input.char_indices().peekable();
    let mut depth = 0usize;
    let mut start = 0usize;

    while let Some((i, c)) = chars.next() {
        match c {
            '"' => {
                while let Some((_, c)) = chars.next() {
                    match c {
                        '\\' => {
                            chars.next();
                        }
                        '"' | '\n' => break,
                        _ => {}
                    }
                }
            }
            // The newline is left for the loop below, unlike in
            // `check_nesting`: consuming it here would weld a statement with
            // a trailing comment onto the next one, and a mistake in either
            // would then blank both.
            '/' if chars.peek().map(|&(_, c)| c) == Some('/') => {
                while let Some(&(_, c)) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            '{' if chars.peek().map(|&(_, c)| c) == Some('{') => {
                chars.next();
                let mut prev = None;
                for (_, c) in chars.by_ref() {
                    if prev == Some('}') && c == '}' {
                        break;
                    }
                    prev = Some(c);
                }
            }
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '\n' if depth == 0 => {
                if !input[start..i].trim().is_empty() {
                    out.push((start, &input[start..i]));
                }
                start = i + 1;
            }
            _ => {}
        }
    }

    if !input[start..].trim().is_empty() {
        out.push((start, &input[start..]));
    }
    out
}
