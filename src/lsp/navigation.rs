use serde_json::{Value, json};

use crate::{ast::Span, props};

use super::{LineIndex, declaration_span, declared_archetype, is_ident_char, item_words, word_at};

/// Resolve an identifier to the declaration relevant to the cursor context.
pub(super) fn definition(text: &str, index: &LineIndex, offset: usize, uri: &str) -> Value {
    let Some((word, span)) = word_at(text, offset) else {
        return Value::Null;
    };
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
            return json!({ "uri": uri, "range": index.range(target) });
        }
    }
    Value::Null
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
            "detail": style.extends.as_ref().map(|parent| format!("extends {}", parent.as_str())),
            "kind": 5,
            "range": index.range(style.span),
            "selectionRange": index.range(style.name.span),
        }));
    }
    for node in &diagram.nodes {
        out.push(json!({
            "name": node.id.as_str(),
            "detail": node.label.clone(),
            "kind": 13,
            "range": index.range(node.span),
            "selectionRange": index.range(node.id.span),
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
