//! PNG output, for the places that cannot take a vector.
//!
//! Everything else this crate writes is hand-rolled — the SVG, the PDF, the
//! HTTP preview — because each of them is a format with a short honest
//! subset. A raster is not: PNG means zlib, and a rasteriser means text
//! shaping, curve flattening and antialiasing. So this one is a dependency,
//! and it is optional (`X-017` narrows rather than retires: raster becomes
//! opt-in, not present).
//!
//! It is a feature of its own rather than part of `window`. Someone rendering
//! images in a build has no display and wants no windowing library, and
//! someone with a window already has the rasteriser — so `window` implies
//! `png` and not the other way round.
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
