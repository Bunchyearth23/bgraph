//! Regressions from the 1.0.1 user report.

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
fn a_nested_frame_can_anchor_to_a_sibling_before_the_parent_is_placed() {
    let layout = drawing(
        "node root\n\
         group outer {\n\
           node n\n\
           group inner { node m }\n\
           layout { place inner below n gap 30 }\n\
         }\n\
         layout { place outer right-of root gap 50 }",
    );
    let root = layout
        .nodes
        .iter()
        .find(|node| node.id == "root")
        .unwrap()
        .rect;
    let n = layout
        .nodes
        .iter()
        .find(|node| node.id == "n")
        .unwrap()
        .rect;
    let inner = layout
        .groups
        .iter()
        .find(|group| group.id == "inner")
        .unwrap()
        .rect;
    let outer = layout
        .groups
        .iter()
        .find(|group| group.id == "outer")
        .unwrap()
        .rect;
    assert!((outer.x - root.right() - 50.0).abs() < 1e-7);
    assert!((inner.y - n.bottom() - 30.0).abs() < 1e-7);
}

#[cfg(feature = "sequence")]
#[test]
fn sequence_frames_paint_below_messages_and_fit_the_canvas() {
    let layout = drawing(
        "type sequence\nstyle edge { stroke: #ff0000 }\n\
         participant a\nparticipant b\n\
         group g \"A long frame title reaching left of the first participant\" {\n\
           fill: #00ff0080\na -> b \"message\"\n\
         }",
    );
    let frame = layout.groups[0].rect;
    assert!(frame.x < 0.0, "exercise a negative frame origin");
    let (dx, dy, width, height) = render_svg::canvas(&layout);
    assert!(frame.x + dx >= 0.0 && frame.y + dy >= 0.0);
    assert!(frame.right() + dx <= width && frame.bottom() + dy <= height);
    let svg = render_svg::render(&layout);
    assert!(svg.find("fill-opacity=\"0.5\"").unwrap() < svg.find("<polyline").unwrap());
    #[cfg(feature = "pdf")]
    {
        let bytes = bgraph::render_pdf::render(&layout).unwrap();
        let pdf = String::from_utf8_lossy(&bytes);
        assert!(pdf.find("0 1 0 rg").unwrap() < pdf.find("1 0 0 RG").unwrap());
    }
}

#[test]
fn an_empty_edge_label_has_no_plate_and_does_not_move_the_picture() {
    let base = "node a\nnode b\nlayout { place b right-of a gap 100 }\n";
    let empty = drawing(&format!("{base}a -> b \"\""));
    let absent = drawing(&format!("{base}a -> b"));
    assert!(render_svg::route_all(&empty)[0].label.is_none());
    assert_eq!(render_svg::render(&empty), render_svg::render(&absent));
}

#[test]
fn a_font_shorthand_with_two_sizes_warns_and_names_the_weight_property() {
    let analysis = analyze_source("node a { font: 12 #111 600 }");
    assert!(!analysis.has_errors());
    assert!(
        analysis.diagnostics.iter().any(|diag| {
            diag.severity == bgraph::check::Severity::Warning
                && diag.message.contains("font-weight")
                && diag.message.contains("font")
        }),
        "{:?}",
        analysis.diagnostics
    );
    assert!(
        !analyze_source("node a { font: 12 #111 font-weight: 600 }")
            .diagnostics
            .iter()
            .any(|diag| diag.message.contains("font-weight"))
    );
}

#[test]
fn a_legend_without_a_position_is_valid_and_a_bad_anchor_is_an_error() {
    let analysis = analyze_source("node a\nlegend");
    assert!(!analysis.has_errors());
    assert!(
        !analysis.diagnostics.iter().any(|diag| {
            diag.severity == bgraph::check::Severity::Warning
                && diag.message.contains("legend")
                && diag.message.contains("position")
        }),
        "{:?}",
        analysis.diagnostics
    );
    assert!(analyze_source("node a\nlegend below missing").has_errors());
    assert!(!analyze_source("node a\nlegend below a").has_errors());
}

