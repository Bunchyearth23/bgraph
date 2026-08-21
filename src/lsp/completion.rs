use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::{
    ast::Span,
    lsp::{Context, LineIndex, declared_archetype},
    props,
};

pub(super) fn item(label: &str, kind: u8, detail: &str, doc: &str) -> Value {
    json!({
        "label": label,
        "kind": kind,
        "detail": detail,
        "documentation": { "kind": "markdown", "value": doc },
        "sortText": format!("{:02}{label}", 99 - kind),
    })
}

/// Build the LSP completion envelope from the cursor's current line.
pub(super) fn response(
    text: &str,
    offset: usize,
    items_for: impl FnOnce(&str) -> Vec<Value>,
) -> Value {
    let prefix = &text[..offset];
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    json!({ "isIncomplete": false, "items": items_for(&prefix[line_start..]) })
}

pub(super) fn snippet(label: &str, kind: u8, detail: &str, doc: &str, insert: &str) -> Value {
    let mut item = item(label, kind, detail, doc);
    item["insertText"] = json!(insert);
    item["insertTextFormat"] = json!(2);
    item
}

pub(super) fn item_snippet(word: &str, slots: &[props::Slot]) -> String {
    let mut out = format!("{word} ${{1:id}} \"${{2:label}}\"");
    for (i, slot) in slots.iter().enumerate() {
        if slot.arity == props::VARIADIC {
            let _ = write!(out, " ${{{}:{}…}}", i + 3, slot.prop);
        } else {
            for n in 0..slot.arity {
                let _ = write!(out, " ${{{}:{}}}", i + 3 + n, slot.prop);
            }
        }
    }
    out.push_str("$0");
    out
}

/// The positional form offered for the statement under the cursor.
pub(super) fn signature(text: &str, offset: usize) -> Value {
    let ctx = Context::at(text, offset);
    if ctx.in_string || ctx.in_comment || ctx.block.is_some() {
        return Value::Null;
    }

    let prefix = &text[..offset];
    let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
    let words: Vec<&str> = prefix[line_start..].split_whitespace().collect();
    let Some(&word) = words.first() else {
        return Value::Null;
    };

    let Some(def) = props::archetype(declared_archetype(text)) else {
        return Value::Null;
    };
    if !def.accepts(word) || def.slots_for(word).is_empty() {
        return Value::Null;
    }

    let mut parts = vec![format!("{word} <id>"), "[\"label\"]".to_string()];
    for slot in def.slots_for(word) {
        if slot.arity == props::VARIADIC {
            parts.push(format!("<{}>…", slot.prop));
        } else {
            for _ in 0..slot.arity {
                parts.push(format!("<{}>", slot.prop));
            }
        }
    }

    let has_label = words.get(2).is_some_and(|word| word.starts_with('"'));
    let written = words.len().saturating_sub(if has_label { 3 } else { 2 });
    let active = written.min(parts.len().saturating_sub(3)) + 2;
    let label = parts.join(" ");
    let mut offsets = Vec::new();
    let mut at = 0usize;
    for part in &parts {
        offsets.push(json!({ "label": [at, at + part.chars().count()] }));
        at += part.chars().count() + 1;
    }

    json!({
        "signatures": [{
            "label": label,
            "documentation": { "kind": "markdown", "value": def.doc },
            "parameters": offsets,
            "activeParameter": active,
        }],
        "activeSignature": 0,
        "activeParameter": active,
    })
}

/// Every editable colour literal, with ranges expressed in LSP positions.
pub(super) fn colours(text: &str) -> Value {
    let index = LineIndex::new(text);
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'#' {
            i += 1;
            continue;
        }
        let start = i;
        let mut end = i + 1;
        while end < bytes.len() && (bytes[end] as char).is_ascii_hexdigit() {
            end += 1;
        }
        let hex = &text[start + 1..end];
        i = end.max(start + 1);
        let expand = |value: &str| -> String { value.chars().flat_map(|c| [c, c]).collect() };
        let (rgb, alpha) = match hex.len() {
            3 => (expand(hex), "ff".to_string()),
            4 => (expand(&hex[0..3]), expand(&hex[3..4])),
            6 => (hex.to_string(), "ff".to_string()),
            8 => (hex[0..6].to_string(), hex[6..8].to_string()),
            _ => continue,
        };
        let channel = |at: usize, value: &str| {
            u8::from_str_radix(&value[at..at + 2], 16).unwrap_or(0) as f64 / 255.0
        };
        out.push(json!({
            "range": index.range(Span { start, end }),
            "color": {
                "red": channel(0, &rgb),
                "green": channel(2, &rgb),
                "blue": channel(4, &rgb),
                "alpha": channel(0, &alpha),
            },
        }));
    }
    json!(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_names_the_slot_being_typed() {
        let source = "type packet\nfield sport \"Source port\" 0 ";
        let help = signature(source, source.len());
        assert_eq!(
            help["signatures"][0]["label"],
            "field <id> [\"label\"] <bits> <bits>"
        );
        assert_eq!(help["activeParameter"], 3);
        let plain = "type class\nclass a \"A\" ";
        assert!(signature(plain, plain.len()).is_null());
    }

    #[test]
    fn colours_include_alpha_and_ignore_non_hex_comments() {
        let source = "graph { background: #0b1120 }\nnode a { fill: #38bdf880 }\n// #not-a-color\n";
        let found = colours(source);
        let list = found.as_array().unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[1]["color"]["alpha"].as_f64().unwrap(), 128.0 / 255.0);
    }
}
