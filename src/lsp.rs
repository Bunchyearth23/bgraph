//! A Language Server for the bgraph DSL, spoken over stdio.
//!
//! JSON-RPC framing is hand-rolled: the protocol surface we need is small and
//! a synchronous request loop is the honest shape for a server whose entire
//! workload is "re-parse a file that fits in a screen".
//!
//! Everything semantic comes from `parser` + `check`, so the squiggles in the
//! editor are exactly the errors the CLI would print. When a document is
//! mid-edit and does not parse, completion and navigation fall back to a
//! lexical scan — a broken file is precisely when help matters most.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};

use serde_json::{Value, json};
use std::fmt::Write as _;

use crate::ast::Span;
use crate::check::{self, Severity};
use crate::props::{self, Ctx};

// ------------------------------------------------------------ positions

/// Maps byte offsets to LSP positions (zero-based line, UTF-16 column).
pub struct LineIndex {
    /// Byte offset of the start of each line.
    starts: Vec<usize>,
    text: String,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        LineIndex {
            starts,
            text: text.to_string(),
        }
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.text.len());
        let line = match self.starts.binary_search(&offset) {
            Ok(exact) => exact,
            Err(next) => next - 1,
        };
        let col = self.text[self.starts[line]..offset]
            .chars()
            .map(char::len_utf16)
            .sum();
        (line, col)
    }

    pub fn offset(&self, line: usize, col: usize) -> usize {
        let Some(&start) = self.starts.get(line) else {
            return self.text.len();
        };
        let end = self
            .starts
            .get(line + 1)
            .copied()
            .unwrap_or(self.text.len());

        let mut utf16 = 0;
        for (i, c) in self.text[start..end].char_indices() {
            if utf16 >= col {
                return start + i;
            }
            utf16 += c.len_utf16();
        }
        end
    }

    fn range(&self, span: Span) -> Value {
        let (sl, sc) = self.line_col(span.start);
        let (el, ec) = self.line_col(span.end);
        json!({
            "start": { "line": sl, "character": sc },
            "end":   { "line": el, "character": ec },
        })
    }
}

// ------------------------------------------------------------ server

#[derive(Default)]
struct Server {
    docs: HashMap<String, String>,
}

pub fn run() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut writer = stdout.lock();
    let mut server = Server::default();

    while let Some(message) = read_message(&mut reader)? {
        let id = message.get("id").cloned();
        let method = message
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let params = message.get("params").cloned().unwrap_or(Value::Null);

        if method == "exit" {
            break;
        }

        match server.handle(&method, &params) {
            Response::Result(result) => {
                if let Some(id) = id {
                    write_message(
                        &mut writer,
                        &json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    )?;
                }
            }
            Response::Nothing => {
                if let Some(id) = id {
                    write_message(
                        &mut writer,
                        &json!({ "jsonrpc": "2.0", "id": id, "result": Value::Null }),
                    )?;
                }
            }
        }

        // Notifications the client must receive after the request is answered.
        for note in server.drain_notifications(&method, &params) {
            write_message(&mut writer, &note)?;
        }
    }

    Ok(())
}

enum Response {
    Result(Value),
    Nothing,
}

impl Server {
    fn handle(&mut self, method: &str, params: &Value) -> Response {
        match method {
            "initialize" => Response::Result(capabilities()),
            "shutdown" => Response::Result(Value::Null),

            "textDocument/didOpen" => {
                let uri = uri_of(params);
                let text = params["textDocument"]["text"].as_str().unwrap_or("");
                self.docs.insert(uri, text.to_string());
                Response::Nothing
            }
            "textDocument/didChange" => {
                let uri = uri_of(params);
                // The server advertises full sync, so the last change is the
                // whole document.
                if let Some(text) = params["contentChanges"]
                    .as_array()
                    .and_then(|c| c.last())
                    .and_then(|c| c["text"].as_str())
                {
                    self.docs.insert(uri, text.to_string());
                }
                Response::Nothing
            }
            "textDocument/didClose" => {
                self.docs.remove(&uri_of(params));
                Response::Nothing
            }
            "textDocument/didSave" => Response::Nothing,

            "textDocument/completion" => Response::Result(self.completion(params)),
            "textDocument/hover" => Response::Result(self.hover(params)),
            "textDocument/definition" => Response::Result(self.definition(params)),
            "textDocument/documentSymbol" => Response::Result(self.symbols(params)),
            "textDocument/signatureHelp" => Response::Result(self.signature(params)),
            "textDocument/documentColor" => Response::Result(self.colours(params)),
            "textDocument/colorPresentation" => Response::Result(colour_presentation(params)),
            "textDocument/documentHighlight" => Response::Result(self.highlights(params)),

            _ => Response::Nothing,
        }
    }

