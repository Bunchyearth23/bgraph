//! Semantic validation, with spans.
//!
//! This is the single source of truth: `layout::solve` runs it and reports the
//! first error, the language server runs it and reports every one of them.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::props::{self, Ctx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone)]
pub struct Diag {
    pub severity: Severity,
    pub message: String,
    pub span: Span,
}

impl Diag {
    fn error(span: Span, message: impl Into<String>) -> Diag {
        Diag {
            severity: Severity::Error,
            message: message.into(),
            span,
        }
    }

    fn warn(span: Span, message: impl Into<String>) -> Diag {
        Diag {
            severity: Severity::Warning,
            message: message.into(),
            span,
        }
    }
}

pub fn check(diagram: &Diagram) -> Vec<Diag> {
    let mut out = Vec::new();

    check_styles(diagram, &mut out);
    let nodes = check_nodes(diagram, &mut out);
    check_edges(diagram, &nodes, &mut out);
    check_graph(diagram, &mut out);
    check_cycles(diagram, &nodes, &mut out);

    out.sort_by_key(|d| (d.span.start, d.span.end));
    out
}

/// The first error, if any — what the CLI reports.
pub fn first_error(diagram: &Diagram) -> Option<Diag> {
    check(diagram)
        .into_iter()
        .find(|d| d.severity == Severity::Error)
}

fn style_names(diagram: &Diagram) -> HashSet<&str> {
    diagram.styles.iter().map(|s| s.name.as_str()).collect()
}

fn check_styles(diagram: &Diagram, out: &mut Vec<Diag>) {
    let mut seen: HashMap<&str, Span> = HashMap::new();
    let known = style_names(diagram);

    for style in &diagram.styles {
        if let Some(_previous) = seen.insert(style.name.as_str(), style.name.span) {
            out.push(Diag::warn(
                style.name.span,
                format!(
                    "style `{}` is declared more than once; the last one wins",
                    style.name.as_str()
                ),
            ));
        }

        if let Some(parent) = &style.extends
            && !known.contains(parent.as_str())
        {
            out.push(Diag::error(
                parent.span,
                format!("unknown style `{}`", parent.as_str()),
            ));
        }

        // `node` and `edge` are the implicit themes; anything else is generic
        // and validated against the union of both vocabularies.
        let ctx = match style.name.as_str() {
            "node" => Some(Ctx::Node),
            "edge" => Some(Ctx::Edge),
            _ => None,
        };
        check_props(&style.props, ctx, false, out);
    }

    check_extends_cycles(diagram, out);
}

fn check_extends_cycles(diagram: &Diagram, out: &mut Vec<Diag>) {
    let by_name: HashMap<&str, &StyleDecl> = diagram
        .styles
        .iter()
        .map(|s| (s.name.as_str(), s))
        .collect();

    for style in &diagram.styles {
        let mut seen = HashSet::new();
        let mut cursor = Some(style);

        while let Some(current) = cursor {
            if !seen.insert(current.name.as_str()) {
                out.push(Diag::error(
                    style.name.span,
                    format!("style `{}` extends itself in a cycle", style.name.as_str()),
                ));
                break;
            }
            cursor = current
                .extends
                .as_ref()
                .and_then(|p| by_name.get(p.as_str()).copied());
        }
    }
}

fn check_nodes<'a>(diagram: &'a Diagram, out: &mut Vec<Diag>) -> HashMap<&'a str, usize> {
    let known_styles = style_names(diagram);
    let mut index = HashMap::new();

    for (i, node) in diagram.nodes.iter().enumerate() {
        if index.insert(node.id.as_str(), i).is_some() {
            out.push(Diag::error(
                node.id.span,
                format!("node `{}` is declared twice", node.id.as_str()),
            ));
        }

        for class in &node.classes {
            if !known_styles.contains(class.as_str()) {
                out.push(Diag::error(
                    class.span,
                    format!("unknown style `.{}`", class.as_str()),
                ));
            }
        }

        check_props(&node.props, Some(Ctx::Node), false, out);
    }

    // Anchors are checked after the whole index exists, so forward references
    // are legal.
    for node in &diagram.nodes {
        let Some(Placement::Rel { anchor, .. }) = &node.placement else {
            continue;
        };
        if anchor.as_str() == node.id.as_str() {
            out.push(Diag::error(
                anchor.span,
                format!("node `{}` is anchored to itself", node.id.as_str()),
            ));
        } else if !index.contains_key(anchor.as_str()) {
            out.push(Diag::error(
                anchor.span,
                format!("unknown node `{}`", anchor.as_str()),
            ));
        }
    }

    index
}

