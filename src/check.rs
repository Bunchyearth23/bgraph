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

    let def = check_archetype(diagram, &mut out);
    let archetype = def.name;
    check_styles(diagram, &mut out);
    let nodes = check_nodes(diagram, def, &mut out);
    check_groups(diagram, &nodes, &mut out);
    check_edges(diagram, &nodes, &mut out);
    check_graph(diagram, &mut out);

    check_contrast(diagram, archetype, &mut out);

    // Anchors only mean something where placement means something.
    match archetype {
        "pie" => check_pie(diagram, &mut out),
        "packet" => check_packet(diagram, &mut out),
        "treemap" => check_treemap(diagram, &mut out),
        "quadrant" => check_quadrant(diagram, &mut out),
        "wave" => check_wave(diagram, &mut out),
        "sequence" => check_sequence(diagram, &mut out),
        "class" => check_class(diagram, &mut out),
        "gantt" => check_gantt(diagram, &mut out),
        "state" => {
            check_cycles(diagram, &nodes, &mut out);
            check_state(diagram, &mut out);
        }
        "sankey" => check_sankey(diagram, &mut out),
        "bar" | "xy" => check_bar(diagram, &mut out),
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
fn check_archetype(diagram: &Diagram, out: &mut Vec<Diag>) -> &'static props::ArchetypeDef {
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

    def
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

fn check_nodes<'a>(
    diagram: &'a Diagram,
    def: &'static props::ArchetypeDef,
    out: &mut Vec<Diag>,
) -> HashMap<&'a str, usize> {
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

        check_keyword(node, def, out);
        check_positional(node, def, out);
        check_props(&node.props, Some(Ctx::Node), props::PROPS, "property", out);
    }

    // Anchors are checked after the whole index exists, so forward references
    // are legal. A group is a perfectly good anchor.
    let anchorable = anchor_names(diagram);
    for (owner, placement) in placements(diagram) {
        let Placement::Rel { anchor, .. } = placement else {
            continue;
        };
        if anchor.as_str() == owner.as_str() {
            out.push(Diag::error(
                anchor.span,
                format!("`{}` is anchored to itself", owner.as_str()),
            ));
        } else if !anchorable.contains(anchor.as_str()) {
            out.push(Diag::error(
                anchor.span,
                format!("unknown node `{}`", anchor.as_str()),
            ));
        }
    }

    index
}

/// Every name a placement may point at.
fn anchor_names(diagram: &Diagram) -> HashSet<&str> {
    diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .chain(diagram.groups.iter().map(|g| g.id.as_str()))
        .collect()
}

fn placements(diagram: &Diagram) -> impl Iterator<Item = (&Spanned<String>, &Placement)> {
    diagram
        .nodes
        .iter()
        .filter_map(|n| n.placement.as_ref().map(|p| (&n.id, p)))
        .chain(
            diagram
                .groups
                .iter()
                .filter_map(|g| g.placement.as_ref().map(|p| (&g.id, p))),
        )
}

fn check_groups(diagram: &Diagram, nodes: &HashMap<&str, usize>, out: &mut Vec<Diag>) {
    let known_styles = style_names(diagram);
    let mut seen: HashSet<&str> = HashSet::new();

    for group in &diagram.groups {
        if nodes.contains_key(group.id.as_str()) || !seen.insert(group.id.as_str()) {
            out.push(Diag::error(
                group.id.span,
                format!(
                    "`{}` is declared twice — groups and items share one namespace",
                    group.id.as_str()
                ),
            ));
        }

        if props::KEYWORDS.iter().any(|(k, _)| *k == group.id.as_str()) {
            out.push(Diag::error(
                group.id.span,
                format!(
                    "`{}` is a reserved word and cannot name a group",
                    group.id.as_str()
                ),
            ));
        }

        // A group holding only edges is not empty — a sequence diagram's
        // `loop` frame spans messages, never participants.
        if group.members.is_empty() && group.edge_members.is_empty() {
            out.push(Diag::warn(
                group.span,
                format!(
                    "group `{}` is empty; its frame has nothing to enclose",
                    group.id.as_str()
                ),
            ));
        }

        for class in &group.classes {
            if !known_styles.contains(class.as_str()) {
                out.push(Diag::error(
                    class.span,
                    format!("unknown style `.{}`", class.as_str()),
                ));
            }
        }

        check_props(&group.props, Some(Ctx::Node), props::PROPS, "property", out);
    }

    // An edge to a frame has no obvious endpoint; say so rather than guess.
    let groups: HashSet<&str> = diagram.groups.iter().map(|g| g.id.as_str()).collect();
    for edge in &diagram.edges {
        for endpoint in [&edge.from, &edge.to] {
            if groups.contains(endpoint.as_str()) {
                out.push(Diag::error(
                    endpoint.span,
                    format!(
                        "`{}` is a group; edges connect items, not frames",
                        endpoint.as_str()
                    ),
                ));
            }
        }
    }
}

