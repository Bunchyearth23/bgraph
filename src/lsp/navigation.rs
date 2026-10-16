use serde_json::{Value, json};

use crate::ast;
use crate::{ast::Span, props};

use super::{LineIndex, declaration_span, declared_archetype, is_ident_char, item_words, word_at};

/// Resolve an identifier to the declaration relevant to the cursor context.
pub(super) fn definition(text: &str, index: &LineIndex, offset: usize, uri: &str) -> Value {
    if let Some(document) = SourceDocument::new(text, uri)
        && let Some(entity) = document.entity_at(offset)
    {
        let target = if document.contains(entity.declaration, offset) {
            entity
                .mentions
                .first()
                .copied()
                .unwrap_or(entity.declaration)
        } else {
            entity.declaration
        };
        return document.location(target);
    }
    let Some((word, span)) = word_at(text, offset) else {
        return Value::Null;
    };
    let diagram = crate::parser::parse(text).ok();
    let style_first = text[..span.start].trim_end().ends_with('.')
        || text[..span.start].trim_end().ends_with("extends");

    // Ask the archetype for all its item words: the universal `node` synonym
    // alone is not enough for a declaration written in the genre's own form.
    let mut order: Vec<&str> = if style_first { vec!["style"] } else { vec![] };
    order.extend(item_words(declared_archetype(text)));
    if !style_first {
        order.push("style");
    }
    for kind in order {
        if let Some(target) = declaration_span(text, kind, &word) {
            // For declaration words, prefer the first related layout mention when
            // available: declaration and layout are now navigable in both
            // directions.
            if target.contains(offset)
                && let Some(related) = diagram
                    .as_ref()
                    .and_then(|diagram| layout_mention(diagram, &word))
            {
                return json!({ "uri": uri, "range": index.range(related) });
            }
            return json!({ "uri": uri, "range": index.range(target) });
        }
    }
    Value::Null
}

fn layout_mention(diagram: &ast::Diagram, name: &str) -> Option<Span> {
    crate::explain::inspect::relations(diagram, name)
        .first()
        .map(|relation| relation.reference)
}

struct EntityReferences {
    id: String,
    declaration: Span,
    mentions: Vec<Span>,
}

/// Source coordinates stay in their owning file until the LSP boundary.
pub(super) struct SourceDocument {
    pub diagram: ast::Diagram,
    pub map: Option<crate::compose::SourceMap>,
    text: String,
    uri: String,
    path: Option<std::path::PathBuf>,
}

impl SourceDocument {
    pub fn new(text: &str, uri: &str) -> Option<Self> {
        let path = super::diagnostic_engine::path_of(uri);
        if let Some(path) = path.as_ref()
            && let Ok(composed) = crate::compose::compose(path, Some(text))
        {
            return Some(Self {
                diagram: composed.diagram,
                map: Some(composed.map),
                text: text.into(),
                uri: uri.into(),
                path: Some(
                    path.canonicalize()
                        .or_else(|_| std::path::absolute(path))
                        .ok()?,
                ),
            });
        }
        Some(Self {
            diagram: crate::parser::parse(text).ok()?,
            map: None,
            text: text.into(),
            uri: uri.into(),
            path,
        })
    }

    pub fn contains(&self, span: Span, offset: usize) -> bool {
        if let Some(map) = &self.map {
            let (owner, _, _) = map.locate(span.start);
            if self.path.as_deref() != Some(owner) {
                return false;
            }
            let base = map.base_of(span.start);
            return Span {
                start: span.start.saturating_sub(base),
                end: span.end.saturating_sub(base),
            }
            .contains(offset);
        }
        span.contains(offset)
    }

    pub fn location(&self, span: Span) -> Value {
        if let Some(map) = &self.map {
            let (path, _, _) = map.locate(span.start);
            let base = map.base_of(span.start);
            let local = Span {
                start: span.start.saturating_sub(base),
                end: span.end.saturating_sub(base),
            };
            let uri: String = path
                .to_string_lossy()
                .bytes()
                .map(|byte| {
                    if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                        (byte as char).to_string()
                    } else {
                        format!("%{byte:02X}")
                    }
                })
                .collect();
            return json!({ "uri": format!("file://{uri}"), "range": LineIndex::new(map.source_of(span.start)).range(local) });
        }
        json!({ "uri": self.uri, "range": LineIndex::new(&self.text).range(span) })
    }

    pub fn source(&self, span: Span) -> &str {
        let source = self
            .map
            .as_ref()
            .map_or(self.text.as_str(), |map| map.text.as_str());
        source.get(span.start..span.end).unwrap_or("")
    }

    fn entities(&self) -> Vec<EntityReferences> {
        self.diagram
            .nodes
            .iter()
            .map(|node| (&node.id, node.id.span))
            .chain(
                self.diagram
                    .groups
                    .iter()
                    .map(|group| (&group.id, group.id.span)),
            )
            .map(|(id, declaration)| {
                let mut mentions: Vec<_> =
                    crate::explain::inspect::relations(&self.diagram, id.as_str())
                        .into_iter()
                        .map(|relation| relation.reference)
                        .collect();
                for edge in &self.diagram.edges {
                    for endpoint in [&edge.from, &edge.to] {
                        if endpoint.as_str() == id.as_str() {
                            mentions.push(endpoint.span);
                        }
                    }
                }
                mentions.dedup();
                EntityReferences {
                    id: id.value.clone(),
                    declaration,
                    mentions,
                }
            })
            .collect()
    }

    fn entity_at(&self, offset: usize) -> Option<EntityReferences> {
        self.entities().into_iter().find(|entity| {
            self.contains(entity.declaration, offset)
                || entity
                    .mentions
                    .iter()
                    .any(|span| self.contains(*span, offset))
        })
    }

    pub fn id_at(&self, offset: usize) -> Option<String> {
        self.entity_at(offset).map(|entity| entity.id)
    }
}