fn check_edges(diagram: &Diagram, nodes: &HashMap<&str, usize>, out: &mut Vec<Diag>) {
    let known_styles = style_names(diagram);

    for edge in &diagram.edges {
        for endpoint in [&edge.from, &edge.to] {
            if !nodes.contains_key(endpoint.as_str()) {
                out.push(Diag::error(
                    endpoint.span,
                    format!("unknown node `{}`", endpoint.as_str()),
                ));
            }
        }

        if edge.from.as_str() == edge.to.as_str() {
            out.push(Diag::warn(
                edge.span,
                format!(
                    "`{}` links to itself; self-loops are not drawn yet",
                    edge.from.as_str()
                ),
            ));
        }

        for class in &edge.classes {
            if !known_styles.contains(class.as_str()) {
                out.push(Diag::error(
                    class.span,
                    format!("unknown style `.{}`", class.as_str()),
                ));
            }
        }

        check_props(&edge.props, Some(Ctx::Edge), false, out);
    }
}

fn check_graph(diagram: &Diagram, out: &mut Vec<Diag>) {
    check_props(&diagram.graph, None, true, out);
}

/// Detects anchor loops the same way the solver does: repeatedly place what
/// can be placed; whatever never becomes placeable forms a cycle.
fn check_cycles(diagram: &Diagram, nodes: &HashMap<&str, usize>, out: &mut Vec<Diag>) {
    let mut resolved = vec![false; diagram.nodes.len()];
    let mut pending: Vec<usize> = (0..diagram.nodes.len()).collect();

    loop {
        let before = pending.len();
        let mut deferred = Vec::new();

        for &i in &pending {
            let anchor = match &diagram.nodes[i].placement {
                Some(Placement::Rel { anchor, .. }) => Some(anchor.as_str()),
                Some(Placement::At { .. }) => None,
                // An unplaced node flows from the previous one in source order.
                None if i > 0 => Some(diagram.nodes[i - 1].id.as_str()),
                None => None,
            };

            match anchor.and_then(|a| nodes.get(a)) {
                // An unknown anchor was already reported; treat it as resolved
                // so it doesn't masquerade as a cycle too.
                Some(&a) if !resolved[a] && a != i => deferred.push(i),
                _ => resolved[i] = true,
            }
        }

        if deferred.is_empty() {
            return;
        }
        if deferred.len() == before {
            let names: Vec<&str> = deferred
                .iter()
                .map(|&i| diagram.nodes[i].id.as_str())
                .collect();
            for &i in &deferred {
                out.push(Diag::error(
                    diagram.nodes[i].id.span,
                    format!("anchor cycle between: {}", names.join(", ")),
                ));
            }
            return;
        }
        pending = deferred;
    }
}

fn check_props(props: &[Prop], ctx: Option<Ctx>, graph: bool, out: &mut Vec<Diag>) {
    for prop in props {
        let Some(def) = props::lookup(&prop.name, graph) else {
            let hint = suggest(&prop.name, graph)
                .map(|s| format!(" — did you mean `{s}`?"))
                .unwrap_or_default();
            let what = if graph { "graph setting" } else { "property" };
            out.push(Diag::error(
                prop.name_span,
                format!("unknown {what} `{}`{hint}", prop.name),
            ));
            continue;
        };

        if let Some(ctx) = ctx
            && !def.ctx.allows(ctx)
        {
            let where_ = match ctx {
                Ctx::Node => "a node",
                Ctx::Edge => "an edge",
                Ctx::Both => "here",
            };
            out.push(Diag::warn(
                prop.name_span,
                format!("`{}` has no effect on {where_}", prop.name),
            ));
        }

        if prop.values.is_empty() {
            out.push(Diag::error(
                prop.name_span,
                format!("`{}` needs a value — {}", prop.name, def.syntax),
            ));
            continue;
        }

        // Only enumerations are validated by value: everything else is a
        // color, a number or free text, and those the grammar already typed.
        if def.words.is_empty() {
            continue;
        }
        for value in &prop.values {
            let Some(word) = value.as_word() else {
                continue;
            };
            // A compound like `stroke` mixes colors, widths and patterns; a
            // bare word there may legitimately be a named color.
            let strict = matches!(
                prop.name.as_str(),
                "shape" | "router" | "route" | "arrow" | "head" | "flow" | "direction" | "shadow"
            );
            if strict && !def.words.contains(&word) {
                out.push(Diag::error(
                    prop.values_span,
                    format!(
                        "`{word}` is not a valid `{}` — try {}",
                        prop.name,
                        def.words.join(", ")
                    ),
                ));
            }
        }
    }
}

/// Cheap edit-distance suggestion so a typo names its own fix.
fn suggest(name: &str, graph: bool) -> Option<&'static str> {
    let table = if graph {
        props::GRAPH_PROPS
    } else {
        props::PROPS
    };
    table
        .iter()
        .map(|p| (distance(name, p.name), p.name))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d)
        .map(|(_, name)| name)
}

fn distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut row = vec![0; b.len() + 1];

    for i in 1..=a.len() {
        row[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            row[j] = (prev[j] + 1).min(row[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut row);
    }

    prev[b.len()]
}
