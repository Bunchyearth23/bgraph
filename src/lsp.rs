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
                .map(|(name, _)| item(&name, 7, "style", &format!("Apply the `{name}` style.")))
                .collect();
        }

        if let Some(block) = ctx.block {
            let table = match block {
                Block::Graph => props::GRAPH_PROPS,
                Block::Type => props::archetype(archetype).map_or(&[][..], |a| a.props),
                _ => props::PROPS,
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

                return props::lookup(&key, table)
                    .map(|def| {
                        def.words
                            .iter()
                            .map(|w| item(w, 20, def.syntax, def.doc))
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
                .filter(|p| relevant_here(p.name, archetype))
                .enumerate()
                .map(|(i, p)| {
                    let mut it = item(p.name, 10, p.syntax, p.doc);
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

        if preceding == "type" {
            return props::ARCHETYPES
                .iter()
                .map(|a| item(a.name, 20, "archetype", a.doc))
                .collect();
        }

        if is_rel_dir(preceding) {
            return declared(text, "node")
                .into_iter()
                .map(|(name, label)| item(&name, 6, "node", &label))
                .collect();
        }

        if preceding == "align" {
            return props::ALIGNMENTS
                .iter()
                .map(|a| item(a, 20, "align", "Cross-axis alignment against the anchor."))
                .collect();
        }

        if preceding == "extends" {
            return declared(text, "style")
                .into_iter()
                .map(|(name, _)| item(&name, 7, "style", &format!("Inherit from `{name}`.")))
                .collect();
        }

        // Placements only make sense where placement is honoured, and only
        // once the node already has an identifier.
        if trimmed.starts_with("node ") && words.len() >= 2 && archetype == "free" {
            return props::PLACEMENTS
                .iter()
                .map(|(name, doc)| item(name, 14, "placement", doc))
                .collect();
        }

        // The first word of a statement: a keyword, or a node id starting an edge.
        if words.len() <= 1 && !ends_with_space {
            let mut out: Vec<Value> = props::KEYWORDS
                .iter()
                .map(|(name, doc)| item(name, 14, "keyword", doc))
                .collect();
            if archetype == "free" && !current.is_empty() {
                out.extend(
                    declared(text, "node")
                        .into_iter()
                        .map(|(name, label)| item(&name, 6, "node", &label)),
                );
            }
            return out;
        }

        Vec::new()
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

        let markdown = if text[line_start..span.start].trim_end().ends_with("type") {
            props::archetype(&word).map(|a| format!("**type** `{}`\n\n{}", a.name, a.doc))
        } else if is_key && ctx.block.is_some() {
            let table = match ctx.block {
                Some(Block::Graph) => props::GRAPH_PROPS,
                Some(Block::Type) => props::archetype(archetype).map_or(&[][..], |a| a.props),
                _ => props::PROPS,
            };
            props::lookup(&word, table).map(|d| format!("```\n{}\n```\n\n{}", d.syntax, d.doc))
        } else if text[line_start..span.start].trim_end().ends_with('.')
            || text[line_start..span.start].trim_end().ends_with("extends")
        {
            self.describe_style(text, &word)
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

        // Resolved geometry is the thing you actually want when hovering.
        if let Ok(layout) = crate::layout::solve(&diagram)
            && let Some(placed) = layout.nodes.get(i)
        {
            let r = placed.rect;
            out.push_str(&format!(
                "\n\n```\nx {:.0}  y {:.0}   {:.0} × {:.0}\n```",
                r.x, r.y, r.w, r.h
            ));
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
            match text[line_start..brace].split_whitespace().next() {
                Some("type") => Block::Type,
                Some("graph") => Block::Graph,
                Some("style") => Block::Style,
                Some("node") => Block::Node,
                _ => Block::Edge,
            }
        });

        Context {
            in_string,
            in_comment,
            block,
        }
    }
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

/// Chart data is noise in an archetype that does not read it.
fn relevant_here(name: &str, archetype: &str) -> bool {
    match name {
        "value" => archetype == "pie",
        "bits" => archetype == "packet",
        _ => true,
    }
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

/// Every `node <id>` / `style <id>` in the file, found by line scan so it
/// survives a syntax error elsewhere.
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

fn item(label: &str, kind: u8, detail: &str, doc: &str) -> Value {
    json!({
        "label": label,
        "kind": kind,
        "detail": detail,
        "documentation": { "kind": "markdown", "value": doc },
    })
}

fn capabilities() -> Value {
    json!({
        "capabilities": {
            "textDocumentSync": 1,               // full document on every change
            "completionProvider": {
                // Space is a trigger again, but `items_for` now answers with
                // nothing wherever a suggestion would be in the way — the
                // quiet comes from the answer, not from refusing to be asked.
                "triggerCharacters": [".", ":", " "],
            },
            "hoverProvider": true,
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
}