/// `node` is a universal synonym; anything else must be the archetype's own
/// word for its elements.
fn check_keyword(node: &NodeDecl, def: &'static props::ArchetypeDef, out: &mut Vec<Diag>) {
    let written = node.keyword.as_str();
    if written == def.item || written == "node" {
        return;
    }

    let elsewhere = props::ARCHETYPES.iter().find(|a| a.item == written);
    let hint = match elsewhere {
        Some(other) => format!(" — `{written}` belongs to `type {}`", other.name),
        None => String::new(),
    };

    out.push(Diag::error(
        node.keyword.span,
        format!(
            "`type {}` declares {}, not {}{hint}",
            def.name,
            plural(def.item),
            plural(written)
        ),
    ));
}

/// Enough English to keep a diagnostic from reading like a bug: `classs` in a
/// message about class diagrams undermines everything else it says.
fn plural(word: &str) -> String {
    let suffix = if word.ends_with('s')
        || word.ends_with('x')
        || word.ends_with("ch")
        || word.ends_with("sh")
    {
        "es"
    } else {
        "s"
    };
    format!("{word}{suffix}")
}

/// Positional values fill the archetype's slots in order.
fn check_positional(node: &NodeDecl, def: &'static props::ArchetypeDef, out: &mut Vec<Diag>) {
    if node.positional.is_empty() {
        return;
    }

    let wanted: usize = def.slots.iter().map(|s| s.arity).sum();
    if wanted == 0 {
        out.push(Diag::error(
            node.positional_span,
            format!("`type {}` takes no data after the label", def.name),
        ));
        return;
    }

    if node.positional.len() > wanted {
        let shape: Vec<String> = def.slots.iter().map(|s| format!("<{}>", s.prop)).collect();
        out.push(Diag::error(
            node.positional_span,
            format!(
                "`{}` takes {wanted} value{} after the label — `{} <id> [\"label\"] {}`",
                def.item,
                if wanted == 1 { "" } else { "s" },
                def.item,
                shape.join(" ")
            ),
        ));
        return;
    }

    // Written twice, once in each form: say which one wins.
    let mut rest = node.positional.len();
    for slot in def.slots {
        if rest == 0 {
            break;
        }
        rest = rest.saturating_sub(slot.arity);
        if let Some(inline) = node.props.iter().find(|p| p.name == slot.prop) {
            out.push(Diag::warn(
                inline.name_span,
                format!(
                    "`{}` is given both after the label and in the block; the block wins",
                    slot.prop
                ),
            ));
        }
    }
}

