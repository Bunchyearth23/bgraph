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

use std::collections::BTreeMap;
use std::fmt::Write;

use anyhow::{Result, bail};

/// A hole, and where it is written.
#[derive(Debug, Clone, PartialEq)]
pub struct Hole {
    /// The name between the braces, trimmed.
    pub name: String,
    /// The row it repeats over, when the name is dotted.
    pub prefix: Option<String>,
    /// Byte range of the whole `{{…}}`, for an editor to point at.
    pub span: crate::ast::Span,
}

/// Every hole in a source, in the order written — repeats included, because an
/// editor wants to point at each occurrence and not only the first.
pub fn holes_in(source: &str) -> Vec<Hole> {
    let mut out = Vec::new();
    let mut at = 0usize;

    while let Some(open) = source[at..].find("{{") {
        let start = at + open;
        let body = &source[start + 2..];
        let Some(close) = body.find("}}") else { break };
        let name = body[..close].trim().to_string();
        let end = start + 2 + close + 2;
        if !name.is_empty() {
            out.push(Hole {
                prefix: name.split_once('.').map(|(p, _)| p.to_string()),
                name,
                span: crate::ast::Span { start, end },
            });
        }
        at = end;
    }

    out
}

/// What is wrong with a template, with somewhere to point.
///
/// [`Template::parse`] refuses these too, but it refuses them as one sentence
/// with no span — which is enough for a command line and useless in an editor.
/// The knowledge lives here rather than in the language server, because the
/// rules are the template's own.
pub fn lint(source: &str) -> Vec<(crate::ast::Span, String)> {
    let mut out = Vec::new();
    let mut offset = 0usize;

    for line in source.split_inclusive('\n') {
        // An unterminated `{{` is text to `fill`, which is the forgiving
        // reading — but on a line that clearly meant to open a hole it is
        // almost always a typo, and saying so costs nothing.
        if let Some(open) = line.find("{{")
            && !line[open..].contains("}}")
        {
            out.push((
                crate::ast::Span {
                    start: offset + open,
                    end: offset + line.trim_end().len(),
                },
                "this `{{` is never closed, so it is written out as text".to_string(),
            ));
        }

        // A line naming two rows has no answer for how many times to write it.
        let mut prefixes: Vec<String> = Vec::new();
        for hole in holes_in(line) {
            if let Some(prefix) = hole.prefix
                && !prefixes.contains(&prefix)
            {
                prefixes.push(prefix);
            }
        }
        if prefixes.len() > 1 {
            out.push((
                crate::ast::Span {
                    start: offset,
                    end: offset + line.trim_end().len(),
                },
                format!(
                    "this line repeats over {} — a line may repeat over one list, or the \
                     number of times it is written would be ambiguous",
                    prefixes
                        .iter()
                        .map(|p| format!("`{p}`"))
                        .collect::<Vec<_>>()
                        .join(" and ")
                ),
            ));
        }

        offset += line.len();
    }

    out
}

/// Bindings read from CSV — one header line, one row per repetition.
///
/// The format most real data arrives in, and the one that maps exactly onto
/// [`Value::Rows`]: a header names the fields, every other line is a row. The
/// whole file becomes **one** binding, named by the caller, because a CSV has
/// one table in it and pretending otherwise would need a convention nobody
/// asked for.
///
/// A field that parses as a number becomes one; everything else is text. No
/// quoting dialect beyond the common one — a field may be wrapped in `"` and
/// double a `"` inside itself, which is what a spreadsheet writes.
#[cfg(feature = "csv")]
pub fn rows_from_csv(name: &str, source: &str) -> Result<Bindings> {
    let (fields, records) = csv_records(source)?;
    let rows = records
        .into_iter()
        .map(|cells| {
            let mut row = Bindings::new();
            for (field, cell) in fields.iter().zip(cells) {
                row.insert(
                    field.clone(),
                    match cell.parse::<f64>() {
                        Ok(n) => Value::Number(n),
                        Err(_) => Value::Text(cell),
                    },
                );
            }
            row
        })
        .collect();
    Ok(Bindings::new().set(name, Value::Rows(rows)))
}

