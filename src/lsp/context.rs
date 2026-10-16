use crate::ast::Span;
use crate::props;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Block {
    Type,
    Graph,
    Style(Option<&'static str>),
    Layout,
    Node,
    Container,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Context {
    pub(super) in_string: bool,
    pub(super) in_comment: bool,
    pub(super) block: Option<Block>,
    pub(super) block_start: Option<usize>,
    pub(super) statement_start: usize,
    pub(super) scope_start: Option<usize>,
}

impl Context {
    pub(super) fn at(text: &str, offset: usize) -> Context {
        let mut stack = Vec::new();
        let mut chars = text[..offset.min(text.len())].char_indices().peekable();
        let mut in_string = false;
        let mut in_comment = false;
        let mut statement_start = 0;
        let mut parentheses = 0usize;
        while let Some((i, c)) = chars.next() {
            match c {
                '\\' if in_string => {
                    chars.next();
                }
                '"' => in_string = !in_string,
                '/' if !in_string && matches!(chars.peek(), Some((_, '/'))) => {
                    in_comment = true;
                    for (j, c) in chars.by_ref() {
                        if c == '\n' {
                            in_comment = false;
                            statement_start = j + 1;
                            parentheses = 0;
                            break;
                        }
                    }
                }
                '{' if !in_string && matches!(chars.peek(), Some((_, '{'))) => {
                    chars.next();
                    while let Some((_, ch)) = chars.next() {
                        if ch == '}' && matches!(chars.peek(), Some((_, '}'))) {
                            chars.next();
                            break;
                        }
                    }
                }
                '{' if !in_string => {
                    let head = &text[statement_start..i];
                    let block = match head.split_whitespace().next() {
                        Some("type") => Block::Type,
                        Some("graph") => Block::Graph,
                        Some("layout") => Block::Layout,
                        Some("group" | "activate" | "component") => Block::Container,
                        Some("style") => {
                            Block::Style(head.split_whitespace().nth(1).and_then(|name| {
                                props::archetype(declared_archetype(text))
                                    .and_then(|def| props::theme_target(def, name))
                                    .map(|target| target.name())
                            }))
                        }
                        _ if declares_an_edge(head) => Block::Edge,
                        _ => Block::Node,
                    };
                    stack.push((i, block));
                    statement_start = i + 1;
                }
                '}' if !in_string => {
                    stack.pop();
                    statement_start = i + 1;
                    parentheses = 0;
                }
                '(' if !in_string => parentheses += 1,
                ')' if !in_string => parentheses = parentheses.saturating_sub(1),
                '\n' if !in_string => {
                    statement_start = i + 1;
                    parentheses = 0;
                }
                ';' | ',' if !in_string && parentheses == 0 => statement_start = i + 1,
                _ => {}
            }
        }
        let block = stack.last().map(|(_, block)| *block);
        let scope_start = stack
            .iter()
            .rev()
            .find_map(|(brace, block)| (*block == Block::Container).then_some(*brace));
        Context {
            in_string,
            in_comment,
            block,
            block_start: stack.last().map(|(brace, _)| *brace),
            statement_start,
            scope_start,
        }
    }
}

pub(super) fn declares_an_edge(head: &str) -> bool {
    const ARROWS: [&str; 7] = ["<->", "<-", "-->", "..>", "--", "->", ".."];
    let mut rest = head;
    while !rest.is_empty() {
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

pub(super) fn declared_archetype(text: &str) -> &str {
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

pub(super) fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

pub(super) fn is_rel_dir(word: &str) -> bool {
    matches!(word, "right-of" | "left-of" | "above" | "below")
}

pub(super) fn word_at(text: &str, offset: usize) -> Option<(String, Span)> {
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
    (start != end).then(|| (text[start..end].to_string(), Span { start, end }))
}

pub(super) fn item_words(archetype: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = props::archetype(archetype)
        .map(|a| a.items().collect())
        .unwrap_or_default();
    if !out.contains(&"node") {
        out.push("node");
    }
    out.push("group");
    out
}

pub(super) fn declared_items(text: &str, archetype: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for word in item_words(archetype) {
        out.extend(declared(text, word));
    }
    out
}

pub(super) fn declared(text: &str, keyword: &str) -> Vec<(String, String)> {
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

pub(super) fn declaration_span(text: &str, keyword: &str, name: &str) -> Option<Span> {
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

pub(super) fn text_of(text: &str, span: Span) -> &str {
    &text[span.start.min(text.len())..span.end.min(text.len())]
}
