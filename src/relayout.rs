//! `bgraph layout` — write the placements into the file, once.
//!
//! The last third of `D-031`. Auto-placement as a **source transformation**,
//! not as an engine: it writes anchors into the file and the renderer still
//! places nothing on its own. An author starts loose, runs this, and from then
//! on every box says where it goes — which is what `D-001` is for, reached
//! without having typed it all out.
//!
//! # It edits the author's file
//!
//! So it **inserts into lines** rather than rewriting them. `fmt` is lexical
//! and rewrites whole lines while deliberately leaving interior alignment
//! alone; this has to go further, because a comment, a column of aligned
//! values and a trailing note are all things the author put there on purpose
//! and none of them are this command's business.
//!
//! # The guard that makes it trustworthy
//!
//! A round trip, and it is the whole promise: run it, render before and after,
//! and the SVG must be identical to the byte. What it writes is by
//! construction the placement the engine had already chosen — an unplaced box
//! flows from the statement before it — so anything else would be a bug, and
//! the test is what says so.

use crate::ast::{Diagram, Placement, RelDir};
use crate::style::GraphOpts;
use anyhow::Result;

/// The same source with a placement written onto every box that had none.
pub fn fill_in(source: &str, diagram: &Diagram) -> Result<String> {
    let opts = GraphOpts::from_props(&diagram.graph)?;
    let word = match opts.flow {
        RelDir::RightOf => "right-of",
        RelDir::LeftOf => "left-of",
        RelDir::Below => "below",
        RelDir::Above => "above",
    };

    // Collected first and applied last-to-first, so an earlier insertion
    // cannot move a later one's offset out from under it.
    let mut edits: Vec<(usize, String)> = Vec::new();

    for (i, node) in diagram.nodes.iter().enumerate() {
        // The first box has nothing to be placed against, and a box that
        // already says where it goes is not this command's business.
        if i == 0 || node.placement.is_some() {
            continue;
        }
        let anchor = diagram.nodes[i - 1].id.as_str();
        let at = insertion_point(source, node.span.start, node.span.end);
        edits.push((at, format!(" {word} {anchor}")));
    }

    let mut out = source.to_string();
    edits.sort_by_key(|(at, _)| *at);
    for (at, text) in edits.into_iter().rev() {
        if at <= out.len() {
            out.insert_str(at, &text);
        }
    }
    Ok(out)
}

/// Where a placement belongs in a statement: after everything the author
/// wrote on the line, and **before the block** if there is one.
///
/// Both orders parse — the grammar takes a placement either side of a block —
/// but a placement written after a block reads as an afterthought, and this is
/// meant to look like something a person would have typed.
fn insertion_point(source: &str, start: usize, end: usize) -> usize {
    let statement = &source[start..end.min(source.len())];

    // A brace inside a string is not a block, and a label may well contain
    // one — the same rule the nesting scanner follows.
    let mut in_string = false;
    let mut chars = statement.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' if in_string => {
                chars.next();
            }
            '"' => in_string = !in_string,
            // Before the space that already sits in front of the block, not
            // after it: inserting at the brace produced `right-of api{ … }`,
            // and the whitespace the author left is theirs to keep.
            '{' if !in_string => {
                let head = &statement[..i];
                return start + head.trim_end().len();
            }
            _ => {}
        }
    }

    // No block: at the end of the statement, minus any trailing space the
    // author left, so the line does not gain two spaces in a row.
    start + statement.trim_end().len()
}

/// Whether anything would change — for a `--check` that reports rather than
/// writes, as `fmt` already does.
pub fn would_change(source: &str, diagram: &Diagram) -> Result<bool> {
    Ok(fill_in(source, diagram)? != source)
}

/// Every box that has no placement of its own, for a report.
pub fn unplaced(diagram: &Diagram) -> Vec<&str> {
    diagram
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| *i > 0 && n.placement.is_none())
        .map(|(_, n)| n.id.as_str())
        .collect()
}

/// A placement written out the way the grammar reads it.
///
/// Public so a test can assert the shape without reaching into the file.
pub fn written(placement: &Placement) -> String {
    match placement {
        Placement::At { x, y } => format!("at ({x}, {y})"),
        Placement::Cell { col, row } => format!("at (col {col}, row {row})"),
        Placement::Rel { dir, anchor, .. } => format!(
            "{} {}",
            match dir {
                RelDir::RightOf => "right-of",
                RelDir::LeftOf => "left-of",
                RelDir::Below => "below",
                RelDir::Above => "above",
            },
            anchor.as_str()
        ),
    }
}