    fn drain_notifications(&self, method: &str, params: &Value) -> Vec<Value> {
        if !matches!(
            method,
            "textDocument/didOpen" | "textDocument/didChange" | "textDocument/didSave"
        ) {
            return Vec::new();
        }

        let uri = uri_of(params);
        let Some(text) = self.docs.get(&uri) else {
            return Vec::new();
        };

        vec![json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": uri,
                "diagnostics": diagnostics(text),
            }
        })]
    }

    fn doc(&self, params: &Value) -> Option<(&String, LineIndex, usize)> {
        let uri = params["textDocument"]["uri"].as_str()?;
        let text = self.docs.get(uri)?;
        let index = LineIndex::new(text);
        let line = params["position"]["line"].as_u64()? as usize;
        let col = params["position"]["character"].as_u64()? as usize;
        let offset = index.offset(line, col);
        Some((text, index, offset))
    }

    // -------------------------------------------------------- completion

    fn completion(&self, params: &Value) -> Value {
        let Some((text, _index, offset)) = self.doc(params) else {
            return json!({ "isIncomplete": false, "items": [] });
        };

        let prefix = &text[..offset];
        let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = &prefix[line_start..];
        let items = self.items_for(text, line, offset);

        json!({ "isIncomplete": false, "items": items })
    }

    /// Completion is deliberately quiet: it stays out of labels and comments,
    /// hides aliases, and only fires on `.` where a class reference can go.
    /// A suggestion that appears while you are typing prose is worse than none.
    fn items_for(&self, text: &str, line: &str, offset: usize) -> Vec<Value> {
        let ctx = Context::at(text, offset);
        if ctx.in_string || ctx.in_comment {
            return Vec::new();
        }

        let trimmed = line.trim_start();
        let archetype = declared_archetype(text);

        // Inside `{{ … }}`: offer the holes this file already uses. A template
        // is written by repeating its own parameter names, and misspelling one
        // silently produces a second parameter nobody binds.
        if let Some(open) = line.rfind("{{")
            && !line[open..].contains("}}")
        {
            let typed = line[open + 2..].trim_start();
            let mut names: Vec<String> = Vec::new();
            for hole in crate::template::holes_in(text) {
                if !names.contains(&hole.name) {
                    names.push(hole.name);
                }
            }
            return names
                .into_iter()
                .filter(|n| n.starts_with(typed) || typed.is_empty())
                .map(|name| {
                    let doc = match name.split_once('.') {
                        Some((prefix, field)) => format!(
                            "`{field}` of each row of `{prefix}`. This line is written once \
                             per row."
                        ),
                        None => format!("A value bound once for the whole file: `{name}`."),
                    };
                    completion(&name, 6, "hole", &doc)
                })
                .collect();
        }

        // `.class` — only after whitespace, so `1.5` and `..>` stay quiet.
        if let Some(dot) = line.rfind('.')
            && line[dot + 1..].chars().all(is_ident_char)
            && line[..dot]
                .chars()
                .next_back()
                .is_none_or(char::is_whitespace)
        {
            return declared(text, "style")
                .into_iter()
                .map(|(name, _)| {
                    completion(&name, 7, "style", &format!("Apply the `{name}` style."))
                })
                .collect();
        }

        if let Some(block) = ctx.block {
            // An item block sees the archetype's own vocabulary first, so
            // `kind:` offers state kinds in a machine and element kinds in an
            // architecture view — never both.
            let items: Vec<&'static props::PropDef> = props::archetype(archetype)
                .map(props::item_table)
                .unwrap_or_default();
            let table: Vec<&'static props::PropDef> = match block {
                Block::Graph => props::GRAPH_PROPS.iter().collect(),
                Block::Type => props::archetype(archetype)
                    .map_or(&[][..], |a| a.props)
                    .iter()
                    .collect(),
                _ => items,
            };

            // `name: <here>` — complete the value, if the property enumerates.
            if let Some(colon) = line.rfind(':') {
                let key: String = line[..colon]
                    .chars()
                    .rev()
                    .take_while(|c| is_ident_char(*c))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect();

                return table
                    .iter()
                    .find(|p| p.name == key)
                    .map(|def| {
                        def.words
                            .iter()
                            .map(|w| completion(w, 20, def.syntax, def.doc))
                            .collect()
                    })
                    .unwrap_or_default();
            }

            let want = match block {
                Block::Node => Some(Ctx::Node),
                Block::Edge => Some(Ctx::Edge),
                _ => None,
            };

            return table
                .iter()
                .filter(|p| !p.alias)
                .filter(|p| want.is_none_or(|c| p.ctx.allows(c)))
                .enumerate()
                .map(|(i, p)| {
                    let mut it = completion(p.name, 10, p.syntax, p.doc);
                    it["insertText"] = json!(format!("{}: ", p.name));
                    // Table order is meaningful; keep it instead of alphabetical.
                    it["sortText"] = json!(format!("{i:03}"));
                    it
                })
                .collect();
        }

        // ------------------------------------------------ statement level

        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let ends_with_space = line.ends_with(char::is_whitespace);
        // The word being completed, or "" when a fresh one is starting.
        let current = if ends_with_space {
            ""
        } else {
            words.last().copied().unwrap_or("")
        };
        let preceding = if ends_with_space {
            words.last().copied().unwrap_or("")
        } else if words.len() >= 2 {
            words[words.len() - 2]
        } else {
            ""
        };

        let item = props::archetype(archetype).map_or("node", |a| a.item);

        if preceding == "type" {
            return props::ARCHETYPES
                .iter()
                .map(|a| completion(a.name, 20, "archetype", a.doc))
                .collect();
        }

        if is_rel_dir(preceding) {
            return declared_items(text, item)
                .into_iter()
                .map(|(name, label)| completion(&name, 6, item, &label))
                .collect();
        }

        if preceding == "align" {
            return props::ALIGNMENTS
                .iter()
                .map(|a| completion(a, 20, "align", "Cross-axis alignment against the anchor."))
                .collect();
        }

        if preceding == "extends" {
            return declared(text, "style")
                .into_iter()
                .map(|(name, _)| completion(&name, 7, "style", &format!("Inherit from `{name}`.")))
                .collect();
        }

        // Placements only make sense where placement is honoured, and only
        // once the item already has an identifier.
        if words.first().is_some_and(|w| *w == item || *w == "node")
            && words.len() >= 2
            && archetype == "free"
        {
            return props::PLACEMENTS
                .iter()
                .map(|(name, doc)| completion(name, 14, "placement", doc))
                .collect();
        }

        // The first word of a statement: a keyword, or a node id starting an edge.
        if words.len() <= 1 && !ends_with_space {
            let mut out: Vec<Value> = props::KEYWORDS
                .iter()
                .map(|(name, doc)| {
                    // The shape each statement takes, so accepting one leaves
                    // a well-formed line rather than a bare word.
                    let insert = match *name {
                        "type" => "type ${1:archetype}$0".to_string(),
                        "graph" => "graph { $0 }".to_string(),
                        "style" => "style ${1:name} {\n  $0\n}".to_string(),
                        "group" => "group ${1:id} \"${2:label}\" {\n  $0\n}".to_string(),
                        "activate" => "activate ${1:participant} {\n  $0\n}".to_string(),
                        other => format!("{other} ${{1:id}} \"${{2:label}}\"$0"),
                    };
                    snippet(name, 14, "keyword", doc, &insert)
                })
                .collect();

            // The archetype's own word first; `node` stays available. A second
            // word carries a second positional shape (`D-023`), so leaving it
            // out would hide half of what the archetype accepts — and its own
            // doc is what tells the two apart, so it is offered with that
            // rather than a generic line.
            if let Some(def) = props::archetype(archetype) {
                let mut at = 0;
                if def.item != "node" {
                    out.insert(
                        at,
                        snippet(
                            def.item,
                            14,
                            &format!("{} <id> [\"label\"] …", def.item),
                            &format!(
                                "Declare a {} — what `type {archetype}` calls its elements.",
                                def.item
                            ),
                            &item_snippet(def.item, def.slots),
                        ),
                    );
                    at += 1;
                }
                for alt in def.alt_items {
                    out.insert(
                        at,
                        snippet(
                            alt.item,
                            14,
                            &format!("{} <id> [\"label\"] …", alt.item),
                            alt.doc,
                            &item_snippet(alt.item, alt.slots),
                        ),
                    );
                    at += 1;
                }
            }
            if archetype == "free" && !current.is_empty() {
                out.extend(
                    declared_items(text, item)
                        .into_iter()
                        .map(|(name, label)| completion(&name, 6, item, &label)),
                );
            }
            return out;
        }

        Vec::new()
    }

    // ----------------------------------------------------- signature help

    /// What the statement under the cursor takes, and which value you are on.
    ///
    /// `slice a "A" 4820` is the idiomatic form (`D-015`), and until now the
    /// only way to learn its shape was to read the manual — the editor knew
    /// and did not say. The active parameter is counted from the values
    /// already written, so it walks along as you type.
    fn signature(&self, params: &Value) -> Value {
        let Some((text, _index, offset)) = self.doc(params) else {
            return Value::Null;
        };
        let ctx = Context::at(text, offset);
        if ctx.in_string || ctx.in_comment || ctx.block.is_some() {
            return Value::Null;
        }

        let prefix = &text[..offset];
        let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let line = &prefix[line_start..];
        let words: Vec<&str> = line.split_whitespace().collect();
        let Some(&word) = words.first() else {
            return Value::Null;
        };

        let archetype = declared_archetype(text);
        let Some(def) = props::archetype(archetype) else {
            return Value::Null;
        };
        if !def.accepts(word) {
            return Value::Null;
        }

        // `<id>` and an optional `"label"` come before the data, so the slots
        // start at the third word.
        let slots = def.slots_for(word);
        if slots.is_empty() {
            return Value::Null;
        }
        let mut parts = vec![format!("{word} <id>"), "[\"label\"]".to_string()];
        for slot in slots {
            if slot.arity == props::VARIADIC {
                parts.push(format!("<{}>…", slot.prop));
            } else {
                for _ in 0..slot.arity {
                    parts.push(format!("<{}>", slot.prop));
                }
            }
        }

        // How many values are already written, ignoring the keyword, the id
        // and a label — a label is optional, so it only counts if it is there.
        let has_label = words.get(2).is_some_and(|w| w.starts_with('"'));
        let written = words.len().saturating_sub(if has_label { 3 } else { 2 });
        // Clamped to the last slot: a variadic one goes on accepting values,
        // and pointing past the end would highlight nothing.
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

    // ---------------------------------------------------------- colours

    /// Every colour literal in the file, so the editor can show a swatch and
    /// open a picker on it. `#` is only ever a colour here (`D-004`), which is
    /// what makes a lexical scan safe.
    fn colours(&self, params: &Value) -> Value {
        let Some(text) = params["textDocument"]["uri"]
            .as_str()
            .and_then(|uri| self.docs.get(uri))
        else {
            return json!([]);
        };
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

            let expand = |s: &str| -> String { s.chars().flat_map(|c| [c, c]).collect() };
            let (rgb, alpha) = match hex.len() {
                3 => (expand(hex), "ff".to_string()),
                4 => (expand(&hex[0..3]), expand(&hex[3..4])),
                6 => (hex.to_string(), "ff".to_string()),
                8 => (hex[0..6].to_string(), hex[6..8].to_string()),
                _ => continue,
            };
            let channel = |at: usize, from: &str| {
                u8::from_str_radix(&from[at..at + 2], 16).unwrap_or(0) as f64 / 255.0
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

    // -------------------------------------------------------- highlights

    /// Every place the identifier under the cursor is written. Reading a
    /// diagram means following an id around the file, and until now that was
    /// the editor's find-in-file rather than anything that knew what an id is.
    fn highlights(&self, params: &Value) -> Value {
        let Some((text, index, offset)) = self.doc(params) else {
            return json!([]);
        };
        let Some((word, _)) = word_at(text, offset) else {
            return json!([]);
        };
        if word.is_empty() || props::KEYWORDS.iter().any(|(k, _)| *k == word) {
            return json!([]);
        }

        let mut out = Vec::new();
        let bytes = text.as_bytes();
        let mut from = 0;
        while let Some(found) = text[from..].find(&word) {
            let start = from + found;
            let end = start + word.len();
            from = end;
            // Whole words only: `a` must not light up inside `api`.
            let before_ok = start == 0 || !is_ident_char(bytes[start - 1] as char);
            let after_ok = end >= bytes.len() || !is_ident_char(bytes[end] as char);
            if before_ok && after_ok {
                out.push(json!({ "range": index.range(Span { start, end }), "kind": 1 }));
            }
        }
        json!(out)
    }

    // -------------------------------------------------------- hover

    fn hover(&self, params: &Value) -> Value {
        let Some((text, index, offset)) = self.doc(params) else {
            return Value::Null;
        };
        let Some((word, span)) = word_at(text, offset) else {
            return Value::Null;
        };

        let prefix = &text[..span.start];
        let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let after = &text[span.end..];
        let is_key = after.trim_start().starts_with(':');
        let ctx = Context::at(text, offset);
        let archetype = declared_archetype(text);

        // A hole reads as an ordinary word to `word_at`, so it is answered
        // before anything else — otherwise `value` inside `{{r.value}}` would
        // be documented as the pie property, which it is not.
        let inside = crate::template::holes_in(text)
            .into_iter()
            .find(|h| h.span.contains(offset));

        let markdown = if let Some(hole) = inside {
            let uses = crate::template::holes_in(text)
                .iter()
                .filter(|h| h.name == hole.name)
                .count();
            let plural = if uses == 1 { "once" } else { "twice or more" };
            Some(match &hole.prefix {
                Some(prefix) => format!(
                    "**hole** `{}`\n\nA field of each row of `{prefix}`. The line it sits on \
                     is written **once per row**, so the number of statements is decided by \
                     the data rather than by the file.\n\nWritten {plural} here.",
                    hole.name
                ),
                None => format!(
                    "**hole** `{}`\n\nBound once for the whole file.\n\nWritten {plural} \
                     here.",
                    hole.name
                ),
            })
        } else if text[line_start..span.start].trim_end().ends_with("type") {
            props::archetype(&word).map(|a| format!("**type** `{}`\n\n{}", a.name, a.doc))
        } else if is_key && ctx.block.is_some() {
            let table: Vec<&'static props::PropDef> = match ctx.block {
                Some(Block::Graph) => props::GRAPH_PROPS.iter().collect(),
                Some(Block::Type) => props::archetype(archetype)
                    .map_or(&[][..], |a| a.props)
                    .iter()
                    .collect(),
                _ => props::archetype(archetype)
                    .map(props::item_table)
                    .unwrap_or_default(),
            };
            table
                .iter()
                .find(|p| p.name == word)
                .map(|d| format!("```\n{}\n```\n\n{}", d.syntax, d.doc))
        } else if text[line_start..span.start].trim_end().ends_with('.')
            || text[line_start..span.start].trim_end().ends_with("extends")
        {
            self.describe_style(text, &word)
        } else if text[line_start..span.start].trim().is_empty()
            && props::archetype(archetype).is_some_and(|d| d.accepts(&word))
            && word != "node"
        {
            // The archetype's own word, at the start of a statement. Its
            // positional shape is the thing you actually want here.
            props::archetype(archetype).map(|def| {
                let slots = def.slots_for(&word);
                let mut form = format!("{word} <id> [\"label\"]");
                for slot in slots {
                    if slot.arity == props::VARIADIC {
                        let _ = write!(form, " <{}>…", slot.prop);
                    } else {
                        for _ in 0..slot.arity {
                            let _ = write!(form, " <{}>", slot.prop);
                        }
                    }
                }
                format!(
                    "```\n{form}\n```\n\nWhat `type {}` calls its elements.\n\n{}",
                    def.name, def.doc
                )
            })
        } else if let Some((_, doc)) = props::PLACEMENTS.iter().find(|(k, _)| *k == word) {
            Some(doc.to_string())
        } else if let Some((_, doc)) = props::KEYWORDS.iter().find(|(k, _)| *k == word) {
            Some(doc.to_string())
        } else {
            self.describe_node(text, &word)
                .or_else(|| self.describe_style(text, &word))
        };

        match markdown {
            Some(md) => json!({
                "contents": { "kind": "markdown", "value": md },
                "range": index.range(span),
            }),
            None => Value::Null,
        }
    }

    fn describe_node(&self, text: &str, id: &str) -> Option<String> {
        let diagram = crate::parser::parse(text).ok()?;
        let (i, node) = diagram
            .nodes
            .iter()
            .enumerate()
            .find(|(_, n)| n.id.as_str() == id)?;

        let mut out = format!("**node** `{id}`");
        if let Some(label) = &node.label {
            out.push_str(&format!(" — {:?}", label));
        }
        if !node.classes.is_empty() {
            let classes: Vec<String> = node
                .classes
                .iter()
                .map(|c| format!(".{}", c.as_str()))
                .collect();
            out.push_str(&format!("\n\nstyles: {}", classes.join(" ")));
        }

        // Resolved geometry *and* the resolved paint. The cascade is the
        // thing that is hard to hold in your head — four layers deep, and the
        // answer was only visible by rendering the file.
        if let Ok(layout) = crate::layout::solve(&diagram)
            && let Some(placed) = layout.nodes.get(i)
        {
            let r = placed.rect;
            let s = &placed.style;
            let mut facts = vec![format!("x {:.0}  y {:.0}   {:.0} × {:.0}", r.x, r.y, r.w, r.h)];
            if let Some(fill) = &s.fill {
                facts.push(format!("fill    {fill}"));
            }
            if let Some(stroke) = &s.stroke {
                facts.push(format!(
                    "stroke  {stroke}{}",
                    s.stroke_width
                        .map(|w| format!("  {w}"))
                        .unwrap_or_default()
                ));
            }
            if let Some(shape) = s.shape {
                facts.push(format!("shape   {shape:?}").to_lowercase());
            }
            out.push_str(&format!("\n\n```\n{}\n```", facts.join("\n")));
        }

        Some(out)
    }

    fn describe_style(&self, text: &str, name: &str) -> Option<String> {
        let diagram = crate::parser::parse(text).ok()?;
        let style = diagram.styles.iter().find(|s| s.name.as_str() == name)?;

        let mut out = format!("**style** `{name}`");
        if let Some(parent) = &style.extends {
            out.push_str(&format!(" extends `{}`", parent.as_str()));
        }
        out.push_str("\n\n```\n");
        for prop in &style.props {
            out.push_str(&format!("{}\n", text_of(text, prop.span).trim()));
        }
        out.push_str("```");
        Some(out)
    }

    // -------------------------------------------------------- navigation

    fn definition(&self, params: &Value) -> Value {
        let Some((text, index, offset)) = self.doc(params) else {
            return Value::Null;
        };
        let Some((word, span)) = word_at(text, offset) else {
            return Value::Null;
        };
        let uri = params["textDocument"]["uri"].as_str().unwrap_or("");

        let before = text[..span.start].trim_end();
        let style_first = before.ends_with('.') || before.ends_with("extends");

        let order: [&str; 2] = if style_first {
            ["style", "node"]
        } else {
            ["node", "style"]
        };

        for kind in order {
            if let Some(target) = declaration_span(text, kind, &word) {
                return json!({ "uri": uri, "range": index.range(target) });
            }
        }

        Value::Null
    }

    fn symbols(&self, params: &Value) -> Value {
        let Some(uri) = params["textDocument"]["uri"].as_str() else {
            return json!([]);
        };
        let Some(text) = self.docs.get(uri) else {
            return json!([]);
        };
        let Ok(diagram) = crate::parser::parse(text) else {
            return json!([]);
        };
        let index = LineIndex::new(text);

        let mut out = Vec::new();

        // The holes first: they are the file's parameters, and a template's
        // outline that did not list them would be an outline of the shape
        // without the thing that varies.
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
                "kind": 26,                      // TypeParameter
                "range": index.range(hole.span),
                "selectionRange": index.range(hole.span),
            }));
        }

        for style in &diagram.styles {
            out.push(json!({
                "name": style.name.as_str(),
                "detail": style.extends.as_ref().map(|p| format!("extends {}", p.as_str())),
                "kind": 5,                       // Class
                "range": index.range(style.span),
                "selectionRange": index.range(style.name.span),
            }));
        }
        for node in &diagram.nodes {
            out.push(json!({
                "name": node.id.as_str(),
                "detail": node.label.clone(),
                "kind": 13,                      // Variable
                "range": index.range(node.span),
                "selectionRange": index.range(node.id.span),
            }));
        }

        json!(out)
    }
}

