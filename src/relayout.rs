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
//! So it **adds scoped layout blocks** rather than rewriting declarations.
//! `fmt` is lexical and rewrites whole lines while deliberately leaving
//! interior alignment alone; this has to go further, because a comment, a
//! column of aligned values and a trailing note are all things the author put
//! there on purpose and none of them are this command's business.
//!
//! # The guard that makes it trustworthy
//!
//! A round trip, and it is the whole promise: run it, render before and after,
//! and the SVG must be identical to the byte. What it writes is by
//! construction the placement the engine had already chosen — an unplaced box
//! flows from the statement before it — so anything else would be a bug, and
//! the test is what says so.

use crate::ast::{Diagram, RelDir};
use crate::placement::Entity;
use crate::style::GraphOpts;
use anyhow::Result;

/// The same source with canonical scoped `layout` blocks that position every
/// box which did not already have a source relation.
pub fn fill_in(source: &str, diagram: &Diagram) -> Result<String> {
    let opts = GraphOpts::from_props(&diagram.graph)?;
    let placement = crate::placement::PlacementPlan::new(diagram);
    let word = match opts.flow {
        RelDir::RightOf => "right-of",
        RelDir::LeftOf => "left-of",
        RelDir::Below => "below",
        RelDir::Above => "above",
    };

    // One pending relation per root or group scope. Origins are deliberately
    // written too: after this command, no rendered position relies on the
    // implicit-flow rule hidden in declaration order.
    let mut root = Vec::new();
    let mut local = vec![Vec::new(); diagram.groups.len()];

    for (i, node) in diagram.nodes.iter().enumerate() {
        if placement.has_source_position(i) {
            continue;
        }
        let relation = match placement.implicit_anchor(i) {
            Some(anchor) => format!("{word} {}", diagram.nodes[anchor].id.as_str()),
            None => "at (0, 0)".to_string(),
        };
        let statement = format!("place {} {relation}", node.id.as_str());
        match placement.owner().get(&Entity::Node(i)).copied() {
            Some(group) => local[group].push(statement),
            None => root.push(statement),
        }
    }

    // Collected first and applied last-to-first, so an earlier insertion
    // cannot move a later one's offset out from under it.
    let mut edits: Vec<(usize, String)> = Vec::new();
    for (group, statements) in local.into_iter().enumerate() {
        if statements.is_empty() {
            continue;
        }
        let span = diagram.groups[group].span;
        let at = span.end.saturating_sub(1);
        edits.push((at, scoped_block(source, span.start, at, &statements, false)));
    }
    if !root.is_empty() {
        edits.push((
            source.len(),
            scoped_block(source, source.len(), source.len(), &root, true),
        ));
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

/// A root or local `layout` block, matching the indentation of its owner.
fn scoped_block(
    source: &str,
    owner_start: usize,
    insertion: usize,
    statements: &[String],
    root: bool,
) -> String {
    let line_start = source[..owner_start.min(source.len())]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let base = source[line_start..owner_start.min(source.len())]
        .chars()
        .take_while(|ch| matches!(ch, ' ' | '\t'))
        .collect::<String>();
    let indent = if root {
        base.clone()
    } else {
        format!("{base}  ")
    };
    let prefix = if source[..insertion.min(source.len())].ends_with('\n') {
        ""
    } else {
        "\n"
    };
    let mut out = format!("{prefix}{indent}layout {{\n");
    for statement in statements {
        out.push_str(&format!("{indent}  {statement}\n"));
    }
    out.push_str(&format!("{indent}}}"));
    if !root {
        out.push_str(&format!("\n{base}"));
    } else {
        out.push('\n');
    }
    out
}

/// Whether anything would change — for a `--check` that reports rather than
/// writes, as `fmt` already does.
pub fn would_change(source: &str, diagram: &Diagram) -> Result<bool> {
    Ok(fill_in(source, diagram)? != source)
}

/// Every box that has no placement of its own, for a report.
pub fn unplaced(diagram: &Diagram) -> Vec<&str> {
    let placement = crate::placement::PlacementPlan::new(diagram);
    diagram
        .nodes
        .iter()
        .enumerate()
        .filter(|(i, _)| !placement.has_source_position(*i))
        .map(|(_, n)| n.id.as_str())
        .collect()
}
