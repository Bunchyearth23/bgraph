//! PNG output, for the places that cannot take a vector.
//!
//! Everything else this crate writes is hand-rolled — the SVG, the PDF, the
//! HTTP preview — because each of them is a format with a short honest
//! subset. A raster is not: PNG means zlib, and a rasteriser means text
//! shaping, curve flattening and antialiasing. So this one is a dependency,
//! and it is optional (`X-017` narrows rather than retires: raster becomes
//! opt-in, not present).
//!
//! Off by default. Someone rendering images in a build asks for it; nobody
//! else pays for it, and the crate's other writers — SVG, PDF, the HTTP
//! preview — are hand-rolled precisely so that this stays the only one.
//!
//! The picture comes through [`crate::render_svg`] rather than from a second
//! drawing path. A raster that disagreed with the vector would be a second
//! renderer, and the whole reason `watch` shows the same SVG the file exports
//! is that there is only ever one.

use anyhow::{Context, Result};

use crate::layout::Layout;

/// Rasterise at `scale` times the diagram's own size.
///
/// A scale rather than a pixel size: the layout already decided the shape, and
/// asking for 1200 pixels wide would mean either distorting it or quietly
/// picking a height — both worse than saying "twice as big".
pub fn render(layout: &Layout, scale: f64) -> Result<Vec<u8>> {
    let svg = crate::render_svg::render(layout);
    let (_, _, width, height) = crate::render_svg::canvas(layout);

    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    resolve_generic_families(&mut options);

    let tree = resvg::usvg::Tree::from_str(&svg, &options)
        .map_err(|e| anyhow::anyhow!("the SVG this crate just wrote did not parse: {e}"))?;

    let scale = if scale.is_finite() && scale > 0.0 {
        scale
    } else {
        1.0
    };
    let w = (width * scale).round().max(1.0) as u32;
    let h = (height * scale).round().max(1.0) as u32;

    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .with_context(|| format!("{w}×{h} is larger than a pixmap can be"))?;

    // No fill. The SVG paints its own surface unless the author asked for
    // `background: none`, and painting one here would take that choice away —
    // a transparent PNG is exactly what `none` was written for.
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale as f32, scale as f32),
        &mut pixmap.as_mut(),
    );

    pixmap
        .encode_png()
        .context("the pixmap could not be encoded as a PNG")
}

/// Tell the renderer what the generic font families mean.
///
/// It moved here when the native window went (`W-136`): the window had this
/// and the PNG writer did not, so `-o diagram.png` had been carrying the
/// original bug all along. A fix worth keeping does not leave with the code
/// that happened to hold it.
///
/// A browser asks fontconfig; `resvg` does not, so `sans-serif` in an SVG
/// resolves to usvg's built-in guess — and the default stack this project
/// emits, `Inter, ui-sans-serif, system-ui, sans-serif`, is a *web* stack:
/// `ui-sans-serif` and `system-ui` are CSS generics that exist in no font
/// database, and `Inter` is on very few machines. Everything therefore lands
/// on the generic, and the generic was whatever usvg guessed — which is how a
/// diagram came out set in a face nothing else on the machine uses.
///
/// The lists are ordered by preference and filtered by what is actually
/// installed, so this states a wish rather than an assumption.
fn resolve_generic_families(options: &mut resvg::usvg::Options) {
    const SANS: &[&str] = &[
        "Inter",
        "Archivo",
        "Noto Sans",
        "DejaVu Sans",
        "Liberation Sans",
        "Arial",
        "Helvetica",
    ];
    const SERIF: &[&str] = &["Noto Serif", "DejaVu Serif", "Liberation Serif", "Georgia"];
    const MONO: &[&str] = &[
        "JetBrainsMono NFM",
        "JetBrains Mono",
        "Noto Sans Mono",
        "DejaVu Sans Mono",
        "Liberation Mono",
    ];

    // Read the installed families once, then hand the database back: the
    // lookup borrows it and the setters need it mutably.
    let families: std::collections::HashSet<String> = options
        .fontdb
        .faces()
        .flat_map(|f| f.families.iter().map(|(name, _)| name.clone()))
        .collect();
    let installed = |names: &[&str]| -> Option<String> {
        names
            .iter()
            .find(|n| families.contains(**n))
            .map(|n| n.to_string())
    };

    let (sans, serif, mono) = (installed(SANS), installed(SERIF), installed(MONO));

    if let Some(sans) = sans {
        options.fontdb_mut().set_sans_serif_family(sans.clone());
        // The last resort when a stack names nothing that exists — which is
        // exactly the case for the stack this project emits.
        options.font_family = sans;
    }
    if let Some(serif) = serif {
        options.fontdb_mut().set_serif_family(serif);
    }
    if let Some(mono) = mono {
        options.fontdb_mut().set_monospace_family(mono);
    }
}
