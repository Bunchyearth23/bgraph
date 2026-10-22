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
