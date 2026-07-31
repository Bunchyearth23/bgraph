//! Templating: a `.bd` file with `{{holes}}` in it, and the values a calling
//! program binds to them.
//!
//! This is the shape `bgraph` is meant to be embedded in — a document
//! generator holds the diagram's *shape* as an authored file and supplies only
//! what varies. Substitution happens on the source text, before parsing, so a
//! bound value can be anything the language accepts: a number, a label, a
//! whole list of series points.
//!
//! Substitution is context-aware rather than blind. A value landing inside a
//! string literal is escaped for one; a value landing in a value position is
//! quoted if it is text. Blind replacement would let a stray quote turn a
//! label into a syntax error, or worse, into different statements.

use std::collections::HashMap;
use std::fmt::Write;

use anyhow::{Result, bail};

/// What a program binds to a hole.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
    /// Several values in one hole — a series, a bit range, a date and a
    /// duration. Rendered space-separated, which is how the language reads
    /// every list it has.
    List(Vec<Value>),
    /// Inserted verbatim. The escape hatch, and the only way to inject
    /// syntax rather than data.
    Raw(String),
}

impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::Text(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::Text(s)
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Value {
        Value::Number(n)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Value {
        Value::Number(n as f64)
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(items: Vec<T>) -> Value {
        Value::List(items.into_iter().map(Into::into).collect())
    }
}

/// The values a render is given, by hole name.
#[derive(Debug, Clone, Default)]
pub struct Bindings(HashMap<String, Value>);

impl Bindings {
    pub fn new() -> Bindings {
        Bindings::default()
    }

    pub fn set(mut self, name: impl Into<String>, value: impl Into<Value>) -> Bindings {
        self.0.insert(name.into(), value.into());
        self
    }

    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<Value>) {
        self.0.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// A parsed template: the source, and the holes found in it.
#[derive(Debug, Clone)]
pub struct Template {
    source: String,
    holes: Vec<String>,
}

impl Template {
    /// Reads a template and checks the shape around the holes. Filling them
    /// with a plausible value and parsing that catches a structural mistake
    /// now rather than at the first render, in production, with a customer's
    /// data in hand.
    pub fn parse(source: &str) -> Result<Template> {
        let holes = scan(source);

        // No single probe fits every position: `0` is a number but not an
        // identifier, `_probe` the reverse. A template is well-formed if
        // *some* plausible binding parses, so try both before complaining.
        let mut failure = None;
        for probe in [Value::Number(0.0), Value::Raw("_probe".into())] {
            let mut values = Bindings::new();
            for name in &holes {
                values.insert(name.clone(), probe.clone());
            }
            match crate::parser::parse(&fill(source, &values)?) {
                Ok(_) => {
                    failure = None;
                    break;
                }
                Err(e) => failure = Some(e),
            }
        }
        if let Some(e) = failure {
            bail!("the template does not parse once its holes are filled: {e}");
        }

        Ok(Template {
            source: source.to_string(),
            holes,
        })
    }

    /// Every hole in the template, in the order they appear, without repeats.
    pub fn holes(&self) -> &[String] {
        &self.holes
    }

    /// Fills the holes and renders. An unbound hole is an error naming it —
    /// silently rendering a blank would put a wrong figure in a document.
    pub fn render(&self, values: &Bindings) -> Result<String> {
        let missing: Vec<&str> = self
            .holes
            .iter()
            .filter(|h| values.get(h).is_none())
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            bail!("nothing bound to {}", quoted_list(&missing));
        }

        crate::render_source(&fill(&self.source, values)?)
    }

    /// The filled source, without rendering — for inspecting what a binding
    /// produced, and what the CLI's `--emit` prints.
    pub fn fill(&self, values: &Bindings) -> Result<String> {
        fill(&self.source, values)
    }
}

/// Template text to SVG in one call.
pub fn render(source: &str, values: &Bindings) -> Result<String> {
    Template::parse(source)?.render(values)
}

/// Hole names, in order of first appearance.
fn scan(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = source;

    while let Some(open) = rest.find("{{") {
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else { break };
        let name = after[..close].trim().to_string();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
        rest = &after[close + 2..];
    }

    names
}

/// Replaces every hole, deciding how by where it sits.
fn fill(source: &str, values: &Bindings) -> Result<String> {
    let mut out = String::with_capacity(source.len());
    let mut rest = source;
    // Tracks whether the text so far leaves us inside a string literal, so a
    // bound value can be escaped for the context it lands in.
    let mut in_string = false;

    while let Some(open) = rest.find("{{") {
        let (before, after) = rest.split_at(open);
        out.push_str(before);
        in_string = string_state(before, in_string);

        let body = &after[2..];
        let Some(close) = body.find("}}") else {
            // An unterminated `{{` is text, not a hole.
            out.push_str(after);
            return Ok(out);
        };

        let name = body[..close].trim();
        match values.get(name) {
            Some(value) => write_value(&mut out, value, in_string),
            // `Template::render` reports unbound holes by name; reaching here
            // means a caller used `fill` directly, so leave it as written.
            None => {
                let _ = write!(out, "{{{{{name}}}}}");
            }
        }

        rest = &body[close + 2..];
    }

    out.push_str(rest);
    Ok(out)
}

/// Whether `text` leaves the reader inside a string literal.
fn string_state(text: &str, mut inside: bool) -> bool {
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if inside => {
                chars.next();
            }
            '"' => inside = !inside,
            '/' if !inside && chars.peek() == Some(&'/') => {
                // A comment runs to the end of the line and holds no strings.
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    inside
}

fn write_value(out: &mut String, value: &Value, in_string: bool) {
    match value {
        Value::Raw(text) => out.push_str(text),
        Value::Number(n) => {
            let text = if *n == n.trunc() && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{n}")
            };
            out.push_str(&text);
        }
        Value::Text(text) if in_string => out.push_str(&escape(text)),
        // In a value position a bare word would have to be an identifier, so
        // text is quoted: `slice a {{name}}` must not become two statements.
        Value::Text(text) => {
            let _ = write!(out, "\"{}\"", escape(text));
        }
        Value::List(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                write_value(out, item, in_string);
            }
        }
    }
}

fn escape(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn quoted_list(names: &[&str]) -> String {
    names
        .iter()
        .map(|n| format!("`{n}`"))
        .collect::<Vec<_>>()
        .join(", ")
}