/// The same CSV dialect for template bindings and `type { from: … }`.
#[cfg(feature = "csv")]
pub(crate) fn csv_records(source: &str) -> Result<(Vec<String>, Vec<Vec<String>>)> {
    let mut records = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut closed = false;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => {
                    quoted = false;
                    closed = true;
                }
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if field.trim().is_empty() && !closed => {
                field.clear();
                quoted = true;
            }
            ',' | '\n' | '\r' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if c == ',' || !row.is_empty() || !field.trim().is_empty() || closed {
                    row.push(field.trim().to_string());
                }
                field.clear();
                closed = false;
                if c != ',' && !row.is_empty() {
                    records.push(std::mem::take(&mut row));
                }
            }
            c if c.is_whitespace() => field.push(c),
            _ if closed || c == '"' => {
                bail!("invalid CSV quoting in record {}", records.len() + 1);
            }
            _ => field.push(c),
        }
    }
    if quoted {
        bail!("unclosed quoted CSV field in record {}", records.len() + 1);
    }
    if !row.is_empty() || !field.trim().is_empty() || closed {
        row.push(field.trim().to_string());
        records.push(row);
    }
    let mut records = records.into_iter();
    let Some(fields) = records.next() else {
        bail!("the CSV is empty — the first line names the fields");
    };
    for (i, field) in fields.iter().enumerate() {
        if field.is_empty() {
            bail!("a column in the header has no name, and a row's fields are named by it");
        }
        if fields[..i].contains(field) {
            bail!("duplicate CSV column `{field}` — every field needs a unique name");
        }
    }
    let rows: Vec<_> = records.collect();
    for (n, cells) in rows.iter().enumerate() {
        if cells.len() != fields.len() {
            bail!(
                "row {} has {} field{} for {} column{} — a row that does not line up with the header has no reading",
                n + 1,
                cells.len(),
                if cells.len() == 1 { "" } else { "s" },
                fields.len(),
                if fields.len() == 1 { "" } else { "s" }
            );
        }
    }
    Ok((fields, rows))
}

/// Whether text can stand as a bare name. The same rule the grammar uses:
/// a leading letter or underscore, then letters, digits, underscores and
/// interior hyphens — a trailing hyphen would make `a-` swallow an arrow.
fn is_identifier(text: &str) -> bool {
    text.split('.').all(is_name_part)
}

fn is_name_part(text: &str) -> bool {
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
                    let J::Object(fields) = item else {
                        unreachable!()
                    };
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
pub struct Bindings(BTreeMap<String, Value>);

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

        let mut probe_rows = Bindings::new();
        for hole in &holes {
            if let Some((prefix, _)) = hole.split_once('.') {
                probe_rows.insert(prefix, Value::Rows(Vec::new()));
            }
        }
        for line in without_components(source).lines() {
            let prefixes = row_prefixes(line, &probe_rows);
            if prefixes.len() > 1 {
                bail!(
                    "this line repeats over {} — a line may repeat over one list",
                    quoted_list(&prefixes.iter().map(String::as_str).collect::<Vec<_>>())
                );
            }
        }
        // Probe each position, not each binding: identifiers and coordinates
        // may occur together, and no single dummy value parses as both.
        let mut probe = String::with_capacity(source.len());
        let mut at = 0;
        let mut in_string = false;
        for hole in holes_in(&without_components(source)) {
            let before = &source[at..hole.span.start];
            probe.push_str(before);
            in_string = string_state(before, in_string);
            probe.push_str(if !in_string && slot_of(&probe) == Slot::Ident {
                "_probe"
            } else {
                "0"
            });
            at = hole.span.end;
        }
        probe.push_str(&source[at..]);
        crate::parser::parse(&probe).map_err(|e| {
            anyhow::anyhow!("the template does not parse once its holes are filled: {e}")
        })?;

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
        crate::render_source(&self.fill_checked(values)?)
    }

    /// Fill every required hole, refusing missing bindings and row fields.
    /// Unlike [`Self::fill`], this never leaves unresolved template data for
    /// the parser to reinterpret as a literal or an omitted chart value.
    pub fn fill_checked(&self, values: &Bindings) -> Result<String> {
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

        for name in &self.holes {
            if let Some((prefix, field)) = name.split_once('.')
                && let Some(Value::Rows(rows)) = values.get(prefix)
            {
                for (index, row) in rows.iter().enumerate() {
                    let Some(value) = row.get(field) else {
                        bail!(
                            "nothing bound to `{name}` in row {} of `{prefix}`",
                            index + 1
                        );
                    };
                    if contains_rows(value) {
                        bail!(
                            "`{name}` in row {} of `{prefix}` contains nested rows; bind a scalar or a list of scalar values",
                            index + 1
                        );
                    }
                }
            }
        }

        // Rows repeat a line; they are not a value. Writing one into a value
        // position is a mistake worth naming rather than quietly dropping.
        let misused: Vec<&str> = self
            .holes
            .iter()
            .filter(|h| !h.contains('.') && values.get(h).is_some_and(contains_rows))
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

        fill(&self.source, values)
    }

    /// The filled source, without rendering — for inspecting what a binding
    /// produced, and what the CLI's `--emit` prints.
    pub fn fill(&self, values: &Bindings) -> Result<String> {
        fill(&self.source, values)
    }
}

