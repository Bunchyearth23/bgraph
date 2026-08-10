#![cfg_attr(not(feature = "all-archetypes"), allow(dead_code))]
//! Semantic validation, with spans.
//!
//! This is the single source of truth: `layout::solve` runs it and reports the
//! first error, the language server runs it and reports every one of them.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::props::{self, Ctx};

/// Whether a diagnostic stops the render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// The diagram cannot be drawn. [`crate::render_source`] reports the first
    /// of these and renders nothing.
    Error,
    /// The diagram draws, but something about it will not read as intended —
    /// a label too dark against its fill, a series with no legend to name it.
    Warning,
}

/// One problem found in a diagram, and where it is.
#[derive(Debug, Clone)]
pub struct Diag {
    /// Whether this stops the render.
    pub severity: Severity,
    /// Written for the person who has to fix it: what is wrong, and with which
    /// item by name.
    pub message: String,
    /// Byte range in the source, for an editor to underline.
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

/// Every problem in a diagram, not just the first.
///
/// [`crate::render_source`] stops at the first [`Severity::Error`]; an editor
/// wants them all at once, and so does anything reporting on a file it did not
/// write. An empty result means the diagram draws.
pub fn check(diagram: &Diagram) -> Vec<Diag> {
    let mut out = Vec::new();

    let def = check_archetype(diagram, &mut out);
    let archetype = def.name;
    check_styles(diagram, def, &mut out);
    let nodes = check_nodes(diagram, def, &mut out);
    check_groups(diagram, def, &nodes, &mut out);
    check_edges(diagram, def, &nodes, &mut out);
    check_graph(diagram, &mut out);

    check_contrast(diagram, archetype, &mut out);

    // Anchors only mean something where placement means something.
    match archetype {
        #[cfg(feature = "pie")]
        "pie" => check_pie(diagram, &mut out),
        #[cfg(feature = "packet")]
        "packet" => check_packet(diagram, "packet", &mut out),
        #[cfg(feature = "bytefield")]
        "bytefield" => check_packet(diagram, "bytefield", &mut out),
        #[cfg(feature = "treemap")]
        "treemap" => check_treemap(diagram, &mut out),
        #[cfg(feature = "quadrant")]
        "quadrant" => check_quadrant(diagram, &mut out),
        #[cfg(feature = "wave")]
        "wave" => check_wave(diagram, &mut out),
        #[cfg(feature = "sequence")]
        "sequence" => check_sequence(diagram, &mut out),
        #[cfg(feature = "class")]
        "class" => check_class(diagram, "class", &mut out),
        #[cfg(feature = "er")]
        "er" => check_er(diagram, &mut out),
        #[cfg(feature = "requirements")]
        "requirements" => check_requirements(diagram, &mut out),
        #[cfg(feature = "kanban")]
        "kanban" => check_kanban(diagram, &mut out),
        #[cfg(feature = "journey")]
        "journey" => check_journey(diagram, &mut out),
        #[cfg(feature = "radar")]
        "radar" => check_radar(diagram, &mut out),
        #[cfg(feature = "rack")]
        "rack" => check_rack(diagram, &mut out),
        #[cfg(feature = "block")]
        "block" => check_block(diagram, &mut out),
        #[cfg(feature = "gantt")]
        "gantt" => check_gantt(diagram, &mut out),
        #[cfg(feature = "timeline")]
        "timeline" => check_timeline(diagram, &mut out),
        #[cfg(feature = "state")]
        "state" => {
            check_cycles(diagram, &nodes, &mut out);
            check_state(diagram, &mut out);
        }
        #[cfg(feature = "sankey")]
        "sankey" => check_sankey(diagram, &mut out),
        #[cfg(any(feature = "bar", feature = "xy"))]
        "bar" | "xy" => {
            check_bar(diagram, &mut out);
            #[cfg(feature = "xy")]
            if archetype == "xy" {
                check_scatter(diagram, &mut out);
            }
        }
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

/// The ids that name a **frame**. An `activate` block's identifier names a
/// participant instead, so it must not read as a group anywhere — an edge
/// touching it is an ordinary message, not an edge into a frame (`D-024`).
fn frame_names(diagram: &Diagram) -> HashSet<&str> {
    diagram
        .groups
        .iter()
        .filter(|g| !g.activates)
        .map(|g| g.id.as_str())
        .collect()
}

fn style_names(diagram: &Diagram) -> HashSet<&str> {
    diagram.styles.iter().map(|s| s.name.as_str()).collect()
}

fn check_styles(diagram: &Diagram, def: &'static props::ArchetypeDef, out: &mut Vec<Diag>) {
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
        check_props_refined(&style.props, ctx, def.item_props, props::PROPS, "property", out);
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
        check_props_refined(
            &node.props,
            Some(Ctx::Node),
            def.item_props,
            props::PROPS,
            "property",
            out,
        );
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
        .chain(diagram.groups.iter().filter(|g| !g.activates).map(|g| g.id.as_str()))
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

fn check_groups(diagram: &Diagram, def: &'static props::ArchetypeDef, nodes: &HashMap<&str, usize>, out: &mut Vec<Diag>) {
    let known_styles = style_names(diagram);
    let mut seen: HashSet<&str> = HashSet::new();

    for group in &diagram.groups {
        // `activate api` names a participant that already exists, so the
        // duplicate-id rule is exactly what it must *not* trip (`D-024`).
        if group.activates {
            if !nodes.contains_key(group.id.as_str()) {
                out.push(Diag::error(
                    group.id.span,
                    format!(
                        "`activate {}` names no participant — a bar belongs to one \
                         that is already on the diagram",
                        group.id.as_str()
                    ),
                ));
            }
            if group.edge_members.is_empty() {
                out.push(Diag::warn(
                    group.span,
                    format!(
                        "`activate {}` spans no messages, so the bar has no length",
                        group.id.as_str()
                    ),
                ));
            }
            continue;
        }
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

        check_props_refined(
            &group.props,
            Some(Ctx::Node),
            def.item_props,
            props::PROPS,
            "property",
            out,
        );
    }

    // An edge to a frame has no obvious endpoint; say so rather than guess.
    let groups = frame_names(diagram);
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
    if def.accepts(written) {
        return;
    }

    let elsewhere = props::ARCHETYPES
        .iter()
        .find(|a| a.items().any(|i| i == written));
    let hint = match elsewhere {
        Some(other) => format!(" — `{written}` belongs to `type {}`", other.name),
        None => String::new(),
    };

    // An archetype with a second form (`D-023`) must name both here, or the
    // message sends the author to the wrong keyword half the time.
    let words: Vec<String> = def.items().map(plural).collect();
    let declares = match words.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, head)) => format!("{} or {last}", head.join(", ")),
        None => unreachable!("every archetype declares at least one word"),
    };

    out.push(Diag::error(
        node.keyword.span,
        format!(
            "`type {}` declares {declares}, not {}{hint}",
            def.name,
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

    // The shape follows the word the author wrote, not the archetype's
    // primary form (`D-023`).
    let written = node.keyword.as_str();
    let slots = def.slots_for(written);
    // `node` is the universal synonym, so it reports under the primary word.
    let word = if written == "node" { def.item } else { written };

    let wanted: usize = slots.iter().map(|s| s.arity).sum();
    if wanted == 0 {
        out.push(Diag::error(
            node.positional_span,
            format!("`type {}` takes no data after the label", def.name),
        ));
        return;
    }

    if node.positional.len() > wanted {
        let shape: Vec<String> = slots.iter().map(|s| format!("<{}>", s.prop)).collect();
        out.push(Diag::error(
            node.positional_span,
            format!(
                "`{word}` takes {wanted} value{} after the label — `{word} <id> [\"label\"] {}`",
                if wanted == 1 { "" } else { "s" },
                shape.join(" ")
            ),
        ));
        return;
    }

    // Written twice, once in each form: say which one wins.
    let mut rest = node.positional.len();
    for slot in slots {
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

fn check_edges(diagram: &Diagram, def: &'static props::ArchetypeDef, nodes: &HashMap<&str, usize>, out: &mut Vec<Diag>) {
    let known_styles = style_names(diagram);

    let groups = frame_names(diagram);
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

        // The **folded** form, not `edge.props`: positional edge data is sugar
        // for the named property, so validating only the inline block let
        // `a -> b nonsense` through `check` entirely. It reached `style.rs`,
        // which bails without a span for the cardinalities and not at all for
        // a word nothing parses — so a typo in the one place this genre puts
        // its meaning was silent, against `D-005`.
        check_props_refined(
            &edge.effective_props(def.edge_slots),
            Some(Ctx::Edge),
            def.item_props,
            props::PROPS,
            "property",
            out,
        );
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
    node.props_for(def)
        .iter()
        .any(|p| p.name == name && p.values.iter().any(|v| v.is_hole()))
}

/// The base each archetype resolves its items on. `free` and `packet` draw
/// boxes; the chart archetypes draw marks and take no box defaults.
pub fn base_style(archetype: &str) -> crate::style::Style {
    match archetype {
        "free" | "packet" | "bytefield" | "rack" => crate::style::Style::node_defaults(),
        _ => crate::charts::chart_defaults(),
    }
}

/// The same for groups. A column's limit rides the cascade like any other
/// datum, so reading it off the raw props would miss one supplied by a class —
/// the sort of gap that stays quiet because the common case works.
fn resolved_groups(diagram: &Diagram, base: crate::style::Style) -> Vec<crate::style::Style> {
    let Ok(registry) = crate::style::Registry::build(&diagram.styles) else {
        return Vec::new();
    };
    diagram
        .groups
        .iter()
        .map(|group| {
            registry
                .resolve(base.clone(), "group", &group.classes, &group.props)
                .unwrap_or_default()
        })
        .collect()
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
                    &node.props_for(def),
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

/// `packet` and `bytefield` are validated identically — the same gaps and the
/// same overlaps matter — so only the unit's name changes (`W-039`). Written
/// once, because a second copy is a second place for the two to drift apart.
fn check_packet(diagram: &Diagram, archetype: &'static str, out: &mut Vec<Diag>) {
    warn_ignored(diagram, archetype, out);

    let (slot, unit) = if archetype == "bytefield" {
        ("bytes", "byte")
    } else {
        ("bits", "bit")
    };

    let styles = resolved_nodes(diagram, crate::style::Style::node_defaults());
    let mut fields: Vec<(u32, u32, &str, Span)> = Vec::new();

    for (i, node) in diagram.nodes.iter().enumerate() {
        match styles.get(i).and_then(|s| s.bits.or(s.bytes)) {
            None if !awaits_binding(node, archetype_of(diagram), slot) => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no `{slot}` — every field of a {archetype} needs a range",
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
                    "`{b_id}` covers {slot} {b_start}–{b_end}, which overlap \
                     `{a_id}` at {a_start}–{a_end}"
                ),
            ));
        } else if b_start > a_end + 1 {
            let (from, to) = (a_end + 1, b_start - 1);
            let range = if from == to {
                format!("{unit} {from} is")
            } else {
                format!("{slot} {from}–{to} are")
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
/// `series` and `scatter` are two readings of the same axes (`D-023`), and one
/// plot cannot be both: an indexed series takes its x from its position in the
/// line, a scatter from the numbers written. Letting them share a plot would
/// put two meanings of x on one axis, and no reader could tell which applied
/// to which mark.
#[cfg(feature = "xy")]
fn check_scatter(diagram: &Diagram, out: &mut Vec<Diag>) {
    let def = archetype_of(diagram);
    let styles = resolved_nodes(diagram, base_style("xy"));

    let mut first: Option<&'static str> = None;
    for node in &diagram.nodes {
        // `node` is the universal synonym and carries the primary form, so it
        // reads as `series` here rather than as a third thing.
        let form = if node.keyword.as_str() == "scatter" {
            "scatter"
        } else {
            def.item
        };
        match first {
            None => first = Some(form),
            Some(seen) if seen != form => out.push(Diag::error(
                node.keyword.span,
                format!(
                    "`{seen}` and `{form}` cannot share a plot — one takes its x from \
                     its position in the line, the other from the numbers written"
                ),
            )),
            Some(_) => {}
        }
    }

    for (i, node) in diagram.nodes.iter().enumerate() {
        let Some(pairs) = styles.get(i).and_then(|s| s.pairs.as_ref()) else {
            continue;
        };
        if pairs.len() % 2 == 1 {
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has {} values — `scatter` reads them two at a time, so the \
                     last x has no y",
                    node.id.as_str(),
                    pairs.len()
                ),
            ));
        }
    }
}

fn check_bar(diagram: &Diagram, out: &mut Vec<Diag>) {
    let archetype = archetype_of(diagram).name;
    warn_ignored(diagram, archetype, out);

    let cfg = &diagram.archetype_props;
    let categories = cfg
        .iter()
        .find(|p| p.name == "categories")
        .map_or(0, |p| p.values.len());

    let styles = resolved_nodes(diagram, base_style(archetype));
    // A `scatter` carries its own x (`D-023`), so it is neither missing its
    // numbers nor measurable against a category list.
    let scattered = styles.iter().any(|s| s.pairs.is_some());

    for (i, node) in diagram.nodes.iter().enumerate() {
        if styles.get(i).and_then(|s| s.pairs.as_ref()).is_some() {
            continue;
        }
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

    if categories == 0 && !scattered {
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
        .filter(|(i, _)| {
            styles
                .get(*i)
                .is_some_and(|s| s.points.is_some() || s.pairs.is_some())
        })
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

fn check_timeline(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "timeline", out);

    let styles = resolved_nodes(diagram, base_style("timeline"));
    for (i, node) in diagram.nodes.iter().enumerate() {
        if awaits_binding(node, archetype_of(diagram), "date") {
            continue;
        }
        if styles.get(i).and_then(|s| s.start).is_none() {
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no date — `event {} [\"label\"] <date>`",
                    node.id.as_str(),
                    node.id.as_str()
                ),
            ));
        }
    }
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

fn check_class(diagram: &Diagram, word: &str, out: &mut Vec<Diag>) {
    check_cycles_free(diagram, out);
    for node in &diagram.nodes {
        if node.members.is_empty() {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has no members; a {word} with none is a plain box",
                    node.id.as_str()
                ),
            ));
        }
    }
}

fn check_block(diagram: &Diagram, out: &mut Vec<Diag>) {
    check_cycles_free(diagram, out);
    for node in &diagram.nodes {
        if let Some(member) = node.members.first() {
            out.push(Diag::warn(
                member.span,
                "a block carries a `description`, not members".to_string(),
            ));
        }
    }
}

/// A rack is validated like a packet with one difference that matters: an
/// empty unit is a rack with **room**, not a rack that does not add up, so
/// gaps are silent here where they are a warning there.
#[cfg(feature = "rack")]
fn check_rack(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "rack", out);

    let cfg = &diagram.archetype_props;
    let units = cfg
        .iter()
        .find(|p| p.name == "units")
        .and_then(|p| p.values.first())
        .and_then(|v| v.as_num())
        .unwrap_or(42.0)
        .max(1.0) as u32;

    let styles = resolved_nodes(diagram, crate::style::Style::node_defaults());
    let mut placed: Vec<(u32, u32, &str, Span)> = Vec::new();

    for (i, node) in diagram.nodes.iter().enumerate() {
        match styles.get(i).and_then(|s| s.u) {
            None if !awaits_binding(node, archetype_of(diagram), "u") => out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` has no `u` — every device sits in a span of rack units",
                    node.id.as_str()
                ),
            )),
            None => {}
            Some((low, high)) => {
                if low < 1 {
                    out.push(Diag::error(
                        node.id.span,
                        format!("`{}` starts below U1; a rack counts from 1", node.id.as_str()),
                    ));
                } else if high > units {
                    out.push(Diag::error(
                        node.id.span,
                        format!(
                            "`{}` reaches U{high}, past the top of a {units}U frame",
                            node.id.as_str()
                        ),
                    ));
                }
                placed.push((low, high, node.id.as_str(), node.id.span));
            }
        }
    }

    placed.sort_by_key(|d| d.0);
    for pair in placed.windows(2) {
        let (a_low, a_high, a_id, _) = pair[0];
        let (b_low, b_high, b_id, b_span) = pair[1];
        if b_low <= a_high {
            out.push(Diag::error(
                b_span,
                format!(
                    "`{b_id}` occupies U{b_low}–U{b_high}, which overlap `{a_id}` at \
                     U{a_low}–U{a_high} — two devices cannot share a unit"
                ),
            ));
        }
    }
}

/// A radar shares one scale across every axis, so the failures worth naming
/// are the ones that make the *shape* lie: too few axes for a shape to exist,
/// and a series that does not reach all the way round.
#[cfg(feature = "radar")]
fn check_radar(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "radar", out);

    let cfg = &diagram.archetype_props;
    let axes = cfg
        .iter()
        .find(|p| p.name == "categories")
        .map_or(0, |p| p.values.len());
    let where_ = diagram
        .archetype
        .as_ref()
        .map(|a| a.span)
        .unwrap_or_default();

    if axes == 0 {
        out.push(Diag::error(
            where_,
            "a radar needs `categories` — one axis per name, and nothing to \
             read without them"
                .to_string(),
        ));
    } else if axes < 3 {
        out.push(Diag::warn(
            where_,
            format!("{axes} axes enclose no area; a radar reads as a shape from three"),
        ));
    }

    if let Some(prop) = cfg.iter().find(|p| p.name == "range")
        && prop.values.len() != 2
    {
        out.push(Diag::error(
            prop.values_span,
            "`range` takes a low and a high".to_string(),
        ));
    }

    let styles = resolved_nodes(diagram, base_style("radar"));
    let mut drawn = 0;
    for (i, node) in diagram.nodes.iter().enumerate() {
        let points = styles.get(i).and_then(|s| s.points.as_ref());
        let Some(points) = points.filter(|p| !p.is_empty()) else {
            if !awaits_binding(node, archetype_of(diagram), "points") {
                out.push(Diag::error(
                    node.id.span,
                    format!(
                        "`{}` has no numbers — `series {} [\"label\"] <n> <n> …`, one per axis",
                        node.id.as_str(),
                        node.id.as_str()
                    ),
                ));
            }
            continue;
        };
        drawn += 1;

        if axes > 0 && points.len() != axes {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has {} number{} for {axes} axes — its polygon will not close \
                     on the same shape as the others",
                    node.id.as_str(),
                    points.len(),
                    if points.len() == 1 { "" } else { "s" }
                ),
            ));
        }
    }

    if drawn > crate::props::PALETTE_LIGHT.len() {
        out.push(Diag::warn(
            where_,
            format!(
                "{drawn} series exhaust the categorical palette ({} hues) — and \
                 overlapping polygons stop being separable well before that",
                crate::props::PALETTE_LIGHT.len()
            ),
        ));
    }
}