fn check_edges(diagram: &Diagram, nodes: &HashMap<&str, usize>, out: &mut Vec<Diag>) {
    let known_styles = style_names(diagram);

    let groups: HashSet<&str> = diagram.groups.iter().map(|g| g.id.as_str()).collect();
    for edge in &diagram.edges {
        for endpoint in [&edge.from, &edge.to] {
            if !nodes.contains_key(endpoint.as_str()) && !groups.contains(endpoint.as_str()) {
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
/// True when a node names the property but leaves it to a `{{hole}}` — the
/// datum is declared, just not bound yet, so "it is missing" would be a lie.
fn awaits_binding(node: &NodeDecl, def: &'static props::ArchetypeDef, name: &str) -> bool {
    node.effective_props(def.slots)
        .iter()
        .any(|p| p.name == name && p.values.iter().any(|v| v.is_hole()))
}

/// The base each archetype resolves its items on. `free` and `packet` draw
/// boxes; the chart archetypes draw marks and take no box defaults.
pub fn base_style(archetype: &str) -> crate::style::Style {
    match archetype {
        "free" | "packet" => crate::style::Style::node_defaults(),
        _ => crate::charts::chart_defaults(),
    }
}

fn resolved_nodes(diagram: &Diagram, base: crate::style::Style) -> Vec<crate::style::Style> {
    let Ok(registry) = crate::style::Registry::build(&diagram.styles) else {
        return Vec::new();
    };
    let def = archetype_of(diagram);
    diagram
        .nodes
        .iter()
        .map(|node| {
            registry
                .resolve(
                    base.clone(),
                    def.item,
                    &node.classes,
                    &node.effective_props(def.slots),
                )
                .unwrap_or_default()
        })
        .collect()
}

/// The declared archetype, or `free`. Unknown names are reported by
/// `check_archetype`; here they simply fall back.
pub fn archetype_of(diagram: &Diagram) -> &'static props::ArchetypeDef {
    diagram
        .archetype
        .as_ref()
        .and_then(|a| props::archetype(a.as_str()))
        .or_else(|| props::archetype("free"))
        .expect("free is always defined")
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

    let styles = resolved_nodes(diagram, base_style("pie"));
    let mut drawn = 0;

    for (i, node) in diagram.nodes.iter().enumerate() {
        match styles.get(i).and_then(|s| s.value) {
            None if !awaits_binding(node, archetype_of(diagram), "value") => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no `value` — every slice of a pie needs one",
                    node.id.as_str()
                ),
            )),
            None => {}
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
            None if !awaits_binding(node, archetype_of(diagram), "bits") => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no `bits` — every field of a packet needs a range",
                    node.id.as_str()
                ),
            )),
            None => {}
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

/// `bar` and `xy` read the same data: named categories and one number per
/// category, per series.
fn check_bar(diagram: &Diagram, out: &mut Vec<Diag>) {
    let archetype = archetype_of(diagram).name;
    warn_ignored(diagram, archetype, out);

    let cfg = &diagram.archetype_props;
    let categories = cfg
        .iter()
        .find(|p| p.name == "categories")
        .map_or(0, |p| p.values.len());

    let styles = resolved_nodes(diagram, base_style(archetype));
    for (i, node) in diagram.nodes.iter().enumerate() {
        let points = styles.get(i).and_then(|s| s.points.as_ref());
        let Some(points) = points.filter(|p| !p.is_empty()) else {
            if awaits_binding(node, archetype_of(diagram), "points") {
                continue;
            }
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no numbers — `series {} [\"label\"] <n> <n> …`",
                    node.id.as_str(),
                    node.id.as_str()
                ),
            ));
            continue;
        };

        if categories > 0 && points.len() != categories {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has {} number{} for {categories} categor{} — the \
                     shorter of the two wins",
                    node.id.as_str(),
                    points.len(),
                    if points.len() == 1 { "" } else { "s" },
                    if categories == 1 { "y" } else { "ies" }
                ),
            ));
        }
    }

    if categories == 0 {
        out.push(Diag::warn(
            diagram
                .archetype
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_default(),
            "no `categories`, so the bars stand along nothing named".to_string(),
        ));
    }

    for key in ["y-axis", "x-axis"] {
        if let Some(prop) = cfg.iter().find(|p| p.name == key)
            && prop.values.len() != 2
        {
            out.push(Diag::error(
                prop.values_span,
                format!("`{key}` takes a minimum and a maximum"),
            ));
        }
    }

    let drawn = diagram
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| styles.get(*i).and_then(|s| s.points.as_ref()).is_some())
        .count();
    if drawn > crate::props::PALETTE_LIGHT.len() {
        out.push(Diag::warn(
            diagram
                .archetype
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_default(),
            format!(
                "{drawn} series exhaust the categorical palette ({} hues) — fold \
                 the tail into an `Other` series rather than repeating a colour",
                crate::props::PALETTE_LIGHT.len()
            ),
        ));
    }
}