// ------------------------------------------------------------ diagnostics

fn diagnostics(text: &str) -> Vec<Value> {
    let index = LineIndex::new(text);

    let diags = match crate::parser::parse(text) {
        Ok(diagram) => check::check(&diagram),
        Err(err) => vec![check::Diag {
            severity: Severity::Error,
            message: err.message,
            span: err.span,
        }],
    };

    // A template's rules are the template's, and nothing was running them
    // until a render did — so a line repeating over two lists was a surprise
    // at the end rather than a squiggle while writing it.
    let mut diags = diags;
    for (span, message) in crate::template::lint(text) {
        diags.push(check::Diag {
            severity: Severity::Warning,
            message,
            span,
        });
    }
    diags.sort_by_key(|d| (d.span.start, d.span.end));

    diags
        .into_iter()
        .map(|d| {
            json!({
                "range": index.range(d.span),
                "severity": match d.severity { Severity::Error => 1, Severity::Warning => 2 },
                "source": "bgraph",
                "message": d.message,
            })
        })
        .collect()
}

// ------------------------------------------------------------ lexical helpers

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Block {
    Type,
    Graph,
    Style,
    Node,
    Edge,
}

/// Where the cursor sits, decided lexically so it keeps working while the
/// document does not parse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Context {
    in_string: bool,
    in_comment: bool,
    /// The statement owning the innermost unclosed `{`.
    block: Option<Block>,
}