/// A journey's scores are read positionally against `actors`, so the two can
/// silently disagree — the same failure `bar` has against `categories`, and
/// the same answer: say which step, and how many were expected.
#[cfg(feature = "journey")]
fn check_journey(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "journey", out);

    let cfg = &diagram.archetype_props;
    let actors = cfg
        .iter()
        .find(|p| p.name == "actors")
        .map_or(0, |p| p.values.len());

    if actors == 0 {
        out.push(Diag::warn(
            diagram
                .archetype
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_default(),
            "no `actors`, so the scores stand for nobody and nothing is drawn".to_string(),
        ));
    }

    let (lo, hi) = match cfg.iter().find(|p| p.name == "range") {
        Some(p) if p.values.len() == 2 => (
            p.values[0].as_num().unwrap_or(1.0),
            p.values[1].as_num().unwrap_or(5.0),
        ),
        Some(p) => {
            out.push(Diag::error(
                p.values_span,
                "`range` takes a low and a high score".to_string(),
            ));
            (1.0, 5.0)
        }
        None => (1.0, 5.0),
    };

    let styles = resolved_nodes(diagram, base_style("journey"));
    for (i, node) in diagram.nodes.iter().enumerate() {
        let scores = styles.get(i).and_then(|s| s.scores.as_ref());
        let Some(scores) = scores.filter(|s| !s.is_empty()) else {
            if !awaits_binding(node, archetype_of(diagram), "scores") {
                out.push(Diag::error(
                    node.id.span,
                    format!(
                        "`{}` has no scores — `step {} [\"label\"] <n> <n> …`, one per actor",
                        node.id.as_str(),
                        node.id.as_str()
                    ),
                ));
            }
            continue;
        };

        if actors > 0 && scores.len() != actors {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has {} score{} for {actors} actor{} — a missing one leaves \
                     that actor's line with a gap here",
                    node.id.as_str(),
                    scores.len(),
                    if scores.len() == 1 { "" } else { "s" },
                    if actors == 1 { "" } else { "s" }
                ),
            ));
        }

        for score in scores {
            if *score < lo.min(hi) || *score > hi.max(lo) {
                out.push(Diag::warn(
                    node.id.span,
                    format!(
                        "`{}` scores {} on a {}–{} scale; it is drawn clamped to the axis",
                        node.id.as_str(),
                        crate::charts::trim_number(*score),
                        crate::charts::trim_number(lo),
                        crate::charts::trim_number(hi)
                    ),
                ));
            }
        }
    }
}

