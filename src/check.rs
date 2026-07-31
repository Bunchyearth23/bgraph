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

    let archetype = check_archetype(diagram, &mut out);
    check_styles(diagram, &mut out);
    let nodes = check_nodes(diagram, &mut out);
    check_edges(diagram, &nodes, &mut out);
    check_graph(diagram, &mut out);

    check_contrast(diagram, archetype, &mut out);

    // Anchors only mean something where placement means something.
    match archetype {
        "pie" => check_pie(diagram, &mut out),
        "packet" => check_packet(diagram, &mut out),
        _ => {
            check_cycles(diagram, &nodes, &mut out);
            check_unused_data(diagram, &mut out);
        }
    }

    out.sort_by_key(|d| (d.span.start, d.span.end));
    out
}

/// The declared archetype, or `free`. Reports an unknown name and any
/// redeclaration, then validates the `type { … }` block against that
/// archetype's own settings.
fn check_archetype(diagram: &Diagram, out: &mut Vec<Diag>) -> &'static str {
    let def = match &diagram.archetype {
        Some(name) => match props::archetype(name.as_str()) {
            Some(def) => def,
            None => {
                let known: Vec<&str> = props::ARCHETYPES.iter().map(|a| a.name).collect();
                out.push(Diag::error(
                    name.span,
                    format!(
                        "unknown archetype `{}` — try {}",
                        name.as_str(),
                        known.join(", ")
                    ),
                ));
                props::archetype("free").expect("free is always defined")
            }
        },
        None => props::archetype("free").expect("free is always defined"),
    };

    for extra in &diagram.extra_archetypes {
        out.push(Diag::error(
            extra.span,
            "`type` is already declared; a file has one archetype".to_string(),
        ));
    }

    if def.props.is_empty() && !diagram.archetype_props.is_empty() {
        let span = diagram
            .archetype_props
            .first()
            .map(|p| p.span)
            .unwrap_or_default();
        out.push(Diag::warn(
            span,
            format!("`type {}` takes no settings", def.name),
        ));
    } else {
        check_props(&diagram.archetype_props, None, def.props, "setting", out);
    }

    def.name
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
        check_props(&style.props, ctx, props::PROPS, "property", out);
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
        // A statement keyword cannot name a node: an edge starting with one
        // is unparseable, so the name would be half-usable at best.
        if props::KEYWORDS.iter().any(|(k, _)| *k == node.id.as_str()) {
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` is a reserved word and cannot name a node",
                    node.id.as_str()
                ),
            ));
        }

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

        check_props(&node.props, Some(Ctx::Node), props::PROPS, "property", out);
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

        for class in &edge.classes {
            if !known_styles.contains(class.as_str()) {
                out.push(Diag::error(
                    class.span,
                    format!("unknown style `.{}`", class.as_str()),
                ));
            }
        }

        check_props(&edge.props, Some(Ctx::Edge), props::PROPS, "property", out);
    }
}

fn check_graph(diagram: &Diagram, out: &mut Vec<Diag>) {
    check_props(
        &diagram.graph,
        None,
        props::GRAPH_PROPS,
        "graph setting",
        out,
    );
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

/// Resolves each node's style the way the layout will, so a `value` or `bits`
/// inherited from a class counts as declared.
fn resolved_nodes(diagram: &Diagram, base: crate::style::Style) -> Vec<crate::style::Style> {
    let Ok(registry) = crate::style::Registry::build(&diagram.styles) else {
        return Vec::new();
    };
    diagram
        .nodes
        .iter()
        .map(|node| {
            registry
                .resolve(base.clone(), "node", &node.classes, &node.props)
                .unwrap_or_default()
        })
        .collect()
}

fn warn_ignored(diagram: &Diagram, archetype: &str, out: &mut Vec<Diag>) {
    for edge in &diagram.edges {
        out.push(Diag::warn(
            edge.span,
            format!("`type {archetype}` draws no edges; this line is ignored"),
        ));
    }
    for node in &diagram.nodes {
        if node.placement.is_some() {
            out.push(Diag::warn(
                node.span,
                format!("`type {archetype}` places its own nodes; the placement is ignored"),
            ));
        }
    }
}

fn check_pie(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "pie", out);

    let styles = resolved_nodes(diagram, crate::charts::chart_defaults());
    let mut drawn = 0;

    for (i, node) in diagram.nodes.iter().enumerate() {
        match styles.get(i).and_then(|s| s.value) {
            None => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no `value` — every slice of a pie needs one",
                    node.id.as_str()
                ),
            )),
            Some(v) if v < 0.0 => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has a negative value; a slice cannot be negative",
                    node.id.as_str()
                ),
            )),
            Some(0.0) => out.push(Diag::warn(
                node.id.span,
                format!("`{}` has value 0 and will not be drawn", node.id.as_str()),
            )),
            Some(_) => drawn += 1,
        }
    }

    let span = diagram
        .archetype
        .as_ref()
        .map(|a| a.span)
        .unwrap_or_default();

    // Reading angles is hard; these are the two ends where a pie stops working.
    if (1..3).contains(&drawn) {
        out.push(Diag::warn(
            span,
            format!(
                "a pie of {drawn} slice{} is hard to read — a bar or a single \
                 headline number compares them better",
                if drawn == 1 { "" } else { "s" }
            ),
        ));
    }
    if drawn > crate::props::PALETTE_LIGHT.len() {
        out.push(Diag::warn(
            span,
            format!(
                "{drawn} slices exhaust the categorical palette ({} hues) — merge \
                 the tail into an `Other` slice rather than repeating a color",
                crate::props::PALETTE_LIGHT.len()
            ),
        ));
    }
}

fn check_packet(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "packet", out);

    let styles = resolved_nodes(diagram, crate::style::Style::node_defaults());
    let mut fields: Vec<(u32, u32, &str, Span)> = Vec::new();

    for (i, node) in diagram.nodes.iter().enumerate() {
        match styles.get(i).and_then(|s| s.bits) {
            None => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no `bits` — every field of a packet needs a range",
                    node.id.as_str()
                ),
            )),
            Some((start, end)) => fields.push((start, end, node.id.as_str(), node.id.span)),
        }
    }

    fields.sort_by_key(|f| f.0);

    for pair in fields.windows(2) {
        let (a_start, a_end, a_id, _) = pair[0];
        let (b_start, b_end, b_id, b_span) = pair[1];

        if b_start <= a_end {
            out.push(Diag::error(
                b_span,
                format!(
                    "`{b_id}` covers bits {b_start}–{b_end}, which overlap \
                     `{a_id}` at {a_start}–{a_end}"
                ),
            ));
        } else if b_start > a_end + 1 {
            let (from, to) = (a_end + 1, b_start - 1);
            let range = if from == to {
                format!("bit {from} is")
            } else {
                format!("bits {from}–{to} are")
            };
            out.push(Diag::warn(
                b_span,
                format!("{range} unassigned between `{a_id}` and `{b_id}`"),
            ));
        }
    }
}

/// `value` and `bits` are data for an archetype that is not in play.
fn check_unused_data(diagram: &Diagram, out: &mut Vec<Diag>) {
    for node in &diagram.nodes {
        for prop in &node.props {
            let owner = match prop.name.as_str() {
                "value" => "pie",
                "bits" => "packet",
                _ => continue,
            };
            out.push(Diag::warn(
                prop.name_span,
                format!("`{}` has no effect outside `type {owner}`", prop.name),
            ));
        }
    }
}

/// Text the author cannot see is the one styling mistake a diagram tool can
/// catch for free. Compares each label against whatever sits behind it.
fn check_contrast(diagram: &Diagram, archetype: &str, out: &mut Vec<Diag>) {
    let surface = crate::style::GraphOpts::from_props(&diagram.graph)
        .map(|o| o.background.unwrap_or_else(|| "#ffffff".into()))
        .unwrap_or_else(|_| "#ffffff".into());

    // Mirror the base each archetype actually resolves on, so a color the
    // renderer never uses is never reported.
    let base = match archetype {
        "pie" => crate::charts::chart_defaults(),
        _ => crate::style::Style::node_defaults(),
    };

    for (node, style) in diagram.nodes.iter().zip(resolved_nodes(diagram, base)) {
        // Unset means the archetype picks a readable ink itself.
        let Some(ink) = style.font_color.as_deref() else {
            continue;
        };
        // A pie's legend sits on the canvas; a box's label sits on its fill.
        let behind = match archetype {
            "pie" => surface.as_str(),
            _ => style.fill.as_deref().unwrap_or(surface.as_str()),
        };

        let Some(ratio) = crate::charts::contrast(ink, behind) else {
            continue;
        };
        if ratio < 3.0 {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` writes {ink} on {behind} — contrast {ratio:.1}:1, \
                     below the 3:1 floor; this text will not be readable",
                    node.id.as_str()
                ),
            ));
        }
    }
}

fn check_props(
    entries: &[Prop],
    ctx: Option<Ctx>,
    table: &'static [props::PropDef],
    what: &str,
    out: &mut Vec<Diag>,
) {
    for prop in entries {
        let Some(def) = props::lookup(&prop.name, table) else {
            let hint = suggest(&prop.name, table)
                .map(|s| format!(" — did you mean `{s}`?"))
                .unwrap_or_default();
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
                "shape"
                    | "router"
                    | "route"
                    | "arrow"
                    | "head"
                    | "flow"
                    | "direction"
                    | "shadow"
                    | "labels"
                    | "legend"
                    | "ruler"
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
fn suggest(name: &str, table: &'static [props::PropDef]) -> Option<&'static str> {
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