impl Context {
    fn at(text: &str, offset: usize) -> Context {
        let mut stack = Vec::new();
        let mut chars = text[..offset.min(text.len())].char_indices().peekable();
        let mut in_string = false;
        let mut in_comment = false;

        while let Some((_, c)) = chars.next() {
            let i = chars.peek().map(|(j, _)| *j).unwrap_or(text.len());
            match c {
                '\\' if in_string => {
                    chars.next();
                }
                '"' => in_string = !in_string,
                '/' if !in_string && matches!(chars.peek(), Some((_, '/'))) => {
                    in_comment = true;
                    for (_, c) in chars.by_ref() {
                        if c == '\n' {
                            in_comment = false;
                            break;
                        }
                    }
                }
                '{' if !in_string => stack.push(i - c.len_utf8()),
                '}' if !in_string => {
                    stack.pop();
                }
                _ => {}
            }
        }

        let block = stack.pop().map(|brace| {
            let line_start = text[..brace].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let head = &text[line_start..brace];
            match head.split_whitespace().next() {
                Some("type") => Block::Type,
                Some("graph") => Block::Graph,
                // A group's block takes node-ish visual properties.
                Some("group") => Block::Node,
                Some("style") => Block::Style,
                // Everything else is an item or an edge, and the arrow is what
                // tells them apart. Matching the *keyword* instead would mean
                // knowing all sixteen of them — which is how `slice a "A" {`
                // came to be offered `arrow` and `router`, and denied `fill`.
                _ if declares_an_edge(head) => Block::Edge,
                _ => Block::Node,
            }
        });

        Context {
            in_string,
            in_comment,
            block,
        }
    }
}

/// Whether a statement's head is an edge: it has an arrow outside a label.
fn declares_an_edge(head: &str) -> bool {
    const ARROWS: [&str; 7] = ["<->", "<-", "-->", "..>", "--", "->", ".."];
    let mut rest = head;

    while !rest.is_empty() {
        // A label may contain anything, arrows included; skip it whole.
        if let Some(quote) = rest.find('"') {
            let (before, after) = rest.split_at(quote);
            if ARROWS.iter().any(|a| before.contains(a)) {
                return true;
            }
            let mut chars = after[1..].char_indices();
            rest = loop {
                match chars.next() {
                    Some((_, '\\')) => {
                        chars.next();
                    }
                    Some((i, '"')) => break &after[1 + i + 1..],
                    Some(_) => {}
                    None => return false,
                }
            };
        } else {
            return ARROWS.iter().any(|a| rest.contains(a));
        }
    }

    false
}

/// The archetype the file declares, by line scan — `free` when it declares none.
fn declared_archetype(text: &str) -> &str {
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("type") else {
            continue;
        };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let name = rest.trim_start();
        let end = name.find(|c: char| !is_ident_char(c)).unwrap_or(name.len());
        if end > 0 {
            return &name[..end];
        }
    }
    "free"
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn is_rel_dir(word: &str) -> bool {
    matches!(word, "right-of" | "left-of" | "above" | "below")
}

/// The identifier surrounding `offset`, if any.
fn word_at(text: &str, offset: usize) -> Option<(String, Span)> {
    if text.is_empty() {
        return None;
    }
    let offset = offset.min(text.len());

    let start = text[..offset]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_ident_char(*c))
        .last()
        .map(|(i, _)| i)
        .unwrap_or(offset);
    let end = text[offset..]
        .char_indices()
        .take_while(|(_, c)| is_ident_char(*c))
        .last()
        .map(|(i, c)| offset + i + c.len_utf8())
        .unwrap_or(offset);

    if start == end {
        return None;
    }
    Some((text[start..end].to_string(), Span { start, end }))
}

/// Items declared with either the archetype's keyword or the universal `node`.
fn declared_items(text: &str, item: &str) -> Vec<(String, String)> {
    let mut out = declared(text, item);
    if item != "node" {
        out.extend(declared(text, "node"));
    }
    // A group is a perfectly good anchor.
    out.extend(declared(text, "group"));
    out
}