fn contains_rows(value: &Value) -> bool {
    match value {
        Value::Rows(_) => true,
        Value::List(items) => items.iter().any(contains_rows),
        _ => false,
    }
}

/// Template text to SVG in one call.
pub fn render(source: &str, values: &Bindings) -> Result<String> {
    Template::parse(source)?.render(values)
}

/// Hole names, in order of first appearance.
fn scan(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    // A component's holes belong to the component: they are bound by each
    // `use` that places it, not by whoever renders the file. Counting them
    // here would make every file with a component a template that can never
    // be satisfied.
    let source = without_components(source);
    let mut rest = source.as_str();

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

/// The source with every `component { … }` block blanked out.
///
/// Blanked rather than removed, so any offset taken from the result still
/// lines up with the original — the language server reports hole positions
/// from this scan.
fn without_components(source: &str) -> String {
    let mut out: Vec<u8> = source.as_bytes().to_vec();
    let mut at = 0usize;

    while let Some(found) = source[at..].find("component") {
        let start = at + found;
        // A word, not a prefix of one, and at the head of a statement.
        let before_ok = start == 0
            || source[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c == '\n' || c == ';' || c == ' ' || c == '\t');
        let Some(open) = source[start..].find('{') else {
            break;
        };
        let open = start + open;
        if !before_ok {
            at = start + "component".len();
            continue;
        }

        // Depth counting, so a nested block does not end the component early.
        let mut depth = 0usize;
        let mut end = open;
        for (i, c) in source[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end <= open {
            break;
        }
        for byte in &mut out[open..end] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
        at = end;
    }

    String::from_utf8(out).unwrap_or_else(|_| source.to_string())
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
                let slot = slot_of(out);
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
/// share a rule, which is why `Template::parse` probes each position separately.
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
    const ARROWS: [&str; 7] = ["<->", "<-", "-->", "..>", "--", "->", ".."];
    let mut tokens = Vec::new();
    let mut rest = before.rsplit('\n').next().unwrap_or(before);
    let mut parentheses = 0usize;
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with("//") {
            break;
        }
        let c = rest.as_bytes()[0];
        if c == b'"' {
            let mut end = 1;
            let mut escaped = false;
            for (i, c) in rest[1..].char_indices() {
                end = i + 1 + c.len_utf8();
                if c == '"' && !escaped {
                    break;
                }
                escaped = c == '\\' && !escaped;
            }
            tokens.push(&rest[..end]);
            rest = &rest[end..];
            continue;
        }
        if matches!(c, b'{' | b'}' | b';') || (c == b',' && parentheses == 0) {
            tokens.clear();
            rest = &rest[1..];
            continue;
        }
        let arrow = ARROWS.into_iter().find(|arrow| rest.starts_with(arrow));
        let end = if let Some(arrow) = arrow {
            arrow.len()
        } else if matches!(c, b'(' | b')' | b',' | b':') {
            if c == b'(' {
                parentheses += 1;
            } else if c == b')' {
                parentheses = parentheses.saturating_sub(1);
            }
            1
        } else {
            rest.char_indices()
                .skip(1)
                .find(|(i, c)| {
                    c.is_whitespace()
                        || "{}();,:\"".contains(*c)
                        || ARROWS.iter().any(|arrow| rest[*i..].starts_with(arrow))
                })
                .map_or(rest.len(), |(i, _)| i)
        };
        tokens.push(&rest[..end]);
        rest = &rest[end..];
    }
    let first = tokens.first().copied();
    let last = tokens.last().copied();
    let edge = tokens.iter().any(|token| ARROWS.contains(token));
    if tokens.contains(&":") {
        return Slot::Value;
    }
    if last.is_some_and(|token| ARROWS.contains(&token) || token.ends_with('.'))
        || (matches!(last, Some("right-of" | "left-of" | "below" | "above"))
            && (matches!(first, Some("place" | "note" | "legend")) || tokens.contains(&"via")))
        || (parentheses == 0
            && (tokens.is_empty()
                || (tokens.len() == 1 && !matches!(first, Some("note" | "include" | "legend")))
                || (!edge
                    && matches!(
                        first,
                        Some("row" | "column" | "equal-width" | "equal-height")
                    )
                    && !tokens.contains(&"gap"))
                || (!edge && first == Some("align") && tokens.len() >= 2)
                || (first == Some("style") && last == Some("extends"))
                || (matches!(first, Some("use" | "include")) && last == Some("as"))))
    {
        Slot::Ident
    } else {
        Slot::Value
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
