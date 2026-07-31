#![forbid(unsafe_code)]
#![warn(missing_docs)]

//! `bgraph` — stylable diagrams from a layout-first DSL.
//!
//! Text in, SVG out. Pipeline: [`parser`] → [`ast`] → [`check`] → [`layout`] →
//! [`render_svg`]. The language server reuses exactly the same front half, so
//! the editor and the CLI can never disagree about what a file means.
//!
//! ```
//! let svg = bgraph::render_source("node a \"Hello\"\nnode b \"World\" right-of a\na -> b")?;
//! assert!(svg.starts_with("<svg"));
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! # Embedding this in a document generator
//!
//! The intended shape is a diagram authored as a file, with holes where the
//! numbers go. A program binds values and renders — it never assembles the
//! diagram statement by statement, because a program that can do that can
//! already write the source text.
//!
//! ```
//! use bgraph::{Bindings, Template};
//!
//! let template = Template::parse(
//!     "type pie { title: \"Revenue, {{year}}\" }\n\
//!      slice emea \"EMEA\" {{emea}}\n\
//!      slice amer \"Americas\" {{amer}}",
//! )?;
//!
//! assert_eq!(template.holes(), ["year", "emea", "amer"]);
//!
//! let svg = template.render(
//!     &Bindings::new()
//!         .set("year", 2026i64)
//!         .set("emea", 4_820i64)
//!         .set("amer", 6_310i64),
//! )?;
//! assert!(svg.contains("Revenue, 2026"));
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! A bound value is escaped for wherever it lands, so a customer-supplied label
//! cannot become a statement. An unbound hole is an error naming it, never a
//! blank — a document with a missing figure is worse than one that failed.
//!
//! # What this promises
//!
//! **No input can bring down the host.** Rendering any text at all returns
//! `Ok` or `Err`: it does not panic, and it does not abort. Nesting is capped
//! at [`parser::MAX_NESTING`] for exactly that reason — the parser descends
//! recursively, and a stack overflow is not something a caller can catch. The
//! `hostile_input_never_panics` test holds this line over a corpus aimed at
//! every division, index and recursion in the engine.
//!
//! There is no `unsafe` in this crate, and none is permitted.
//!
//! # What semver covers
//!
//! Everything documented: [`render_source`], [`template`], [`parser`] and
//! [`check`]. That is the whole of what a program embeds.
//!
//! The vocabulary in [`props`] and the engine behind it — [`ast`], [`layout`],
//! [`style`], [`charts`], [`render_svg`] — are public to be inspected, not
//! relied on. The vocabulary's real documentation is `docs/reference.md`,
//! which is generated from those same tables and is the thing that stays put.
//! They move when the engine moves; pinning them would mean the diagrams could
//! never be improved.

pub mod check;
pub mod parser;
pub mod template;

// The engine. See the note above on what semver covers.
#[allow(missing_docs)]
pub mod ast;
#[allow(missing_docs)]
pub mod charts;
#[allow(missing_docs)]
pub mod layout;
#[allow(missing_docs)]
pub mod props;
#[allow(missing_docs)]
pub mod render_svg;
#[allow(missing_docs)]
pub mod style;

// Support for the `bgraph` and `bgraph-lsp` binaries, not part of the library.
#[allow(missing_docs)]
#[doc(hidden)]
pub mod docgen;
#[allow(missing_docs)]
#[doc(hidden)]
pub mod lsp;
#[allow(missing_docs)]
#[doc(hidden)]
pub mod watch;

#[cfg(test)]
mod tests;

use anyhow::Result;

pub use template::{Bindings, Template, Value};

/// Source text to SVG, the whole rite in one call.
///
/// Parses, checks, lays out and renders. The result is a complete SVG
/// document, sized to its contents and painting its own background, so a file
/// exports to exactly what the preview shows.
///
/// # Errors
///
/// Returns the first problem found — a syntax error carrying the byte range it
/// points at, or a check failure naming the item at fault. Use [`check::check`]
/// to collect *every* problem instead of the first, which is what the language
/// server does.
pub fn render_source(source: &str) -> Result<String> {
    let diagram = parser::parse_diagram(source)?;
    let layout = layout::solve(&diagram)?;
    Ok(render_svg::render(&layout))
}
