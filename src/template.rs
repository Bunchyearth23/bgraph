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

/// Whether text can stand as a bare name. The same rule the grammar uses:
/// a leading letter or underscore, then letters, digits, underscores and
/// interior hyphens — a trailing hyphen would make `a-` swallow an arrow.
fn is_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    !text.ends_with('-')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// What a program binds to a hole.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Escaped for wherever it lands: inside a string literal it is escaped
    /// for one, in a value position it is quoted. Hostile text cannot become
    /// a statement.
    Text(String),
    /// Written without a decimal point when it has none, so `4820.0` reaches
    /// the diagram as `4820`.
    Number(f64),
    /// Several values in one hole — a series, a bit range, a date and a
    /// duration. Rendered space-separated, which is how the language reads
    /// every list it has.
    List(Vec<Value>),
    /// Inserted verbatim. The escape hatch, and the only way to inject
    /// syntax rather than data.
    Raw(String),
    /// Several bindings, one per repetition. A *line* holding `{{name.field}}`
    /// is emitted once for each row, which is how a chart gets a number of
    /// slices that is not known when the template is written.
    ///
    /// Deliberately the whole of the repetition this supports: one line, one
    /// list, no nesting and no conditions. Anything more is a template
    /// language, and a much larger promise than this makes.
    Rows(Vec<Bindings>),
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

/// Bindings read from JSON — the shape a program already has its data in.
///
/// This exists so the **binary** can do what the library can. `--set` binds a
/// scalar, which is enough for a title and a year and nothing else: the case
/// templating was built for is a chart whose number of slices is not known
/// when the template is written, and that needs a list or a set of rows.
///
/// The mapping is total and the refusals are deliberate:
///
/// | JSON | becomes |
/// |---|---|
/// | string | [`Value::Text`] — escaped for wherever it lands |
/// | number | [`Value::Number`] |
/// | `true` / `false` | [`Value::Text`], written as it reads |
/// | array of scalars | [`Value::List`] |
/// | array of objects | [`Value::Rows`] |
/// | anything else | an error naming the hole |
///
/// A `null` is refused rather than skipped: a hole nobody filled is exactly
/// the silent no-op this language does not do anywhere else. A mixed array is
/// refused because it is ambiguous — one line repeated, or one line of
/// values? An object nested inside a row is refused because repetition is one
/// line and one list, with no nesting (`D-021`).
#[cfg(feature = "json")]
impl Bindings {
    /// Read a JSON object into bindings. See the table above for the mapping
    /// and for what is refused.
    pub fn from_json(source: &str) -> Result<Bindings> {
        let parsed: serde_json::Value =
            serde_json::from_str(source).map_err(|e| anyhow::anyhow!("{e}"))?;
        let serde_json::Value::Object(map) = parsed else {
            bail!("the data must be an object of hole names, not a bare value");
        };

        let mut out = Bindings::new();
        for (name, value) in map {
            out.insert(name.clone(), from_json_value(&name, &value, true)?);
        }
        Ok(out)
    }
}

#[cfg(feature = "json")]
fn from_json_value(name: &str, value: &serde_json::Value, top: bool) -> Result<Value> {
    use serde_json::Value as J;
    Ok(match value {
        J::String(s) => Value::Text(s.clone()),
        J::Number(n) => Value::Number(
            n.as_f64()
                .ok_or_else(|| anyhow::anyhow!("`{name}` is a number this cannot represent"))?,
        ),
        J::Bool(b) => Value::Text(b.to_string()),
        J::Null => bail!(
            "`{name}` is null — bind a value or leave the hole out, but do not \
             ask for nothing to be written"
        ),
        J::Array(items) if items.is_empty() => Value::List(Vec::new()),
        J::Array(items) => {
            let objects = items.iter().filter(|i| i.is_object()).count();
            if objects == items.len() {
                if !top {
                    bail!("`{name}` nests rows inside rows, and repetition is one line deep");
                }
                let mut rows = Vec::with_capacity(items.len());
                for item in items {
                    let J::Object(fields) = item else { unreachable!() };
                    let mut row = Bindings::new();
                    for (field, value) in fields {
                        row.insert(field.clone(), from_json_value(field, value, false)?);
                    }
                    rows.push(row);
                }
                Value::Rows(rows)
            } else if objects == 0 {
                let mut list = Vec::with_capacity(items.len());
                for item in items {
                    list.push(from_json_value(name, item, false)?);
                }
                Value::List(list)
            } else {
                bail!(
                    "`{name}` mixes objects and plain values — one is a set of rows, \
                     the other a list, and nothing can tell which was meant"
                );
            }
        }
        J::Object(_) => bail!(
            "`{name}` is an object; bind its fields separately, or make it an array \
             of objects to repeat a line"
        ),
    })
}

