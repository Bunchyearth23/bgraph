// `unsafe` is forbidden everywhere except the wasm boundary, where four
// functions have to hand raw pointers to a caller that is not Rust. Scoped to
// that module rather than lifted for the crate.
#![cfg_attr(not(feature = "wasm"), forbid(unsafe_code))]
#![warn(missing_docs)]

//! `bgraph` — stylable diagrams from a layout-first DSL.
//!
//! Text in, SVG out. Pipeline: [`parser`] → [`ast`] → [`check`] → [`layout`] →
//! [`render_svg`]. The language server reuses exactly the same front half, so
//! the editor and the CLI can never disagree about what a file means.
//!
//! ```
//! let svg = bgraph::render_source("node a \"Hello\"\nnode b \"World\"\nlayout {\n  place a at (0, 0)\n  place b right-of a\n}\na -> b")?;
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
//! When the *number* of items is not known either, a line holding a
//! `{{row.field}}` hole is written once per row:
//!
//! ```
//! use bgraph::{Bindings, Template};
//!
//! let template = Template::parse(
//!     "type pie\nslice {{region.id}} \"{{region.name}}\" {{region.revenue}}",
//! )?;
//!
//! let svg = template.render(&Bindings::new().rows(
//!     "region",
//!     vec![
//!         Bindings::new()
//!             .set("id", bgraph::Value::Raw("emea".into()))
//!             .set("name", "EMEA")
//!             .set("revenue", 4_820i64),
//!         Bindings::new()
//!             .set("id", bgraph::Value::Raw("amer".into()))
//!             .set("name", "Americas")
//!             .set("revenue", 6_310i64),
//!     ],
//! ))?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! That is deliberately the whole of the repetition on offer: one line, one
//! list, no nesting and no conditions. Anything past that is a template
//! language, and a much larger promise than this makes.
//!
//! # Building only what you use
//!
//! Every archetype is a cargo feature, so a program that needs one chart does
//! not compile the other fourteen:
//!
//! ```toml
//! bgraph = { version = "0.1", default-features = false, features = ["pie"] }
//! ```
//!
//! An archetype that is not compiled in is *unknown*, not broken: `check` says
//! so and lists what is available.
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
pub mod compose;
#[allow(missing_docs)]
pub mod constraint_layout;
#[allow(missing_docs)]
pub mod diff;
#[allow(missing_docs)]
pub mod explain;
pub mod parser;
pub mod template;

// The engine. See the note above on what semver covers.
#[allow(missing_docs)]
pub mod ast;
#[allow(missing_docs)]
pub mod charts;

#[cfg(feature = "bpmn")]
pub mod bpmn;
pub mod examples;
pub mod fmt;
#[allow(missing_docs)]
pub mod layout;
pub mod ortho;
#[allow(missing_docs)]
pub mod placement;
#[allow(missing_docs)]
pub mod props;
pub mod relayout;
pub mod render_json;
#[allow(missing_docs)]
#[cfg(feature = "pdf")]
pub mod render_pdf;
#[allow(missing_docs)]
#[cfg(feature = "png")]
pub mod render_png;
pub mod render_svg;
pub mod report;
#[allow(missing_docs)]
pub mod style;
pub mod subset;
#[allow(missing_docs)]
pub mod themes;
#[allow(missing_docs)]
#[cfg(feature = "wasm")]
pub mod wasm;

// Support for the `bgraph` and `bgraph-lsp` binaries, not part of the library.
#[allow(missing_docs)]
#[doc(hidden)]
pub mod docgen;
#[allow(missing_docs)]
#[doc(hidden)]
pub mod init;
#[allow(missing_docs)]
#[doc(hidden)]
pub mod lsp;
#[allow(missing_docs)]
#[doc(hidden)]
pub mod watch;

// The suite covers every archetype, so a reduced build cannot run it. Saying
// so at compile time beats a hundred failures that all mean the same thing.
#[cfg(all(test, not(feature = "all-archetypes")))]
compile_error!("the test suite covers every archetype — run it with default features");

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
    render_diagram(diagram)
}

fn render_diagram(diagram: ast::Diagram) -> Result<String> {
    // No filesystem here, and deliberately: this function is the semver
    // surface and the wasm build rests on it, so it cannot start opening
    // files because a statement asked it to (`D-029`). It says which call
    // does rather than failing vaguely.
    if let Some(first) = diagram.includes.first() {
        anyhow::bail!(
            "`include \"{}\"` needs a file to resolve it against, and \
             `render_source` is given text — use `bgraph::render_file`",
            first.path.as_str()
        );
    }
    let layout = layout::solve(&diagram)?;
    Ok(render_svg::render(&layout))
}

/// A file on disk to SVG, resolving whatever it includes.
///
/// The counterpart to [`render_source`]: same pipeline, but a path to resolve
/// `include` against. A file with no includes renders identically either way.
///
/// # Errors
///
/// The first problem found, as [`render_source`] does — plus the ones only a
/// set of files can have: a circular include, or a name declared in two of
/// them with no `as` to separate them.
pub fn render_file(path: &std::path::Path) -> Result<String> {
    let composed = compose::load(path)?;
    let layout = layout::solve(&composed.diagram)?;
    Ok(render_svg::render(&layout))
}

/// The same diagram as a PDF, as bytes ready to write.
///
/// Covered by the semver promise alongside [`render_source`] (`D-018`): an
/// embedding program should not have to ship a renderer to get a page, which
/// is the whole point of `D-014`. Vector output, so there is no rasteriser
/// here and no dependency behind it — see `D-027` for why PNG is a different
/// question.
#[cfg(feature = "pdf")]
pub fn render_source_pdf(source: &str) -> Result<Vec<u8>> {
    let diagram = parser::parse_diagram(source)?;
    let layout = layout::solve(&diagram)?;
    Ok(render_pdf::render(&layout))
}
