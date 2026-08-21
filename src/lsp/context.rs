use crate::ast::Span;
use crate::props;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Block {
    Type,
    Graph,
    Style(Option<&'static str>),
    Node,
    Container,
    Edge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Context {
    pub(super) in_string: bool,
    pub(super) in_comment: bool,
    pub(super) block: Option<Block>,
}

impl Context {
    pub(super) fn at(text: &str, offset: usize) -> Context {
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
                Some("group") | Some("activate") | Some("component") => Block::Container,
                Some("style") => Block::Style(
                    head.split_whitespace()
                        .nth(1)
                        .and_then(props::theme_style)
                        .map(|t| t.name),
                ),
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