/// The values a render is given, by hole name.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Bindings(HashMap<String, Value>);

impl Bindings {
    /// Empty bindings.
    pub fn new() -> Bindings {
        Bindings::default()
    }

    /// Binds a value and returns the bindings, for chaining at a call site.
    pub fn set(mut self, name: impl Into<String>, value: impl Into<Value>) -> Bindings {
        self.0.insert(name.into(), value.into());
        self
    }

    /// Binds a value in place. The form to use when building in a loop.
    pub fn insert(&mut self, name: impl Into<String>, value: impl Into<Value>) {
        self.0.insert(name.into(), value.into());
    }

    /// What is bound to `name`, if anything.
    /// Binds a list of rows. A line holding `{{name.field}}` is written once
    /// per row, with `field` read from that row.
    pub fn rows(mut self, name: impl Into<String>, rows: Vec<Bindings>) -> Bindings {
        self.0.insert(name.into(), Value::Rows(rows));
        self
    }

    /// What is bound to `name`, if anything.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    /// Whether nothing is bound.
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
            // A dotted hole is a row hole; probing with a single row is enough
            // to see whether the line it sits on is well-formed.
            let mut probe_rows: HashMap<String, Bindings> = HashMap::new();
            for name in &holes {
                match name.split_once('.') {
                    Some((prefix, field)) => {
                        let row = probe_rows.entry(prefix.to_string()).or_default();
                        row.insert(field.to_string(), probe.clone());
                    }
                    None => values.insert(name.clone(), probe.clone()),
                }
            }
            for (prefix, row) in probe_rows {
                values.insert(prefix, Value::Rows(vec![row]));
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
            .filter(|h| match h.split_once('.') {
                Some((prefix, _)) => !matches!(values.get(prefix), Some(Value::Rows(_))),
                None => values.get(h).is_none(),
            })
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            bail!("nothing bound to {}", quoted_list(&missing));
        }