/// Every `<keyword> <id>` in the file, found by line scan so it survives a
/// syntax error elsewhere.
fn declared(text: &str, keyword: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix(keyword) else {
            continue;
        };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let mut parts = rest.trim_start().splitn(2, |c: char| c.is_whitespace());
        let Some(name) = parts.next().filter(|n| n.chars().all(is_ident_char)) else {
            continue;
        };

        let detail = parts
            .next()
            .and_then(|r| r.trim().strip_prefix('"'))
            .and_then(|r| r.split('"').next())
            .unwrap_or("")
            .to_string();

        out.push((name.to_string(), detail));
    }

    out.dedup_by(|a, b| a.0 == b.0);
    out
}

/// Byte span of the identifier in `<keyword> <name>`, for go-to-definition.
fn declaration_span(text: &str, keyword: &str, name: &str) -> Option<Span> {
    let mut offset = 0;

    for line in text.split_inclusive('\n') {
        let indent = line.len() - line.trim_start().len();
        let trimmed = line.trim_start();

        if let Some(rest) = trimmed.strip_prefix(keyword)
            && rest.starts_with(char::is_whitespace)
        {
            let gap = rest.len() - rest.trim_start().len();
            let start = offset + indent + keyword.len() + gap;
            let ident: String = text[start..]
                .chars()
                .take_while(|c| is_ident_char(*c))
                .collect();
            if ident == name {
                return Some(Span {
                    start,
                    end: start + ident.len(),
                });
            }
        }
        offset += line.len();
    }

    None
}

fn text_of(text: &str, span: Span) -> &str {
    &text[span.start.min(text.len())..span.end.min(text.len())]
}

// ------------------------------------------------------------ protocol

fn uri_of(params: &Value) -> String {
    params["textDocument"]["uri"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

fn completion(label: &str, kind: u8, detail: &str, doc: &str) -> Value {
    json!({
        "label": label,
        "kind": kind,
        "detail": detail,
        "documentation": { "kind": "markdown", "value": doc },
        // Alphabetical order puts `align` above the archetype's own word,
        // which is the one thing you always want first. The kind is a rough
        // priority, so ordering by it and then by name says what the list is
        // for rather than what the alphabet is.
        "sortText": format!("{:02}{label}", 99 - kind),
    })
}

/// The same, but typing the completion writes the whole form.
///
/// This is the ergonomic difference between a list of words and a list of
/// *statements*: accepting `slice` should leave the cursor in the identifier
/// with the label already quoted, not leave you to type the rest from memory.
/// `$1`, `$2` are tab stops; `$0` is where the cursor ends.
fn snippet(label: &str, kind: u8, detail: &str, doc: &str, insert: &str) -> Value {
    let mut item = completion(label, kind, detail, doc);
    item["insertText"] = json!(insert);
    item["insertTextFormat"] = json!(2);
    item
}

/// The form an archetype's item statement takes, as a snippet: the keyword,
/// an identifier, a quoted label, then one tab stop per positional slot named
/// after the property it fills.
fn item_snippet(word: &str, slots: &[props::Slot]) -> String {
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

/// The colour a picker produced, written back as the shortest hex that says
/// it — `#38bdf8` rather than `#38bdf8ff`, since alpha defaults to opaque
/// (`D-016`).
fn colour_presentation(params: &Value) -> Value {
    let c = &params["color"];
    let byte = |key: &str| (c[key].as_f64().unwrap_or(0.0) * 255.0).round() as u8;
    let (r, g, b, a) = (byte("red"), byte("green"), byte("blue"), byte("alpha"));
    let label = if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    };
    json!([{ "label": label }])
}

fn capabilities() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,               // full document on every change
            "completionProvider": {
                // Space is a trigger again, but `items_for` now answers with
                // nothing wherever a suggestion would be in the way — the
                // quiet comes from the answer, not from refusing to be asked.
                // `{` opens a hole *and* a block, so it fires often — which is
                // fine, because `items_for` answers with nothing unless the
                // two characters before the cursor are `{{` (`D-011`: the
                // quiet comes from the answer, not from refusing to be asked).
                "triggerCharacters": [".", ":", " ", "{"],
            },
            "hoverProvider": true,
            // Positional data is the idiomatic form in this language, and it
            // is the one form nothing could tell you the shape of while you
            // were typing it.
            "signatureHelpProvider": { "triggerCharacters": [" "], "retriggerCharacters": [" "] },
            "colorProvider": true,
            "documentHighlightProvider": true,
            "definitionProvider": true,
            "documentSymbolProvider": true,
        },
        "serverInfo": { "name": "bgraph-lsp", "version": env!("CARGO_PKG_VERSION") },
    })
}

fn read_message(reader: &mut impl BufRead) -> std::io::Result<Option<Value>> {
    let mut length = None;

    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            return Ok(None);
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some(value) = header
            .strip_prefix("Content-Length:")
            .or_else(|| header.strip_prefix("content-length:"))
        {
            length = value.trim().parse::<usize>().ok();
        }
    }

    let Some(length) = length else {
        return Ok(None);
    };

    let mut buffer = vec![0; length];
    reader.read_exact(&mut buffer)?;

    Ok(serde_json::from_slice(&buffer).ok())
}