fn check_sankey(diagram: &Diagram, out: &mut Vec<Diag>) {
    for node in &diagram.nodes {
        if node.placement.is_some() {
            out.push(Diag::warn(
                node.span,
                "`type sankey` places its own nodes; the placement is ignored".to_string(),
            ));
        }
        if let Some(prop) = node.props.iter().find(|p| p.name == "value") {
            out.push(Diag::error(
                prop.name_span,
                format!(
                    "`{}` cannot declare a value — a node is as tall as what \
                     flows through it",
                    node.id.as_str()
                ),
            ));
        }
    }

    let def = archetype_of(diagram);
    let Ok(registry) = crate::style::Registry::build(&diagram.styles) else {
        return;
    };

    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    let mut touched = vec![false; diagram.nodes.len()];
    let mut edges = Vec::new();

    for edge in &diagram.edges {
        let value = registry
            .resolve(
                crate::charts::chart_defaults(),
                "edge",
                &edge.classes,
                &edge.effective_props(def.edge_slots),
            )
            .ok()
            .and_then(|s| s.value);

        match value {
            None => out.push(Diag::error(
                edge.span,
                format!(
                    "`{} -> {}` has no value — a flow needs a magnitude",
                    edge.from.as_str(),
                    edge.to.as_str()
                ),
            )),
            Some(v) if v <= 0.0 => out.push(Diag::warn(
                edge.span,
                format!(
                    "`{} -> {}` carries {v} and will not be drawn",
                    edge.from.as_str(),
                    edge.to.as_str()
                ),
            )),
            Some(_) => {}
        }

        if let (Some(&from), Some(&to)) =
            (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        {
            touched[from] = true;
            touched[to] = true;
            edges.push((from, to));
        }
    }

    for (i, node) in diagram.nodes.iter().enumerate() {
        if !touched[i] {
            out.push(Diag::warn(
                node.id.span,
                format!("`{}` has no flows and will not be drawn", node.id.as_str()),
            ));
        }
    }

    // Layering is a longest-path walk; a cycle has no left-to-right reading.
    if let Some(cycle) = first_cycle(diagram.nodes.len(), &edges) {
        let names: Vec<&str> = cycle
            .iter()
            .map(|i| diagram.nodes[*i].id.as_str())
            .collect();
        out.push(Diag::error(
            diagram.nodes[cycle[0]].id.span,
            format!(
                "flows form a cycle ({}); a sankey reads left to right",
                names.join(" → ")
            ),
        ));
    }
}

/// Any cycle in the flow graph, as the nodes that make it up.
fn first_cycle(count: usize, edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    let mut degree = vec![0usize; count];
    for (_, to) in edges {
        degree[*to] += 1;
    }
    let mut queue: Vec<usize> = (0..count).filter(|i| degree[*i] == 0).collect();
    let mut removed = 0;

    while let Some(node) = queue.pop() {
        removed += 1;
        for (from, to) in edges {
            if *from == node {
                degree[*to] -= 1;
                if degree[*to] == 0 {
                    queue.push(*to);
                }
            }
        }
    }

    if removed == count {
        return None;
    }
    Some((0..count).filter(|i| degree[*i] > 0).collect())
}

fn check_gantt(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "gantt", out);

    let styles = resolved_nodes(diagram, base_style("gantt"));
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = styles.get(i);
        if awaits_binding(node, archetype_of(diagram), "start") {
            continue;
        }
        match (style.and_then(|s| s.start), style.and_then(|s| s.length)) {
            (None, _) => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no start — `task {} [\"label\"] <date> <date|duration>`",
                    node.id.as_str(),
                    node.id.as_str()
                ),
            )),
            (Some(_), None) => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has a start but no end — give it a duration like `6d`, \
                     or the date it finishes",
                    node.id.as_str()
                ),
            )),
            (Some(_), Some(days)) if days <= 0.0 => out.push(Diag::error(
                node.id.span,
                format!("`{}` ends before it starts", node.id.as_str()),
            )),
            _ => {}
        }
    }

    if let Some(prop) = diagram.archetype_props.iter().find(|p| p.name == "today")
        && prop.values.first().and_then(|v| v.as_date()).is_none()
    {
        out.push(Diag::error(
            prop.values_span,
            "`today` takes a date, written `2026-09-14`".to_string(),
        ));
    }
}

