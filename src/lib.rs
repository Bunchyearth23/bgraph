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
mod labels;
#[allow(missing_docs)]
pub mod layout;
mod math;
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
mod spacing;
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
pub mod output;
#[allow(missing_docs)]
#[doc(hidden)]
pub mod watch;

// The suite covers every archetype, so a reduced build cannot run it. Saying
// so at compile time beats a hundred failures that all mean the same thing.
#[cfg(all(test, not(feature = "all-archetypes")))]
compile_error!("the test suite covers every archetype — run it with default features");

#[cfg(test)]
mod tests;

#[cfg(test)]
mod analysis_tests;

#[cfg(test)]
mod multifile_analysis_tests;

use anyhow::{Context, Result};

pub use template::{Bindings, Template, Value};

/// The common diagnostics payload for one source text.
///
/// It is the shape used by CLI/LSP/watch/wasm to make sure the same
/// validation stages run in the same order.
// No `Debug` is needed here; analysis objects can be consumed in the CLI, LSP,
// and tests without requiring every downstream type to support pretty-printing.
pub struct SourceAnalysis {
    /// The diagram that was recovered from the source, when possible.
    pub diagram: Option<ast::Diagram>,
    /// A solved layout for the diagram, when `check` and `solve` both succeed.
    pub layout: Option<layout::Layout>,
    /// Syntax, validation and geometry diagnostics, in source order where possible.
    pub diagnostics: Vec<check::Diag>,
}

impl SourceAnalysis {
    /// Whether any diagnostic in this analysis is fatal.
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diag| diag.severity == check::Severity::Error)
    }

    /// Consume an analysis, returning its layout or all fatal diagnostics.
    pub fn into_layout(self) -> Result<layout::Layout> {
        let errors: Vec<_> = self
            .diagnostics
            .iter()
            .filter(|diag| diag.severity == check::Severity::Error)
            .map(|diag| diag.message.as_str())
            .collect();
        if !errors.is_empty() {
            anyhow::bail!("{}", errors.join("\n"));
        }
        self.layout.ok_or_else(|| {
            anyhow::anyhow!("diagram did not produce a layout after successful checks")
        })
    }
}

/// Parse with recovery, run [`check`], then run geometry-aware diagnostics when
/// layout is available.
///
/// This is the full pipeline used by all public analysis surfaces.
pub fn analyze_source(source: &str) -> SourceAnalysis {
    let (mut diagram, syntax) = parser::parse_recovering(source);
    let mut diagnostics: Vec<check::Diag> = syntax
        .into_iter()
        .map(|err| check::Diag {
            severity: check::Severity::Error,
            message: err.message,
            span: err.span,
        })
        .collect();

    let mut composition_failed = false;
    if let Some(diagram) = diagram.as_mut() {
        if let Some(include) = diagram.includes.first() {
            diagnostics.push(check::Diag {
                severity: check::Severity::Error,
                span: include.span,
                message: format!(
                    "`include \"{}\"` needs a file to resolve it against, and \
                     `render_source` is given text — use `bgraph::render_file`",
                    include.path.as_str()
                ),
            });
            composition_failed = true;
        } else {
            let expansion = compose::expand(diagram);
            composition_failed = !expansion.is_empty();
            diagnostics.extend(expansion);
        }
    }
    let (layout, extra) = if composition_failed {
        (None, Vec::new())
    } else {
        diagram
            .as_ref()
            .map_or_else(|| (None, Vec::new()), analyze_diagram)
    };

    diagnostics.extend(extra);
    diagnostics.sort_by_key(|diag| (diag.span.start, diag.span.end));
    SourceAnalysis {
        diagram,
        layout,
        diagnostics,
    }
}

/// Run `check` and geometry diagnostics on an already parsed diagram.
pub fn analyze_diagram(diagram: &ast::Diagram) -> (Option<layout::Layout>, Vec<check::Diag>) {
    let mut diagnostics = check::check(diagram);
    let layout = if diagnostics
        .iter()
        .any(|diag| diag.severity == check::Severity::Error)
    {
        None
    } else {
        match layout::solve(diagram) {
            Ok(layout) => {
                diagnostics.extend(check::after_layout(diagram, &layout));
                Some(layout)
            }
            Err(error) => {
                diagnostics.extend(check::layout_error(diagram, &error));
                None
            }
        }
    };
    diagnostics.sort_by_key(|diag| (diag.span.start, diag.span.end));
    (layout, diagnostics)
}

