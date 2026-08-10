//! How a diagnostic reaches a person, and how it reaches a build.
//!
//! Two forms of the same fact, and neither is the polite version of the other.
//!
//! The **flat** form — `file:line:col: severity: message` — is what editors,
//! `make`, CI logs and every grep in a shell script already understand. It is
//! not a fallback; it is the interchange format, and it must not move. A test
//! pins it byte for byte.
//!
//! The **rich** form shows the offending line with a caret under the span, the
//! way `rustc` does, because a column number is a thing to go and count while
//! a caret is a thing to look at. It is used only when stdout is a terminal —
//! the moment output is redirected, it is being read by a program, and a
//! program wants the flat form. `--ci` says so outright for the cases where
//! that guess would be wrong.
//!
//! Colour follows the same reasoning, plus `NO_COLOR`: someone who set that
//! variable has already answered the question.

use crate::check::{Diag, Severity};

/// The interchange form. **Do not change this** without changing what parses
/// it — the editors, the CI job, and anything a reader has in a shell alias.
pub fn flat(path: &str, line: usize, col: usize, diag: &Diag) -> String {
    format!(
        "{path}:{}:{}: {}: {}",
        line + 1,
        col + 1,
        label(diag.severity),
        diag.message
    )
}

/// The form for a person: the source line, and a caret under what is wrong.
///
/// ```text
/// error: unknown property `fil`
///   --> diagram.bd:4:12
///    |
///  4 |   node a "A" { fil: #fff }
///    |                ^^^
/// ```
pub fn rich(
    path: &str,
    source: &str,
    line: usize,
    col: usize,
    diag: &Diag,
    colour: bool,
) -> String {
    let (bold, dim, tint, off) = if colour {
        (
            "\x1b[1m",
            "\x1b[34m",
            match diag.severity {
                Severity::Error => "\x1b[1;31m",
                Severity::Warning => "\x1b[1;33m",
            },
            "\x1b[0m",
        )
    } else {
        ("", "", "", "")
    };

    let text = source.lines().nth(line).unwrap_or("");
    // The gutter is as wide as the line number it holds, so the bars line up
    // whether the file has nine lines or nine hundred.
    let number = (line + 1).to_string();
    let pad = " ".repeat(number.len());

    // A span may cross a newline — an unclosed block reaches to end of file.
    // Underlining to the end of *this* line says where it starts without
    // claiming the rest of the file is the mistake.
    let width = diag
        .span
        .end
        .saturating_sub(diag.span.start)
        .min(text.chars().count().saturating_sub(col))
        .max(1);

    let mut out = String::new();
    out.push_str(&format!(
        "{tint}{}{off}{bold}: {}{off}\n",
        label(diag.severity),
        diag.message
    ));
    out.push_str(&format!(
        "{pad}{dim}-->{off} {path}:{}:{}\n",
        line + 1,
        col + 1
    ));
    out.push_str(&format!("{pad} {dim}|{off}\n"));
    out.push_str(&format!("{dim}{number} |{off} {text}\n"));
    out.push_str(&format!(
        "{pad} {dim}|{off} {}{tint}{}{off}",
        " ".repeat(col),
        "^".repeat(width)
    ));
    out
}

fn label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}

/// Whether to draw the rich form at all: a terminal, and not told otherwise.
///
/// Taken together rather than as two questions, because they are one decision
/// and splitting it invites a caller to answer half of it.
pub fn decorate(ci: bool) -> (bool, bool) {
    use std::io::IsTerminal;

    let terminal = std::io::stdout().is_terminal();
    let rich = terminal && !ci;
    // `NO_COLOR` is a convention worth honouring: whoever set it has already
    // answered this, and asking again by ignoring it is the rude version.
    let colour = rich && std::env::var_os("NO_COLOR").is_none();
    (rich, colour)
}
