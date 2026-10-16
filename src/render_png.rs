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
    if let Some((family, bytes)) = &layout.opts.embed_font {
        load_embedded_font(&mut options, family, bytes)?;
    }

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

fn load_embedded_font(
    options: &mut resvg::usvg::Options,
    family: &str,
    bytes: &[u8],
) -> Result<()> {
    // usvg does not load the SVG's data-URL @font-face. Register its CSS
    // alias explicitly: a file stem need not match the font's internal name.
    let mut embedded = resvg::usvg::fontdb::Database::new();
    embedded.load_font_data(bytes.to_vec());
    anyhow::ensure!(
        !embedded.is_empty(),
        "the embedded font `{family}` contains no usable font faces"
    );
    let database = options.fontdb_mut();
    let replaced: Vec<_> = database
        .faces()
        .filter(|face| face.families.iter().any(|(name, _)| name == family))
        .cloned()
        .collect();
    for mut face in replaced {
        database.remove_face(face.id);
        face.families.retain(|(name, _)| name != family);
        if !face.families.is_empty() {
            face.id = resvg::usvg::fontdb::ID::dummy();
            database.push_face_info(face);
        }
    }
    for face in embedded.faces() {
        let mut face = face.clone();
        face.id = resvg::usvg::fontdb::ID::dummy();
        // Keep the parsed language without exposing the internal family as
        // another alias that could compete with unrelated installed fonts.
        face.families = vec![(family.to_owned(), face.families[0].1)];
        database.push_face_info(face);
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    const FONT: &[u8] = include_bytes!("../tests/fixtures/fonts/DejaVuSans.ttf");
    const FAMILY: &str = "Bgraph PNG Fixture";

    #[test]
    fn embedded_font_alias_renders_without_system_fonts() {
        let mut options = resvg::usvg::Options::default();
        assert!(options.fontdb.is_empty());
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="180" height="40"><text x="5" y="28" font-size="24" font-family="Bgraph PNG Fixture">Office 123</text></svg>"#;
        let raster = |options: &resvg::usvg::Options| {
            let tree = resvg::usvg::Tree::from_str(svg, options).unwrap();
            let mut pixmap = tiny_skia::Pixmap::new(180, 40).unwrap();
            resvg::render(
                &tree,
                tiny_skia::Transform::identity(),
                &mut pixmap.as_mut(),
            );
            pixmap
        };
        let missing = raster(&options);
        assert!(missing.pixels().iter().all(|pixel| pixel.alpha() == 0));

        load_embedded_font(&mut options, FAMILY, FONT).unwrap();
        let embedded = raster(&options);
        assert!(embedded.pixels().iter().any(|pixel| pixel.alpha() != 0));
        assert_ne!(embedded.data(), missing.data());
        assert_eq!(options.fontdb.faces().count(), 1);
        let face = options.fontdb.faces().next().unwrap();
        assert_eq!(face.families[0].0, FAMILY);
        assert_eq!(
            options
                .fontdb
                .with_face_data(face.id, |bytes, _| bytes == FONT),
            Some(true)
        );
    }

    #[test]
    fn embedded_font_takes_precedence_over_an_installed_alias() {
        let mut options = resvg::usvg::Options::default();
        load_embedded_font(&mut options, FAMILY, FONT).unwrap();
        let previous = options.fontdb.faces().next().unwrap().id;
        let mut bold = options.fontdb.face(previous).unwrap().clone();
        bold.id = resvg::usvg::fontdb::ID::dummy();
        bold.weight = resvg::usvg::fontdb::Weight::BOLD;
        let bold = options.fontdb_mut().push_face_info(bold);

        load_embedded_font(&mut options, FAMILY, FONT).unwrap();
        assert!(options.fontdb.face(previous).is_none());
        assert!(options.fontdb.face(bold).is_none());
        assert_eq!(options.fontdb.faces().count(), 1);
        assert_eq!(options.fontdb.faces().next().unwrap().families[0].0, FAMILY);
    }

    #[test]
    fn replacing_an_embedded_alias_preserves_other_installed_families() {
        use resvg::usvg::fontdb::{Family, Query, Source, Stretch, Style, Weight};

        let mut options = resvg::usvg::Options::default();
        options.fontdb_mut().load_font_data(FONT.to_vec());
        let mut installed = options.fontdb.faces().next().unwrap().clone();
        options.fontdb_mut().remove_face(installed.id);
        let language = installed.families[0].1;
        installed.families = vec![(FAMILY.into(), language), ("Other family".into(), language)];
        installed.weight = Weight::BOLD;
        installed.style = Style::Italic;
        installed.stretch = Stretch::Condensed;
        options.fontdb_mut().push_face_info(installed.clone());

        load_embedded_font(&mut options, FAMILY, FONT).unwrap();
        let embedded = options
            .fontdb
            .query(&Query {
                families: &[Family::Name(FAMILY)],
                ..Query::default()
            })
            .unwrap();
        let retained = options
            .fontdb
            .query(&Query {
                families: &[Family::Name("Other family")],
                weight: installed.weight,
                style: installed.style,
                stretch: installed.stretch,
            })
            .unwrap();
        assert_ne!(embedded, retained);
        assert_eq!(
            options.fontdb.face(embedded).unwrap().weight,
            Weight::NORMAL
        );
        let retained = options.fontdb.face(retained).unwrap();
        assert_eq!(retained.families, vec![("Other family".into(), language)]);
        assert_eq!(retained.index, installed.index);
        assert_eq!(retained.weight, installed.weight);
        assert_eq!(retained.style, installed.style);
        assert_eq!(retained.stretch, installed.stretch);
        let (Source::Binary(retained), Source::Binary(installed)) =
            (&retained.source, &installed.source)
        else {
            panic!("the fixture was loaded as binary data");
        };
        assert!(std::sync::Arc::ptr_eq(retained, installed));
    }

    #[test]
    fn an_invalid_embedded_font_is_reported() {
        let mut options = resvg::usvg::Options::default();
        let error = load_embedded_font(&mut options, FAMILY, b"not a font").unwrap_err();
        assert!(error.to_string().contains(FAMILY));
        assert!(options.fontdb.is_empty());
    }
}
