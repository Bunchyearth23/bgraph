//! W-167: external captions and edge labels retain their meaning after avoidance.
#![cfg(feature = "bpmn")]

use bgraph::layout::{Anchor, Annotation, Layout, Rect};

fn analyze(source: &str) -> bgraph::SourceAnalysis {
    let analysis = bgraph::analyze_source(source);
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    analysis
}

fn annotation_bounds(text: &Annotation) -> Rect {
    let w = bgraph::layout::text_width(std::slice::from_ref(&text.text), text.size);
    let x = match text.anchor {
        Anchor::Start => text.x,
        Anchor::Middle => text.x - w / 2.0,
        Anchor::End => text.x - w,
    };
    Rect {
        x,
        y: text.y - text.size * 0.6,
        w,
        h: text.size * 1.2,
    }
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.right() && a.right() > b.x && a.y < b.bottom() && a.bottom() > b.y
}

fn caption<'a>(layout: &'a Layout, text: &str) -> &'a Annotation {
    layout.annotations.iter().find(|a| a.text == text).unwrap()
}

const PROCESS: &str = r#"
type bpmn
style edge { head: none }
data c "c" object
gateway d "d" exclusive
task a "a"
event b "b" intermediate
layout {
  place c at (0, 0)
  place d right-of c gap 40
  place a right-of d gap 40
  place b below d gap 64
}
c -> d
d -> a
d -> b
"#;

#[test]
fn a_gateway_caption_clears_its_vertical_flow_without_moving_absolute_anchors() {
    let analysis = analyze(PROCESS);
    assert!(
        analysis.diagnostics.is_empty(),
        "{:?}",
        analysis.diagnostics
    );
    let layout = analysis.layout.unwrap();
    let gate = &layout.nodes[1];
    let text = annotation_bounds(caption(&layout, "d"));
    assert!(
        text.right() < gate.rect.cx() || text.x > gate.rect.cx(),
        "caption still straddles the departing vertical flow: {text:?}"
    );
    assert_eq!((layout.nodes[0].rect.x, layout.nodes[0].rect.y), (0.0, 0.0));
    let svg = bgraph::render_svg::render(&layout);
    assert_eq!(
        svg,
        bgraph::render_svg::render(&analyze(PROCESS).layout.unwrap())
    );
}

#[test]
fn explicit_caption_offsets_including_zero_remain_pinned_through_the_cascade() {
    for offset in ["0 0", "12 -3"] {
        for (styles, classes, own) in [
            (String::new(), "", format!("label-offset: {offset}")),
            (
                format!("style gateway {{ label-offset: {offset} }}"),
                "",
                String::new(),
            ),
            (
                format!("style pinned {{ label-offset: {offset} }}"),
                " .pinned",
                String::new(),
            ),
        ] {
            let source = format!(
                "type bpmn\n{styles}\ngateway g \"Caption\" exclusive{classes} {{ {own} }}\n\
                 task blocker \"Blocker\" {{ size: 100 40 }}\n\
                 layout {{\n place g at (0, 0)\n place blocker at (0, 60)\n}}"
            );
            let analysis = analyze(&source);
            let layout = analysis.layout.unwrap();
            let expected = if offset == "0 0" {
                (0.0, 0.0)
            } else {
                (12.0, -3.0)
            };
            assert_eq!(layout.nodes[0].style.label_offset, Some(expected));
            assert_eq!(
                caption(&layout, "Caption").x,
                layout.nodes[0].rect.cx() + expected.0
            );
            assert!(
                analysis
                    .diagnostics
                    .iter()
                    .any(|d| d.message.contains("the caption of `g`")),
                "a pinned collision must remain visible to the author: {:?}",
                analysis.diagnostics
            );
        }
    }
}

#[test]
fn declared_caption_sides_and_offsets_are_respected() {
    for side in ["top", "right", "bottom", "left"] {
        let source = format!(
            "type bpmn\nstyle event {{ label-position: {side} label-offset: 0 0 }}\n\
             event e \"Caption\" intermediate\nlayout {{ place e at (0, 0) }}"
        );
        let layout = analyze(&source).layout.unwrap();
        let shape = layout.nodes[0].rect;
        let text = annotation_bounds(caption(&layout, "Caption"));
        assert_eq!(layout.nodes[0].style.label_offset, Some((0.0, 0.0)));
        assert!(
            match side {
                "top" => text.bottom() < shape.y,
                "right" => text.x > shape.right(),
                "bottom" => text.y > shape.bottom(),
                "left" => text.right() < shape.x,
                _ => unreachable!(),
            },
            "{side}: {text:?} against {shape:?}"
        );
    }
}

#[test]
fn notes_and_generated_legend_boxes_do_not_become_caption_indices() {
    let source = PROCESS.replace("\"d\"", "\"First line\\nSecond line\"")
        + "\nnote \"Note text\" left-of d gap 20\n\
           style healthy { fill: #dcfce7 legend: \"Healthy\" }\n\
           legend below a gap 50\n";
    let analysis = analyze(&source);
    let layout = analysis.layout.unwrap();
    assert!(layout.nodes.len() > analysis.diagram.unwrap().nodes.len());
    for text in ["First line", "Second line"] {
        let bounds = annotation_bounds(caption(&layout, text));
        assert!(
            layout.nodes.iter().all(|n| !overlaps(bounds, n.rect)),
            "{text} overlaps a node or generated legend"
        );
    }
    let svg = bgraph::render_svg::render(&layout);
    for text in ["First line", "Second line", "Note text", "Healthy"] {
        assert!(svg.contains(text), "{text} missing from SVG");
    }
}

