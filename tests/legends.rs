use bgraph::layout::Rect;
use bgraph::{analyze_source, layout::Layout, render_svg};

fn drawing(source: &str) -> Layout {
    let analysis = analyze_source(source);
    assert!(
        !analysis.has_errors(),
        "{source}\n{:?}",
        analysis.diagnostics
    );
    analysis.layout.unwrap()
}

// ---------------------------------------------------------------- measurement
//
// A legend is judged by its geometry, not by eye: where it landed, what it
// covers, whether it stayed on the page. The helpers below turn a layout into
// the numbers a report can quote, so "it looks fine" is never the evidence.

/// A rectangle together with the thing that owns it, so a failure names it.
#[derive(Clone, Debug)]
struct Boxed {
    what: String,
    rect: Rect,
}

fn intersects(a: &Rect, b: &Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

/// Everything the drawing occupies, before a legend is added.
fn content(layout: &Layout) -> Vec<Boxed> {
    let mut out = Vec::new();
    for node in &layout.nodes {
        out.push(Boxed {
            what: if node.id.is_empty() {
                "chart box".to_string()
            } else {
                format!("node {}", node.id)
            },
            rect: node.rect,
        });
    }
    for group in &layout.groups {
        out.push(Boxed {
            what: format!("group {}", group.id),
            rect: group.rect,
        });
    }
    for annotation in &layout.annotations {
        out.push(Boxed {
            what: format!("text {:?}", annotation.text),
            rect: bgraph::charts::annotation_bounds(annotation),
        });
    }
    out
}

/// The lowest thing the drawing reaches, in diagram coordinates.
fn content_bottom(layout: &Layout) -> f64 {
    content(layout)
        .iter()
        .map(|b| b.rect.bottom())
        .fold(f64::MIN, f64::max)
}

const LEGEND_TITLE: &str = "Legend title";
const CLASS_ENTRY: &str = "Class meaning";
const INLINE_ENTRY: &str = "Inline meaning";

/// A legend with one class-based entry, one inline entry and a title.
fn legend_source(source: &str) -> String {
    format!(
        "{source}\nstyle legendkey {{ fill: #123456 stroke: #654321 }}\nlegend {{\n  title: \"{LEGEND_TITLE}\"\n  entry \"{CLASS_ENTRY}\" .legendkey\n  entry \"{INLINE_ENTRY}\" {{ fill: #abcdef }}\n}}\n"
    )
}

/// What the legend drew: its swatches and the text it wrote.
fn legend_elements(layout: &Layout, swatch_count: usize) -> Vec<Boxed> {
    let mut out = Vec::new();
    for node in layout.nodes.iter().skip(layout.nodes.len() - swatch_count) {
        out.push(Boxed {
            what: "legend swatch".to_string(),
            rect: node.rect,
        });
    }
    for annotation in &layout.annotations {
        if matches!(
            annotation.text.as_str(),
            LEGEND_TITLE | CLASS_ENTRY | INLINE_ENTRY
        ) {
            out.push(Boxed {
                what: format!("legend text {:?}", annotation.text),
                rect: bgraph::charts::annotation_bounds(annotation),
            });
        }
    }
    out
}

/// The relative luminance of a `#rrggbb` colour, as WCAG defines it.
fn luminance(hex: &str) -> f64 {
    let h = hex.trim_start_matches('#');
    let channel = |from: usize| {
        let byte = u8::from_str_radix(&h[from..from + 2], 16).unwrap_or(0) as f64 / 255.0;
        if byte <= 0.039_28 {
            byte / 12.92
        } else {
            ((byte + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(0) + 0.7152 * channel(2) + 0.0722 * channel(4)
}

/// The WCAG contrast ratio of two colours, from 1 (identical) to 21.
fn contrast(a: &str, b: &str) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    (hi + 0.05) / (lo + 0.05)
}

#[test]
fn explicit_entries_use_their_own_labels_and_cascade_in_block_order() {
    let layout = drawing(
        r##"
style node { fill: #000000 }
style base { fill: #dcfce7 stroke: #166534 2 dashed legend: "Old name" }
style live extends base { shape: circle }
node api "API" .live
legend {
  title: "Status <&>"
  font-size: 12
  font-weight: 600
  entry "Available\nServing requests" .live { stroke: #123456 }
  entry "Planned" { fill: #fef3c7 stroke: #92400e }
}
"##,
    );
    let labels: Vec<_> = layout.annotations.iter().map(|a| a.text.as_str()).collect();
    assert_eq!(
        labels,
        ["Status <&>", "Available", "Serving requests", "Planned"]
    );
    let swatches: Vec<_> = layout.nodes.iter().filter(|n| n.id.is_empty()).collect();
    assert_eq!(swatches.len(), 2);
    assert_eq!(swatches[0].style.fill.as_deref(), Some("#dcfce7"));
    assert_eq!(swatches[0].style.stroke.as_deref(), Some("#123456"));
    assert_eq!(swatches[0].style.stroke_width, Some(2.0));
    assert_eq!(swatches[0].style.dash.as_deref(), Some("dashed"));
    assert_eq!(swatches[0].style.shape, Some(bgraph::style::Shape::Ellipse));
    assert_eq!(layout.annotations[1].weight, "600");
    assert!(swatches[1].rect.y > layout.annotations[2].y);
    assert!(render_svg::render(&layout).contains("Status &lt;&amp;&gt;"));
    #[cfg(feature = "pdf")]
    assert!(
        bgraph::render_pdf::render(&layout)
            .unwrap()
            .starts_with(b"%PDF-")
    );
}

#[test]
fn every_archetype_draws_an_explicit_legend_below_its_existing_content() {
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let before = drawing(&source);
        let (_, dy, _, height) = render_svg::canvas(&before);
        let bottom = height - dy - before.opts.padding;
        let with_legend = drawing(&format!(
            "{source}\nlegend {{\n title: \"Legend title\"\n entry \"Custom meaning\" {{ fill: #123456 }}\n}}\n"
        ));
        assert_eq!(
            with_legend.nodes.len(),
            before.nodes.len() + 1,
            "{}",
            def.name
        );
        for (old, new) in before.nodes.iter().zip(&with_legend.nodes) {
            assert_eq!(
                (old.rect.x, old.rect.y, old.rect.w, old.rect.h),
                (new.rect.x, new.rect.y, new.rect.w, new.rect.h),
                "{}",
                def.name
            );
        }
        let title = with_legend
            .annotations
            .iter()
            .find(|a| a.text == "Legend title")
            .unwrap();
        assert!(
            title.y - title.size >= bottom,
            "{}: legend {} above content bottom {bottom}",
            def.name,
            title.y
        );
        let svg = render_svg::render(&with_legend);
        assert!(
            svg.contains("Custom meaning") && svg.contains("#123456"),
            "{}",
            def.name
        );
        let (dx, dy, width, height) = render_svg::canvas(&with_legend);
        let swatch = with_legend.nodes.last().unwrap().rect;
        assert!(swatch.x + dx >= 0.0 && swatch.y + dy >= 0.0);
        assert!(swatch.right() + dx <= width && swatch.bottom() + dy <= height);
    }
}

#[test]
fn automatic_legends_clear_moved_labels_and_previous_legends() {
    let layout = drawing(
        r##"
node a "Moved label" { label-offset: 0 160 }
legend { entry "First" { fill: #dcfce7 } }
legend { entry "Second" { fill: #fef3c7 } }
"##,
    );
    let node = &layout.nodes[0];
    let moved_bottom = node.rect.cy() + 160.0 + 14.0 * 1.3 / 2.0;
    assert!(layout.nodes[1].rect.y > moved_bottom);
    assert!(layout.nodes[2].rect.y > layout.nodes[1].rect.bottom());
}

#[test]
fn relative_and_absolute_legends_keep_their_declared_position() {
    let layout = drawing(
        r##"
group frame { node a }
layout { place frame at (120, 60) }
legend right-of frame gap 30 align top { entry "Relative" }
legend at (-100, 300) { entry "Absolute" }
"##,
    );
    assert_eq!(layout.nodes[1].rect.x, layout.groups[0].rect.right() + 30.0);
    assert_eq!(layout.nodes[2].rect.x, -100.0);
    assert_eq!(layout.nodes[2].rect.y, 307.0);
}

#[test]
fn a_dark_surface_gets_readable_legend_ink_unless_overridden() {
    let layout = drawing(
        r##"
graph { background: #111827 }
node a
legend { entry "Default ink" }
legend {
  font-color: #facc15
  entry "Custom ink"
}
"##,
    );
    assert_eq!(layout.annotations[0].color, "#f1f5f9");
    assert_eq!(layout.annotations[1].color, "#facc15");
}

#[test]
fn invalid_legend_properties_classes_and_sizes_are_diagnostics() {
    for body in [
        "legend { typo: 1 }",
        "legend { title: 42 }",
        "legend { font-size: -1 }",
        "legend { font-size: large }",
        "legend .missing { entry \"Meaning\" }",
        "legend { entry \"Meaning\" .missing }",
        "legend { entry \"Meaning\" { fill: #12345 } }",
        "legend { entry \"Meaning\" { shape: unknown } }",
        "legend { entry \"Meaning\" { router: ortho } }",
        "style tiny { font: -2 }\nlegend .tiny { entry \"Meaning\" }",
    ] {
        let source = format!("node a\n{body}");
        let analysis = analyze_source(&source);
        assert!(analysis.has_errors(), "accepted invalid legend: {body}");
    }
}

#[test]
fn legacy_style_keys_work_without_a_position_and_entries_stay_contextual() {
    let layout = drawing(
        r##"
style live { fill: #dcfce7 stroke: #166534 legend: "Healthy" }
node entry .live
legend
"##,
    );
    assert_eq!(layout.annotations[0].text, "Healthy");
    assert_eq!(layout.nodes[1].style.stroke.as_deref(), Some("#166534"));
    let source = "node entry\ngroup entry_group { node child }\nentry -> child";
    assert!(!analyze_source(source).has_errors());
}

#[test]
fn formatting_keeps_legend_entries_and_their_descriptions() {
    let source = r##"node a
legend { title: "Meaning"; entry "First" { fill: #dcfce7 }; entry "Second" }
"##;
    let formatted = bgraph::fmt::format(source).unwrap();
    assert_eq!(
        render_svg::render(&drawing(source)),
        render_svg::render(&drawing(&formatted))
    );
}

#[cfg(feature = "packet")]
#[test]
fn managed_graphs_accept_absolute_keys_and_refuse_unsupported_anchors() {
    let source = "type packet\nfield a 0 31\n";
    let layout = drawing(&format!("{source}legend at (0, 300) {{ entry \"Field\" }}"));
    assert_eq!(layout.nodes.last().unwrap().rect.y, 307.0);
    let analysis = analyze_source(&format!("{source}legend below a {{ entry \"Field\" }}"));
    assert!(analysis.has_errors());
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|d| d.message.contains("automatic placement"))
    );
}

// ------------------------------------------------------- the whole surface
//
// The nine tests above cover the *rules*. These three cover the *surface*:
// every archetype, every placement rule, measured rather than judged.

#[test]
fn every_archetype_measures_a_legend_that_clears_its_drawing() {
    let mut table = String::new();
    let mut covered = 0usize;
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let before = drawing(&source);
        let with = drawing(&legend_source(&source));

        // The legend adds exactly its own two swatches; the drawing is untouched.
        assert_eq!(
            with.nodes.len(),
            before.nodes.len() + 2,
            "{}: the legend changed the drawing",
            def.name
        );
        for (old, new) in before.nodes.iter().zip(&with.nodes) {
            assert_eq!(
                (old.rect.x, old.rect.y, old.rect.w, old.rect.h),
                (new.rect.x, new.rect.y, new.rect.w, new.rect.h),
                "{}",
                def.name
            );
        }

        // Both swatches carry the paint they were asked for: one from a class,
        // one written inline.
        let swatches = &with.nodes[before.nodes.len()..];
        assert_eq!(swatches.len(), 2, "{}", def.name);
        assert_eq!(
            swatches[0].style.fill.as_deref(),
            Some("#123456"),
            "{}",
            def.name
        );
        assert_eq!(
            swatches[0].style.stroke.as_deref(),
            Some("#654321"),
            "{}",
            def.name
        );
        assert_eq!(
            swatches[1].style.fill.as_deref(),
            Some("#abcdef"),
            "{}",
            def.name
        );

        // The title and both entries were written.
        for text in [LEGEND_TITLE, CLASS_ENTRY, INLINE_ENTRY] {
            assert!(
                with.annotations.iter().any(|a| a.text == text),
                "{}: {text} was not drawn",
                def.name
            );
        }

        // The legend sits below the drawing and covers none of it.
        let legend = legend_elements(&with, 2);
        let bottom = content_bottom(&before);
        let top = legend.iter().map(|b| b.rect.y).fold(f64::MAX, f64::min);
        let gap = top - bottom;
        assert!(
            gap >= 0.0,
            "{}: the legend starts {gap:.1} above the drawing bottom {bottom:.1}",
            def.name
        );
        let drawn = content(&before);
        let overlaps: Vec<String> = legend
            .iter()
            .flat_map(|l| {
                drawn
                    .iter()
                    .filter(move |c| intersects(&l.rect, &c.rect))
                    .map(move |c| format!("{} over {}", l.what, c.what))
            })
            .collect();
        assert!(overlaps.is_empty(), "{}: {overlaps:?}", def.name);

        // Everything the legend drew is on the page.
        let (dx, dy, width, height) = render_svg::canvas(&with);
        let mut slack = f64::MAX;
        for element in &legend {
            let r = element.rect;
            assert!(
                r.x + dx >= 0.0
                    && r.y + dy >= 0.0
                    && r.right() + dx <= width
                    && r.bottom() + dy <= height,
                "{}: {} left the canvas",
                def.name,
                element.what
            );
            slack = slack
                .min(r.x + dx)
                .min(r.y + dy)
                .min(width - (r.right() + dx))
                .min(height - (r.bottom() + dy));
        }

        table.push_str(&format!(
            "{:<13} {:<8} bottom {:>8.1}  legend top {:>8.1}  gap {:>7.1}  overlaps {:>2}  slack {:>6.1}  swatches 2  title yes\n",
            def.name,
            if def.layout.accepts_layout_block() {
                "declared"
            } else {
                "managed"
            },
            bottom,
            top,
            gap,
            overlaps.len(),
            slack
        ));
        covered += 1;
    }
    assert_eq!(covered, bgraph::props::ARCHETYPES.len());
    println!("{table}");
}

#[test]
fn every_archetype_writes_its_legend_into_the_svg() {
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let with = drawing(&legend_source(&source));
        let svg = render_svg::render(&with);
        for text in [LEGEND_TITLE, CLASS_ENTRY, INLINE_ENTRY] {
            assert!(svg.contains(text), "{}: {text} is not in the SVG", def.name);
        }
        // The swatch paint reached the picture, not just the layout.
        assert!(
            svg.contains("#123456"),
            "{}: the class swatch is missing",
            def.name
        );
        assert!(
            svg.contains("#abcdef"),
            "{}: the inline swatch is missing",
            def.name
        );
    }
}

/// The archetypes that draw a key of their own. The `legend { … }` block is a
/// second thing: it must not swallow the chart's own key, nor move it.
const NATIVE_KEY_ARCHETYPES: &[&str] = &["pie", "bar", "xy", "radar", "journey", "venn"];

#[test]
fn a_native_chart_key_survives_the_legend_block_untouched() {
    let mut checked = 0usize;
    for name in NATIVE_KEY_ARCHETYPES {
        let Some(def) = bgraph::props::ARCHETYPES.iter().find(|d| d.name == *name) else {
            // A reduced build does not carry this archetype.
            continue;
        };
        let source = bgraph::init::skeleton(def.name).unwrap();
        let before = drawing(&source);
        let with = drawing(&legend_source(&source));

        // The chart's own key is there before the legend block, and it is
        // still there, in the same place, afterwards. The heading is exempt:
        // it is centred on the whole drawing, so a wider drawing moves it —
        // by design, and not part of the key.
        let key_of = |layout: &Layout| -> Vec<(String, i64, i64)> {
            layout
                .annotations
                .iter()
                .filter(|a| !a.title)
                .filter(|a| !matches!(a.text.as_str(), LEGEND_TITLE | CLASS_ENTRY | INLINE_ENTRY))
                .map(|a| {
                    (
                        a.text.clone(),
                        (a.x * 1000.0).round() as i64,
                        (a.y * 1000.0).round() as i64,
                    )
                })
                .collect()
        };
        let native = key_of(&before);
        assert!(!native.is_empty(), "{name}: the chart drew no key at all");
        assert_eq!(native, key_of(&with), "{name}: the chart's key changed");
        checked += 1;
    }
    assert!(checked <= NATIVE_KEY_ARCHETYPES.len());
    #[cfg(feature = "all-archetypes")]
    assert_eq!(
        checked,
        NATIVE_KEY_ARCHETYPES.len(),
        "an archetype with a native key was not exercised"
    );
}

/// The legacy key — a style that carries its own meaning — still works in
/// every archetype, and still draws a swatch.
#[test]
fn every_archetype_keeps_the_legacy_style_key() {
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let with = drawing(&format!(
            "{source}\nstyle legacykey {{ fill: #123456 legend: \"Legacy meaning\" }}\nlegend\n"
        ));
        assert!(
            with.annotations.iter().any(|a| a.text == "Legacy meaning"),
            "{}: the legacy key drew no text",
            def.name
        );
        assert!(
            with.nodes
                .iter()
                .any(|n| n.style.fill.as_deref() == Some("#123456")),
            "{}: the legacy key drew no swatch",
            def.name
        );
    }
}

#[test]
fn every_archetype_legend_text_is_readable_against_its_background() {
    let mut table = String::new();
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let with = drawing(&legend_source(&source));
        let background = with.opts.background.clone().unwrap_or("#ffffff".into());
        for annotation in &with.annotations {
            if !matches!(
                annotation.text.as_str(),
                LEGEND_TITLE | CLASS_ENTRY | INLINE_ENTRY
            ) {
                continue;
            }
            let ratio = contrast(&annotation.color, &background);
            assert!(
                ratio >= 4.5,
                "{}: legend text {:?} on {background} has a contrast of {ratio:.2}",
                def.name,
                annotation.text
            );
            if annotation.text == LEGEND_TITLE {
                table.push_str(&format!(
                    "{:<13} ink {:<9} on {:<9} contrast {:>5.2}\n",
                    def.name, annotation.color, background, ratio
                ));
            }
        }
    }
    println!("{table}");
}

#[test]
fn every_archetype_accepts_an_absolute_legend_where_it_was_asked() {
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let with = drawing(&format!(
            "{source}\nlegend at (200, 500) {{ entry \"Absolute\" }}\n"
        ));
        let swatch = with.nodes.last().unwrap();
        // The block's top-left is the requested point; the swatch sits half a
        // line below it, centred on its own line.
        assert_eq!(swatch.rect.x, 200.0, "{}", def.name);
        let text = with
            .annotations
            .iter()
            .find(|a| a.text == "Absolute")
            .unwrap_or_else(|| panic!("{}: the absolute legend wrote no text", def.name));
        // The title's middle sits half a line below the block's top edge, so
        // the block's own top edge is `y - size`.
        assert!(
            (text.y - text.size - 500.0).abs() < 1e-9,
            "{}: the absolute legend landed at {}",
            def.name,
            text.y - text.size
        );
        assert!(swatch.rect.y >= 500.0, "{}", def.name);
    }
}

#[test]
fn relative_legends_are_accepted_by_declared_archetypes_and_refused_by_managed_ones() {
    let mut declared = 0usize;
    let mut managed = 0usize;
    let mut table = String::new();
    for def in bgraph::props::ARCHETYPES {
        let source = bgraph::init::skeleton(def.name).unwrap();
        let diagram = bgraph::parser::parse(&source).unwrap();
        let anchor = diagram
            .nodes
            .iter()
            .map(|n| n.id.as_str())
            .find(|id| !id.is_empty())
            .map(str::to_string)
            .or_else(|| {
                diagram
                    .groups
                    .iter()
                    .map(|g| g.id.as_str())
                    .find(|id| !id.is_empty())
                    .map(str::to_string)
            });
        let Some(anchor) = anchor else {
            panic!(
                "{}: the example names nothing to anchor a legend to",
                def.name
            );
        };
        let with_anchor =
            format!("{source}\nlegend below {anchor} align left {{ entry \"Anchored\" }}\n");
        let analysis = analyze_source(&with_anchor);

        if def.layout.accepts_layout_block() {
            declared += 1;
            assert!(
                !analysis.has_errors(),
                "{}: a declared layout refused a relative legend: {:?}",
                def.name,
                analysis.diagnostics
            );
            let layout = analysis.layout.unwrap();
            let swatch = layout.nodes.last().unwrap();
            let anchor_rect = layout
                .nodes
                .iter()
                .find(|n| n.id == anchor)
                .map(|n| n.rect)
                .or_else(|| {
                    layout
                        .groups
                        .iter()
                        .find(|g| g.id == anchor)
                        .map(|g| g.rect)
                })
                .unwrap_or_else(|| panic!("{}: {anchor} was never placed", def.name));
            assert_eq!(
                swatch.rect.x, anchor_rect.x,
                "{}: the legend left its anchor",
                def.name
            );
            assert!(
                swatch.rect.y > anchor_rect.bottom(),
                "{}: the legend is not below its anchor",
                def.name
            );
            table.push_str(&format!(
                "{:<13} declared  anchor {:<10} at ({:>7.1},{:>7.1})  legend at ({:>7.1},{:>7.1})  below by {:>6.1}\n",
                def.name,
                anchor,
                anchor_rect.x,
                anchor_rect.y,
                swatch.rect.x,
                swatch.rect.y,
                swatch.rect.y - anchor_rect.bottom()
            ));
        } else {
            managed += 1;
            assert!(
                analysis.has_errors(),
                "{}: a managed layout accepted a relative legend",
                def.name
            );
            assert!(
                analysis
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("automatic placement")),
                "{}: the refusal did not say what to write instead: {:?}",
                def.name,
                analysis.diagnostics
            );
        }
    }
    println!("{table}");
    assert_eq!(declared + managed, bgraph::props::ARCHETYPES.len());
    assert!(declared > 0, "no declared archetype was exercised");
    // The archetype table is feature-gated, so the full count only means
    // something in the full build.
    #[cfg(feature = "all-archetypes")]
    {
        assert_eq!(bgraph::props::ARCHETYPES.len(), 30);
        assert_eq!(declared, 9, "the declared archetypes changed");
        assert_eq!(managed, 21, "the managed archetypes changed");
    }
}