fn write_message(writer: &mut impl Write, message: &Value) -> std::io::Result<()> {
    let body = serde_json::to_vec(message)?;
    write!(writer, "Content-Length: {}\r\n\r\n", body.len())?;
    writer.write_all(&body)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server holding one document, and the params a request would carry —
    /// the tests below drive the real handlers rather than their innards.
    fn server(text: &str) -> Server {
        let mut docs = HashMap::new();
        docs.insert("file:///t.bd".to_string(), text.to_string());
        Server { docs }
    }

    fn at(text: &str, offset: usize) -> Value {
        let (line, character) = LineIndex::new(text).line_col(offset);
        json!({
            "textDocument": { "uri": "file:///t.bd" },
            "position": { "line": line, "character": character },
        })
    }

    #[test]
    fn line_index_round_trips_utf16_positions() {
        let text = "node a\nnode é \"héllo\"\nnode c";
        let index = LineIndex::new(text);

        for offset in 0..=text.len() {
            if !text.is_char_boundary(offset) {
                continue;
            }
            let (line, col) = index.line_col(offset);
            assert_eq!(index.offset(line, col), offset, "offset {offset}");
        }
    }

    #[test]
    fn enclosing_block_identifies_the_owner() {
        let text = "graph {\n  pad";
        assert_eq!(Context::at(text, text.len()).block, Some(Block::Graph));

        let text = "style x {\n  fill";
        assert_eq!(Context::at(text, text.len()).block, Some(Block::Style));

        let text = "node a \"A\" {\n  fill";
        assert_eq!(Context::at(text, text.len()).block, Some(Block::Node));

        let text = "a -> b {\n  rou";
        assert_eq!(Context::at(text, text.len()).block, Some(Block::Edge));

        let text = "graph { padding: 4 }\nnode a";
        assert_eq!(Context::at(text, text.len()).block, None);
    }

    #[test]
    fn braces_inside_strings_and_comments_are_ignored() {
        let text = "node a \"{\"\nnode b";
        assert_eq!(Context::at(text, text.len()).block, None);

        let text = "// a { comment\nnode b";
        assert_eq!(Context::at(text, text.len()).block, None);
    }

    #[test]
    fn declared_lists_nodes_and_styles_with_labels() {
        let text = "style dark {\n fill: #000\n}\nnode a \"Alpha\"\nnode b\n";
        assert_eq!(
            declared(text, "node"),
            vec![
                ("a".to_string(), "Alpha".to_string()),
                ("b".to_string(), String::new())
            ]
        );
        assert_eq!(declared(text, "style")[0].0, "dark");
    }

    #[test]
    fn declaration_span_points_at_the_identifier() {
        let text = "node alpha \"A\"\nnode beta \"B\"\n";
        let span = declaration_span(text, "node", "beta").expect("beta is declared");
        assert_eq!(&text[span.start..span.end], "beta");
    }

    #[test]
    fn word_at_grabs_hyphenated_identifiers() {
        let text = "node a right-of b";
        let (word, _) = word_at(text, 10).expect("cursor is inside a word");
        assert_eq!(word, "right-of");
    }

    #[test]
    fn diagnostics_carry_positions() {
        let text = "node a .ghost\n";
        let diags = diagnostics(text);
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["severity"], 1);
        assert_eq!(diags[0]["range"]["start"]["line"], 0);
        assert_eq!(diags[0]["range"]["start"]["character"], 8);
        assert!(
            diags[0]["message"]
                .as_str()
                .unwrap()
                .contains("unknown style")
        );
    }

    #[test]
    fn a_syntax_error_is_reported_at_its_location() {
        let diags = diagnostics("node a {\n");
        assert_eq!(diags.len(), 1);
        assert!(
            diags[0]["message"]
                .as_str()
                .unwrap()
                .contains("syntax error")
        );
    }

    #[test]
    fn completion_offers_styles_after_a_dot() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "style dark {\n fill: #000\n}\nnode a .";
        let items = server.items_for(text, "node a .", text.len());
        assert!(items.iter().any(|i| i["label"] == "dark"));
    }

    #[test]
    fn completion_offers_node_ids_after_an_anchor_keyword() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "node alpha\nnode beta right-of ";
        let items = server.items_for(text, "node beta right-of ", text.len());
        assert!(items.iter().any(|i| i["label"] == "alpha"));
    }

    #[test]
    fn completion_offers_properties_inside_a_block_by_context() {
        let server = Server {
            docs: HashMap::new(),
        };

        let text = "node a {\n  ";
        let items = server.items_for(text, "  ", text.len());
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"shape"));
        assert!(!labels.contains(&"router"), "router is edge-only");

        let text = "a -> b {\n  ";
        let items = server.items_for(text, "  ", text.len());
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"router"));
        assert!(!labels.contains(&"shape"), "shape is node-only");

        let text = "graph {\n  ";
        let items = server.items_for(text, "  ", text.len());
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"flow"));
        assert!(!labels.contains(&"fill"));
    }

    #[test]
    fn completion_offers_enum_values_after_a_colon() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "node a {\n  shape: ";
        let items = server.items_for(text, "  shape: ", text.len());
        assert!(items.iter().any(|i| i["label"] == "cylinder"));
    }

    #[test]
    fn completion_offers_keywords_at_statement_level() {
        let server = Server {
            docs: HashMap::new(),
        };
        let items = server.items_for("", "", 0);
        let labels: Vec<&str> = items.iter().filter_map(|i| i["label"].as_str()).collect();
        assert!(labels.contains(&"node"));
        assert!(labels.contains(&"style"));
        assert!(labels.contains(&"graph"));
    }

    #[test]
    fn hover_on_a_node_reports_its_resolved_geometry() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "node a \"A\" at (10, 20) { size: 100 50 }\nnode b right-of a gap 30\n";

        let hover = server.describe_node(text, "b").expect("b is declared");
        assert!(hover.contains("**node** `b`"), "{hover}");
        // a ends at x=110, plus a 30 gap.
        assert!(hover.contains("x 140"), "{hover}");
    }

    #[test]
    fn hover_on_a_style_lists_its_properties() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "style base { radius: 4 }\nstyle leaf extends base {\n  shape: cylinder\n}\n";

        let hover = server
            .describe_style(text, "leaf")
            .expect("leaf is declared");
        assert!(hover.contains("extends `base`"), "{hover}");
        assert!(hover.contains("shape: cylinder"), "{hover}");
    }

    #[test]
    fn a_template_squiggles_while_it_is_written() {
        // A line repeating over two lists used to be a surprise at render
        // time; nothing was running the template's own rules in the editor.
        let source = "type pie\nslice {{a.id}} \"{{b.name}}\" 1\n";
        let found = diagnostics(source);
        assert!(
            found.iter().any(|d| d["message"]
                .as_str()
                .unwrap_or_default()
                .contains("repeats over")),
            "{found:?}"
        );
        // And the span is the line, not the whole file.
        let d = found
            .iter()
            .find(|d| d["message"].as_str().unwrap_or_default().contains("repeats over"))
            .unwrap();
        assert_eq!(d["range"]["start"]["line"].as_u64().unwrap(), 1);
    }

    #[test]
    fn completion_inside_a_hole_offers_the_holes_already_used() {
        // A template is written by repeating its own parameter names, and
        // misspelling one silently produces a parameter nobody binds.
        let source = "type pie { title: \"{{quarter}}\" }\nslice {{r.id}} \"x\" 1\nslice {{";
        let items = server(source).items_for(source, "slice {{", source.len());
        let labels: Vec<&str> = items
            .iter()
            .filter_map(|i| i["label"].as_str())
            .collect();
        assert!(labels.contains(&"r.id"), "{labels:?}");
        assert!(labels.contains(&"quarter"), "{labels:?}");
    }

    #[test]
    fn hover_on_a_hole_says_what_it_binds() {
        // `value` inside `{{r.value}}` is not the pie property, and answering
        // with that documentation would be worse than answering nothing.
        let source = "type pie\nslice {{r.value}} \"x\" 1";
        let on_hole = source.find("r.value").unwrap() + 2;
        let md = server(source).hover(&at(source, on_hole))["contents"]["value"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        assert!(md.contains("**hole**"), "{md}");
        assert!(md.contains("once per row"), "{md}");
        assert!(!md.contains("slice"), "not the archetype's property: {md}");
    }

    #[test]
    fn the_outline_lists_a_templates_parameters() {
        let source = "type pie { title: \"{{quarter}}\" }\nslice {{r.id}} \"{{r.name}}\" 1";
        let symbols = server(source).symbols(&json!({
            "textDocument": { "uri": "file:///t.bd" }
        }));
        let names: Vec<&str> = symbols
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|s| s["name"].as_str())
            .collect();
        assert!(names.contains(&"{{quarter}}"), "{names:?}");
        assert!(names.contains(&"{{r.id}}"), "{names:?}");
    }

    #[test]
    fn signature_help_names_the_slot_you_are_typing() {
        // The idiomatic form is positional, and it was the one thing nothing
        // could tell you the shape of while you were writing it.
        let source = "type packet\nfield sport \"Source port\" 0 ";
        let help = server(source).signature(&at(source, source.len()));

        let sig = &help["signatures"][0];
        assert_eq!(
            sig["label"].as_str().unwrap(),
            "field <id> [\"label\"] <bits> <bits>"
        );
        // Two values written after the label, so the cursor is on the second.
        assert_eq!(help["activeParameter"].as_u64().unwrap(), 3);

        // Nothing to say where there is no positional shape.
        let plain = "type class\nclass a \"A\" ";
        assert!(server(plain).signature(&at(plain, plain.len())).is_null());
    }

    #[test]
    fn every_colour_gets_a_swatch() {
        // `#` is only ever a colour (`D-004`), which is what makes a lexical
        // scan of the file safe.
        let source = "graph { background: #0b1120 }\nnode a { fill: #38bdf880 }\n// #not-a-color-here\n";
        let colours = server(source).colours(&json!({
            "textDocument": { "uri": "file:///t.bd" }
        }));
        let list = colours.as_array().unwrap();
        // Two colours. `#not-a-color-here` has no hex digits after the `#`
        // and yields nothing, which is what keeps the scan safe.
        assert_eq!(list.len(), 2, "{colours}");

        // The alpha channel is read, not assumed.
        assert_eq!(list[1]["color"]["alpha"].as_f64().unwrap(), 128.0 / 255.0);
        assert_eq!(list[0]["color"]["alpha"].as_f64().unwrap(), 1.0);
    }

    #[test]
    fn a_colour_comes_back_as_the_shortest_hex_that_says_it() {
        // Alpha defaults to opaque (`D-016`), so an opaque colour does not
        // need to write it.
        let opaque = colour_presentation(&json!({
            "color": { "red": 0.0, "green": 0.0, "blue": 1.0, "alpha": 1.0 }
        }));
        assert_eq!(opaque[0]["label"].as_str().unwrap(), "#0000ff");

        let translucent = colour_presentation(&json!({
            "color": { "red": 1.0, "green": 1.0, "blue": 1.0, "alpha": 0.5 }
        }));
        assert_eq!(translucent[0]["label"].as_str().unwrap(), "#ffffff80");
    }

    #[test]
    fn highlighting_an_id_finds_it_everywhere_and_only_whole() {
        // `a` must not light up inside `api`.
        let source = "node a \"A\"\nnode api \"API\"\na -> api\n";
        let hits = server(source).highlights(&at(source, 5));
        assert_eq!(hits.as_array().unwrap().len(), 2, "{hits}");
    }

    #[test]
    fn hover_on_an_archetypes_own_word_gives_its_form() {
        // The word you are least likely to be sure about, and it said nothing.
        let source = "type pie\nslice a \"A\" 10";
        let md = server(source).hover(&at(source, 10))["contents"]["value"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(md.contains("slice <id> [\"label\"] <value>"), "{md}");
        assert!(md.contains("type pie"), "{md}");
    }

    #[test]
    fn hover_on_a_node_shows_the_paint_the_cascade_settled_on() {
        // Four layers deep, and the answer used to be visible only by
        // rendering the file.
        let source = "style node { fill: #123456 }\nnode a \"A\" { stroke: #abcdef 2 }";
        let on_a = source.find("node a").unwrap() + 5;
        let md = server(source).hover(&at(source, on_a))["contents"]["value"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        assert!(md.contains("#123456"), "the inherited fill: {md}");
        assert!(md.contains("#abcdef"), "the declared stroke: {md}");
    }

    #[test]
    fn a_completion_writes_the_whole_statement() {
        // A list of words leaves you to type the rest from memory; a list of
        // statements does not.
        let items = server("type pie\n").items_for("type pie\n", "", 9);
        let slice = items
            .iter()
            .find(|i| i["label"] == "slice")
            .unwrap_or_else(|| panic!("{items:?}"));
        assert_eq!(slice["insertTextFormat"].as_u64().unwrap(), 2, "a snippet");
        assert!(
            slice["insertText"]
                .as_str()
                .unwrap()
                .starts_with("slice ${1:id} \"${2:label}\""),
            "{slice}"
        );
        assert!(
            slice["insertText"].as_str().unwrap().contains("value"),
            "the slot is named in the snippet: {slice}"
        );
    }

    #[test]
    fn hover_on_a_property_key_documents_it() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "node a { shape: rect }";
        let params = json!({
            "textDocument": { "uri": "file:///t.bd" },
            "position": { "line": 0, "character": 10 },
        });

        let mut server = server;
        server.docs.insert("file:///t.bd".into(), text.into());
        let hover = server.hover(&params);
        let value = hover["contents"]["value"].as_str().unwrap_or_default();
        assert!(value.contains("Outline geometry"), "{value}");
    }

    fn server_with(text: &str) -> Server {
        let mut server = Server::default();
        server.docs.insert("file:///t.bd".into(), text.into());
        server
    }

    fn labels_at(text: &str, line: &str) -> Vec<String> {
        server_with(text)
            .items_for(text, line, text.len())
            .iter()
            .filter_map(|i| i["label"].as_str().map(String::from))
            .collect()
    }

    #[test]
    fn completion_stays_out_of_labels_and_comments() {
        // Typing prose is exactly when a popup is in the way.
        assert!(labels_at("node a \"Cli", "node a \"Cli").is_empty());
        assert!(labels_at("// note: sha", "// note: sha").is_empty());
        assert!(labels_at("node a { fill: #fff } // sh", "node a { fill: #fff } // sh").is_empty());
    }

    #[test]
    fn a_dot_only_opens_styles_where_a_class_can_go() {
        let text = "style dark { fill: #000 }\n";

        // A decimal is not a class reference.
        let line = "node a { opacity: 0.";
        assert!(labels_at(&format!("{text}{line}"), line).is_empty());

        // Neither is a dotted arrow.
        let line = "a ..";
        assert!(!labels_at(&format!("{text}{line}"), line).contains(&"dark".to_string()));

        let line = "node a .";
        assert!(labels_at(&format!("{text}{line}"), line).contains(&"dark".to_string()));
    }

    #[test]
    fn aliases_are_hidden_from_completion() {
        let labels = labels_at("node a {\n  ", "  ");
        assert!(labels.contains(&"stroke".to_string()));
        assert!(!labels.contains(&"border".to_string()), "{labels:?}");
        assert!(!labels.contains(&"color".to_string()));
        assert!(!labels.contains(&"text-color".to_string()));
    }

    #[test]
    fn statement_level_stops_suggesting_once_a_word_is_finished() {
        // Trailing space used to re-open the keyword list on every word.
        assert!(
            labels_at("node alpha ", "node alpha ")
                .iter()
                .all(|l| l != "graph")
        );
        assert!(labels_at("n", "n").contains(&"node".to_string()));
    }

    #[test]
    fn archetype_names_complete_after_type() {
        let labels = labels_at("type ", "type ");
        assert!(labels.contains(&"pie".to_string()));
        assert!(labels.contains(&"packet".to_string()));
        assert!(labels.contains(&"free".to_string()));
    }

    #[test]
    fn a_type_block_completes_that_archetypes_settings() {
        let text = "type pie {\n  ";
        let labels = labels_at(text, "  ");
        assert!(labels.contains(&"donut".to_string()));
        assert!(!labels.contains(&"padding".to_string()), "{labels:?}");

        let text = "type packet {\n  ";
        let labels = labels_at(text, "  ");
        assert!(labels.contains(&"ruler".to_string()));
        assert!(!labels.contains(&"donut".to_string()));
    }

    #[test]
    fn chart_data_is_offered_only_where_it_is_read() {
        let labels = labels_at("type pie\nnode a {\n  ", "  ");
        assert!(labels.contains(&"value".to_string()));
        assert!(!labels.contains(&"bits".to_string()));

        let labels = labels_at("type packet\nnode a {\n  ", "  ");
        assert!(labels.contains(&"bits".to_string()));
        assert!(!labels.contains(&"value".to_string()));

        let labels = labels_at("node a {\n  ", "  ");
        assert!(!labels.contains(&"value".to_string()));
        assert!(!labels.contains(&"bits".to_string()));
    }

    #[test]
    fn placements_are_not_offered_where_placement_is_ignored() {
        assert!(labels_at("node a ", "node a ").contains(&"right-of".to_string()));
        assert!(!labels_at("type pie\nnode a ", "node a ").contains(&"right-of".to_string()));
    }

    #[test]
    fn declared_archetype_defaults_to_free() {
        assert_eq!(declared_archetype("node a\n"), "free");
        assert_eq!(declared_archetype("type packet\nnode a\n"), "packet");
        assert_eq!(declared_archetype("type pie {\n}\n"), "pie");
        // `typed` is not `type`.
        assert_eq!(declared_archetype("node typed\n"), "free");
    }

    #[test]
    fn hover_documents_an_archetype() {
        let text = "type packet";
        let server = server_with(text);
        let params = json!({
            "textDocument": { "uri": "file:///t.bd" },
            "position": { "line": 0, "character": 7 },
        });
        let hover = server.hover(&params);
        let value = hover["contents"]["value"].as_str().unwrap_or_default();
        assert!(value.contains("**type** `packet`"), "{value}");
    }

    #[test]
    fn completion_offers_the_archetypes_own_keyword() {
        let labels = labels_at("type pie\ns", "s");
        assert_eq!(
            labels.first().map(String::as_str),
            Some("slice"),
            "{labels:?}"
        );
        assert!(labels.contains(&"node".to_string()), "node stays available");

        let labels = labels_at("type packet\nf", "f");
        assert_eq!(labels.first().map(String::as_str), Some("field"));

        // `free` has no second word to offer.
        let labels = labels_at("n", "n");
        assert_eq!(
            labels.iter().filter(|l| *l == "node").count(),
            1,
            "{labels:?}"
        );
    }

    #[test]
    fn references_are_found_under_either_keyword() {
        // In an archetype with its own word, both spellings declare items, so
        // both must be offered as references.
        let text = "type pie\nslice alpha \"A\" 1\nnode beta \"B\" { value: 2 }\n";
        let found: Vec<String> = declared_items(text, "slice")
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        assert!(found.contains(&"alpha".to_string()), "{found:?}");
        assert!(found.contains(&"beta".to_string()), "{found:?}");
    }

    #[test]
    fn placements_follow_the_archetypes_keyword() {
        assert!(labels_at("node a ", "node a ").contains(&"right-of".to_string()));
        // A pie ignores placement, so it is not offered — with either word.
        assert!(!labels_at("type pie\nslice a ", "slice a ").contains(&"right-of".to_string()));
    }
}