const LINK: &str = r#"
node a "A" { size: 40 40 }
node b "B" { size: 40 40 }
a -> b "Flow"
layout {
  place a at (0, 0)
  place b at (400, 0)
}
"#;

#[test]
fn edge_labels_avoid_annotations_and_report_explicitly_pinned_collisions() {
    let analysis = analyze(LINK);
    let diagram = analysis.diagram.unwrap();
    let mut layout = analysis.layout.unwrap();
    for title in [false, true] {
        layout.annotations = vec![Annotation {
            text: "Reserved text".into(),
            x: 220.0,
            y: 20.0,
            anchor: Anchor::Middle,
            size: 18.0,
            color: "#111111".into(),
            weight: "400".into(),
            family: String::new(),
            title,
        }];
        layout.edges[0].style.label_offset = None;
        let routes = bgraph::render_svg::route_all(&layout);
        let label = routes[0].label.as_ref().unwrap().1;
        assert!(!overlaps(label, annotation_bounds(&layout.annotations[0])));
        assert!(bgraph::check::after_layout(&diagram, &layout).is_empty());

        layout.edges[0].style.label_offset = Some((0.0, 0.0));
        let routes = bgraph::render_svg::route_all(&layout);
        let label = routes[0].label.as_ref().unwrap().1;
        assert_eq!((label.cx(), label.cy()), (220.0, 20.0));
        assert!(
            bgraph::check::after_layout(&diagram, &layout)
                .iter()
                .any(|d| d.message.contains("overlaps the text `Reserved text`"))
        );
    }
}

#[test]
fn edge_labels_avoid_shifted_node_text_outside_the_node_box() {
    let source =
        format!("{LINK}\nnode c \"Shifted caption\" {{ size: 140 40 label-offset: 0 -100 }}\n");
    let source = source.replace(
        "place b at (400, 0)",
        "place b at (400, 0)\n place c at (150, 100)",
    );
    let layout = analyze(&source).layout.unwrap();
    let label = bgraph::render_svg::route_all(&layout)[0]
        .label
        .as_ref()
        .unwrap()
        .1;
    let c = &layout.nodes[2];
    let text = Rect {
        x: c.rect.x,
        y: 20.0 - 14.0 * 0.6,
        w: c.rect.w,
        h: 14.0 * 1.2,
    };
    assert!(
        !overlaps(label, text),
        "edge label covers shifted node text: {label:?}"
    );
}

#[test]
fn edge_labels_avoid_group_titles_and_report_a_pinned_collision() {
    let source = format!(
        "{LINK}\ngroup frame \"Group title\" {{\n\
           font-size: 14\nlabel-offset: 0 -150\nnode c \"C\"\n\
         }}\n"
    )
    .replace(
        "place b at (400, 0)",
        "place b at (400, 0)\n place frame at (150, 150)",
    );
    let analysis = analyze(&source);
    let diagram = analysis.diagram.unwrap();
    let mut layout = analysis.layout.unwrap();
    let group = &layout.groups[0];
    let title = Rect {
        x: group.rect.x + 12.0,
        y: group.rect.y + 14.0 * 0.8 - 150.0,
        w: bgraph::layout::text_width(&group.lines, 14.0),
        h: 14.0,
    };
    let routes = bgraph::render_svg::route_all(&layout);
    assert!(!overlaps(routes[0].label.as_ref().unwrap().1, title));
    layout.edges[0].style.label_offset = Some((0.0, 0.0));
    assert!(
        bgraph::check::after_layout(&diagram, &layout)
            .iter()
            .any(|d| d
                .message
                .contains("overlaps another label or a group title"))
    );
}

#[test]
fn multiline_caption_footprints_reserve_relative_gaps_and_group_bounds() {
    let layout = analyze(
        r#"type bpmn
group flow "Flow" {
  event e "First line\nA longer second line" intermediate { label-position: right }
  task next "Next"
  layout { place next right-of e gap 17 }
}
layout { place flow at (0, 0) }"#,
    )
    .layout
    .unwrap();
    let frame = layout.groups[0].rect;
    let next = layout.nodes.iter().find(|n| n.id == "next").unwrap().rect;
    for text in ["First line", "A longer second line"] {
        let text = annotation_bounds(caption(&layout, text));
        assert!(text.x >= frame.x && text.y >= frame.y);
        assert!(text.right() <= frame.right() && text.bottom() <= frame.bottom());
        assert!(next.x - text.right() >= 17.0, "{next:?} against {text:?}");
    }
}

#[cfg(feature = "json")]
#[test]
fn json_uses_solved_captions_and_the_shared_edge_label_geometry() {
    let source = PROCESS.replace("d -> a", "d -> a \"Flow\"");
    let layout = analyze(&source).layout.unwrap();
    let routes = bgraph::render_svg::route_all(&layout);
    let (dx, dy, _, _) = bgraph::render_svg::canvas(&layout);
    let json: serde_json::Value =
        serde_json::from_str(&bgraph::render_json::render(&layout)).unwrap();
    for (edge, route) in json["edges"].as_array().unwrap().iter().zip(routes) {
        if let Some((_, rect)) = route.label {
            assert!((edge["label_rect"]["x"].as_f64().unwrap() - rect.x - dx).abs() < 1e-7);
            assert!((edge["label_rect"]["y"].as_f64().unwrap() - rect.y - dy).abs() < 1e-7);
        }
    }
    for (text, actual) in layout
        .annotations
        .iter()
        .zip(json["annotations"].as_array().unwrap())
    {
        assert!((actual["x"].as_f64().unwrap() - text.x - dx).abs() < 1e-7);
        assert!((actual["y"].as_f64().unwrap() - text.y - dy).abs() < 1e-7);
    }
}
