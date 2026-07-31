//! `bgraph` — stylable diagrams from a layout-first DSL.
//!
//! Pipeline: `parser` → `ast` → `check` → `layout` → `render_svg`.
//! The `lsp` module reuses exactly the same front half, so the editor and the
//! CLI can never disagree about what a file means.

pub mod ast;
pub mod check;
pub mod layout;
pub mod lsp;
pub mod parser;
pub mod props;
pub mod render_svg;
pub mod style;
pub mod watch;

#[cfg(test)]
mod tests;

use anyhow::Result;

/// Source text to SVG, the whole rite in one call.
pub fn render_source(source: &str) -> Result<String> {
    let diagram = parser::parse_diagram(source)?;
    let layout = layout::solve(&diagram)?;
    Ok(render_svg::render(&layout))
}
