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

fn diags(source: &str) -> Vec<crate::check::Diag> {
    let diagram = parse_diagram(source).expect("source should parse");
    crate::check::check(&diagram)
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
    assert!(crate::props::lookup("definitely-not-a-property", crate::props::PROPS).is_none());
}

// ---------------------------------------------------------------- archetypes

#[test]
fn a_file_without_type_is_free() {
    let l = build("node a\nnode b\na -> b");
    assert!(matches!(l.chart, crate::layout::Chart::Free));
    assert_eq!(l.edges.len(), 1);
}

#[test]
fn an_unknown_archetype_is_reported() {
    let e = err("type flowchart\nnode a");
    assert!(e.contains("unknown archetype `flowchart`"), "{e}");
    assert!(e.contains("free, pie, packet"), "{e}");
}

#[test]
fn type_is_declared_once() {
    assert!(err("type pie\ntype packet").contains("already declared"));
}

#[test]
fn free_takes_no_type_settings() {
    let diags = diags("type free { donut: 0.5 }\nnode a");
    assert!(
        diags
            .iter()
            .any(|d| d.message.contains("takes no settings"))
    );
}

#[test]
fn pie_slices_span_the_whole_circle() {
    let l = build(
        r#"
        type pie
        node a "A" { value: 25 }
        node b "B" { value: 25 }
        node c "C" { value: 50 }
    "#,
    );

    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices.len(), 3);
    assert_eq!(pie.slices[0].start, 0.0);
    assert_eq!(pie.slices[0].end, 90.0);
    assert_eq!(pie.slices[2].end, 360.0);
    // Legend swatches, one per slice.
    assert_eq!(l.nodes.len(), 3);
}

#[test]
fn pie_takes_palette_colors_but_a_declared_fill_wins() {
    let l = build(
        r#"
        type pie
        node a { value: 1 fill: #123456 }
        node b { value: 1 }
    "#,
    );

    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices[0].fill, "#123456");
    assert_eq!(pie.slices[1].fill, crate::props::PALETTE_LIGHT[1]);
}

#[test]
fn pie_steps_the_palette_for_a_dark_surface() {
    let l = build("type pie\ngraph { background: #0b1120 }\nnode a { value: 1 }");
    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices[0].fill, crate::props::PALETTE_DARK[0]);
}

#[test]
fn a_pie_slice_needs_a_value() {
    let e = err("type pie\nnode a \"A\"");
    assert!(e.contains("has no `value`"), "{e}");
}

#[test]
fn a_value_may_come_from_a_class() {
    let l = build(
        r#"
        type pie
        style half { value: 50 }
        node a .half
        node b .half
        node c .half
    "#,
    );
    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices.len(), 3);
}

#[test]
fn pie_warns_where_a_pie_stops_working() {
    let two = diags("type pie\nnode a { value: 1 }\nnode b { value: 1 }");
    assert!(two.iter().any(|d| d.message.contains("hard to read")));

    let many: String = (0..10)
        .map(|i| format!("node n{i} {{ value: 1 }}\n"))
        .collect();
    let lots = diags(&format!("type pie\n{many}"));
    assert!(
        lots.iter()
            .any(|d| d.message.contains("exhaust the categorical palette"))
    );
}

#[test]
fn pie_and_packet_ignore_edges_and_placement_out_loud() {
    let d = diags("type pie\nnode a { value: 1 } \nnode b at (0,0) { value: 1 }\na -> b");
    assert!(d.iter().any(|x| x.message.contains("draws no edges")));
    assert!(
        d.iter()
            .any(|x| x.message.contains("the placement is ignored"))
    );
}

#[test]
fn packet_places_fields_by_bit_range() {
    let l = build(
        r#"
        type packet { width: 32 cell: 10 40 }
        node a "A" { bits: 0 15 }
        node b "B" { bits: 16 31 }
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Packet));
    assert_eq!(l.nodes[0].rect.x, 0.0);
    assert_eq!(l.nodes[0].rect.w, 160.0);
    assert_eq!(l.nodes[1].rect.x, 160.0);
    assert_eq!(l.nodes[0].rect.y, l.nodes[1].rect.y);
}

