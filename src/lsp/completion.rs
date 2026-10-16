use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::{
    ast::Span,
    lsp::{Block, Context, LineIndex, declared_archetype},
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
    let ctx = Context::at(text, offset);
    let mut items = items_for(&text[ctx.statement_start..offset]);
    let word = super::context::word_at(text, offset)
        .map(|(_, span)| span)
        .unwrap_or(Span {
            start: offset,
            end: offset,
        });
    let range = LineIndex::new(text).range(word);
    for item in &mut items {
        let insert = item.get("insertText").unwrap_or(&item["label"]).clone();
        item["textEdit"] = json!({"range": range, "newText": insert});
    }
    json!({ "isIncomplete": !items.is_empty(), "items": items })
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
    if ctx.in_string || ctx.in_comment {
        return Value::Null;
    }

    let prefix = &text[..offset];
    let line = &prefix[ctx.statement_start..];
    let words: Vec<&str> = line.split_whitespace().collect();
    if ctx.block == Some(Block::Layout) {
        return layout_signature(text, line, &words);
    }
    if ctx.block.is_some_and(|block| block != Block::Container) {
        return Value::Null;
    }
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

fn layout_signature(text: &str, line: &str, words: &[&str]) -> Value {
    if !props::archetype(declared_archetype(text))
        .is_some_and(|def| def.layout.accepts_layout_block())
    {
        return Value::Null;
    }
    let slot = words
        .len()
        .saturating_sub(usize::from(!line.ends_with(char::is_whitespace)));
    let Some(&word) = words.first() else {
        return Value::Null;
    };
    let (label, parameters, active, doc): (String, &[&str], usize, &str) = match word {
        "place" if words.get(2).is_some_and(|word| super::is_rel_dir(word)) => {
            let active = if words.contains(&"align") {
                3
            } else if words.contains(&"gap") {
                2
            } else {
                usize::from(slot >= 3)
            };
            (
                format!("place target {} anchor gap distance align axis", words[2]),
                &["target", "anchor", "distance", "axis"],
                active,
                "Place an existing element relative to an anchor. gap and align are optional.",
            )
        }
        "place" => {
            let active = if slot < 2 {
                0
            } else if line.contains(',') {
                2
            } else {
                1
            };
            if line.contains("(col") {
                (
                    "place target at (col column, row row-index)".into(),
                    &["target", "column", "row-index"],
                    active,
                    "Place an element in a cell of the declared graph grid.",
                )
            } else {
                (
                    "place target at (x, y)".into(),
                    &["target", "x", "y"],
                    active,
                    "Set the top-left corner. Both coordinates accept zero and negative numbers.",
                )
            }
        }
        "align" => (
            "align axis first other …".into(),
            &["axis", "first", "other"],
            slot.saturating_sub(1).min(2),
            "Align nodes on top, bottom, left, right, center-x or center-y.",
        ),
        "equal-width" | "equal-height" => (
            format!("{word} first other …"),
            &["first", "other"],
            slot.saturating_sub(1).min(1),
            "Give nodes a common size large enough for their labels.",
        ),
        "row" | "column" => (
            format!("{word} first next … gap distance"),
            &["first", "next", "distance"],
            if words.contains(&"gap") {
                2
            } else {
                slot.saturating_sub(1).min(1)
            },
            "Arrange direct nodes in order. The optional gap has no square brackets.",
        ),
        _ => return Value::Null,
    };
    let mut from = 0;
    let parameters: Vec<_> = parameters.iter().map(|parameter| {
        let start = from + label[from..].find(parameter).expect("signature parameter");
        let end = start + parameter.len();
        from = end;
        json!({"label": [label[..start].encode_utf16().count(), label[..end].encode_utf16().count()]})
    }).collect();
    json!({
        "signatures": [{"label": label, "parameters": parameters,
            "documentation": {"kind": "markdown", "value": doc}, "activeParameter": active}],
        "activeSignature": 0, "activeParameter": active,
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
