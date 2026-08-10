//! Named palettes: `--theme dark`.
//!
//! A theme **is** a style file. Not "like" one — the text below is `.bds`
//! source, parsed by the same parser and resolved by the same registry as a
//! file someone writes themselves. That is one mechanism instead of two, and
//! it means anyone can read a theme to see exactly what it does, copy it, and
//! change it.
//!
//! Themes are laid down *before* the diagram's own styles, and the registry
//! lets the last declaration of a name win — so `--theme dark` on a file that
//! declares `style node { fill: #fff }` gives a dark diagram with a white box,
//! which is what the author asked for. A theme that overrode the file would
//! make `--theme` destructive rather than useful.
//!
//! The one thing a theme carries that a `.bds` cannot is the surface, because
//! `background` is a `graph` setting and a `.bds` holds styles only (`D-029`).
//! It is applied only when the file did not say, for the same reason.

use anyhow::{Result, bail};

use crate::ast::{Diagram, Prop, Span, Value};

pub struct Theme {
    pub name: &'static str,
    pub doc: &'static str,
    /// The surface, used only when the file declares no `background`.
    pub background: &'static str,
    /// `.bds` source — read it to know what the theme does.
    pub styles: &'static str,
}

pub const THEMES: &[Theme] = &[
    Theme {
        name: "light",
        doc: "Ink on paper. What the renderer already does with no theme at \
              all, written out so it can be asked for by name.",
        background: "#ffffff",
        styles: "\
style node  { fill: #ffffff stroke: #cbd5e1 1.4 font: 13 #0f172a radius: 6 }
style edge  { stroke: #64748b 1.5 font: 12 #475569 }
style group { fill: #f8fafc stroke: #cbd5e1 1.2 font: 12 #334155 }
",
    },
    Theme {
        name: "dark",
        doc: "For a slide deck or a dark editor. The ink is deliberately not \
              pure white: #f1f5f9 on a near-black surface reads as text, and \
              #ffffff reads as glare.",
        background: "#0b1120",
        styles: "\
style node  { fill: #1e293b stroke: #38bdf8 1.4 font: 13 #f1f5f9 radius: 6 }
style edge  { stroke: #64748b 1.5 font: 12 #94a3b8 }
style group { fill: #111c30 stroke: #334155 1.2 font: 12 #cbd5e1 }
",
    },
    Theme {
        name: "print",
        doc: "Black and white, for a page that may be photocopied. No fill \
              worth the toner, and every distinction carried by the stroke — \
              a colour scheme that survives a grayscale printer is one that \
              never relied on colour.",
        background: "#ffffff",
        styles: "\
style node  { fill: #ffffff stroke: #1a1a1a 1.4 font: 13 #1a1a1a radius: 3 }
style edge  { stroke: #1a1a1a 1.2 font: 12 #1a1a1a }
style group { fill: #ffffff stroke: #1a1a1a 1.0 font: 12 #1a1a1a }
",
    },
];

/// Put a named theme underneath a diagram's own styles.
pub fn apply(diagram: &mut Diagram, name: &str) -> Result<()> {
    let Some(theme) = THEMES.iter().find(|t| t.name == name) else {
        let known: Vec<&str> = THEMES.iter().map(|t| t.name).collect();
        bail!(
            "unknown theme `{name}` — the themes are: {}",
            known.join(", ")
        );
    };

    // The theme's own text is not part of the composed buffer, so its spans
    // would point at somebody else's bytes. They are zeroed rather than left
    // wrong: nothing should ever report one, and a test builds a registry from
    // every theme so a mistake in one is caught here rather than in a user's
    // file.
    let mut parsed = crate::parser::parse(theme.styles)
        .map_err(|e| anyhow::anyhow!("theme `{name}` does not parse: {}", e.message))?;
    for style in &mut parsed.styles {
        style.span = Span::default();
        style.name.span = Span::default();
        for prop in &mut style.props {
            prop.span = Span::default();
            prop.name_span = Span::default();
            prop.values_span = Span::default();
        }
    }

    // Underneath, so the file's own declarations win.
    parsed.styles.append(&mut diagram.styles);
    diagram.styles = parsed.styles;

    if !diagram
        .graph
        .iter()
        .any(|p| matches!(p.name.as_str(), "background" | "bg"))
    {
        diagram.graph.push(Prop {
            name: "background".into(),
            name_span: Span::default(),
            values: vec![Value::Color(theme.background.into())],
            values_span: Span::default(),
            span: Span::default(),
        });
    }

    Ok(())
}