pub(super) fn references(text: &str, offset: usize, uri: &str, include_declaration: bool) -> Value {
    let Some(document) = SourceDocument::new(text, uri) else {
        return json!([]);
    };
    let Some(entity) = document.entity_at(offset) else {
        return json!([]);
    };
    let mut spans = entity.mentions;
    if include_declaration {
        spans.push(entity.declaration);
    }
    spans.sort_by_key(|span| (span.start, span.end));
    spans.dedup();
    json!(
        spans
            .into_iter()
            .map(|span| document.location(span))
            .collect::<Vec<_>>()
    )
}

/// Build a document outline from the syntax tree and template parameters.
pub(super) fn symbols(text: &str) -> Value {
    let Ok(diagram) = crate::parser::parse(text) else {
        return json!([]);
    };
    let index = LineIndex::new(text);
    let mut out = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    for hole in crate::template::holes_in(text) {
        if seen.contains(&hole.name) {
            continue;
        }
        seen.push(hole.name.clone());
        out.push(json!({
            "name": format!("{{{{{}}}}}", hole.name),
            "detail": match &hole.prefix {
                Some(prefix) => format!("one per row of {prefix}"),
                None => "bound once".to_string(),
            },
            "kind": 26,
            "range": index.range(hole.span),
            "selectionRange": index.range(hole.span),
        }));
    }
    for style in &diagram.styles {
        out.push(json!({
            "name": style.name.as_str(),
            "detail": style.extends.as_ref().map(|parent| format!("extends {}", parent.as_str())).unwrap_or_default(),
            "kind": 5,
            "range": index.range(style.span),
            "selectionRange": index.range(style.name.span),
        }));
    }
    for node in &diagram.nodes {
        out.push(json!({
            "name": node.id.as_str(),
            "detail": format!("{} · {} layout relations", node.label.as_deref().unwrap_or(""), crate::explain::inspect::relations(&diagram, node.id.as_str()).len()),
            "kind": 13,
            "range": index.range(node.span),
            "selectionRange": index.range(node.id.span),
        }));
    }
    for group in &diagram.groups {
        out.push(json!({
            "name": group.id.as_str(),
            "detail": group.label.clone().unwrap_or_default(),
            "kind": 3,
            "range": index.range(group.span),
            "selectionRange": index.range(group.id.span),
        }));
    }
    for layout in &diagram.layouts {
        let relations = layout.statements.len();
        let scope = layout
            .scope
            .as_ref()
            .map(|scope| format!("in group {}", scope.as_str()))
            .unwrap_or_else(|| "file scope".to_string());
        out.push(json!({
            "name": "layout",
            "detail": format!("{scope} · {relations} relation{}", if relations == 1 { "" } else { "s" }),
            "kind": 3,
            "range": index.range(layout.span),
            "selectionRange": index.range(layout.span),
            "children": layout.statements.iter().map(|statement| {
                let span = match statement {
                    ast::LayoutStmt::Place { span, .. } | ast::LayoutStmt::Series { span, .. } => *span,
                    ast::LayoutStmt::Constraint { constraint } => constraint.span,
                };
                json!({ "name": text.get(span.start..span.end).unwrap_or("").trim(), "kind": 24,
                    "range": index.range(span), "selectionRange": index.range(span) })
            }).collect::<Vec<_>>(),
        }));
    }
    // LSP clients expect a document outline in source order.  Building the
    // three categories separately is convenient, but leaking that order
    // makes the outline jump around as templates or styles are added.
    out.sort_by_key(|symbol| {
        (
            symbol["range"]["start"]["line"]
                .as_u64()
                .unwrap_or_default(),
            symbol["range"]["start"]["character"]
                .as_u64()
                .unwrap_or_default(),
        )
    });
    json!(out)
}

/// Highlight complete identifier occurrences, never a substring of another id.
pub(super) fn highlights(text: &str, index: &LineIndex, offset: usize) -> Value {
    let Some((word, _)) = word_at(text, offset) else {
        return json!([]);
    };
    if word.is_empty() || props::KEYWORDS.iter().any(|(keyword, _)| *keyword == word) {
        return json!([]);
    }

    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut from = 0;
    while let Some(found) = text[from..].find(&word) {
        let start = from + found;
        let end = start + word.len();
        from = end;
        let before_ok = start == 0 || !is_ident_char(bytes[start - 1] as char);
        let after_ok = end >= bytes.len() || !is_ident_char(bytes[end] as char);
        if before_ok && after_ok {
            out.push(json!({ "range": index.range(Span { start, end }), "kind": 1 }));
        }
    }
    json!(out)
}
