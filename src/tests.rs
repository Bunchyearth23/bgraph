use crate::layout::{self, Layout};
use crate::parser::parse_diagram;
use crate::render_svg;
use crate::style::{Head, Router, Shape};

fn build(source: &str) -> Layout {
    let diagram = parse_diagram(source).expect("source should parse");
    layout::solve(&diagram).expect("layout should solve")
}

fn err(source: &str) -> String {
    let diagram = match parse_diagram(source) {
        Ok(d) => d,
        Err(e) => return format!("{e:#}"),
    };
    match layout::solve(&diagram) {
        Ok(_) => panic!("expected an error"),
        Err(e) => format!("{e:#}"),
    }
}

#[test]
fn parses_nodes_edges_and_labels() {
    let l = build(
        r#"
        node a "Alpha"
        node b "Beta"
        a -> b "flows"
    "#,
    );

    assert_eq!(l.nodes.len(), 2);
    assert_eq!(l.nodes[0].lines, vec!["Alpha"]);
    assert_eq!(l.edges.len(), 1);
    assert_eq!(l.edges[0].label.as_deref(), Some("flows"));
    assert!(l.edges[0].arrow.head_end);
    assert!(!l.edges[0].arrow.head_start);
}

#[test]
fn label_defaults_to_the_identifier() {
    let l = build("node lonely");
    assert_eq!(l.nodes[0].lines, vec!["lonely"]);
}