/// A board is only worth keeping if it says when a column is over its limit —
/// that is the one thing kanban is *for*, and the reason this genre exists
/// rather than being a `free` diagram with boxes in rows.
#[cfg(feature = "kanban")]
fn check_kanban(diagram: &Diagram, out: &mut Vec<Diag>) {
    warn_ignored(diagram, "kanban", out);

    if diagram.groups.is_empty() {
        out.push(Diag::warn(
            diagram
                .archetype
                .as_ref()
                .map(|a| a.span)
                .unwrap_or_default(),
            "no columns — a board's columns are its `group`s".to_string(),
        ));
    }

    let in_a_column: HashSet<&str> = diagram
        .groups
        .iter()
        .flat_map(|g| g.members.iter().map(|m| m.as_str()))
        .collect();

    for node in &diagram.nodes {
        if !in_a_column.contains(node.id.as_str()) {
            out.push(Diag::error(
                node.id.span,
                format!(
                    "`{}` is in no column — a card belongs inside a `group`",
                    node.id.as_str()
                ),
            ));
        }
        // `limit` has to be `Ctx::Node` to be accepted on a group at all, so
        // it is also accepted on a card, where it means nothing. Said out
        // loud rather than ignored, per `D-005`.
        if let Some(prop) = node.props.iter().find(|p| p.name == "limit") {
            out.push(Diag::error(
                prop.name_span,
                format!(
                    "`limit` belongs to a column, not to `{}` — write it on the `group`",
                    node.id.as_str()
                ),
            ));
        }
    }

    let styles = resolved_groups(diagram, crate::style::Style::group_defaults());
    for (i, group) in diagram.groups.iter().enumerate() {
        let Some(n) = styles.get(i).and_then(|s| s.limit) else {
            continue;
        };
        // The span is the declaration's if it was written here, and the
        // group's name if it arrived through a class.
        let span = group
            .props
            .iter()
            .find(|p| p.name == "limit")
            .map_or(group.id.span, |p| p.values_span);
        if n < 1.0 {
            out.push(Diag::error(
                span,
                "a work-in-progress limit is at least one card".to_string(),
            ));
            continue;
        }
        let held = group.members.len();
        if held as f64 > n {
            out.push(Diag::warn(
                group.id.span,
                format!(
                    "`{}` holds {held} cards, over its limit of {} — that is what the \
                     limit is for",
                    group.id.as_str(),
                    crate::charts::trim_number(n)
                ),
            ));
        }
    }
}

