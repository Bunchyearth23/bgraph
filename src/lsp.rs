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

    fn items_for(&self, text: &str, line: &str, offset: usize) -> Vec<Value> {
        let trimmed = line.trim_start();

        // `.` opens a style reference, wherever it appears.
        if let Some(dot) = line.rfind('.')
            && line[dot + 1..].chars().all(is_ident_char)
        {
            return declared(text, "style")
                .into_iter()
                .map(|(name, _)| item(&name, 7, "style", &format!("Apply the `{name}` style.")))
                .collect();
        }

        if let Some(kind) = enclosing_block(text, offset) {
            let graph = kind == Block::Graph;

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

                if let Some(def) = props::lookup(&key, graph) {
                    return def
                        .words
                        .iter()
                        .map(|w| item(w, 20, def.syntax, def.doc))
                        .collect();
                }
                return Vec::new();
            }

            let ctx = match kind {
                Block::Graph => None,
                Block::Node => Some(Ctx::Node),
                Block::Edge => Some(Ctx::Edge),
                Block::Style => None,
            };
            let table = if graph {
                props::GRAPH_PROPS
            } else {
                props::PROPS
            };

            return table
                .iter()
                .filter(|p| ctx.is_none_or(|c| p.ctx.allows(c)))
                .map(|p| {
                    let mut it = item(p.name, 10, p.syntax, p.doc);
                    it["insertText"] = json!(format!("{}: ", p.name));
                    it
                })
                .collect();
        }

        // Statement level.
        let words: Vec<&str> = trimmed.split_whitespace().collect();
        let last = words.last().copied().unwrap_or("");
        let ends_with_space = line.ends_with(char::is_whitespace);
        let anchor_position = matches!(last, "right-of" | "left-of" | "above" | "below")
            || (ends_with_space && words.len() >= 2 && is_rel_dir(words[words.len() - 1]));

        if anchor_position || (words.len() >= 2 && is_rel_dir(words[words.len() - 2])) {
            return declared(text, "node")
                .into_iter()
                .map(|(name, label)| item(&name, 6, "node", &label))
                .collect();
        }

        if trimmed.starts_with("align") {
            return props::ALIGNMENTS
                .iter()
                .map(|a| item(a, 20, "align", "Cross-axis alignment against the anchor."))
                .collect();
        }

        // A node statement that already has an id accepts placements next.
        if trimmed.starts_with("node ") && words.len() >= 2 {
            return props::PLACEMENTS
                .iter()
                .map(|(name, doc)| item(name, 14, "placement", doc))
                .collect();
        }

        if words.len() <= 1 {
            let mut out: Vec<Value> = props::KEYWORDS
                .iter()
                .map(|(name, doc)| item(name, 14, "keyword", doc))
                .collect();
            // An edge starts with a node name, so offer those too.
            out.extend(
                declared(text, "node")
                    .into_iter()
                    .map(|(name, label)| item(&name, 6, "node", &label)),
            );
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
        let in_block = enclosing_block(text, offset);

        let markdown = if is_key && in_block.is_some() {
            let graph = in_block == Some(Block::Graph);
            props::lookup(&word, graph).map(|d| format!("```\n{}\n```\n\n{}", d.syntax, d.doc))
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
    Graph,
    Style,
    Node,
    Edge,
}

/// Which kind of statement owns the innermost unclosed `{` before `offset`.
/// Purely lexical, so it keeps working while the document is unparseable.
fn enclosing_block(text: &str, offset: usize) -> Option<Block> {
    let mut stack = Vec::new();
    let mut chars = text[..offset.min(text.len())].char_indices().peekable();
    let mut in_string = false;

    while let Some((i, c)) = chars.next() {
        match c {
            '"' => in_string = !in_string,
            '/' if !in_string && matches!(chars.peek(), Some((_, '/'))) => {
                // Skip to end of line.
                for (_, c) in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            '{' if !in_string => stack.push(i),
            '}' if !in_string => {
                stack.pop();
            }
            _ => {}
        }
    }

    let brace = stack.pop()?;
    let line_start = text[..brace].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line = &text[line_start..brace];
    let first = line.split_whitespace().next().unwrap_or("");

    Some(match first {
        "graph" => Block::Graph,
        "style" => Block::Style,
        "node" => Block::Node,
        _ => Block::Edge,
    })
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
        assert_eq!(enclosing_block(text, text.len()), Some(Block::Graph));

        let text = "style x {\n  fill";
        assert_eq!(enclosing_block(text, text.len()), Some(Block::Style));

        let text = "node a \"A\" {\n  fill";
        assert_eq!(enclosing_block(text, text.len()), Some(Block::Node));

        let text = "a -> b {\n  rou";
        assert_eq!(enclosing_block(text, text.len()), Some(Block::Edge));

        let text = "graph { padding: 4 }\nnode a";
        assert_eq!(enclosing_block(text, text.len()), None);
    }

    #[test]
    fn braces_inside_strings_and_comments_are_ignored() {
        let text = "node a \"{\"\nnode b";
        assert_eq!(enclosing_block(text, text.len()), None);

        let text = "// a { comment\nnode b";
        assert_eq!(enclosing_block(text, text.len()), None);
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
}