#[test]
fn escaped_newline_splits_the_label() {
    let l = build(r#"node a "one\ntwo""#);
    assert_eq!(l.nodes[0].lines, vec!["one", "two"]);
}

#[test]
fn comments_do_not_swallow_hex_colors() {
    let l = build(
        r#"
        // a comment
        node a "A" { fill: #0b1120 }  // trailing comment
    "#,
    );
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#0b1120"));
}

#[test]
fn absolute_placement_is_honoured() {
    let l = build(r#"node a "A" at (100, 50) { size: 80 40 }"#);
    let r = l.nodes[0].rect;
    assert_eq!((r.x, r.y, r.w, r.h), (100.0, 50.0, 80.0, 40.0));
}

#[test]
fn relative_placement_offsets_by_the_gap() {
    let l = build(
        r#"
        node a "A" at (0, 0) { size: 100 60 }
        node b "B" right-of a gap 40 { size: 100 60 }
        node c "C" below a gap 20 align left { size: 100 60 }
    "#,
    );

    assert_eq!(l.nodes[1].rect.x, 140.0);
    assert_eq!(l.nodes[1].rect.y, 0.0); // centres aligned, equal heights
    assert_eq!(l.nodes[2].rect.y, 80.0);
    assert_eq!(l.nodes[2].rect.x, 0.0); // align left
}

#[test]
fn alignment_selects_the_cross_axis_edge() {
    let l = build(
        r#"
        node a "A" at (0, 0) { size: 100 100 }
        node top "T" right-of a gap 10 align top { size: 20 20 }
        node bot "B" right-of a gap 10 align bottom { size: 20 20 }
        node mid "M" right-of a gap 10 align center { size: 20 20 }
    "#,
    );

    assert_eq!(l.nodes[1].rect.y, 0.0);
    assert_eq!(l.nodes[2].rect.y, 80.0);
    assert_eq!(l.nodes[3].rect.y, 40.0);
}

#[test]
fn forward_references_resolve_across_passes() {
    let l = build(
        r#"
        node a "A" right-of b gap 10 { size: 50 50 }
        node b "B" at (0, 0) { size: 50 50 }
    "#,
    );
    assert_eq!(l.nodes[0].rect.x, 60.0);
}

#[test]
fn anchor_cycles_are_reported() {
    assert!(err("node a right-of b\nnode b right-of a").contains("anchor cycle"));
    assert!(err("node a right-of a").contains("itself"));
}

#[test]
fn unknown_references_are_reported() {
    assert!(err("node a\na -> ghost").contains("unknown node `ghost`"));
    assert!(err("node a right-of ghost").contains("unknown node `ghost`"));
    assert!(err("node a .ghost").contains("unknown style `.ghost`"));
    assert!(err("node a\nnode a").contains("declared twice"));
}

#[test]
fn typos_in_properties_and_keywords_fail_loudly() {
    assert!(err("node a { filll: #fff }").contains("unknown property"));
    assert!(err("node a { shape: blob }").contains("`blob` is not a valid `shape`"));
    assert!(
        err("node a\nnode b\na -> b { router: teleport }")
            .contains("`teleport` is not a valid `router`")
    );
    assert!(err("graph { wat: 1 }").contains("unknown graph setting"));
}

#[test]
fn styles_cascade_defaults_then_class_then_inline() {
    let l = build(
        r#"
        style node { fill: #111111 stroke: #222222 }
        style boxy { fill: #333333 radius: 20 }
        node a "A" .boxy { fill: #444444 }
    "#,
    );

    let s = &l.nodes[0].style;
    assert_eq!(s.fill.as_deref(), Some("#444444")); // inline wins
    assert_eq!(s.radius, Some(20.0)); // from the class
    assert_eq!(s.stroke.as_deref(), Some("#222222")); // from implicit `node`
}

#[test]
fn extends_walks_the_chain_child_first() {
    let l = build(
        r#"
        style base { fill: #111111 radius: 4 }
        style mid extends base { radius: 8 shape: cylinder }
        style leaf extends mid { radius: 12 }
        node a "A" .leaf
    "#,
    );

    let s = &l.nodes[0].style;
    assert_eq!(s.radius, Some(12.0));
    assert_eq!(s.shape, Some(Shape::Cylinder));
    assert_eq!(s.fill.as_deref(), Some("#111111"));
}

#[test]
fn extends_cycles_and_missing_parents_are_reported() {
    assert!(err("style a extends b {}\nstyle b extends a {}\nnode n .a").contains("cycle"));
    assert!(err("style a extends ghost {}\nnode n .a").contains("unknown style `ghost`"));
}

#[test]
fn compound_stroke_reads_paint_width_and_pattern() {
    let l = build(
        r#"
        node a
        node b
        a -> b { stroke: #ff0000 3 dashed }
    "#,
    );

    let s = &l.edges[0].style;
    assert_eq!(s.stroke.as_deref(), Some("#ff0000"));
    assert_eq!(s.stroke_width, Some(3.0));
    assert_eq!(s.dash_array().as_deref(), Some("7 5"));
}

#[test]
fn compound_font_reads_values_by_kind_in_any_order() {
    let l = build(r#"node a { font: #ff0000 bold 22 "Serif" }"#);
    let s = &l.nodes[0].style;
    assert_eq!(s.font_size, Some(22.0));
    assert_eq!(s.font_family.as_deref(), Some("Serif"));
    assert_eq!(s.font_color.as_deref(), Some("#ff0000"));
    assert_eq!(s.font_weight.as_deref(), Some("bold"));
}

#[test]
fn arrow_tokens_set_ends_and_pattern() {
    let l = build(
        r#"
        node a
        node b
        a <-> b
        a -- b
        a --> b
        a ..> b
    "#,
    );

    assert!(l.edges[0].arrow.head_start && l.edges[0].arrow.head_end);
    assert!(!l.edges[1].arrow.head_start && !l.edges[1].arrow.head_end);
    assert_eq!(l.edges[2].style.dash_array().as_deref(), Some("7 5"));
    assert_eq!(l.edges[3].style.dash_array().as_deref(), Some("1.5 4"));
}

#[test]
fn an_explicit_style_wins_over_the_arrow_token_pattern() {
    let l = build(
        r#"
        node a
        node b
        a --> b { dash: solid }
    "#,
    );
    assert_eq!(l.edges[0].style.dash_array(), None);
}

#[test]
fn graph_settings_reach_the_layout() {
    let l = build(
        r#"
        graph { padding: 12 gap: 5 background: #010203 }
        node a { size: 10 10 }
        node b { size: 10 10 }
    "#,
    );

    assert_eq!(l.opts.padding, 12.0);
    assert_eq!(l.opts.background.as_deref(), Some("#010203"));
    assert_eq!(l.nodes[1].rect.x, 15.0); // implicit flow uses the graph gap
}

#[test]
fn flow_direction_drives_implicit_placement() {
    let l = build(
        r#"
        graph { flow: down gap: 10 }
        node a { size: 20 20 }
        node b { size: 20 20 }
    "#,
    );
    assert_eq!(l.nodes[1].rect.y, 30.0);
    assert_eq!(l.nodes[1].rect.x, 0.0);
}

#[test]
fn viewbox_covers_every_node_plus_padding() {
    let svg = render_svg::render(&build(
        r#"
        graph { padding: 10 }
        node a at (0, 0) { size: 100 50 }
        node b at (200, 100) { size: 100 50 }
    "#,
    ));

    // 300 wide + 2*10 padding, 150 tall + 2*10 padding
    assert!(svg.contains(r#"viewBox="0 0 320 170""#), "{svg}");
}

#[test]
fn negative_coordinates_are_translated_into_view() {
    let svg = render_svg::render(&build(
        r#"
        graph { padding: 10 }
        node a at (-100, -100) { size: 50 50 }
    "#,
    ));

    assert!(svg.contains(r#"viewBox="0 0 70 70""#), "{svg}");
    assert!(svg.contains(r#"translate(110,110)"#), "{svg}");
}

#[test]
fn labels_are_xml_escaped() {
    let svg = render_svg::render(&build(r#"node a "<script> & \"quotes\"""#));
    assert!(!svg.contains("<script>"));
    assert!(svg.contains("&lt;script&gt; &amp; &quot;quotes&quot;"));
}

#[test]
fn every_shape_emits_geometry() {
    for shape in [
        "rect", "stadium", "ellipse", "diamond", "hexagon", "cylinder",
    ] {
        let svg = render_svg::render(&build(&format!(r#"node a "A" {{ shape: {shape} }}"#)));
        assert!(
            svg.contains("<rect")
                || svg.contains("<ellipse")
                || svg.contains("<polygon")
                || svg.contains("<path"),
            "{shape} produced no geometry"
        );
    }
}

#[test]
fn straight_edges_stop_at_the_shape_boundary() {
    let l = build(
        r#"
        node a at (0, 0) { size: 100 100 }
        node b at (200, 0) { size: 100 100 }
        a -> b
    "#,
    );
    let svg = render_svg::render(&l);

    // Leaves a's right face at x=100, reaches b's left face at x=200, same y.
    assert!(svg.contains(r#"points="100,50 200,50""#), "{svg}");
}

#[test]
fn ortho_edges_produce_an_elbow() {
    let l = build(
        r#"
        node a at (0, 0) { size: 100 100 }
        node b at (300, 200) { size: 100 100 }
        a -> b { router: ortho }
    "#,
    );
    assert_eq!(l.edges[0].style.router, Some(Router::Ortho));

    let svg = render_svg::render(&l);
    assert!(
        svg.contains(r#"points="100,50 200,50 200,250 300,250""#),
        "{svg}"
    );
}

#[test]
fn arrow_heads_emit_one_marker_per_color() {
    let svg = render_svg::render(&build(
        r#"
        node a
        node b
        node c
        a -> b { stroke: #ff0000 }
        b -> c { stroke: #ff0000 }
        a -> c { stroke: #00ff00 }
    "#,
    ));

    assert_eq!(svg.matches("<marker").count(), 2);
    assert!(svg.contains("marker-end="));
}

#[test]
fn arrow_none_suppresses_the_marker() {
    let svg = render_svg::render(&build(
        r#"
        node a
        node b
        a -> b { arrow: none }
    "#,
    ));

    assert!(!svg.contains("<marker"));
    assert!(!svg.contains("marker-end="));
    assert_eq!(
        build("node a\nnode b\na -> b { arrow: open }").edges[0]
            .style
            .head,
        Some(Head::Open)
    );
}

#[test]
fn an_empty_diagram_still_renders() {
    let svg = render_svg::render(&build(""));
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
}

#[test]
fn the_showcase_file_stays_valid() {
    let source = include_str!("../diagram.bd");
    let l = build(source);
    assert_eq!(l.nodes.len(), 7);
    assert_eq!(l.edges.len(), 7);
    assert!(render_svg::render(&l).starts_with("<svg"));
}

// The property table in `props.rs` drives validation, completion and hover;
// `style.rs` does the interpreting. These two tests are what keep them honest.

fn probe(def: &crate::props::PropDef) -> crate::ast::Prop {
    let value = match def.words.first() {
        Some(word) => crate::ast::Value::Word(word.to_string()),
        None => crate::ast::Value::Num(1.0),
    };
    crate::ast::Prop {
        name: def.name.to_string(),
        name_span: crate::ast::Span::default(),
        values: vec![value],
        values_span: crate::ast::Span::default(),
        span: crate::ast::Span::default(),
    }
}

#[test]
fn every_documented_property_is_understood_by_the_style_engine() {
    for def in crate::props::PROPS {
        let props = vec![probe(def)];
        crate::style::layer_from_props(&props)
            .unwrap_or_else(|e| panic!("`{}` is documented but rejected: {e}", def.name));

        for word in def.words {
            let props = vec![crate::ast::Prop {
                values: vec![crate::ast::Value::Word(word.to_string())],
                ..probe(def)
            }];
            crate::style::layer_from_props(&props).unwrap_or_else(|e| {
                panic!("`{}: {word}` is documented but rejected: {e}", def.name)
            });
        }
    }

    for def in crate::props::GRAPH_PROPS {
        let props = vec![probe(def)];
        crate::style::GraphOpts::from_props(&props)
            .unwrap_or_else(|e| panic!("graph `{}` is documented but rejected: {e}", def.name));
    }
}

#[test]
fn the_style_engine_rejects_anything_undocumented() {
    let bogus = crate::ast::Prop {
        name: "definitely-not-a-property".into(),
        name_span: crate::ast::Span::default(),
        values: vec![crate::ast::Value::Num(1.0)],
        values_span: crate::ast::Span::default(),
        span: crate::ast::Span::default(),
    };

    assert!(crate::style::layer_from_props(std::slice::from_ref(&bogus)).is_err());
    assert!(crate::style::GraphOpts::from_props(&[bogus]).is_err());
    assert!(crate::props::lookup("definitely-not-a-property", false).is_none());
}