#[test]
fn a_packet_field_crossing_a_row_is_split() {
    let l = build(
        r#"
        type packet { width: 32 cell: 10 40 ruler: off }
        node a "A" { bits: 24 47 }
    "#,
    );

    // One declared field, two drawn segments — only the first is named.
    assert_eq!(l.nodes.len(), 2);
    assert_eq!(l.nodes[0].lines, vec!["A"]);
    assert!(l.nodes[1].lines.is_empty());
    assert_eq!(l.nodes[0].rect.x, 240.0);
    assert_eq!(l.nodes[0].rect.w, 80.0);
    assert_eq!(l.nodes[1].rect.x, 0.0);
    assert_eq!(l.nodes[1].rect.w, 160.0);
    assert!(l.nodes[1].rect.y > l.nodes[0].rect.y);
}

#[test]
fn packet_reports_overlaps_and_notes_gaps() {
    let e = err("type packet\nnode a { bits: 0 15 }\nnode b { bits: 8 23 }");
    assert!(e.contains("overlap"), "{e}");

    let d = diags("type packet\nnode a { bits: 0 7 }\nnode b { bits: 12 15 }");
    assert!(
        d.iter()
            .any(|x| x.message.contains("bits 8–11 are unassigned")),
        "{d:?}"
    );
}

#[test]
fn a_packet_field_needs_bits() {
    assert!(err("type packet\nnode a \"A\"").contains("has no `bits`"));
}

#[test]
fn chart_data_outside_its_archetype_is_flagged() {
    let d = diags("node a { value: 5 }");
    assert!(
        d.iter()
            .any(|x| x.message.contains("no effect outside `type pie`"))
    );

    let d = diags("node a { bits: 0 7 }");
    assert!(
        d.iter()
            .any(|x| x.message.contains("no effect outside `type packet`"))
    );
}

#[test]
fn the_chart_examples_stay_valid() {
    for source in [
        include_str!("../examples/pie.bd"),
        include_str!("../examples/packet.bd"),
    ] {
        let l = build(source);
        assert!(render_svg::render(&l).starts_with("<svg"));
    }
}