fn check_class(diagram: &Diagram, out: &mut Vec<Diag>) {
    check_cycles_free(diagram, out);
    for node in &diagram.nodes {
        if node.members.is_empty() {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has no members; a class with none is a plain box",
                    node.id.as_str()
                ),
            ));
        }
    }
}

/// Archetypes that keep the free solver still need its cycle check.
fn check_cycles_free(diagram: &Diagram, out: &mut Vec<Diag>) {
    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();
    check_cycles(diagram, &index, out);
}

fn check_state(diagram: &Diagram, out: &mut Vec<Diag>) {
    use crate::style::StateKind;

    let styles = resolved_nodes(diagram, crate::style::Style::node_defaults());
    let kinds: Vec<StateKind> = styles
        .iter()
        .map(|s| s.kind.unwrap_or(StateKind::State))
        .collect();

    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut has_in = vec![false; diagram.nodes.len()];
    let mut has_out = vec![false; diagram.nodes.len()];
    for edge in &diagram.edges {
        if let Some(&i) = index.get(edge.from.as_str()) {
            has_out[i] = true;
        }
        if let Some(&i) = index.get(edge.to.as_str()) {
            has_in[i] = true;
        }
    }

    if !kinds.is_empty() && !kinds.contains(&StateKind::Initial) {
        out.push(Diag::warn(
            diagram
                .archetype
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_default(),
            "no `kind: initial` state, so the machine has no declared entry".to_string(),
        ));
    }

    for (i, node) in diagram.nodes.iter().enumerate() {
        match kinds[i] {
            StateKind::Initial if has_in[i] => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` is the entry point; nothing transitions into it",
                    node.id.as_str()
                ),
            )),
            StateKind::Final if has_out[i] => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` is final; nothing transitions out of it",
                    node.id.as_str()
                ),
            )),
            StateKind::Initial | StateKind::Final if node.label.is_some() => out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` is drawn as a mark, so its label is not shown",
                    node.id.as_str()
                ),
            )),
            _ => {}
        }

        // A state nothing reaches is either a mistake or an orphan; either
        // way the reader cannot tell how the machine gets there.
        let contained = diagram
            .groups
            .iter()
            .any(|g| g.members.iter().any(|m| m.as_str() == node.id.as_str()));
        if !has_in[i] && kinds[i] != StateKind::Initial && !contained {
            out.push(Diag::warn(
                node.id.span,
                format!("nothing transitions into `{}`", node.id.as_str()),
            ));
        }
    }
}

fn check_sequence(diagram: &Diagram, out: &mut Vec<Diag>) {
    for node in &diagram.nodes {
        if node.placement.is_some() {
            out.push(Diag::warn(
                node.span,
                "`type sequence` ranks its participants in declaration order; \
                 the placement is ignored"
                    .to_string(),
            ));
        }
    }

    if diagram.edges.is_empty() {
        out.push(Diag::warn(
            diagram
                .archetype
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_default(),
            "no messages, so the diagram is a row of participants".to_string(),
        ));
    }

    // A frame is drawn around rows; one holding participants instead of
    // messages would have nothing to span.
    for group in &diagram.groups {
        if group.edge_members.is_empty() {
            out.push(Diag::warn(
                group.span,
                format!(
                    "group `{}` holds no messages — in a sequence a frame spans \
                     rows, not participants",
                    group.id.as_str()
                ),
            ));
        }
    }
}

fn check_wave(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "wave", out);

    let styles = resolved_nodes(diagram, base_style("wave"));
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = styles.get(i);
        let Some(spec) = style.and_then(|s| s.wave.as_deref()) else {
            if awaits_binding(node, archetype_of(diagram), "wave") {
                continue;
            }
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no wave — `signal {} [\"label\"] \"<characters>\"`",
                    node.id.as_str(),
                    node.id.as_str()
                ),
            ));
            continue;
        };

        for (column, c) in spec.chars().enumerate() {
            if !props::WAVE_ALPHABET.contains(c) {
                out.push(Diag::error(
                    node.id.span,
                    format!(
                        "`{c}` at position {column} is not a wave character — the \
                         alphabet is {}",
                        props::WAVE_ALPHABET
                    ),
                ));
            }
        }

        if spec.starts_with('.') {
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` starts with `.`, which repeats the previous character — \
                     there is none",
                    node.id.as_str()
                ),
            ));
        }

        // Every data segment wants a name; an unnamed bus says nothing.
        let segments = data_segments(spec);
        let named = style.and_then(|s| s.data.as_ref()).map_or(0, Vec::len);
        if segments > named {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has {segments} data segment{} but {named} name{} — the rest \
                     will be blank",
                    node.id.as_str(),
                    if segments == 1 { "" } else { "s" },
                    if named == 1 { "" } else { "s" }
                ),
            ));
        }
    }
}

/// How many distinct data runs a wave contains. Only an explicit data
/// character opens one — `.` continues whatever it follows.
fn data_segments(spec: &str) -> usize {
    spec.chars()
        .filter(|c| *c == '=' || matches!(c, '2'..='9'))
        .count()
}

fn check_quadrant(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "quadrant", out);

    let cfg = &diagram.archetype_props;
    let bound = |k: &str, i: usize, fallback: f64| {
        cfg.iter()
            .find(|p| p.name == k)
            .and_then(|p| p.values.get(i))
            .and_then(|v| v.as_num())
            .unwrap_or(fallback)
    };
    let (lo, hi) = (bound("range", 0, 0.0), bound("range", 1, 1.0));

    let styles = resolved_nodes(diagram, base_style("quadrant"));
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = styles.get(i);
        match (style.and_then(|s| s.x), style.and_then(|s| s.y)) {
            (Some(x), Some(y)) => {
                for (axis, v) in [("x", x), ("y", y)] {
                    if v < lo.min(hi) || v > lo.max(hi) {
                        out.push(Diag::warn(
                            node.id.span,
                            format!(
                                "`{}` has {axis} = {v}, outside the range {lo}–{hi}; \
                                 it will be pinned to the edge",
                                node.id.as_str()
                            ),
                        ));
                    }
                }
            }
            _ if awaits_binding(node, archetype_of(diagram), "x")
                || awaits_binding(node, archetype_of(diagram), "y") => {}
            _ => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` needs two coordinates — `point {} [\"label\"] <x> <y>`",
                    node.id.as_str(),
                    node.id.as_str()
                ),
            )),
        }
    }

    for key in ["x-axis", "y-axis"] {
        if let Some(prop) = cfg.iter().find(|p| p.name == key)
            && prop.values.len() != 2
        {
            out.push(Diag::error(
                prop.values_span,
                format!("`{key}` names both ends, so it takes exactly two labels"),
            ));
        }
    }
    if let Some(prop) = cfg.iter().find(|p| p.name == "quadrants")
        && prop.values.len() != 4
    {
        out.push(Diag::error(
            prop.values_span,
            "`quadrants` takes four names, anticlockwise from the top right".to_string(),
        ));
    }
}