/// A requirements view is a `class` box with a relation vocabulary. What it
/// adds on top is that both halves of the picture can be *silently empty*:
/// a requirement with no text says nothing, and a relation with no kind is a
/// line whose meaning the reader has to guess.
#[cfg(feature = "requirements")]
fn check_requirements(diagram: &Diagram, out: &mut Vec<Diag>) {
    check_class(diagram, "requirement", out);

    let def = archetype_of(diagram);
    for edge in &diagram.edges {
        let props = edge.effective_props(def.edge_slots);
        if !props.iter().any(|p| p.name == "link") {
            out.push(Diag::warn(
                edge.positional_span,
                format!(
                    "`{} -> {}` says nothing about what it asserts — write one \
                     of {}",
                    edge.from.as_str(),
                    edge.to.as_str(),
                    props::REQ_LINKS.join(", ")
                ),
            ));
        }
    }

    // A requirement nobody has planned to check is the failure this genre
    // exists to make visible, so it is said out loud rather than left to the
    // reader to notice by its absence.
    let styles = resolved_nodes(diagram, base_style("requirements"));
    for (i, node) in diagram.nodes.iter().enumerate() {
        if styles.get(i).is_some_and(|s| s.verify.is_none()) {
            out.push(Diag::warn(
                node.id.span,
                format!(
                    "`{}` has no `verify` — nothing says how it will be shown to be met",
                    node.id.as_str()
                ),
            ));
        }
    }
}

