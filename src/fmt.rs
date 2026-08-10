//! A formatter for `.bd` files.
//!
//! The language has had a canonical form since the first example was written —
//! two-space indents inside a block, no trailing whitespace, at most one blank
//! line in a row — and nothing made it hold. A diagram is read far more often
//! than it is written, so the shape of the source is worth as much as the
//! shape of the picture.
//!
//! **Interior alignment is left exactly as written.** `examples/packet.bd`
//! lines its bit ranges up in columns and that is a real style, not an
//! accident; a formatter that crushed it would be arguing with the author
//! about something the grammar has no opinion on. Indentation is structural
//! and is therefore the formatter's business; column alignment is taste and is
//! not.
//!
//! It is **lexical on purpose**, like the nesting guard and the template
//! scanner. Formatting through the AST would drop every comment and every
//! blank line, which are the two things an author uses to say what the
//! grammar cannot — and a formatter that quietly deletes your notes is one
//! nobody runs twice.

/// Reformat a source, or report why it will not.
///
/// Refuses a file that does not parse rather than reflowing something it does
/// not understand: an unbalanced brace reindents everything after it, and the
/// diff would bury the one line that was wrong.
pub fn format(source: &str) -> anyhow::Result<String> {
    crate::parser::parse_diagram(source)?;
    Ok(reflow(source))
}

/// The formatting itself, on text that is known to parse.
fn reflow(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut depth: usize = 0;
    let mut blanks = 0usize;

    for raw in source.lines() {
        let line = raw.trim();

        if line.is_empty() {
            // At most one blank line in a row, and none at the top of a block:
            // a run of them is a paragraph break someone stopped using.
            blanks += 1;
            continue;
        }

        // A closing brace belongs at the indent of the line that opened it —
        // which is one *less* than the depth we are still nominally at. That
        // is a display decision only: the running depth comes from the braces
        // the line actually holds, counted once.
        let closes_first = line.starts_with('}');
        let indent = if closes_first {
            depth.saturating_sub(1)
        } else {
            depth
        };

        if blanks > 0 && !out.is_empty() && !closes_first {
            out.push('\n');
        }
        blanks = 0;

        for _ in 0..indent {
            out.push_str("  ");
        }
        out.push_str(line.trim_end());
        out.push('\n');

        // Braces inside a string or after a `//` are text, so they are counted
        // by the same scan that decides where the line goes.
        depth = (depth as isize + net_braces(line)).max(0) as usize;
    }

    out
}

/// How many braces the line leaves open, ignoring any inside a string or after
/// a `//`.
fn net_braces(line: &str) -> isize {
    let mut net = 0isize;
    let mut in_string = false;
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '\\' if in_string => {
                chars.next();
            }
            '"' => in_string = !in_string,
            '/' if !in_string && chars.peek() == Some(&'/') => break,
            '{' if !in_string => net += 1,
            '}' if !in_string => net -= 1,
            _ => {}
        }
    }

    net
}