/// Analysis of a file and its includes, with diagnostic source locations.
#[doc(hidden)]
pub struct FileAnalysis {
    /// The shared syntax, semantic and geometry analysis.
    pub analysis: SourceAnalysis,
    /// Source ownership for every diagnostic span.
    pub map: compose::SourceMap,
}

impl FileAnalysis {
    /// Return the layout or every fatal diagnostic with its owning file.
    pub fn into_layout(self) -> Result<layout::Layout> {
        let errors: Vec<_> = self
            .analysis
            .diagnostics
            .iter()
            .filter(|diag| diag.severity == check::Severity::Error)
            .map(|diag| {
                let (path, line, column) = self.map.locate(diag.span.start);
                format!(
                    "{}:{}:{}: {}",
                    path.display(),
                    line + 1,
                    column + 1,
                    diag.message
                )
            })
            .collect();
        if !errors.is_empty() {
            anyhow::bail!("{}", errors.join("\n"));
        }
        self.analysis.into_layout()
    }
}

/// Analyze source using a file base, preserving diagnostics from every include.
#[doc(hidden)]
pub fn analyze_file_source(path: &std::path::Path, source: &str) -> Result<FileAnalysis> {
    analyze_file_source_with_theme(path, source, None)
}

/// Analyze a file source after applying a CLI theme, through the same stages.
#[doc(hidden)]
pub fn analyze_file_source_with_theme(
    path: &std::path::Path,
    source: &str,
    theme: Option<&str>,
) -> Result<FileAnalysis> {
    analyze_composed(compose::compose(path, Some(source))?, theme)
}

pub(crate) fn analyze_composed(
    mut composed: compose::Composed,
    theme: Option<&str>,
) -> Result<FileAnalysis> {
    if let Some(theme) = theme {
        themes::apply(&mut composed.diagram, theme)?;
    }
    let (layout, mut diagnostics) = analyze_diagram(&composed.diagram);
    diagnostics.extend(composed.diagnostics);
    diagnostics.sort_by_key(|diag| (diag.span.start, diag.span.end));
    Ok(FileAnalysis {
        analysis: SourceAnalysis {
            diagram: Some(composed.diagram),
            layout,
            diagnostics,
        },
        map: composed.map,
    })
}

/// Analyze a file and all readable included sources.
#[doc(hidden)]
pub fn analyze_file(path: &std::path::Path) -> Result<FileAnalysis> {
    let source =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    analyze_file_source(path, &source)
}

/// Resolve an already parsed diagram through the common analysis pipeline.
#[doc(hidden)]
pub fn analyzed_layout(diagram: &ast::Diagram) -> Result<layout::Layout> {
    let (layout, diagnostics) = analyze_diagram(diagram);
    SourceAnalysis {
        diagram: None,
        layout,
        diagnostics,
    }
    .into_layout()
}

/// Source text to SVG, the whole rite in one call.
///
/// Parses, checks, lays out and renders. The result is a complete SVG
/// document, sized to its contents and painting its own background, so a file
/// exports to exactly what the preview shows.
///
/// # Errors
///
/// Returns the fatal syntax, validation or geometry diagnostics. Use
/// [`analyze_source`] to inspect their spans and nonfatal warnings too.
pub fn render_source(source: &str) -> Result<String> {
    let analysis = analyze_source(source);
    let layout = analysis.into_layout()?;
    Ok(render_svg::render(&layout))
}

/// A file on disk to SVG, resolving whatever it includes.
///
/// The counterpart to [`render_source`]: same pipeline, but a path to resolve
/// `include` against. A file with no includes renders identically either way.
///
/// # Errors
///
/// Reports fatal diagnostics with their owning file and position, including
/// circular includes and names declared in two files without separate prefixes.
pub fn render_file(path: &std::path::Path) -> Result<String> {
    let layout = analyze_file(path)?.into_layout()?;
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
    let layout = analyze_source(source).into_layout()?;
    render_pdf::render(&layout)
}