#[cfg(test)]
mod block_vocabulary_tests {
    use super::*;

    fn words_after_kind(source: &str) -> Vec<String> {
        let server = Server {
            docs: HashMap::new(),
        };
        server
            .items_for(source, "  kind: ", source.len())
            .iter()
            .filter_map(|i| i["label"].as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn kind_completes_the_declared_archetypes_words_and_not_the_others() {
        // The whole reason an archetype may refine a shared property: one word
        // means different things in two genres, and offering both would be
        // exactly the noise that made autocompletion get in the way.
        let machine = words_after_kind("type state\nstate a \"A\" {\n  kind: ");
        assert!(machine.contains(&"initial".to_string()), "{machine:?}");
        assert!(!machine.contains(&"person".to_string()), "{machine:?}");

        let architecture = words_after_kind("type block\nblock a \"A\" {\n  kind: ");
        assert!(architecture.contains(&"person".to_string()), "{architecture:?}");
        assert!(!architecture.contains(&"initial".to_string()), "{architecture:?}");
    }

    #[test]
    fn a_block_only_property_stays_out_of_other_genres() {
        let server = Server {
            docs: HashMap::new(),
        };
        let names = |source: &str, line: &str| -> Vec<String> {
            server
                .items_for(source, line, source.len())
                .iter()
                .filter_map(|i| i["label"].as_str().map(str::to_string))
                .collect()
        };

        let architecture = names("type block\nblock a \"A\" {\n  ", "  ");
        assert!(architecture.contains(&"tech".to_string()), "{architecture:?}");

        let free = names("node a \"A\" {\n  ", "  ");
        assert!(!free.contains(&"tech".to_string()), "{free:?}");
        // The shared vocabulary is still all there.
        assert!(free.contains(&"fill".to_string()), "{free:?}");
    }
}

#[cfg(test)]
mod archetype_completion_tests {
    use super::*;

    fn offered(source: &str, line: &str) -> Vec<String> {
        let server = Server {
            docs: HashMap::new(),
        };
        server
            .items_for(source, line, source.len())
            .iter()
            .filter_map(|i| i["label"].as_str().map(str::to_string))
            .collect()
    }

    #[test]
    fn each_genre_offers_its_own_data_and_no_one_elses() {
        // The filter this replaces knew two properties out of thirteen, and
        // had one of them wrong: it hid `value` in a treemap, where it is
        // required.
        for (source, item, wanted, unwanted) in [
            ("type treemap\nbox a \"A\" {\n  ", "box", "value", "wave"),
            ("type pie\nslice a \"A\" {\n  ", "slice", "value", "bits"),
            ("type wave\nsignal s \"S\" {\n  ", "signal", "wave", "value"),
            ("type quadrant\npoint p \"P\" {\n  ", "point", "x", "points"),
            ("type gantt\ntask t \"T\" {\n  ", "task", "start", "value"),
            ("type packet\nfield f \"F\" {\n  ", "field", "bits", "start"),
            ("type bar\nseries s {\n  ", "series", "points", "wave"),
            ("type timeline\nevent e \"E\" {\n  ", "event", "date", "start"),
            ("type block\nblock b \"B\" {\n  ", "block", "tech", "wave"),
        ] {
            let names = offered(source, "  ");
            assert!(
                names.iter().any(|n| n == wanted),
                "`{item}` should offer `{wanted}`: {names:?}"
            );
            assert!(
                !names.iter().any(|n| n == unwanted),
                "`{item}` should not offer `{unwanted}`"
            );
            // The shared vocabulary is available everywhere, as it should be.
            assert!(names.iter().any(|n| n == "fill"), "`{item}` lost `fill`");
        }
    }

    #[test]
    fn every_archetype_completes_after_type_and_offers_its_own_keyword() {
        let names = offered("type ", "type ");
        for def in props::ARCHETYPES {
            assert!(
                names.iter().any(|n| n == def.name),
                "`type` completion is missing `{}`: {names:?}",
                def.name
            );
        }

        // And inside a file, the archetype's own word leads a statement.
        for def in props::ARCHETYPES {
            let source = format!("type {}\n", def.name);
            let offered = offered(&source, "");
            assert!(
                offered.iter().any(|n| n == def.item),
                "`type {}` should offer `{}`: {offered:?}",
                def.name,
                def.item
            );
        }
    }

    #[test]
    fn an_item_block_is_not_mistaken_for_an_edge_block() {
        // Every archetype names its elements its own way, and the block owner
        // used to be matched against a list of five words — so `slice`, `box`,
        // `signal`, `task`, `field`, `series`, `event`, `entity`, `point` and
        // the rest all fell through to "edge", losing `fill`, `shape` and
        // `size` and gaining `arrow` and `router`.
        for def in props::ARCHETYPES {
            let source = format!("type {}\n{} a \"A\" {{\n  ", def.name, def.item);
            let names = offered(&source, "  ");
            assert!(
                names.iter().any(|n| n == "shape"),
                "`{} {{ … }}` should offer node properties: {names:?}",
                def.item
            );
            assert!(
                !names.iter().any(|n| n == "router"),
                "`{} {{ … }}` should not offer edge properties",
                def.item
            );
        }

        // And an edge still reads as one, label or no label.
        for head in [
            "a -> b {\n  ",
            "a -> b \"calls\" {\n  ",
            "a <-> b \"a -> b\" {\n  ",
            "a ..> b {\n  ",
        ] {
            let names = offered(head, "  ");
            assert!(
                names.iter().any(|n| n == "router"),
                "`{head}` should offer edge properties: {names:?}"
            );
        }

        // A label holding an arrow does not make an item into an edge.
        let names = offered("node a \"from -> to\" {\n  ", "  ");
        assert!(names.iter().any(|n| n == "shape"), "{names:?}");
        assert!(!names.iter().any(|n| n == "router"), "{names:?}");
    }

    #[test]
    fn hover_documents_a_property_in_the_genre_that_reads_it() {
        let server = Server {
            docs: HashMap::new(),
        };
        let text = "type timeline\nevent e \"E\" {\n  date: 2026-03-01\n}\n";
        let mut server = server;
        server.docs.insert("file:///t.bd".into(), text.into());
        let hover = server.hover(&json!({
            "textDocument": { "uri": "file:///t.bd" },
            "position": { "line": 2, "character": 3 },
        }));
        let value = hover["contents"]["value"].as_str().unwrap_or_default();
        assert!(value.contains("date: <date>"), "{value}");
        assert!(value.contains("When the event happened"), "{value}");
    }
}