#[test]
fn the_surface_is_explicit_by_default() {
    // What the preview shows and what the file exports must be the same
    // thing; a transparent default made them differ.
    let svg = render_svg::render(&build("node a"));
    assert!(
        svg.contains(r##"viewBox="0 0 140 110.9"><rect x="0" y="0" width="140" height="110.9" fill="#ffffff"/>"##),
        "{svg}"
    );

    // Opting out is explicit, and then nothing is painted before the content.
    for source in [
        "graph { background: none }\nnode a",
        "graph { background: transparent }\nnode a",
    ] {
        let svg = render_svg::render(&build(source));
        assert!(svg.contains(r#"110.9"><g transform="#), "{svg}");
    }
}

#[test]
fn chart_text_follows_the_declared_style() {
    let l = build(
        r#"
        type pie
        style node { font-color: #ff0000 font-size: 20 }
        node a "A" { value: 60 }
        node b "B" { value: 40 }
    "#,
    );

    // Title, on-slice labels and legend entries all take the declared ink.
    assert!(
        l.annotations.iter().all(|a| a.color == "#ff0000"),
        "chart text ignored font-color"
    );
    assert!(l.annotations.iter().any(|a| a.size == 20.0));
}

#[test]
fn unreadable_text_is_reported() {
    let d = diags("type pie\nstyle node { font-color: #ffffff }\nnode a { value: 1 }");
    assert!(
        d.iter().any(|x| x.message.contains("contrast 1.0:1")),
        "white on the default white surface must be flagged: {d:?}"
    );

    // The same color on a dark canvas is fine.
    let d = diags(
        "type pie\ngraph { background: #0b1120 }\nstyle node { font-color: #ffffff }\nnode a { value: 1 }",
    );
    assert!(!d.iter().any(|x| x.message.contains("contrast")), "{d:?}");
}

#[test]
fn a_label_unreadable_on_its_own_box_is_reported() {
    let d = diags("style node { fill: #101010 font-color: #1a1a1a }\nnode a \"A\"");
    assert!(
        d.iter().any(|x| x.message.contains("not be readable")),
        "{d:?}"
    );
}

#[test]
fn the_reference_manual_is_up_to_date() {
    // `docs/reference.md` is generated from the same tables the compiler
    // reads. If this fails, run `cargo run --bin bgraph -- docs -o docs/reference.md`.
    let checked_in = include_str!("../docs/reference.md");
    assert_eq!(
        crate::docgen::reference(),
        checked_in,
        "docs/reference.md is stale — regenerate it with `bgraph docs`"
    );
}

#[test]
fn every_documented_word_is_accepted() {
    // The manual lists these as valid; the engine must agree.
    for shape in crate::props::SHAPES {
        let source = format!("node a {{ shape: {shape} }}");
        assert!(parse_diagram(&source).is_ok_and(|d| crate::check::first_error(&d).is_none()));
    }
    for router in crate::props::ROUTERS {
        let source = format!("node a\nnode b\na -> b {{ router: {router} }}");
        assert!(parse_diagram(&source).is_ok_and(|d| crate::check::first_error(&d).is_none()));
    }
    for head in crate::props::HEADS {
        let source = format!("node a\nnode b\na -> b {{ arrow: {head} }}");
        assert!(parse_diagram(&source).is_ok_and(|d| crate::check::first_error(&d).is_none()));
    }
    for flow in crate::props::FLOWS {
        let source = format!("graph {{ flow: {flow} }}\nnode a");
        assert!(parse_diagram(&source).is_ok_and(|d| crate::check::first_error(&d).is_none()));
    }
    for labels in crate::props::PIE_LABELS {
        let source = format!("type pie {{ labels: {labels} }}\nnode a {{ value: 1 }}");
        assert!(parse_diagram(&source).is_ok_and(|d| crate::check::first_error(&d).is_none()));
    }
    for align in crate::props::ALIGNMENTS {
        let source = format!("node a\nnode b right-of a align {align}");
        assert!(parse_diagram(&source).is_ok_and(|d| crate::check::first_error(&d).is_none()));
    }
}

// ------------------------------------------------- the two grammars agree

// `src/syntax.pest` is authoritative; `tree-sitter-bgraph/grammar.js` feeds
// the editor. Nothing but this stops them drifting apart, and the drift is
// silent until someone opens a file.

const CORPUS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/grammar");

fn corpus_files(kind: &str) -> Vec<std::path::PathBuf> {
    let dir = std::path::Path::new(CORPUS).join(kind);
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "bd"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no corpus files in {}", dir.display());
    files
}

/// `tree-sitter parse` exits non-zero when the tree contains an ERROR node.
/// Run from the grammar directory so the CLI finds the parser.
fn tree_sitter_accepts(path: &std::path::Path) -> Option<bool> {
    let status = std::process::Command::new("tree-sitter")
        .args(["parse", "-q"])
        .arg(std::fs::canonicalize(path).ok()?)
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tree-sitter-bgraph"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;
    Some(status.success())
}

#[test]
fn the_pest_grammar_matches_the_corpus() {
    for path in corpus_files("accept") {
        let source = std::fs::read_to_string(&path).expect("readable");
        assert!(
            parse_diagram(&source).is_ok(),
            "{} should parse, but: {:?}",
            path.display(),
            parse_diagram(&source).err().map(|e| e.to_string())
        );
    }

    for path in corpus_files("reject") {
        let source = std::fs::read_to_string(&path).expect("readable");
        assert!(
            parse_diagram(&source).is_err(),
            "{} should be rejected",
            path.display()
        );
    }
}

#[test]
fn the_tree_sitter_grammar_agrees_with_pest() {
    let mut probe = corpus_files("accept");
    probe.extend(corpus_files("reject"));
    for extra in [
        concat!(env!("CARGO_MANIFEST_DIR"), "/diagram.bd"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/pie.bd"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/examples/packet.bd"),
    ] {
        probe.push(std::path::PathBuf::from(extra));
    }

    // The CLI is needed to regenerate the parser anyway. Without it the
    // check cannot run, and says so rather than passing quietly.
    if tree_sitter_accepts(&probe[0]).is_none() {
        println!(
            "SKIPPED: `tree-sitter` is not on PATH, so pest and tree-sitter were \
             not cross-checked. Install it with `cargo install tree-sitter-cli`."
        );
        return;
    }

    let mut disagreements = Vec::new();
    for path in probe {
        let source = std::fs::read_to_string(&path).expect("readable");
        let pest = parse_diagram(&source).is_ok();
        let ts = tree_sitter_accepts(&path).expect("the CLI was found a moment ago");

        if pest != ts {
            disagreements.push(format!(
                "{}: pest {}, tree-sitter {}",
                path.display(),
                if pest { "accepts" } else { "rejects" },
                if ts { "accepts" } else { "rejects" }
            ));
        }
    }

    assert!(
        disagreements.is_empty(),
        "the two grammars disagree:\n  {}",
        disagreements.join("\n  ")
    );
}

#[test]
fn a_self_loop_is_drawn_above_its_node() {
    let l = build(
        r#"
        node a "A" at (0, 0) { size: 100 60 }
        a -> a "retry"
    "#,
    );
    let svg = render_svg::render(&l);

    // The loop is a polyline, so it inherits dashes, opacity and arrow heads.
    assert!(svg.contains("<polyline"), "{svg}");
    assert!(
        svg.contains("marker-end="),
        "a self-loop still points somewhere"
    );

    // It reaches above the node, and the canvas grows to hold it.
    let height: f64 = svg
        .split(r#"height=""#)
        .nth(1)
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse().ok())
        .expect("the svg declares a height");
    assert!(
        height > 60.0 + 42.0,
        "the loop must not be clipped: {height}"
    );
}

#[test]
fn an_edge_label_steps_off_a_line_it_would_sit_on() {
    // `a -> b` runs horizontally at y=30; `c -> d` crosses it at that exact
    // midpoint, so the label cannot stay there.
    let l = build(
        r#"
        node a at (0, 0) { size: 60 60 }
        node b at (200, 0) { size: 60 60 }
        node c at (120, -120) { size: 20 20 }
        node d at (120, 120) { size: 20 20 }
        a -> b "on the crossing"
        c -> d
    "#,
    );
    let svg = render_svg::render(&l);

    // The plate is the rect drawn with rx="3"; find its centre.
    let plate = svg.split(r#" rx="3""#).next().unwrap();
    let y: f64 = plate
        .rsplit(r#"y=""#)
        .next()
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse().ok())
        .expect("the label plate declares a y");

    // Anything but the untranslated midpoint means it moved out of the way.
    assert!(
        (y - 30.0).abs() > 4.0,
        "the label stayed on the crossing line at y={y}"
    );
}

#[test]
fn statement_keywords_cannot_name_a_node() {
    for word in ["type", "graph", "style", "node"] {
        let e = err(&format!("node {word} \"x\""));
        assert!(e.contains("reserved word"), "`{word}`: {e}");

        // And an edge cannot start with one — this is the parse-level half of
        // the same rule, and what keeps pest and tree-sitter in step.
        let source = format!("node a\n{word} -> a");
        assert!(
            parse_diagram(&source).is_err(),
            "`{word} -> a` must not parse"
        );
    }

    // Words that only appear inside a statement stay usable as names.
    for word in ["at", "gap", "align", "extends", "pie", "packet"] {
        let source = format!("node {word}\nnode b\n{word} -> b");
        assert!(parse_diagram(&source).is_ok(), "`{word}` should be usable");
    }
}

// ------------------------------------------------------------- alpha

#[test]
fn the_fourth_channel_is_alpha_and_defaults_to_opaque() {
    use crate::render_svg::split_alpha;

    // Opaque: no alpha attribute is worth emitting.
    assert_eq!(split_alpha("#ff0000"), ("#ff0000".into(), None));
    assert_eq!(split_alpha("#f00"), ("#ff0000".into(), None));
    assert_eq!(split_alpha("#ff0000ff"), ("#ff0000".into(), None));
    assert_eq!(split_alpha("#f00f"), ("#ff0000".into(), None));

    // Translucent.
    let (rgb, alpha) = split_alpha("#ff000080");
    assert_eq!(rgb, "#ff0000");
    assert!((alpha.expect("alpha") - 0.502).abs() < 0.002, "{alpha:?}");

    let (rgb, alpha) = split_alpha("#f008");
    assert_eq!(rgb, "#ff0000");
    assert!((alpha.expect("alpha") - 0.533).abs() < 0.002, "{alpha:?}");

    // A named color carries no alpha and must pass through untouched.
    assert_eq!(split_alpha("tomato"), ("tomato".into(), None));
    assert_eq!(split_alpha("none"), ("none".into(), None));
}

#[test]
fn alpha_is_emitted_as_its_own_attribute() {
    // Inline eight-digit hex is dropped silently by several SVG-to-PDF paths.
    let svg = render_svg::render(&build(
        r#"node a "A" { fill: #1e293b80 stroke: #38bdf8 font-color: #ffffff40 }"#,
    ));

    assert!(
        svg.contains(r##"fill="#1e293b" fill-opacity="0.5""##),
        "{svg}"
    );
    assert!(
        svg.contains(r##"stroke="#38bdf8" stroke-width"##),
        "opaque needs no attribute: {svg}"
    );
    assert!(
        !svg.contains("#1e293b80"),
        "no eight-digit hex should reach the output: {svg}"
    );
    assert!(
        svg.contains(r##"fill="#ffffff" fill-opacity="0.25""##),
        "{svg}"
    );
}

#[test]
fn opacity_is_no_longer_a_property() {
    let e = err("node a { opacity: 0.5 }");
    assert!(e.contains("unknown property `opacity`"), "{e}");
    assert!(crate::props::lookup("opacity", crate::props::PROPS).is_none());
}

#[test]
fn a_malformed_hex_length_is_reported_by_check() {
    // Both grammars take any run of hex digits — tree-sitter regexes have no
    // lookahead, so pinning lengths there would have split `#1234567` in two.
    // `check` is where the length is closed.
    for bad in ["#12345", "#1234567", "#123456789", "#12"] {
        let source = format!("node a {{ fill: {bad} }}");
        assert!(parse_diagram(&source).is_ok(), "`{bad}` should parse");
        let e = err(&source);
        assert!(
            e.contains("hex digits; a color needs 3, 4, 6 or 8"),
            "`{bad}`: {e}"
        );
    }
    for good in ["#123", "#1234", "#123456", "#12345678"] {
        let source = format!("node a {{ fill: {good} }}");
        let diagram = parse_diagram(&source).expect("should parse");
        assert!(
            crate::check::first_error(&diagram).is_none(),
            "`{good}` should be a color"
        );
    }
}

#[test]
fn alpha_does_not_confuse_contrast_or_the_palette() {
    // The alpha channel is not a color: both read the opaque part.
    let d = diags("style node { fill: #101010ff font-color: #1a1a1a80 }\nnode a \"A\"");
    assert!(
        d.iter().any(|x| x.message.contains("not be readable")),
        "{d:?}"
    );

    let l = build("type pie\ngraph { background: #0b1120ff }\nnode a { value: 1 }");
    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices[0].fill, crate::props::PALETTE_DARK[0]);
}

#[test]
fn contrast_is_measured_on_what_the_eye_receives() {
    use crate::charts::composite;

    // Half of sky blue over near-black is much darker than sky blue.
    assert_eq!(composite("#ffffff80", "#000000"), "#808080");
    assert_eq!(composite("#ffffff", "#000000"), "#ffffff");
    assert_eq!(composite("#ffffff00", "#123456"), "#123456");

    // A pale label on a barely-there fill over a dark canvas is readable, and
    // must not be reported. The same label on the opaque fill is not.
    let translucent = diags(
        "graph { background: #0f172a }\nnode a \"A\" { fill: #38bdf820 font-color: #e2e8f0 }",
    );
    assert!(
        !translucent.iter().any(|d| d.message.contains("readable")),
        "alpha was ignored when compositing: {translucent:?}"
    );

    let opaque =
        diags("graph { background: #0f172a }\nnode a \"A\" { fill: #38bdf8 font-color: #e2e8f0 }");
    assert!(
        opaque.iter().any(|d| d.message.contains("readable")),
        "{opaque:?}"
    );
}