fn check_treemap(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "treemap", out);

    let styles = resolved_nodes(diagram, base_style("treemap"));
    let owned: HashSet<&str> = diagram
        .groups
        .iter()
        .flat_map(|g| g.members.iter().map(|m| m.as_str()))
        .collect();

    for (i, node) in diagram.nodes.iter().enumerate() {
        match styles.get(i).and_then(|s| s.value) {
            None if !awaits_binding(node, archetype_of(diagram), "value") => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no value — every tile of a treemap needs an area",
                    node.id.as_str()
                ),
            )),
            None => {}
            Some(v) if v < 0.0 => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has a negative value; area cannot be negative",
                    node.id.as_str()
                ),
            )),
            Some(0.0) => out.push(Diag::warn(
                node.id.span,
                format!("`{}` has value 0 and will not be drawn", node.id.as_str()),
            )),
            Some(_) => {}
        }
    }

    // A group's area is the sum of what it holds; declaring one would be a
    // second, contradictory truth.
    for group in &diagram.groups {
        if let Some(prop) = group.props.iter().find(|p| p.name == "value") {
            out.push(Diag::error(
                prop.name_span,
                format!(
                    "group `{}` cannot declare a value — its area is the sum of \
                     what it contains",
                    group.id.as_str()
                ),
            ));
        }
    }

    let roots = diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .chain(diagram.groups.iter().map(|g| g.id.as_str()))
        .filter(|name| !owned.contains(name))
        .count();

    let span = diagram
        .archetype
        .as_ref()
        .map(|a| a.span)
        .unwrap_or_default();
    if roots > crate::props::PALETTE_LIGHT.len() {
        out.push(Diag::warn(
            span,
            format!(
                "{roots} top-level tiles exhaust the categorical palette ({} hues) \
                 — group the tail rather than repeating a colour",
                crate::props::PALETTE_LIGHT.len()
            ),
        ));
    }
}