fn check_er(diagram: &Diagram, out: &mut Vec<Diag>) {
    check_class(diagram, "entity", out);

    // A relation with cardinality at one end only is almost always a slip —
    // the second word was forgotten — and it reads as a deliberate blank.
    for edge in &diagram.edges {
        let props = edge.effective_props(archetype_of(diagram).edge_slots);
        let has = |name: &str| props.iter().any(|p| p.name == name);
        if has("from-card") != has("to-card") {
            out.push(Diag::warn(
                edge.positional_span,
                format!(
                    "`{} -> {}` gives cardinality at one end only; \
                     write both, as in `one many`",
                    edge.from.as_str(),
                    edge.to.as_str()
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
        .chain(diagram.groups.iter().filter(|g| !g.activates).map(|g| g.id.as_str()))
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

/// Members are the one thing a `free` graph can carry and not draw. Data
/// properties no longer need a list here: each lives with the archetype that
/// reads it, so writing one elsewhere is an unknown property, and
/// `check_props_refined` says which genre it belongs to.
fn check_unused_data(diagram: &Diagram, out: &mut Vec<Diag>) {
    for node in &diagram.nodes {
        if let Some(member) = node.members.first() {
            out.push(Diag::warn(
                member.span,
                "members are only drawn by `type class` and `type er`".to_string(),
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
    check_props_refined(entries, ctx, &[], table, what, out)
}

/// The same, with an archetype's refinements consulted first — so `kind`
/// carries `state`'s words in a machine and `block`'s in an architecture view.
fn check_props_refined(
    entries: &[Prop],
    ctx: Option<Ctx>,
    refinements: &'static [props::PropDef],
    table: &'static [props::PropDef],
    what: &str,
    out: &mut Vec<Diag>,
) {
    for prop in entries {
        let Some(def) = props::lookup(&prop.name, refinements)
            .or_else(|| props::lookup(&prop.name, table))
        else {
            // A property that exists, but in another genre, is a different
            // mistake from a typo — and the reader can only fix it if told
            // which genre reads it.
            let hint = match props::genres_reading(&prop.name).as_slice() {
                [] => suggest(&prop.name, refinements)
                    .or_else(|| suggest(&prop.name, table))
                    .map(|s| format!(" — did you mean `{s}`?"))
                    .unwrap_or_default(),
                owners => format!(
                    " — it belongs to {}",
                    owners
                        .iter()
                        .map(|o| format!("`type {o}`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
            };
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
            // Strict by default, and the exceptions are named: a compound
            // paint property mixes colors, widths and patterns, so a bare word
            // there may legitimately be a named color. Listing the *strict*
            // side instead is how `kind`, `ticks` and `mark` went unvalidated
            // — a new enumeration must be caught by construction, not by
            // someone remembering to add it.
            let lenient = matches!(prop.name.as_str(), "stroke" | "border" | "color");
            if !lenient && !def.words.contains(&word) {
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