        // Rows repeat a line; they are not a value. Writing one into a value
        // position is a mistake worth naming rather than quietly dropping.
        let misused: Vec<&str> = self
            .holes
            .iter()
            .filter(|h| !h.contains('.') && matches!(values.get(h), Some(Value::Rows(_))))
            .map(String::as_str)
            .collect();
        if !misused.is_empty() {
            bail!(
                "{} bound to rows but written as a value — write `{{{{{}.field}}}}` \
                 on the line that should repeat",
                quoted_list(&misused),
                misused[0]
            );
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

/// Replaces every hole, a line at a time.
///
/// The line is the unit because it is the unit the language already uses: a
/// statement ends at a newline. So "repeat this" can mean "repeat this line",
/// which needs no block syntax and no way to get the nesting wrong.
fn fill(source: &str, values: &Bindings) -> Result<String> {
    let mut out = String::with_capacity(source.len());
    // Whether the text so far leaves us inside a string literal, so a bound
    // value can be escaped for the context it lands in.
    let mut in_string = false;

    for line in source.split_inclusive('\n') {
        let prefixes = row_prefixes(line, values);
        match prefixes.as_slice() {
            [] => fill_line(&mut out, line, values, None, &mut in_string)?,
            [prefix] => {
                let Some(Value::Rows(rows)) = values.get(prefix) else {
                    unreachable!("`row_prefixes` only returns bound rows")
                };
                // No rows means no line: a chart of nothing draws nothing,
                // which is more useful than a line full of empty holes.
                //
                // The last line of a file carries no newline, so copies of it
                // need one put between them or they run together into one
                // statement.
                let separate = !line.ends_with('\n');
                for (i, row) in rows.iter().enumerate() {
                    if separate && i > 0 {
                        out.push('\n');
                    }
                    fill_line(&mut out, line, values, Some((prefix, row)), &mut in_string)?;
                }
            }
            many => bail!(
                "one line repeats over {} — a line may repeat over one list, \
                 or the number of times it is written would be ambiguous",
                many.iter()
                    .map(|p| format!("`{p}`"))
                    .collect::<Vec<_>>()
                    .join(" and ")
            ),
        }
    }

    Ok(out)
}

/// The row lists this line repeats over: the prefixes of its dotted holes that
/// are actually bound to rows. A dotted name bound to nothing is left alone,
/// so `fill` used directly still round-trips what it cannot resolve.
fn row_prefixes(line: &str, values: &Bindings) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut rest = line;

    while let Some(open) = rest.find("{{") {
        let body = &rest[open + 2..];
        let Some(close) = body.find("}}") else { break };
        if let Some((prefix, _)) = body[..close].trim().split_once('.')
            && matches!(values.get(prefix), Some(Value::Rows(_)))
            && !found.iter().any(|p| p == prefix)
        {
            found.push(prefix.to_string());
        }
        rest = &body[close + 2..];
    }

    found
}

/// One pass over one line, with an optional row in scope.
fn fill_line(
    out: &mut String,
    line: &str,
    values: &Bindings,
    row: Option<(&str, &Bindings)>,
    in_string: &mut bool,
) -> Result<()> {
    let mut rest = line;

    while let Some(open) = rest.find("{{") {
        let (before, after) = rest.split_at(open);
        out.push_str(before);
        *in_string = string_state(before, *in_string);

        let body = &after[2..];
        let Some(close) = body.find("}}") else {
            // An unterminated `{{` is text, not a hole.
            out.push_str(after);
            return Ok(());
        };

        let name = body[..close].trim();
        match resolve(values, row, name) {
            // A bare `{{rows}}` in a value position has no meaning — rows are
            // consumed by repeating the line, not by being written into it.
            // `Template::render` reports it, along with unbound holes; reaching
            // here means `fill` was used directly, so leave it as written.
            Some(Value::Rows(_)) | None => {
                let _ = write!(out, "{{{{{name}}}}}");
            }
            Some(value) => {
                let slot = slot_of(before);
                // A name the grammar cannot read is refused here rather than
                // written and left to fail as a parse error three lines away,
                // where nothing points back at the binding that caused it.
                if slot == Slot::Ident
                    && !*in_string
                    && let Value::Text(text) = value
                    && !is_identifier(text)
                {
                    bail!(
                        "`{name}` is bound to {text:?}, which cannot be a name — this hole \
                         declares an identifier, and a name is a letter or `_` followed by \
                         letters, digits, `_` or interior `-`"
                    );
                }
                write_value_at(out, value, *in_string, slot);
            }
        }

        rest = &body[close + 2..];
    }

    out.push_str(rest);
    *in_string = string_state(rest, *in_string);
    Ok(())
}

/// A hole's value: from the row in scope when it names that row's prefix,
/// from the outer bindings otherwise.
fn resolve<'a>(
    values: &'a Bindings,
    row: Option<(&str, &'a Bindings)>,
    name: &str,
) -> Option<&'a Value> {
    if let Some((prefix, bound)) = row
        && let Some((head, field)) = name.split_once('.')
        && head == prefix
    {
        return bound.get(field);
    }
    values.get(name)
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

/// Where on its line a hole sits, which decides how text is written into it.
///
/// A value position quotes text, because a bare word there would have to be an
/// identifier and `slice a {{name}}` must not become two statements. An
/// **identifier** position is the opposite: quoting there produces a string
/// where the grammar wants a name, and the file stops parsing. The two cannot
/// share a rule, which is why `Template::parse` probes with both.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Ident,
    Value,
}

/// Which one, from the tokens already on the line.
///
/// Positional rather than keyword-based on purpose: a rule that asked "is the
/// word before this an item keyword?" would misread `node slice {{v}}`, since
/// nothing stops an item being *named* `slice`.
fn slot_of(before: &str) -> Slot {
    let line = before.rsplit('\n').next().unwrap_or(before);
    let tokens: Vec<&str> = line.split_whitespace().collect();

    match tokens.as_slice() {
        // First word of a statement: an edge's tail.
        [] => Slot::Ident,
        // Second word: the identifier a statement declares — unless the first
        // was a property key, which owns the rest of the line.
        [first] if !first.ends_with(':') && !first.contains('{') => Slot::Ident,
        // The head of an edge.
        rest if matches!(
            rest[rest.len() - 1],
            "->" | "-->" | "..>" | "<->" | "--" | ".."
        ) =>
        {
            Slot::Ident
        }
        _ => Slot::Value,
    }
}

fn write_value_at(out: &mut String, value: &Value, in_string: bool, slot: Slot) {
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
        // In an identifier position the grammar wants a name, so text is
        // written bare — which is what lets a row of data supply the id of the
        // statement it fills. `render` is what refuses text that is not a
        // valid identifier; quoting it here would produce a file that parses
        // as something else.
        Value::Text(text) if slot == Slot::Ident && is_identifier(text) => out.push_str(text),
        // In a value position a bare word would have to be an identifier, so
        // text is quoted: `slice a {{name}}` must not become two statements.
        Value::Text(text) => {
            let _ = write!(out, "\"{}\"", escape(text));
        }
        // Never reached: `fill_line` writes a row hole back verbatim, and
        // `render` refuses one before it gets this far.
        Value::Rows(_) => {}
        Value::List(items) => {
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                write_value_at(out, item, in_string, slot);
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