#[test]
fn cylinder_labels_clear_both_caps_even_with_small_padding() {
    for genre in ["free", "network", "block"] {
        if !bgraph::props::ARCHETYPES
            .iter()
            .any(|def| def.name == genre)
        {
            continue;
        }
        let layout = drawing(&format!(
            "type {genre}\nnode db \"First\\nSecond\\nThird\\nFourth\\nFifth\" {{ shape: cylinder padding: 2 font-size: 14 }}"
        ));
        let node = &layout.nodes[0];
        let cap = (node.rect.h * 0.16).min(14.0);
        if genre == "block" {
            for label in &layout.annotations {
                assert!(
                    label.y - label.size / 2.0 >= node.rect.y + 2.0 * cap,
                    "{genre}: {} at {}",
                    label.text,
                    label.y
                );
                assert!(
                    label.y + label.size / 2.0 <= node.rect.bottom() - cap,
                    "{genre}: {} at {}",
                    label.text,
                    label.y
                );
            }
        } else {
            let svg = render_svg::render(&layout);
            let text = svg.split("<text ").nth(1).unwrap();
            let first_y: f64 = text
                .split(" y=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap()
                .parse()
                .unwrap();
            assert!(
                first_y - 7.0 >= node.rect.y + 2.0 * cap,
                "{genre}: {first_y}, {:?}",
                node.rect
            );
            let last_y = first_y + 4.0 * 14.0 * 1.3;
            assert!(
                last_y + 7.0 <= node.rect.bottom() - cap,
                "{genre}: {last_y}, {:?}",
                node.rect
            );
        }
    }
}

#[test]
fn group_dimension_conflicts_name_the_winning_style_layer() {
    for (styles, class, inline, origin) in [
        ("style node { width: 230 }", "", "", "style node"),
        (
            "style node { width: 230 }\nstyle group { width: 230 }",
            "",
            "",
            "style group",
        ),
        (
            "style node { width: 230 }\nstyle narrow { width: 230 }",
            ".narrow",
            "",
            "style narrow",
        ),
        ("style node { width: 230 }", "", "width: 230", "own block"),
    ] {
        let source = format!("{styles}\ngroup g {class} {{\n{inline}\nnode a\nnode b\n}}");
        let analysis = analyze_source(&source);
        assert!(analysis.has_errors(), "{source}");
        assert!(
            analysis.diagnostics.iter().any(|diag| {
                diag.message.contains("g.width == 230") && diag.message.contains(origin)
            }),
            "{source}\n{:?}",
            analysis.diagnostics
        );
    }
}

#[cfg(feature = "json")]
#[test]
fn json_exposes_the_layout_origin_for_inverse_canvas_conversion() {
    let layout = drawing(
        "graph { title: \"Coordinates\" padding: 36 }\n\
         group g \"Frame\" {\npadding: 16\nnode a\n}\n\
         node b\nlayout {\nplace g at (-100, -50)\nplace b right-of g gap 100\n}\n\
         a -> b via (right-of g gap 40) { router: ortho }",
    );
    let json: serde_json::Value =
        serde_json::from_str(&bgraph::render_json::render(&layout)).unwrap();
    let x = json["canvas"]["layout_origin"]["x"].as_f64().unwrap();
    let y = json["canvas"]["layout_origin"]["y"].as_f64().unwrap();
    for (placed, exported) in layout.nodes.iter().zip(json["nodes"].as_array().unwrap()) {
        assert!((exported["rect"]["x"].as_f64().unwrap() - x - placed.rect.x).abs() < 1e-7);
        assert!((exported["rect"]["y"].as_f64().unwrap() - y - placed.rect.y).abs() < 1e-7);
    }
    let group = &layout.groups[0];
    assert_eq!(
        layout.edges[0].via,
        [(group.rect.right() + 40.0, group.rect.cy())]
    );
    for ((px, py), point) in render_svg::route_all(&layout)[0]
        .points
        .iter()
        .zip(json["edges"][0]["points"].as_array().unwrap())
    {
        assert!((point[0].as_f64().unwrap() - x - px).abs() < 1e-7);
        assert!((point[1].as_f64().unwrap() - y - py).abs() < 1e-7);
    }
}

#[cfg(feature = "png")]
#[test]
fn rounded_waypoints_with_an_empty_label_have_a_continuous_raster_stroke() {
    let layout = drawing(
        "style node { stroke: #ff0000 2 }\n\
         node a { size: 20 20 }\nnode b { size: 20 20 }\n\
         layout {\nplace a at (0, 50)\nplace b at (300, 50)\n}\n\
         a -- b \"\" via (80, 0) (240, 0) {\n\
           router: ortho bend: 12 from: right to: left stroke: #ff0000 2 head: none\n\
         }",
    );
    let png = bgraph::render_png::render(&layout, 3.0).unwrap();
    let pixels = tiny_skia::Pixmap::decode_png(&png).unwrap();
    let (dx, dy, _, _) = render_svg::canvas(&layout);
    for pair in render_svg::route_all(&layout)[0].points.windows(2) {
        let length = (pair[1].0 - pair[0].0).hypot(pair[1].1 - pair[0].1);
        for step in 0..=length.ceil() as usize {
            let t = step as f64 / length.ceil().max(1.0);
            let x = ((pair[0].0 + t * (pair[1].0 - pair[0].0) + dx) * 3.0).round() as u32;
            let y = ((pair[0].1 + t * (pair[1].1 - pair[0].1) + dy) * 3.0).round() as u32;
            let colour = pixels.pixel(x, y).unwrap();
            assert!(
                colour.red() > 200 && colour.green() < 80,
                "gap at ({x}, {y}): {colour:?}"
            );
        }
    }
}