/// `value` and `bits` are data for an archetype that is not in play.
fn check_unused_data(diagram: &Diagram, out: &mut Vec<Diag>) {
    for node in &diagram.nodes {
        if let Some(member) = node.members.first() {
            out.push(Diag::warn(
                member.span,
                "members are only drawn by `type class`".to_string(),
            ));
        }
        for prop in &node.props {
            let owner = match prop.name.as_str() {
                "value" => "pie` or `type treemap",
                "bits" => "packet",
                "x" | "y" => "quadrant",
                "wave" | "data" => "wave",
                "points" => "bar",
                "kind" | "guard" => "state",
                "start" | "length" => "gantt",
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

    // A treemap's tiles are painted from the palette, not from any style the
    // author wrote, and the renderer already guarantees white reads on all of
    // them. There is nothing here to compare.
    if archetype == "treemap" {
        return;
    }

    // Mirror the base each archetype actually resolves on, so a colour the
    // renderer never uses is never reported.
    let base = base_style(archetype);

    for (node, style) in diagram.nodes.iter().zip(resolved_nodes(diagram, base)) {
        // Unset means the archetype picks a readable ink itself.
        let Some(ink) = style.font_color.as_deref() else {
            continue;
        };
        // Chart text sits on the canvas; a box's label sits on its own fill.
        let nominal = match archetype {
            "pie" | "quadrant" => surface.as_str(),
            _ => style.fill.as_deref().unwrap_or(surface.as_str()),
        };

        // What the eye receives, not what was written: a translucent fill is
        // mostly its backdrop, and translucent ink is mostly its fill.
        let behind = crate::charts::composite(nominal, &surface);
        let ink_seen = crate::charts::composite(ink, &behind);

        let Some(ratio) = crate::charts::contrast(&ink_seen, &behind) else {
            continue;
        };
        if ratio < 3.0 {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` writes {ink_seen} on {behind} — contrast {ratio:.1}:1, \
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

        for value in &prop.values {
            if let Value::Color(c) = value {
                let digits = c.len() - 1;
                if !matches!(digits, 3 | 4 | 6 | 8) {
                    out.push(Diag::error(
                        prop.values_span,
                        format!(
                            "`{c}` has {digits} hex digits; a color needs 3, 4, 6 or 8 \
                             (#rgb, #rgba, #rrggbb, #rrggbbaa)"
                        ),
                    ));
                }
            }
        }

        if prop.values.is_empty() {
            out.push(Diag::error(
                prop.name_span,
                format!("`{}` needs a value — {}", prop.name, def.syntax),
            ));
            continue;
        }

        // A hole is bound at render time; validating what is not there yet
        // would report every template as broken.
        if prop.values.iter().any(|v| v.is_hole()) {
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
