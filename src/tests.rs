use crate::layout::{self, Layout};
use crate::parser::{MAX_NESTING, parse_diagram};
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
    // Derived from the table, so adding an archetype cannot leave this stale.
    for def in crate::props::ARCHETYPES {
        assert!(e.contains(def.name), "`{}` is missing from: {e}", def.name);
    }
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
fn the_design_note_still_lists_every_keyword() {
    // `vocabulary.md` is what `D-015` sends a reader to for the reasoning, and
    // it had gone a month without noticing that an archetype may now declare a
    // *second* word. A design note nobody checks describes the design as it
    // was when someone last remembered it.
    let note = include_str!("../docs/vocabulary.md");

    for def in crate::props::ARCHETYPES {
        assert!(
            note.contains(&format!("| `{}` |", def.name)),
            "`type {}` is missing from the design note's table",
            def.name
        );
        for item in def.items() {
            assert!(
                note.contains(&format!("`{item}`")),
                "`{item}` is `type {}`'s word and the note does not give it",
                def.name
            );
        }
    }
}

#[test]
fn the_manual_names_every_genre_and_its_word() {
    // The manual carries a table of every `type` and the word it calls its
    // items. That table is a copy of `props.rs`, and a copy is a thing that
    // stops being true — a genre added without reaching the manual would
    // leave a document that quietly describes an older language.
    let manual = include_str!("../docs/manual.typ");

    for def in crate::props::ARCHETYPES {
        assert!(
            manual.contains(&format!("[{}]", def.name)),
            "`type {}` is missing from the manual's table",
            def.name
        );
        for item in def.items() {
            assert!(
                manual.contains(&format!("[`{item}`]")),
                "`{item}` is `type {}`'s word and the manual does not give it",
                def.name
            );
        }
    }

    // And the count in the prose, which is the sentence a reader trusts.
    let genres = crate::props::ARCHETYPES.len() - 1;
    assert_eq!(genres, 28, "the manual says twenty-eight genres and `free`");
    assert!(
        manual.contains("Twenty-eight genres and `free`"),
        "the prose count"
    );
}

// ---------------------------------------------------------------------- W-064

#[cfg(test)]
fn after(source: &str) -> Vec<String> {
    let diagram = parse_diagram(source).expect("parses");
    let solved = layout::solve(&diagram).expect("solves");
    crate::check::after_layout(&solved)
        .into_iter()
        .map(|d| d.message)
        .collect()
}

#[test]
fn an_ortho_edge_through_a_box_is_reported() {
    // `X-004`: the router turns corners and avoids nothing, so it could draw a
    // line straight through a node and say nothing at all.
    let found = after(
        "node a \"A\" at (0, 0)\nnode mid \"in the way\" right-of a gap 40\n\
         node b \"B\" right-of mid gap 40\na -> b { router: ortho }",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("passes through a box"), "{found:?}");
    assert!(
        found[0].contains("router: straight"),
        "the message says a way out"
    );
}

#[test]
fn a_clear_ortho_edge_is_silent() {
    // A warning that fired on the ordinary case would be one nobody reads.
    assert!(
        after("node a \"A\" at (0, 0)\nnode b \"B\" below a gap 90\na -> b { router: ortho }")
            .is_empty()
    );

    // Its own endpoints are not obstacles: an edge leaves one box and enters
    // another by definition.
    assert!(
        after("node a \"A\" at (0, 0)\nnode b \"B\" right-of a\na -> b { router: ortho }")
            .is_empty()
    );
}

#[test]
fn a_straight_edge_is_not_asked_about_obstacles() {
    // It is clipped against both shapes and, being straight, crossing a third
    // box is a placement the author chose rather than a router's decision.
    let found = after(
        "node a \"A\" at (0, 0)\nnode mid \"in the way\" right-of a gap 40\n\
         node b \"B\" right-of mid gap 40\na -> b",
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn every_example_is_free_of_geometric_complaints() {
    // The suite already renders them; this asserts the second pass has nothing
    // to say about any of them, so a shipped example is not demonstrating the
    // very thing the warning is about.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "bd") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("readable");
        if source.contains("{{") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(after(&source).is_empty(), "{name}: {:?}", after(&source));
    }
}

// ---------------------------------------------------------------------- W-063

#[test]
fn the_formatter_fixes_structure_and_leaves_taste_alone() {
    let messy = "type pie {\ntitle: \"x\"\n      radius: 10\n}\n\n\n\nslice a \"A\" 1   \n";
    let tidy = crate::fmt::format(messy).expect("parses");

    assert_eq!(
        tidy, "type pie {\n  title: \"x\"\n  radius: 10\n}\n\nslice a \"A\" 1\n",
        "{tidy:?}"
    );

    // Columns lined up by hand survive: indentation is structural, alignment
    // is taste, and only the first is the formatter's business.
    let aligned = "field a \"A\"    0 15\nfield bb \"B\"  16 31\n";
    assert_eq!(crate::fmt::format(aligned).expect("parses"), aligned);
}

#[test]
fn the_formatter_counts_braces_it_can_see() {
    // A brace inside a string or after a `//` is text. Counting it would
    // reindent everything after the line by one, for good.
    let source = "node a \"a { b\"\nnode b // a } here\nnode c\n";
    assert_eq!(crate::fmt::format(source).expect("parses"), source);
}

#[test]
fn the_formatter_refuses_a_file_it_does_not_understand() {
    // An unbalanced brace reindents everything after it, and the diff would
    // bury the one line that was actually wrong.
    assert!(crate::fmt::format("node a \"never closed").is_err());
}

#[test]
fn every_example_is_in_the_shape_the_formatter_writes() {
    // The formatter agreeing with the house style is the claim; a repository
    // whose own files fail it would mean it had invented a different one.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    for entry in std::fs::read_dir(dir).expect("readable") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "bd") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&path).expect("readable");
        assert_eq!(
            crate::fmt::format(&source).unwrap_or_else(|e| panic!("{name}: {e:#}")),
            source,
            "{name} is not in canonical shape — run `bgraph fmt examples/{name}`"
        );
    }
}

// ---------------------------------------------------------------------- W-066

#[test]
fn the_viewer_shows_a_picture_it_did_not_draw() {
    // An `.svg` is already the picture. Serving it as it is means the viewer
    // works on a file bgraph never wrote — and since the reload is driven by
    // the file's timestamp, it follows another tool regenerating it.
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples"));
    let svg = dir.join("venn.svg");

    let served = crate::watch::render_source(&svg, None).expect("serves");
    let on_disk = std::fs::read_to_string(&svg).expect("readable");
    assert_eq!(served, on_disk, "verbatim, not re-rendered");

    // And a source is still rendered, so one command covers both.
    let rendered = crate::watch::render_source(&dir.join("venn.bd"), None).expect("renders");
    assert!(rendered.starts_with("<svg"));
}

#[test]
fn the_extension_decides_what_the_viewer_is_looking_at() {
    // The same rule `-o` uses: a flag would be a second way of saying what the
    // filename already says.
    use std::path::Path;
    assert!(crate::watch::is_svg(Path::new("a.svg")));
    assert!(
        crate::watch::is_svg(Path::new("A.SVG")),
        "case is not meaning"
    );
    assert!(!crate::watch::is_svg(Path::new("a.bd")));
    assert!(!crate::watch::is_svg(Path::new("svg")));
}

// ---------------------------------------------------------------------- W-067

#[test]
fn the_window_is_opt_in_and_the_core_stays_small() {
    // The whole reason the SVG writer, the HTTP preview and the PDF writer are
    // hand-rolled. A window is the one thing that cannot be, so it is the one
    // thing behind a feature — and a manifest that quietly turned it on would
    // undo every one of those decisions at once.
    let manifest = include_str!("../Cargo.toml");

    let defaults = manifest
        .split_once("default = [")
        .and_then(|(_, tail)| tail.split_once(']'))
        .expect("a default feature list")
        .0;
    assert!(
        !defaults.contains("window"),
        "the window is on by default: {defaults}"
    );

    for crate_name in ["minifb", "resvg", "tiny-skia"] {
        let line = manifest
            .lines()
            .find(|l| l.trim_start().starts_with(crate_name))
            .unwrap_or_else(|| panic!("`{crate_name}` is not in the manifest"));
        assert!(
            line.contains("optional = true"),
            "`{crate_name}` is not optional: {line}"
        );
    }
}

#[cfg(feature = "window")]
#[test]
fn the_window_reads_the_same_picture_the_server_does() {
    // One source of the picture, two ways of looking at it. A window that
    // rendered by its own path would be a second renderer.
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples"));
    let svg = crate::watch::render_source(&dir.join("venn.svg"), None).expect("reads");
    assert!(svg.starts_with("<svg"));

    // And it parses as a picture, which is what the window rasterises.
    let mut options = resvg::usvg::Options::default();
    options.fontdb_mut().load_system_fonts();
    assert!(resvg::usvg::Tree::from_str(&svg, &options).is_ok());
}

// ---------------------------------------------------------------------- W-062

#[cfg(feature = "csv")]
#[test]
fn a_csv_is_one_table_of_rows() {
    // The format real data arrives in, and the one that maps exactly onto
    // repetition: a header names the fields, every other line writes the
    // statement once.
    let bound = crate::template::rows_from_csv(
        "r",
        "id,name,value\norganic,Organic search,4820\npaid,\"Paid, all networks\",410\n",
    )
    .expect("reads");

    let t = Template::parse("type pie\nslice {{r.id}} \"{{r.name}}\" {{r.value}}").expect("parses");
    let filled = t.fill(&bound).expect("fills");

    assert!(
        filled.contains("slice organic \"Organic search\" 4820"),
        "{filled}"
    );
    // A quoted field keeps its comma, which is the only reason the dialect
    // needs quoting at all.
    assert!(
        filled.contains("slice paid \"Paid, all networks\" 410"),
        "{filled}"
    );
}

#[cfg(feature = "csv")]
#[test]
fn a_csv_refuses_a_row_that_does_not_line_up() {
    // A short row read positionally would silently shift every field after
    // the gap into the wrong column.
    let e = format!(
        "{:#}",
        crate::template::rows_from_csv("r", "a,b,c\n1,2\n").expect_err("should refuse")
    );
    assert!(e.contains("2 fields for 3 columns"), "{e}");

    let e = format!(
        "{:#}",
        crate::template::rows_from_csv("r", "").expect_err("should refuse")
    );
    assert!(e.contains("empty"), "{e}");

    let e = format!(
        "{:#}",
        crate::template::rows_from_csv("r", "a,,c\n1,2,3\n").expect_err("should refuse")
    );
    assert!(e.contains("no name"), "{e}");
}

#[cfg(feature = "csv")]
#[test]
fn a_csv_field_that_is_a_number_becomes_one() {
    // Otherwise every value would arrive quoted, and a chart would refuse the
    // data it was given.
    let bound = crate::template::rows_from_csv("r", "n,s\n42,42abc\n").expect("reads");
    let t =
        Template::parse("node a \"x\" { width: {{r.n}} font-family: {{r.s}} }").expect("parses");
    let filled = t.fill(&bound).expect("fills");
    assert!(filled.contains("width: 42"), "a number is bare: {filled}");
    assert!(
        filled.contains("font-family: \"42abc\""),
        "text is quoted: {filled}"
    );
}

// ---------------------------------------------------------------------- W-061

#[cfg(feature = "json")]
#[test]
fn the_preview_fills_a_template_from_its_data() {
    // Writing a template against its real numbers is the loop this genre of
    // file is written in, and it was the only one with no live view.
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples"));
    let svg = crate::watch::render_source(&dir.join("report.bd"), Some(&dir.join("report.json")))
        .expect("renders");

    assert!(svg.starts_with("<svg"), "{}", &svg[..40.min(svg.len())]);
    assert!(
        svg.contains("Organic search"),
        "a row from the data reached the page"
    );

    // Without data it renders as written, holes and all — the opposite of a
    // one-shot render, and deliberately: a hole parses, so the shape is
    // drawable, and the shape is what you are editing before the data is
    // wired up. Refusing here would blank the preview for the whole first
    // half of writing a template.
    let bare = crate::watch::render_source(&dir.join("report.bd"), None).expect("draws the shape");
    assert!(
        bare.contains("{{"),
        "the holes are shown as written: {}",
        &bare[..60]
    );
}

// ---------------------------------------------------------------------- W-058

#[test]
fn a_hole_stands_where_a_name_goes() {
    // The CLI always rendered this — `Template::parse` substitutes before
    // parsing — but the *editor* parses the template as written, so a file
    // like this showed as a syntax error with its highlighting collapsed.
    for source in [
        "type pie\nslice {{r.id}} \"{{r.name}}\" {{r.value}}",
        "node {{a}} \"A\"\nnode b \"B\" right-of {{a}}",
        "node {{a}}\nnode b\n{{a}} -> b \"x\"",
        "group {{g}} \"G\" {\n  node c\n}",
    ] {
        parse_diagram(source).unwrap_or_else(|e| panic!("{source}\n{e:#}"));
    }
}

#[test]
fn a_hole_may_not_stand_where_the_archetype_goes() {
    // The archetype decides which properties exist and which words name an
    // item, so nothing else in the file is checkable until it is known.
    let e = err("type {{genre}}\nnode a");
    assert!(!e.is_empty(), "a genre from data is refused");
}

#[test]
fn a_template_reports_what_only_a_render_used_to_catch() {
    // `Template::parse` refuses these, but as one sentence with no span —
    // enough for a command line, useless in an editor.
    let two = "type pie\nslice {{a.id}} \"{{b.name}}\" 1\n";
    let found = crate::template::lint(two);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].1.contains("repeats over"), "{found:?}");
    assert!(
        found[0].1.contains("`a`") && found[0].1.contains("`b`"),
        "{found:?}"
    );

    let open = "node a \"{{never closed\n";
    let found = crate::template::lint(open);
    assert!(found[0].1.contains("never closed"), "{found:?}");

    // And a well-formed template is silent.
    assert!(crate::template::lint("slice {{r.id}} \"{{r.name}}\" {{r.value}}").is_empty());
}

#[test]
fn holes_are_found_with_their_positions_and_their_row() {
    let holes = crate::template::holes_in("slice {{r.id}} \"{{r.name}}\" {{total}}");
    assert_eq!(holes.len(), 3);
    assert_eq!(holes[0].name, "r.id");
    assert_eq!(holes[0].prefix.as_deref(), Some("r"));
    assert_eq!(holes[2].prefix, None, "an undotted hole belongs to no row");

    // The spans cover the braces, so an editor points at the whole hole.
    let source = "slice {{r.id}}";
    let h = &crate::template::holes_in(source)[0];
    assert_eq!(&source[h.span.start..h.span.end], "{{r.id}}");
}

// ---------------------------------------------------------------------- W-057

#[cfg(feature = "json")]
#[test]
fn json_binds_what_set_cannot() {
    // The point of the whole thing: `--set` binds a scalar, and the case
    // templating exists for is a chart whose number of slices is not known
    // when the template is written.
    let data = r#"{
        "quarter": "Q3 2026",
        "cats": ["a", "b"],
        "r": [
            { "id": "north", "name": "North", "value": 10 },
            { "id": "south", "name": "South", "value": 20 }
        ]
    }"#;
    let bound = Bindings::from_json(data).expect("reads");

    let t = Template::parse(
        "type pie { title: \"{{quarter}}\" }\nslice {{r.id}} \"{{r.name}}\" {{r.value}}",
    )
    .expect("parses");
    let filled = t.fill(&bound).expect("fills");

    assert_eq!(
        filled,
        "type pie { title: \"Q3 2026\" }\nslice north \"North\" 10\nslice south \"South\" 20",
        "{filled}"
    );
}

#[cfg(feature = "json")]
#[test]
fn an_identifier_hole_is_written_bare_and_a_value_hole_is_quoted() {
    // The two cannot share a rule. A quoted name does not parse; a bare value
    // becomes a second statement. The position on the line is what decides.
    let bound =
        Bindings::from_json(r#"{ "r": [{ "id": "north", "who": "North" }] }"#).expect("reads");

    // `{{r.id}}` is the second word — an identifier. `{{r.who}}` is data.
    let t = Template::parse("node {{r.id}} \"x\" .cls\nnode b\n{{r.id}} -> b \"{{r.who}}\"")
        .expect("parses");
    let filled = t.fill(&bound).expect("fills");

    assert!(filled.starts_with("node north \"x\""), "{filled}");
    assert!(
        filled.contains("north -> b \"North\""),
        "the head of an edge is a name too: {filled}"
    );
}

#[cfg(feature = "json")]
#[test]
fn a_property_key_does_not_make_the_next_word_a_name() {
    // `fill: {{c}}` puts the hole second on its line, but the first token owns
    // the rest of it — the naive "second word is the id" rule got this wrong.
    let bound = Bindings::from_json(r#"{ "c": "red" }"#).expect("reads");
    let t = Template::parse("node a {\n  fill: {{c}}\n}").expect("parses");
    assert!(t.fill(&bound).expect("fills").contains("fill: \"red\""));

    // And an item may be *named* after a keyword, so the rule cannot ask
    // whether the previous word is one.
    let bound = Bindings::from_json(r#"{ "v": "text" }"#).expect("reads");
    let t = Template::parse("node slice {{v}}").expect("parses");
    assert!(
        t.fill(&bound)
            .expect("fills")
            .contains("node slice \"text\"")
    );
}

#[cfg(feature = "json")]
#[test]
fn a_name_the_grammar_cannot_read_is_refused_by_name() {
    // Written and left to fail as a parse error three lines away, nothing
    // would point back at the binding that caused it.
    let bound = Bindings::from_json(r#"{ "r": [{ "id": "two words" }] }"#).expect("reads");
    let t = Template::parse("node {{r.id}} \"x\"").expect("parses");
    let e = format!("{:#}", t.fill(&bound).expect_err("should refuse"));

    assert!(e.contains("r.id"), "{e}");
    assert!(e.contains("two words"), "{e}");
    assert!(e.contains("cannot be a name"), "{e}");
}

#[cfg(feature = "json")]
#[test]
fn json_refuses_what_it_cannot_read_unambiguously() {
    let cases = [
        (r#"[1, 2]"#, "must be an object"),
        (r#"{ "x": null }"#, "is null"),
        (
            r#"{ "x": [1, { "a": 2 }] }"#,
            "mixes objects and plain values",
        ),
        (r#"{ "x": { "a": 1 } }"#, "is an object"),
        (r#"{ "x": [{ "a": [{ "b": 1 }] }] }"#, "one line deep"),
    ];
    for (source, wanted) in cases {
        let e = format!("{:#}", Bindings::from_json(source).expect_err(source));
        assert!(e.contains(wanted), "{source} → {e}");
    }

    // And the shapes it does read.
    let ok = Bindings::from_json(r#"{ "a": "x", "b": 2, "c": true, "d": [1, 2], "e": [] }"#)
        .expect("reads");
    assert_eq!(
        ok.get("c"),
        Some(&crate::template::Value::Text("true".into()))
    );
}

// ---------------------------------------------------------------------- W-042

#[cfg(feature = "pdf")]
#[test]
fn a_pdf_is_a_pdf() {
    // Hand-written file structure: the header, the object table, the offsets
    // and the pointer to them all have to agree, and nothing in the rest of
    // the suite would notice if they stopped.
    let pdf = crate::render_source_pdf("node a \"A\"\nnode b \"B\" right-of a\na -> b \"x\"")
        .expect("renders");
    let text = String::from_utf8_lossy(&pdf).to_string();

    assert!(text.starts_with("%PDF-1.4"), "header");
    assert!(text.trim_end().ends_with("%%EOF"), "trailer");

    // Every offset in the cross-reference table must land on its object.
    let start = text
        .rsplit_once("startxref\n")
        .and_then(|(_, tail)| tail.split_whitespace().next())
        .and_then(|n| n.parse::<usize>().ok())
        .expect("startxref");
    assert!(
        text[start..].starts_with("xref"),
        "startxref points at the table"
    );

    let offsets: Vec<usize> = text
        .lines()
        .filter_map(|l| l.strip_suffix(" 00000 n "))
        .filter_map(|n| n.parse().ok())
        .collect();
    assert_eq!(offsets.len(), 6, "catalog, pages, page, content, two fonts");
    for (i, offset) in offsets.iter().enumerate() {
        assert!(
            text[*offset..].starts_with(&format!("{} 0 obj", i + 1)),
            "object {} is not at the offset the table gives",
            i + 1
        );
    }
}

#[cfg(feature = "pdf")]
#[test]
fn the_page_and_the_viewbox_are_the_same_surface() {
    // Two writers, one canvas — a second implementation of the bounds would
    // be a second place for them to drift.
    let source = "node a \"A\"\nnode b \"B\" below a gap 40\na -> b";
    let svg = crate::render_source(source).expect("svg");
    let pdf = crate::render_source_pdf(source).expect("pdf");
    let pdf = String::from_utf8_lossy(&pdf).to_string();

    let view: Vec<f64> = svg
        .split_once("viewBox=\"")
        .and_then(|(_, tail)| tail.split_once('"'))
        .expect("a viewBox")
        .0
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();
    let media: Vec<f64> = pdf
        .split_once("/MediaBox [")
        .and_then(|(_, tail)| tail.split_once(']'))
        .expect("a MediaBox")
        .0
        .split_whitespace()
        .filter_map(|n| n.parse().ok())
        .collect();

    assert!((view[2] - media[2]).abs() < 0.01, "{view:?} {media:?}");
    assert!((view[3] - media[3]).abs() < 0.01, "{view:?} {media:?}");
}

#[cfg(feature = "pdf")]
#[test]
fn a_pdf_draws_the_edges_the_svg_draws() {
    // The first version joined two centres with a bare line: no label, no
    // arrowhead, no routing, no clipping. It validated, it opened, and it was
    // wrong about every edge in the default archetype.
    let source = "node a \"A\"\nnode b \"B\" right-of a gap 60\nnode c \"C\" below a\n\
                  a -> b \"carries\"\na ..> c { router: ortho }";
    let pdf =
        String::from_utf8_lossy(&crate::render_source_pdf(source).expect("renders")).to_string();

    assert!(pdf.contains("(carries) Tj"), "the edge label is drawn");

    // An `ortho` edge is an elbow, so its path has an interior point — a
    // straight centre-to-centre line would have exactly two.
    let moves = pdf.matches(" m\n").count();
    let lines = pdf.matches(" l\n").count();
    assert!(
        lines > moves,
        "paths have interior points: {moves} moves, {lines} lines"
    );

    // And an arrowhead is a filled triangle at the end of a line.
    assert!(pdf.contains("f\n"), "a head is filled");
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_text_is_measured_exactly_rather_than_estimated() {
    // The one place `X-005` does not apply: a PDF viewer must have Helvetica,
    // and the advance table `W-040` added is Helvetica's — so the centring
    // here is right rather than close.
    let pdf = crate::render_source_pdf("type pie\nslice a \"Organic\" 10").expect("renders");
    let text = String::from_utf8_lossy(&pdf).to_string();
    assert!(text.contains("/Helvetica"), "the base-14 font is named");
    assert!(text.contains("Tj"), "text is shown, not drawn as paths");
}

#[cfg(feature = "pdf")]
#[test]
fn every_example_also_renders_as_a_pdf() {
    // The sweep, again: an archetype whose geometry the PDF writer cannot
    // express would otherwise only be found by opening a file.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut seen = 0;
    for entry in std::fs::read_dir(dir).expect("examples/ is readable") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "bd") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("readable");
        // A template's holes are unbound, so it has nothing to render yet.
        if source.contains("{{") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let pdf = crate::render_source_pdf(&source).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        assert!(pdf.starts_with(b"%PDF"), "{name}");
        seen += 1;
    }
    assert!(seen >= 25, "only {seen} examples found");
}

// ---------------------------------------------------------------------- W-038

#[test]
fn the_textmate_grammar_knows_every_word_the_compiler_does() {
    // `X-009` in its VS Code form. A tree-sitter query naming a node type the
    // grammar dropped stops the language loading, loudly; a TextMate grammar
    // missing a keyword just stops colouring it, which nobody notices — so the
    // list is re-derived here rather than trusted.
    let grammar = include_str!("../editors/vscode/syntaxes/bgraph.tmLanguage.json");
    // Found by its scope name and then by position, rather than by a literal
    // prefix of the pattern: anchoring on the words themselves made this test
    // fail the first time a keyword was added at the front of the list, which
    // is exactly the change it exists to allow.
    let statement = grammar
        .lines()
        .skip_while(|l| !l.contains("keyword.control.bgraph"))
        .find(|l| l.contains("\"match\""))
        .expect("the statement pattern is in the grammar");

    for (word, _) in crate::props::KEYWORDS {
        assert!(
            statement.contains(&format!("{word}|")) || statement.contains(&format!("{word})")),
            "`{word}` is a statement keyword the VS Code grammar does not colour"
        );
    }

    for def in crate::props::ARCHETYPES {
        for item in def.items() {
            assert!(
                statement.contains(&format!("{item}|")) || statement.contains(&format!("{item})")),
                "`{item}` is `type {}`'s word and the VS Code grammar does not colour it",
                def.name
            );
        }
    }
}

#[test]
fn the_packaging_script_takes_every_fact_from_the_manifest() {
    // A VSIX carries the name, the version, the publisher and the engine
    // range a second time. Every one of them is read out of `package.json` by
    // the script rather than written into it — a version in two files is a
    // version that is wrong in one of them.
    let script = include_str!("../editors/vscode/package.sh");
    let manifest = include_str!("../editors/vscode/package.json");

    for field in ["p.name", "p.version", "p.publisher", "p.engines.vscode"] {
        assert!(script.contains(field), "the script does not read `{field}`");
    }

    // The one version string in the manifest must not also appear in the
    // script — if it does, someone has written it down twice.
    let version = manifest
        .split_once("\"version\": \"")
        .and_then(|(_, tail)| tail.split_once('"'))
        .expect("a version")
        .0;
    assert!(
        !script.contains(version),
        "`{version}` is hardcoded in package.sh as well as package.json"
    );

    // The archive layout VS Code requires, so a rename cannot quietly produce
    // a file no editor will open.
    for part in [
        "[Content_Types].xml",
        "extension.vsixmanifest",
        "extension/package.json",
    ] {
        assert!(script.contains(part), "the archive is missing `{part}`");
    }
}

#[test]
fn the_vscode_manifest_claims_the_language_this_project_is() {
    // The three facts a manifest can get wrong in a way that only shows up on
    // someone else's machine: the extension, the scope name the grammar
    // declares, and the entry point.
    let manifest = include_str!("../editors/vscode/package.json");
    assert!(manifest.contains("\".bd\""), "the file extension");
    assert!(manifest.contains("source.bgraph"), "the scope name");
    assert!(manifest.contains("./src/extension.js"), "the entry point");

    let grammar = include_str!("../editors/vscode/syntaxes/bgraph.tmLanguage.json");
    assert!(
        grammar.contains("\"scopeName\": \"source.bgraph\""),
        "the grammar must answer to the scope the manifest contributes"
    );

    // The server the client launches is the one this repository builds.
    let client = include_str!("../editors/vscode/src/extension.js");
    assert!(client.contains("bgraph-lsp"), "the default server binary");
}

// ---------------------------------------------------------------------- W-040

#[test]
fn text_is_measured_per_character_not_by_counting_them() {
    // A flat ratio charged `l` and `W` the same, which is wrong by a factor of
    // four — the larger of the two errors in `X-005`, and the only one that
    // can be fixed without the font file.
    let thin = crate::layout::advance("llllllllll", 14.0);
    let wide = crate::layout::advance("WWWWWWWWWW", 14.0);
    assert!(wide / thin > 3.5, "ten Ws against ten ls: {wide} vs {thin}");

    // A space is not free and not full width either.
    let with = crate::layout::advance("a a", 14.0);
    let without = crate::layout::advance("aa", 14.0);
    assert!(with > without && with < without * 1.6, "{with} {without}");
}

#[test]
fn a_box_is_sized_for_its_widest_line_not_its_longest() {
    // These used to be the same question, because width was character count.
    // `WWW` is three characters and far wider than the eleven in `lllllllllll`.
    let short_but_wide = build("node a \"WWWWWWW\"");
    let long_but_thin = build("node a \"lllllllllllllll\"");
    assert!(
        short_but_wide.nodes[0].rect.w > long_but_thin.nodes[0].rect.w,
        "seven Ws are wider than fifteen ls: {} vs {}",
        short_but_wide.nodes[0].rect.w,
        long_but_thin.nodes[0].rect.w
    );
}

#[test]
fn text_outside_printable_ascii_keeps_the_flat_estimate() {
    // A table for one Latin typeface has nothing to say about CJK or emoji,
    // and pretending otherwise would be a worse guess than admitting it.
    let cjk = crate::layout::advance("日本語", 10.0);
    assert!((cjk - 3.0 * 0.56 * 10.0).abs() < 0.01, "{cjk}");
}

// ---------------------------------------------------------------------- W-049

#[cfg(test)]
const HISTORY: &str = "type gitgraph { branches: \"main\" \"side\" spacing: 100 60 }\n\
     commit c1 \"one\" \"main\"\ncommit s1 \"two\" \"side\"\ncommit c2 \"merge\" \"main\"\n\
     c1 -> s1\nc1 -> c2\ns1 -> c2\n";

#[test]
fn a_commit_sits_one_step_past_its_latest_parent() {
    // A merge has to land after *both* sides of it, or the picture shows
    // history arriving before what it merges.
    let l = build(HISTORY);
    let x = |i: usize| l.nodes[i].rect.x;
    assert!(x(1) > x(0), "s1 follows c1");
    assert!(x(2) > x(1), "the merge follows both parents");
}

#[test]
fn a_branch_is_a_rail_and_the_rails_are_declared_order() {
    let l = build(HISTORY);
    let y = |i: usize| l.nodes[i].rect.y;
    assert_eq!(y(0), y(2), "c1 and c2 share the main rail");
    assert!(
        y(1) > y(0),
        "side sits below main, as `branches` lists them"
    );

    // A branch nobody declared still gets a rail, at the bottom.
    let l = build(
        "type gitgraph { branches: \"main\" }\n\
         commit a \"a\" \"main\"\ncommit b \"b\" \"other\"\na -> b\n",
    );
    assert!(l.nodes[1].rect.y > l.nodes[0].rect.y);
}

#[test]
fn a_merge_is_two_parents_and_two_lines() {
    let l = build(HISTORY);
    assert_eq!(l.polylines.len(), 3, "three parent links");
}

#[test]
fn a_history_written_backwards_is_refused() {
    // The layout numbers steps in declaration order, so a parent written
    // after its child would be measured against a depth that does not exist.
    let e = err("type gitgraph\ncommit b \"b\" \"main\"\ncommit a \"a\" \"main\"\na -> b\n");
    assert!(e.contains("declared after it"), "{e}");

    let d = diags("type gitgraph\ncommit a \"a\" \"main\"\ncommit b \"b\" \"main\"\n");
    assert!(d.iter().any(|x| x.message.contains("no parent")), "{d:?}");
}

// ---------------------------------------------------------------------- W-041

#[cfg(test)]
// `node` is the universal synonym, so one tree serves all three genres —
// which is the claim under test.
const TREE: &str = "node a \"A\"\nnode b \"B\"\nnode c \"C\"\nnode d \"D\"\n\
     a -> b\na -> c\nb -> d\n";

#[test]
fn a_parent_sits_over_the_middle_of_what_it_holds() {
    // The whole engine in one assertion: subtrees take disjoint spans, and a
    // parent is centred on the span it was given.
    let l = build(&format!("type orgchart {{ row: 100 spacing: 20 }}\n{TREE}"));
    let mid = |i: usize| l.nodes[i].rect.x + l.nodes[i].rect.w / 2.0;

    // B and C are siblings; A is centred between them.
    assert!(mid(1) < mid(2), "declaration order runs left to right");
    assert!(
        (mid(0) - (mid(1) + mid(2)) / 2.0).abs() < 1.0,
        "A at {} between {} and {}",
        mid(0),
        mid(1),
        mid(2)
    );
    // D hangs under B, not under A.
    assert!((mid(3) - mid(1)).abs() < 1.0);
}

#[test]
fn depth_is_the_row_in_an_orgchart_and_the_indent_in_a_treeview() {
    // One tree, two readings — which is what makes the engine worth writing
    // once for three genres.
    let org = build(&format!("type orgchart {{ row: 100 }}\n{TREE}"));
    assert_eq!(org.nodes[0].rect.y, 0.0);
    assert!(org.nodes[1].rect.y >= 100.0, "a level down is a row down");
    assert!(org.nodes[3].rect.y >= 200.0, "two levels, two rows");

    let view = build(&format!("type treeview {{ row: 20 spacing: 10 }}\n{TREE}"));
    // Pre-order, one row each: A, B, D, C. `row` is a *minimum* — asking for
    // 20 with a taller box drew every entry over the one above it, which is
    // illegible rather than tight, so the step clears the box.
    let step = view.nodes[1].rect.y - view.nodes[0].rect.y;
    assert!(step >= view.nodes[0].rect.h, "a row clears its box: {step}");
    assert_eq!(view.nodes[0].rect.y, 0.0);
    assert_eq!(view.nodes[3].rect.y, step * 2.0, "D follows B");
    assert_eq!(
        view.nodes[2].rect.y,
        step * 3.0,
        "C comes after B's subtree"
    );
    assert!(view.nodes[1].rect.x > view.nodes[0].rect.x, "depth indents");
}

#[test]
fn a_mindmap_puts_branches_on_both_sides_of_its_root() {
    // The point of the shape: the root stays central however lopsided the
    // tree is.
    let l = build(
        "type mindmap { row: 120 spacing: 20 }\n\
         node r \"R\"\nnode a \"A\"\nnode b \"B\"\n\
         r -> a\nr -> b\n",
    );
    let mid = |i: usize| l.nodes[i].rect.y + l.nodes[i].rect.h / 2.0;
    assert!(
        (mid(1) < mid(0)) != (mid(2) < mid(0)),
        "one branch each way: {} {} {}",
        mid(0),
        mid(1),
        mid(2)
    );
}

#[test]
fn every_link_is_drawn() {
    // Structure declared and not drawn would leave a reader inventing it.
    let l = build(&format!("type orgchart\n{TREE}"));
    assert_eq!(l.polylines.len(), 3, "three edges, three connectors");
}

#[test]
fn a_tree_reports_what_stops_it_being_one() {
    let e = err("type orgchart\nnode a \"A\"\nnode b \"B\"\nnode c \"C\"\na -> c\nb -> c");
    assert!(e.contains("already reports to"), "{e}");

    let e = err("type orgchart\nnode a \"A\"\nnode b \"B\"\na -> b\nb -> a");
    assert!(e.contains("has no root") || e.contains("cycle"), "{e}");

    let d = diags("type orgchart\nnode a \"A\"\nnode b \"B\"");
    assert!(
        d.iter().any(|x| x.message.contains("separate trees")),
        "{d:?}"
    );
}

// ---------------------------------------------------------------------- W-045

#[cfg(test)]
const LOOM: &str = "type harness\n\
     connector j1 \"J1\" {\n  - 1: +12V\n  - 2: GND\n}\n\
     connector j2 \"J2\" right-of j1 {\n  - 1: VCC\n  - 2: RETURN\n}\n";

#[test]
fn a_wire_lands_on_the_pin_it_names() {
    // What `X-016` was about. The wire used to join the two *boxes* and state
    // its pins in a label; now it leaves the pin row it names and arrives at
    // the one at the other end, which is what a loom drawing is for.
    let l = build(&format!(
        "{LOOM}j1 -> j2 \"power\" 1 2\nj1 -> j2 \"ground\" 2 1"
    ));

    assert!(l.edges.is_empty(), "wires are not routed as ordinary edges");
    assert_eq!(l.polylines.len(), 2, "one polyline per wire");

    let ends = |i: usize| {
        let p = &l.polylines[i].points;
        (p[0].1, p[p.len() - 1].1)
    };
    let (a_from, a_to) = ends(0);
    let (b_from, b_to) = ends(1);

    // Pin 1 is above pin 2 in both connectors, and the two wires cross —
    // which is only visible because each one starts and ends on a row.
    assert!(
        a_from < b_from,
        "pin 1 leaves above pin 2: {a_from} {b_from}"
    );
    assert!(a_to > b_to, "and arrives below it: {a_to} {b_to}");
}

#[test]
fn a_wire_keeps_its_own_label_and_drops_the_pin_pair() {
    // The geometry says the pins now, so repeating them beside the line would
    // be the same fact twice.
    let l = build(&format!("{LOOM}j1 -> j2 \"power\" 1 1"));
    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"power"), "{texts:?}");
    assert!(!texts.iter().any(|t| t.contains('→')), "{texts:?}");
}

#[test]
fn a_wire_must_land_on_a_pin_that_exists() {
    // The two mistakes that make a harness drawing useless to the person
    // holding the crimp tool.
    let e = err(&format!("{LOOM}j1 -> j2 \"x\" 9 1"));
    assert!(e.contains("has no pin 9"), "{e}");

    let e = err(&format!("{LOOM}j1 -> j2 \"a\" 1 1\nj1 -> j2 \"b\" 1 2"));
    assert!(e.contains("already wired"), "{e}");
}

#[test]
fn a_wire_with_no_pins_is_reported() {
    let d = diags(&format!("{LOOM}j1 -> j2 \"power\""));
    assert!(
        d.iter().any(|x| x.message.contains("names no pins")),
        "{d:?}"
    );
}

#[test]
fn a_connector_is_a_ruled_box_of_its_pins() {
    let l = build(&format!("{LOOM}j1 -> j2 1 1"));
    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"- 1: +12V"), "{texts:?}");
    assert!(texts.contains(&"- 2: RETURN"), "{texts:?}");
}

// ---------------------------------------------------------------------- W-053

#[test]
fn two_sets_have_their_lens_solved_to_the_area_asked_for() {
    // The one thing a venn exists to show. Drawn by eye it is a chart that
    // lies about its own subject, so the distance is solved, not chosen.
    let l = build("type venn { radius: 100 }\nset a \"A\" 100\nset b \"B\" 100\na -> b 50");

    let (ra, rb) = (l.nodes[0].rect.w / 2.0, l.nodes[1].rect.w / 2.0);
    let ax = l.nodes[0].rect.x + ra;
    let bx = l.nodes[1].rect.x + rb;
    let d = (bx - ax).abs();

    // Half of each set is shared, so the lens must be half a circle's area.
    let want = std::f64::consts::PI * ra * rb * 0.5;
    let got = crate::charts::lens_area(ra, rb, d);
    assert!(
        (got - want).abs() / want < 0.02,
        "lens {got} against {want}, centres {d} apart"
    );
}

#[test]
fn a_circle_is_scaled_by_area_not_by_radius() {
    // Radius would square every difference: a set twice as big would read as
    // four times.
    let l = build("type venn { radius: 100 }\nset a \"A\" 100\nset b \"B\" 25\na -> b 5");
    let (big, small) = (l.nodes[0].rect.w, l.nodes[1].rect.w);
    assert!(
        (big / small - 2.0).abs() < 0.05,
        "four times the count is twice the width: {big} vs {small}"
    );
}

#[test]
fn a_venn_refuses_what_no_arrangement_can_show() {
    let e = err("type venn\nset a \"A\" 10");
    assert!(e.contains("needs two sets"), "{e}");

    let e = err("type venn\nset a \"A\" 1\nset b \"B\" 1\nset c \"C\" 1\nset d \"D\" 1");
    assert!(e.contains("cannot be drawn as circles"), "{e}");
    assert!(
        e.contains("upset"),
        "the message names the honest alternative: {e}"
    );

    // An overlap bigger than a set is a counting mistake, not a drawing one.
    let e = err("type venn\nset a \"A\" 10\nset b \"B\" 100\na -> b 50");
    assert!(e.contains("more than the 10 in `a`"), "{e}");

    let e = err("type venn\nset a \"A\"\nset b \"B\" 5\na -> b 1");
    assert!(e.contains("has no `count`"), "{e}");
}

#[test]
fn three_sets_say_that_their_areas_are_indicative() {
    // Three circles cannot honour all seven regions. Saying so beats
    // pretending, and the numbers stay exact.
    let d = diags("type venn\nset a \"A\" 5\nset b \"B\" 5\nset c \"C\" 5\na -> b 1");
    assert!(d.iter().any(|x| x.message.contains("indicative")), "{d:?}");
}

// ---------------------------------------------------------------------- W-043

#[test]
fn a_role_picks_the_outline_and_leaves_the_palette_alone() {
    // A topology's colours usually already mean something to its author —
    // environment, tenancy, trust zone — so the archetype chooses the shape
    // and keeps out of the fill.
    let l = build(
        "type network\n\
         host r \"R\" router\n\
         host s \"S\" storage right-of r\n\
         host c \"C\" cloud right-of s\n",
    );
    assert_eq!(l.nodes[0].style.shape, Some(Shape::Diamond));
    assert_eq!(l.nodes[1].style.shape, Some(Shape::Cylinder));
    assert_eq!(l.nodes[2].style.shape, Some(Shape::Ellipse));

    // A declared shape still wins over the role's.
    let l = build("type network\nhost r \"R\" router { shape: rect }");
    assert_eq!(l.nodes[0].style.shape, Some(Shape::Rect));
}

#[test]
fn an_address_is_a_line_of_the_box_and_a_speed_a_line_of_the_link() {
    // Declared and never drawn would be worse than refused.
    let l = build(
        "type network\nhost a \"A\" server { addr: \"10.0.0.1\" }\n\
         host b \"B\" server right-of a\na -> b \"trunk\" \"10 GbE\"\n",
    );
    assert_eq!(l.nodes[0].lines, vec!["A", "10.0.0.1"]);
    assert_eq!(l.edges[0].label.as_deref(), Some("trunk\n[10 GbE]"));
}

#[test]
fn an_unreachable_host_is_reported() {
    // Either a mistake or a fact the diagram is failing to state.
    let d = diags(
        "type network\nhost a \"A\" server\nhost b \"B\" server right-of a\n\
         host lost \"Lost\" server below a\na -> b\n",
    );
    assert!(
        d.iter()
            .any(|x| x.message.contains("on no link and in no segment")),
        "{d:?}"
    );

    // A host inside a segment is attached by being there.
    let d = diags(
        "type network\ngroup dmz \"DMZ\" {\n  host a \"A\" server\n}\n\
         host b \"B\" server right-of a\na -> b\n",
    );
    assert!(!d.iter().any(|x| x.message.contains("on no link")), "{d:?}");
}

#[test]
fn an_unknown_role_is_a_hard_error() {
    let e = err("type network\nhost a \"A\" toaster");
    assert!(e.contains("toaster"), "{e}");
}

// ---------------------------------------------------------------------- W-047

#[cfg(test)]
const CALL: &str = "type sequence { column: 120 row: 40 }\n\
     participant a \"A\"\n\
     participant b \"B\"\n\
     a -> b \"before\"\n\
     activate b {\n  b -> a \"work\"\n  a --> b \"done\"\n}\n\
     b --> a \"after\"\n";

#[test]
fn an_activation_bar_spans_the_messages_its_block_holds() {
    // D-024: the bar is a group whose `edge_members` are the messages, so its
    // extent is a row span — exactly what a sequence frame already measures.
    let l = build(CALL);

    // Two participants plus the bar.
    assert_eq!(l.nodes.len(), 3, "{:?}", l.nodes.len());
    let bar = &l.nodes[2].rect;
    assert!(bar.w < 20.0, "a bar is thin: {bar:?}");

    // It starts around the first message inside and ends around the last,
    // never covering the messages outside the block.
    assert!(bar.h > 40.0, "it spans two rows: {bar:?}");
    assert!(bar.h < 40.0 * 3.0, "and not the whole exchange: {bar:?}");
}

#[test]
fn an_activation_names_a_participant_rather_than_declaring_one() {
    // The reason `activate` needed its own word: a group id and an item id
    // share one namespace, so the duplicate-id rule is precisely what this
    // must not trip.
    let l = build(CALL);
    assert_eq!(l.nodes[1].lines, vec!["B"], "B is still a participant");

    let e = err("type sequence\nparticipant a \"A\"\nactivate nobody {\n  a -> a \"x\"\n}");
    assert!(e.contains("names no participant"), "{e}");
}

#[test]
fn an_activation_is_not_a_frame_anywhere() {
    // An edge touching the activated participant is an ordinary message, not
    // an edge into a frame — which is what it read as until `frame_names`
    // started telling the two apart.
    let d = diags(CALL);
    assert!(!d.iter().any(|x| x.message.contains("is a group")), "{d:?}");

    let d = diags("type sequence\nparticipant a \"A\"\nparticipant b \"B\"\nactivate a {\n}");
    assert!(
        d.iter().any(|x| x.message.contains("spans no messages")),
        "an empty bar has no length: {d:?}"
    );
}

#[test]
fn an_ordinary_frame_still_frames() {
    // `activate` must not have taken `group` with it.
    let l = build(
        "type sequence\nparticipant a \"A\"\nparticipant b \"B\"\n\
         group retry \"retry\" {\n  a -> b \"try\"\n}\n",
    );
    assert_eq!(l.groups.len(), 1);
    assert_eq!(l.groups[0].lines, vec!["retry"]);
}

// ---------------------------------------------------------------------- W-044

#[test]
fn a_rack_counts_from_one_at_the_bottom() {
    // The one thing this archetype can get wrong in a way that still looks
    // right: a rack labelled top-down is correct about everything except
    // where to put your hands.
    let l = build(
        "type rack { units: 10 cell: 200 20 ruler: off }\n\
         device low \"Low\" 1 1\n\
         device high \"High\" 10 10\n",
    );

    assert_eq!(l.nodes.len(), 2);
    assert!(
        l.nodes[0].rect.y > l.nodes[1].rect.y,
        "U1 sits below U10: {:?}",
        l.nodes.iter().map(|n| n.rect.y).collect::<Vec<_>>()
    );
}

#[test]
fn a_device_is_as_tall_as_the_units_it_occupies() {
    let l = build(
        "type rack { units: 10 cell: 200 20 ruler: off }\n\
         device one \"1U\" 1 1\n\
         device four \"4U\" 5 8\n",
    );
    assert_eq!(l.nodes[0].rect.h, 20.0);
    assert_eq!(l.nodes[1].rect.h, 80.0, "U5 through U8 inclusive is four");
}

#[test]
fn every_unit_is_numbered_on_the_rail() {
    // A rail with every number is what you count against while holding a
    // server; a rail labelled every five is a rail you have to do sums on.
    let l = build("type rack { units: 6 cell: 200 20 }\ndevice a \"A\" 1 1");
    let rail: Vec<&str> = l
        .annotations
        .iter()
        .map(|a| a.text.as_str())
        .filter(|t| t.parse::<u32>().is_ok())
        .collect();
    for u in ["1", "2", "3", "4", "5", "6"] {
        assert!(rail.contains(&u), "U{u} missing from {rail:?}");
    }
}

#[test]
fn a_rack_refuses_what_cannot_physically_hold() {
    let e = err("type rack { units: 10 }\ndevice a \"A\" 1 4\ndevice b \"B\" 3 6");
    assert!(e.contains("overlap"), "{e}");
    assert!(e.contains("cannot share a unit"), "{e}");

    let e = err("type rack { units: 10 }\ndevice a \"A\" 8 12");
    assert!(e.contains("past the top of a 10U frame"), "{e}");

    let e = err("type rack { units: 10 }\ndevice a \"A\" 0 2");
    assert!(e.contains("counts from 1"), "{e}");

    let e = err("type rack\ndevice a \"A\"");
    assert!(e.contains("has no `u`"), "{e}");
}

#[test]
fn empty_units_are_room_not_a_mistake() {
    // A packet with unassigned bits does not add up; a rack with empty units
    // has space. The same shape of gap, two different meanings.
    let d = diags("type rack { units: 10 }\ndevice a \"A\" 1 2\ndevice b \"B\" 7 8");
    assert!(!d.iter().any(|x| x.message.contains("unassigned")), "{d:?}");
}

// ---------------------------------------------------------------------- W-046

#[cfg(test)]
const RADAR: &str = "type radar { categories: \"N\" \"E\" \"S\" \"W\" radius: 100 range: 0 100 legend: off }\n\
     series a \"A\" 100 50 0 50\n";

#[test]
fn a_radar_places_its_axes_clockwise_from_the_top() {
    // The order the axes are written is the order they are read, and twelve
    // o'clock is where a reader starts.
    let l = build(RADAR);
    assert_eq!(l.nodes.len(), 4, "a mark per axis");

    let centre = (100.0 + l.nodes[0].rect.x + 3.5, l.nodes[0].rect.y + 3.5);
    let _ = centre;
    let at = |i: usize| {
        let r = &l.nodes[i].rect;
        (r.x + r.w / 2.0, r.y + r.h / 2.0)
    };
    let (n, e, s, w) = (at(0), at(1), at(2), at(3));

    // 100 on the first axis is the rim, straight up; 0 on the third is the
    // centre; 50 on the second and fourth sit either side of it.
    assert!(n.1 < s.1, "the first axis points up: {n:?} vs {s:?}");
    assert!(e.0 > w.0, "the second is to the right of the fourth");
    assert!((e.1 - w.1).abs() < 0.5, "and level with it: {e:?} {w:?}");
}

#[test]
fn a_radar_draws_one_closed_polygon_per_series() {
    let l = build(
        "type radar { categories: \"a\" \"b\" \"c\" legend: off }\n\
         series one \"One\" 1 2 3\n\
         series two \"Two\" 3 2 1\n",
    );
    assert_eq!(l.polylines.len(), 2);
    for shape in &l.polylines {
        assert!(shape.closed, "a radar shape closes on itself");
        assert_eq!(shape.points.len(), 3);
        assert!(shape.fill.is_some(), "filled, so overlap is visible");
    }
}

#[test]
fn a_radar_starts_its_rings_at_zero() {
    // Starting them at the smallest value would exaggerate every difference,
    // which is the classic way this chart misleads.
    let l =
        build("type radar { categories: \"a\" \"b\" \"c\" legend: off }\nseries s \"S\" 90 95 100");
    let ticks: Vec<&str> = l
        .annotations
        .iter()
        .map(|a| a.text.as_str())
        .filter(|t| t.parse::<f64>().is_ok())
        .collect();
    // No numeric ring labels are drawn, so the proof is geometric: the marks
    // must not all sit on the rim.
    assert!(ticks.is_empty());
    let l2 =
        build("type radar { categories: \"a\" \"b\" \"c\" legend: off }\nseries s \"S\" 0 50 100");
    let spread = l2.nodes.iter().map(|n| n.rect.x).fold(0.0_f64, f64::max)
        - l2.nodes.iter().map(|n| n.rect.x).fold(f64::MAX, f64::min);
    assert!(spread > 1.0, "the scale spans the data, not a sliver");
}

#[test]
fn a_radar_reports_the_shapes_it_cannot_honestly_draw() {
    let e = err("type radar\nseries s \"S\" 1 2 3");
    assert!(e.contains("needs `categories`"), "{e}");

    let d = diags("type radar { categories: \"a\" \"b\" }\nseries s \"S\" 1 2");
    assert!(
        d.iter().any(|x| x.message.contains("enclose no area")),
        "{d:?}"
    );

    let d = diags("type radar { categories: \"a\" \"b\" \"c\" }\nseries s \"S\" 1 2");
    assert!(d.iter().any(|x| x.message.contains("for 3 axes")), "{d:?}");

    let e = err("type radar { categories: \"a\" \"b\" \"c\" }\nseries s \"S\"");
    assert!(e.contains("has no numbers"), "{e}");
}

// ---------------------------------------------------------------------- W-052

#[cfg(test)]
const JOURNEY: &str = "type journey { actors: \"A\" \"B\" size: 400 200 legend: off }\n\
     step one \"One\" 1 5\n\
     step two \"Two\" 5 1\n";

#[test]
fn a_journey_draws_one_line_per_actor() {
    // The whole point of the genre: the step where two people diverge is the
    // thing you see first, which needs both lines to exist.
    let l = build(JOURNEY);
    assert_eq!(l.polylines.len(), 2, "one line each");
    assert_eq!(l.nodes.len(), 4, "a mark per actor per step");

    // Scores are read in the order `actors` declares them, so the two lines
    // must cross: A goes 1→5 while B goes 5→1.
    let (a, b) = (&l.polylines[0].points, &l.polylines[1].points);
    assert!(a[0].1 > a[1].1, "A climbs: {a:?}");
    assert!(b[0].1 < b[1].1, "B falls: {b:?}");
}

#[test]
fn a_journey_labels_every_whole_score() {
    // A satisfaction score has no meaning between two whole steps, so the axis
    // is not handed to `nice_scale`.
    let l = build(JOURNEY);
    let ticks: Vec<&str> = l
        .annotations
        .iter()
        .map(|a| a.text.as_str())
        .filter(|t| t.parse::<u32>().is_ok())
        .collect();
    for want in ["1", "2", "3", "4", "5"] {
        assert!(ticks.contains(&want), "{want} missing from {ticks:?}");
    }
}

#[test]
fn sections_band_over_the_steps_they_hold() {
    let l = build(
        "type journey { actors: \"A\" size: 400 200 legend: off }\n\
         group early \"Early\" {\n  step one \"One\" 3\n  step two \"Two\" 4\n}\n\
         step three \"Three\" 5\n",
    );

    assert_eq!(l.groups.len(), 1);
    let band = &l.groups[0];
    assert_eq!(band.lines, vec!["Early"]);
    // It spans the first two steps and stops short of the third.
    let third = l.nodes.iter().map(|n| n.rect.x).fold(0.0_f64, f64::max);
    assert!(band.rect.right() < third, "{:?} vs {third}", band.rect);
}

#[test]
fn a_journey_reports_what_it_cannot_draw() {
    // Scores are positional against `actors`, so the two can silently
    // disagree — the same failure `bar` has against `categories`.
    let d = diags("type journey { actors: \"A\" \"B\" }\nstep one \"One\" 3");
    assert!(
        d.iter().any(|x| x.message.contains("for 2 actors")),
        "{d:?}"
    );

    let d = diags("type journey\nstep one \"One\" 3");
    assert!(
        d.iter().any(|x| x.message.contains("stand for nobody")),
        "{d:?}"
    );

    let d = diags("type journey { actors: \"A\" }\nstep one \"One\" 9");
    assert!(
        d.iter().any(|x| x.message.contains("on a 1–5 scale")),
        "{d:?}"
    );

    let e = err("type journey { actors: \"A\" }\nstep one \"One\"");
    assert!(e.contains("has no scores"), "{e}");
}

#[test]
fn scores_belong_to_the_journey_alone() {
    let e = err("node a { scores: 1 2 }");
    assert!(e.contains("`type journey`"), "{e}");
}

// ---------------------------------------------------------------------- W-050

/// Two columns, three cards, written the way the archetype means them.
#[cfg(test)]
const BOARD: &str = "type kanban { column: 200 }\n\
     group todo \"To do\" {\n  card a \"A\"\n  card b \"B\"\n}\n\
     group done \"Done\" {\n  card c \"C\"\n}\n";

#[test]
fn a_board_puts_its_columns_in_declaration_order() {
    // The archetype places everything: a kanban whose columns had to be
    // anchored by hand would be busywork with nothing gained in the reading.
    let l = build(BOARD);

    assert_eq!(l.groups.len(), 2);
    assert!(
        l.groups[0].rect.x < l.groups[1].rect.x,
        "left to right in declaration order: {:?}",
        l.groups.iter().map(|g| g.rect.x).collect::<Vec<_>>()
    );
    // Every column is drawn to the same height, or the board reads as ragged
    // shelves rather than as one thing.
    assert_eq!(l.groups[0].rect.bottom(), l.groups[1].rect.bottom());
}

#[test]
fn cards_stack_inside_their_column() {
    let l = build(BOARD);
    assert_eq!(l.nodes.len(), 3);

    let (a, b) = (&l.nodes[0].rect, &l.nodes[1].rect);
    assert_eq!(a.x, b.x, "one column, one x");
    assert!(b.y > a.y, "declaration order runs down the column");
    assert!(b.y >= a.bottom(), "cards do not overlap");

    // The third card is in the second column, so it starts at the top again.
    assert!(l.nodes[2].rect.x > a.x);
    assert_eq!(l.nodes[2].rect.y, a.y);
}

#[test]
fn a_column_over_its_limit_says_so() {
    // The one thing a board is kept for. Without this the genre is a `free`
    // diagram with boxes in rows.
    let d =
        diags("type kanban\ngroup doing \"Doing\" {\n  limit: 2\n  card a\n  card b\n  card c\n}");
    assert!(
        d.iter().any(|x| x.message.contains("over its limit of 2")),
        "{d:?}"
    );

    let d = diags("type kanban\ngroup doing \"Doing\" {\n  limit: 3\n  card a\n  card b\n}");
    assert!(
        !d.iter().any(|x| x.message.contains("over its limit")),
        "within the limit is silent: {d:?}"
    );
}

#[test]
fn the_header_counts_the_cards_against_the_limit() {
    let l = build("type kanban\ngroup doing \"Doing\" {\n  limit: 3\n  card a\n  card b\n}");
    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"Doing  2/3"), "{texts:?}");

    // With no limit there is nothing to count against, so only the tally.
    let l = build("type kanban\ngroup todo \"To do\" {\n  card a\n}");
    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"To do  1"), "{texts:?}");
}

#[test]
fn a_limit_rides_the_cascade_like_any_other_datum() {
    // Read off the raw props this would be missed, and the common case would
    // still work — which is how a gap like that stays quiet.
    let d = diags(
        "type kanban\nstyle wip { limit: 1 }\n\
         group doing \"Doing\" .wip {\n  card a\n  card b\n}",
    );
    assert!(
        d.iter().any(|x| x.message.contains("over its limit of 1")),
        "a limit from a class counts too: {d:?}"
    );
}

#[test]
fn a_card_outside_a_column_is_refused_and_so_is_a_limit_on_one() {
    let e = err("type kanban\ngroup todo \"To do\" {\n  card a\n}\ncard loose \"Loose\"");
    assert!(e.contains("is in no column"), "{e}");

    // `limit` must be `Ctx::Node` to be accepted on a group at all, so it is
    // also accepted on a card — where it means nothing, and says so.
    let e = err("type kanban\ngroup todo \"To do\" {\n  card a { limit: 2 }\n}");
    assert!(e.contains("belongs to a column"), "{e}");

    let e = err("type kanban\ngroup todo \"To do\" {\n  limit: 0\n  card a\n}");
    assert!(e.contains("at least one card"), "{e}");
}

// ---------------------------------------------------------------------- W-051

#[test]
fn a_requirement_is_a_ruled_box_with_its_text_inside() {
    // The same picture `class` and `er` draw. Asserted rather than assumed,
    // because "it reuses the class box" is the claim that goes quiet.
    let l = build(
        r#"
        type requirements
        requirement r1 "Stopping distance" {
          verify: test
          - REQ-001
          - The vehicle shall stop within 40 m.
        }
    "#,
    );

    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"- REQ-001"), "{texts:?}");
    assert!(
        texts.contains(&"- The vehicle shall stop within 40 m."),
        "{texts:?}"
    );
    // The name's box, plus the thin node that rules off the members.
    assert!(l.nodes.len() >= 2, "{}", l.nodes.len());
}

#[test]
fn compartments_are_asked_of_the_archetype_not_matched_by_name() {
    // This was a hand-written `name == "class" || name == "er"` in the layout,
    // which is the shape of list nobody remembers to extend — so it is a flag
    // on the archetype now, and that is what this pins.
    for genre in ["class", "er", "requirements"] {
        assert!(
            crate::props::archetype(genre).expect("exists").members,
            "{genre} carries member lines"
        );
    }
    for genre in ["pie", "packet", "xy", "state", "block"] {
        assert!(
            !crate::props::archetype(genre).expect("exists").members,
            "{genre} does not"
        );
    }
}

#[test]
fn a_link_is_drawn_in_guillemets_and_shares_the_line_with_a_label() {
    // A relation whose kind is declared and never drawn would be worse than
    // one that is refused: the reader would be guessing from the arrow alone.
    // A member line runs to the end of the line, so it cannot share one with
    // the closing brace — the same trap a doc example fell into once.
    const TWO: &str = "type requirements\nrequirement a {\n  - A\n}\nrequirement b {\n  - B\n}\n";

    let bare = build(&format!("{TWO}a -> b derive"));
    assert_eq!(bare.edges[0].label.as_deref(), Some("«derive»"));

    let named = build(&format!("{TWO}a -> b \"why\" satisfy"));
    assert_eq!(named.edges[0].label.as_deref(), Some("why\n«satisfy»"));
}

#[test]
fn positional_edge_data_is_validated_like_any_other_property() {
    // `check_edges` looked at the inline block only, so a bad word written
    // *positionally* — the idiomatic form — went through untouched. `er` had
    // the hole too: `one many` is checked, `nonsense` was not.
    let e = err("type er\nentity a \"A\"\nentity b \"B\"\na -> b nonsense one");
    assert!(e.contains("nonsense"), "{e}");
    assert!(e.contains("from-card"), "{e}");
}

#[test]
fn an_unknown_link_is_a_hard_error() {
    // D-005: a silent no-op would hide the typo in a file you cannot compile.
    let e = err(
        "type requirements\nrequirement a {\n  - A\n}\nrequirement b {\n  - B\n}\na -> b implies",
    );
    assert!(e.contains("implies"), "{e}");
}

#[test]
fn a_requirements_view_says_what_it_cannot_check() {
    // Both halves of this genre can be silently empty, and both are the
    // failure it exists to make visible.
    let d =
        diags("type requirements\nrequirement a {\n  - A\n}\nrequirement b {\n  - B\n}\na -> b");
    assert!(
        d.iter().any(|x| x.message.contains("what it asserts")),
        "a link with no kind: {d:?}"
    );

    let d = diags("type requirements\nrequirement a \"A\" {\n  - A\n}");
    assert!(
        d.iter().any(|x| x.message.contains("has no `verify`")),
        "a requirement nobody plans to check: {d:?}"
    );
}

#[test]
fn requirement_words_belong_to_this_genre_alone() {
    // `link`, `risk` and `verify` are its own; written elsewhere they are
    // unknown properties whose message names the genre that does read them.
    for source in [
        "node a\nnode b\na -> b { link: derive }",
        "node a { risk: high }",
        "node a { verify: test }",
    ] {
        let e = err(source);
        assert!(e.contains("`type requirements`"), "{source}: {e}");
    }
}

// ---------------------------------------------------------------------- W-039

#[test]
fn a_bytefield_is_a_packet_one_order_of_magnitude_up() {
    // Same layout, same splitting, same renderer — only the unit differs. The
    // geometry is asserted rather than assumed, because "it reuses packet" is
    // exactly the claim that stops being true quietly.
    let l = build(
        r#"
        type bytefield { width: 16 cell: 10 40 }
        node a "A" { bytes: 0 3 }
        node b "B" { bytes: 4 15 }
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Packet));
    assert_eq!(l.nodes[0].rect.x, 0.0);
    assert_eq!(l.nodes[0].rect.w, 40.0, "four bytes at ten each");
    assert_eq!(l.nodes[1].rect.x, 40.0);
    assert_eq!(l.nodes[0].rect.y, l.nodes[1].rect.y);
}

#[test]
fn a_bytefield_row_is_sixteen_bytes_by_default() {
    // A hex dump is read sixteen across; a packet is read thirty-two bits
    // across. Sharing one default would make one of the two wrong.
    let l = build("type bytefield { cell: 10 40 ruler: off }\nnode a \"A\" { bytes: 0 31 }");

    // Thirty-two bytes at sixteen per row is two rows, so two segments.
    assert_eq!(l.nodes.len(), 2, "{:?}", l.nodes.len());
    assert!(l.nodes[1].rect.y > l.nodes[0].rect.y);
    assert_eq!(l.nodes[0].rect.w, 160.0);
}

#[test]
fn the_bytefield_ruler_counts_in_fours() {
    // Bits group by eight, bytes by four — that is where a reader's eye
    // already puts the marks.
    let l = build("type bytefield { width: 16 cell: 10 40 }\nnode a \"A\" { bytes: 0 15 }");
    let ticks: Vec<&str> = l
        .annotations
        .iter()
        .map(|a| a.text.as_str())
        .filter(|t| t.parse::<u32>().is_ok())
        .collect();
    assert!(
        ticks.contains(&"4") && ticks.contains(&"12"),
        "byte offsets every four: {ticks:?}"
    );
    assert!(!ticks.contains(&"2"), "not every two: {ticks:?}");
}

#[test]
fn a_bytefield_field_needs_bytes_and_says_so_in_its_own_unit() {
    // The diagnostics are the shared ones, but a message about `bits` in a
    // byte diagram would send the author to the wrong property.
    let e = err("type bytefield\nnode a \"A\"");
    assert!(e.contains("has no `bytes`"), "{e}");
    assert!(e.contains("bytefield"), "{e}");

    let e = err("type bytefield\nnode a { bytes: 0 3 }\nnode b { bytes: 2 7 }");
    assert!(e.contains("covers bytes 2–7"), "{e}");

    let d = diags("type bytefield\nnode a { bytes: 0 3 }\nnode b { bytes: 6 7 }");
    assert!(
        d.iter()
            .any(|x| x.message.contains("bytes 4–5 are unassigned")),
        "{d:?}"
    );
}

#[test]
fn bits_and_bytes_do_not_answer_for_each_other() {
    // They are indistinguishable by value — only the archetype knows which
    // unit was meant — so each is refused in the other's genre by name.
    let e = err("type bytefield\nnode a { bits: 0 3 }");
    assert!(e.contains("`type packet`"), "{e}");

    let e = err("type packet\nnode a { bytes: 0 3 }");
    assert!(e.contains("`type bytefield`"), "{e}");
}

#[test]
fn chart_data_outside_its_archetype_is_named_with_the_genre_that_reads_it() {
    // It used to be a warning that the property "has no effect" — which left
    // the reader to guess where it *does*. Each data property now lives with
    // the genre that reads it, so writing one elsewhere is an unknown
    // property, and the message says whose it is.
    let e = err("node a { value: 5 }");
    assert!(
        e.contains("`type pie`") && e.contains("`type treemap`"),
        "{e}"
    );

    let e = err("node a { bits: 0 7 }");
    assert!(e.contains("`type packet`"), "{e}");

    let e = err("type pie\nslice a \"A\" 1 { wave: \"01\" }");
    assert!(e.contains("`type wave`"), "{e}");

    // A name that is nobody's is still a typo, and gets the other message.
    let e = err("node a { fll: #fff }");
    assert!(e.contains("did you mean `fill`"), "{e}");
}

#[test]
fn each_data_property_belongs_to_exactly_the_genres_that_read_it() {
    // Wiring these tables by hand once put `wave`'s properties on `free`,
    // where they silently became the answer for every diagram. Two names
    // collide — `wave` is an archetype *and* a property — so the mapping is
    // asserted rather than eyeballed.
    for (prop, owners) in [
        ("value", &["pie", "sankey", "treemap"][..]),
        ("bits", &["packet"]),
        ("bytes", &["bytefield"]),
        ("u", &["rack"]),
        ("wire", &["harness"]),
        ("branch", &["gitgraph"]),
        ("tag", &["gitgraph"]),
        ("count", &["venn"]),
        ("shared", &["venn"]),
        ("role", &["network"]),
        ("addr", &["network"]),
        ("speed", &["network"]),
        ("x", &["quadrant"]),
        ("y", &["quadrant"]),
        ("wave", &["wave"]),
        ("data", &["wave"]),
        ("points", &["bar", "radar", "xy"]),
        ("pairs", &["xy"]),
        ("scores", &["journey"]),
        ("limit", &["kanban"]),
        ("risk", &["requirements"]),
        ("verify", &["requirements"]),
        ("link", &["requirements"]),
        ("start", &["gantt"]),
        ("length", &["gantt"]),
        ("guard", &["state"]),
        ("from-card", &["er"]),
        ("to-card", &["er"]),
        ("date", &["timeline"]),
        ("tech", &["block"]),
        // `description` used to be here. It is shared now: every genre emits
        // one as an SVG `<title>`, and `block` is simply the genre that also
        // draws it. One property, one meaning — the case `D-015` is for.
        ("kind", &["block", "state"]),
    ] {
        let mut found = crate::props::genres_reading(prop);
        found.sort_unstable();
        let mut want = owners.to_vec();
        want.sort_unstable();
        assert_eq!(found, want, "`{prop}`");
    }

    // And nothing archetype-specific is left on the shared table, where it
    // would be offered in every genre.
    for def in crate::props::PROPS {
        assert!(
            crate::props::genres_reading(def.name).is_empty(),
            "`{}` is on both the shared table and an archetype's",
            def.name
        );
    }
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

/// The `tree-sitter` CLI rebuilds the parser library on demand, into a path
/// it shares. Cargo runs tests in parallel, so two of them invoking it at
/// once could catch the library mid-write — which failed the corpus guard
/// twice, spuriously. Serialising the calls costs nothing and a flaky guard
/// stops being believed.
static TREE_SITTER: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn tree_sitter_lock() -> std::sync::MutexGuard<'static, ()> {
    TREE_SITTER.lock().unwrap_or_else(|e| e.into_inner())
}

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
    let _serial = tree_sitter_lock();
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

    // The examples were named by hand here — three of them, out of nineteen —
    // so every file added since was cross-checked by nothing, and the list
    // still pointed at `diagram.bd` after that file went away, which failed
    // the whole guard rather than the one entry. Sweep the directory: a list
    // written by hand is a list that stops being true.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut examples: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {dir}: {e}"))
        .filter_map(|entry| {
            let path = entry.expect("readable entry").path();
            (path.extension().is_some_and(|e| e == "bd")).then_some(path)
        })
        .collect();
    examples.sort();
    assert!(
        examples.len() >= 16,
        "only {} examples found — the sweep is not finding them",
        examples.len()
    );
    probe.extend(examples);

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

// ------------------------------------------------- per-archetype vocabulary

#[test]
fn each_archetype_has_its_own_word_for_its_elements() {
    let l = build(
        r#"
        type pie
        slice organic "Organic search" 4820
        slice direct  "Direct"         2410
        slice referral 1180
    "#,
    );

    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices.len(), 3);
    // 4820 + 2410 + 1180 = 8410; the first slice is 4820/8410 of the circle.
    assert!((pie.slices[0].end - 4820.0 / 8410.0 * 360.0).abs() < 0.01);

    let l = build(
        r#"
        type packet { width: 32 cell: 10 40 }
        field sport "Source port"      0 15
        field dport "Destination port" 16 31
    "#,
    );
    assert_eq!(l.nodes[0].rect.w, 160.0);
    assert_eq!(l.nodes[1].rect.x, 160.0);
}

#[test]
fn positional_data_is_sugar_for_the_property() {
    let terse = build("type pie\nslice a \"A\" 30\nslice b \"B\" 70");
    let verbose = build("type pie\nnode a \"A\" { value: 30 }\nnode b \"B\" { value: 70 }");

    let (crate::layout::Chart::Pie(x), crate::layout::Chart::Pie(y)) =
        (&terse.chart, &verbose.chart)
    else {
        panic!("expected pies");
    };
    assert_eq!(x.slices[0].end, y.slices[0].end);
}

#[test]
fn node_stays_a_universal_synonym() {
    // Mixed on purpose: nothing that worked before stops working.
    let l = build("type pie\nslice a \"A\" 30\nnode b \"B\" { value: 70 }");
    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices.len(), 2);
}

#[test]
fn a_foreign_keyword_says_where_it_belongs() {
    let e = err("type pie\nfield a \"A\" 0 15");
    assert!(e.contains("declares slices, not fields"), "{e}");
    assert!(e.contains("belongs to `type packet`"), "{e}");

    let e = err("type pie\nwidget a \"A\" 1");
    assert!(e.contains("declares slices, not widgets"), "{e}");

    // A bare `alpha beta` is now an item declaration with an unknown keyword,
    // not a syntax error — X-007 records that deliberate change.
    let e = err("alpha beta");
    assert!(e.contains("declares nodes, not alphas"), "{e}");
}

#[test]
fn positional_arity_is_checked() {
    let e = err("type pie\nslice a \"A\" 10 20");
    assert!(e.contains("takes 1 value after the label"), "{e}");

    let e = err("node a \"A\" 10");
    assert!(e.contains("takes no data after the label"), "{e}");

    let d = diags("type pie\nslice a \"A\" 10 { value: 99 }");
    assert!(
        d.iter().any(|x| x.message.contains("the block wins")),
        "{d:?}"
    );
}

#[test]
fn the_first_string_is_always_the_label() {
    let l = build("type pie\nslice a \"Organic\" 10\nslice b 20");
    assert_eq!(l.nodes.len(), 2);

    // Without a label the identifier is displayed; the number is still data.
    let crate::layout::Chart::Pie(pie) = &l.chart else {
        panic!("expected a pie");
    };
    assert_eq!(pie.slices.len(), 2);
}

#[test]
fn the_implicit_theme_follows_the_keyword() {
    // `style slice` is idiomatic in a pie; `style node` still reaches it.
    let by_keyword = build("type pie\nstyle slice { font-color: #ff0000 }\nslice a \"A\" 1");
    let by_node = build("type pie\nstyle node { font-color: #ff0000 }\nslice a \"A\" 1");
    for l in [&by_keyword, &by_node] {
        assert!(
            l.annotations.iter().any(|a| a.color == "#ff0000"),
            "the implicit theme did not reach the chart text"
        );
    }

    // Declared together, the specific name wins.
    let both = build(
        "type pie\nstyle node { font-color: #00ff00 }\nstyle slice { font-color: #0000ff }\nslice a \"A\" 1",
    );
    assert!(both.annotations.iter().all(|a| a.color == "#0000ff"));
}

#[test]
fn a_statement_runs_to_its_separator() {
    // Requiring a separator is what makes `slice a b` unambiguous: everything
    // up to the newline belongs to one statement. So `node a node b` is a
    // single item whose positional values are `node` and `b` — which `free`
    // takes none of, and says so.
    let one = parse_diagram("node a node b").expect("parses as one statement");
    assert_eq!(one.nodes.len(), 1, "two statements would need a separator");
    assert!(err("node a node b").contains("takes no data after the label"));

    for separated in ["node a\nnode b", "node a; node b", "node a, node b"] {
        assert_eq!(
            parse_diagram(separated).expect("parses").nodes.len(),
            2,
            "{separated:?}"
        );
    }
}

#[test]
fn the_editor_queries_match_the_grammar() {
    // A query naming a node type the grammar no longer has does not degrade
    // gracefully: Zed fails to load the language and stops recognising the
    // file type at all. Renaming `node_declaration` to `item_declaration` in
    // W-029 broke exactly this way, and shipped.
    let queries = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/editors/zed/languages/bgraph"
    ));
    let sample = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/packet.bd");

    let mut files: Vec<_> = std::fs::read_dir(queries)
        .expect("the extension ships queries")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "scm"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no .scm files in {}", queries.display());

    let run = |path: &std::path::Path| -> Option<std::process::Output> {
        let _serial = tree_sitter_lock();
        std::process::Command::new("tree-sitter")
            .arg("query")
            .arg(path)
            .arg(sample)
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tree-sitter-bgraph"))
            .output()
            .ok()
    };

    if run(&files[0]).is_none() {
        println!(
            "SKIPPED: `tree-sitter` is not on PATH, so the editor queries were \
             not checked against the grammar."
        );
        return;
    }

    let mut broken = Vec::new();
    for path in &files {
        let out = run(path).expect("the CLI was found a moment ago");
        if !out.status.success() {
            broken.push(format!(
                "{}: {}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                String::from_utf8_lossy(&out.stderr)
                    .lines()
                    .last()
                    .unwrap_or("failed")
            ));
        }
    }

    assert!(
        broken.is_empty(),
        "editor queries no longer compile against the grammar:\n  {}",
        broken.join("\n  ")
    );
}

#[test]
fn the_grammar_corpus_expectations_still_hold() {
    // `tree-sitter test`'s summary line counts *parses*, not expectation
    // matches — "failed parses: 0" says nothing about whether the trees are
    // what the corpus claims. Only the exit code does, and reading the wrong
    // line let the corpus go stale across a whole rename.
    let _serial = tree_sitter_lock();
    let status = std::process::Command::new("tree-sitter")
        .arg("test")
        .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tree-sitter-bgraph"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    let Ok(status) = status else {
        println!(
            "SKIPPED: `tree-sitter` is not on PATH, so the grammar corpus was \
             not checked. Install it with `cargo install tree-sitter-cli`."
        );
        return;
    };

    assert!(
        status.success(),
        "the grammar corpus no longer matches the parser — run \
         `cd tree-sitter-bgraph && tree-sitter test` to see which cases, and \
         `--update` once you have read the diff"
    );
}

// ---------------------------------------------------------------- groups

#[test]
fn a_group_is_sized_by_its_members() {
    let l = build(
        r#"
        group box "Box" {
          padding: 10
          node a at (0, 0) { size: 50 40 }
          node b at (100, 60) { size: 50 40 }
        }
    "#,
    );

    assert_eq!(l.groups.len(), 1);
    let r = l.groups[0].rect;
    // Members span (0,0)..(150,100); the frame adds its padding all round, and
    // a *titled* group reserves a band at the top for the name — without it
    // the title had nowhere to go but across the border or onto the first
    // member.
    assert_eq!((r.x, r.y, r.w, r.h), (-10.0, -26.0, 170.0, 136.0));
    assert_eq!(l.groups[0].lines, vec!["Box"]);

    // Untitled, the band is not reserved: the size is the members plus the
    // padding and nothing else.
    let bare = build("group box {\n  padding: 10\n  node a at (0, 0) { size: 50 40 }\n}");
    let r = bare.groups[0].rect;
    assert_eq!((r.x, r.y, r.w, r.h), (-10.0, -10.0, 70.0, 60.0));
}

#[test]
fn groups_nest() {
    let l = build(
        r#"
        group outer {
          padding: 10
          node a at (0, 0) { size: 20 20 }
          group inner {
            padding: 5
            node b at (100, 0) { size: 20 20 }
          }
        }
    "#,
    );

    assert_eq!(l.groups.len(), 2);
    let inner = l
        .groups
        .iter()
        .min_by(|x, y| x.rect.w.total_cmp(&y.rect.w))
        .unwrap();
    let outer = l
        .groups
        .iter()
        .max_by(|x, y| x.rect.w.total_cmp(&y.rect.w))
        .unwrap();
    assert_eq!((inner.rect.x, inner.rect.w), (95.0, 30.0));
    // The outer frame encloses the inner frame, not merely the inner nodes.
    assert_eq!((outer.rect.x, outer.rect.w), (-10.0, 145.0));
}

#[test]
fn a_group_can_be_anchored_to() {
    let l = build(
        r#"
        group cluster {
          padding: 10
          node a at (0, 0) { size: 40 40 }
        }
        node after right-of cluster gap 20 { size: 30 30 }
    "#,
    );

    // The frame spans -10..50; `after` starts a gap beyond its right edge.
    assert_eq!(l.groups[0].rect.right(), 50.0);
    assert_eq!(l.nodes[1].rect.x, 70.0);
}

#[test]
fn a_placed_group_translates_its_whole_subtree() {
    let l = build(
        r#"
        node origin at (0, 0) { size: 10 10 }
        group moved at (200, 100) {
          padding: 0
          node a at (0, 0) { size: 40 40 }
          node b right-of a gap 10 { size: 40 40 }
        }
    "#,
    );

    let frame = l.groups[0].rect;
    assert_eq!((frame.x, frame.y), (200.0, 100.0));
    // Members keep their relative arrangement and move with the frame.
    assert_eq!((l.nodes[1].rect.x, l.nodes[1].rect.y), (200.0, 100.0));
    assert_eq!(l.nodes[2].rect.x, 250.0);
}

#[test]
fn an_unplaced_node_flows_from_its_sibling_not_the_previous_line() {
    let l = build(
        r#"
        graph { gap: 10 }
        node outside at (0, 0) { size: 20 20 }
        group g {
          padding: 0
          node first { size: 20 20 }
          node second { size: 20 20 }
        }
    "#,
    );

    // `first` is the first declaration inside `g`, so it starts the group's
    // own flow rather than trailing `outside`.
    assert_eq!((l.nodes[1].rect.x, l.nodes[1].rect.y), (0.0, 0.0));
    assert_eq!(l.nodes[2].rect.x, 30.0);
}

#[test]
fn groups_share_the_item_namespace() {
    assert!(err("group g {\n node g\n}").contains("declared twice"));
    assert!(err("group node {\n node a\n}").contains("reserved word"));
    assert!(err("node a\nnode b right-of ghost").contains("unknown node `ghost`"));
}

#[test]
fn an_edge_to_a_frame_is_refused() {
    let e = err("group g {\n node a\n}\nnode b\ng -> b");
    assert!(
        e.contains("is a group; edges connect items, not frames"),
        "{e}"
    );
}

#[test]
fn an_empty_group_says_so() {
    let d = diags("group g {\n}");
    assert!(d.iter().any(|x| x.message.contains("is empty")), "{d:?}");
}

#[test]
fn a_group_cycle_is_reported() {
    // The frame cannot be placed relative to something it encloses.
    let e = err("group g right-of a {\n node a\n}");
    assert!(e.contains("anchor cycle"), "{e}");
}

#[test]
fn a_group_is_drawn_behind_what_it_encloses() {
    let svg = render_svg::render(&build(
        "group g \"G\" {\n node a \"A\" at (0,0) { size: 40 40 }\n}",
    ));
    let frame = svg.find("rx=\"10\"").expect("the frame is drawn");
    let node = svg.find("rx=\"8\"").expect("the node is drawn");
    assert!(frame < node, "the frame must come first in document order");
}

// --------------------------------------------------------------- treemap

#[test]
fn a_treemap_fills_its_rectangle() {
    let l = build(
        r#"
        type treemap { size: 400 300 gap: 1 }
        box a "A" 50
        box b "B" 30
        box c "C" 20
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Treemap));
    assert_eq!(l.nodes.len(), 3);

    // Areas are proportional and together cover the frame.
    let total: f64 = l.nodes.iter().map(|n| n.rect.w * n.rect.h).sum();
    assert!(
        (total - 400.0 * 300.0).abs() / (400.0 * 300.0) < 0.05,
        "{total}"
    );

    let areas: Vec<f64> = l.nodes.iter().map(|n| n.rect.w * n.rect.h).collect();
    assert!((areas[0] / areas[1] - 50.0 / 30.0).abs() < 0.1, "{areas:?}");
    assert!((areas[0] / areas[2] - 50.0 / 20.0).abs() < 0.1, "{areas:?}");
}

#[test]
fn a_groups_area_is_the_sum_of_what_it_holds() {
    let l = build(
        r#"
        type treemap { size: 400 300 gap: 1 header: 0 }
        group left {
          box a "A" 30
          box b "B" 30
        }
        box c "C" 40
    "#,
    );

    assert_eq!(l.groups.len(), 1);
    let frame = l.groups[0].rect;
    let solo = l.nodes.iter().find(|n| n.rect.w * n.rect.h > 0.0).unwrap();
    let _ = solo;
    // 60 of 100 belongs to the group.
    let share = frame.w * frame.h / (400.0 * 300.0);
    assert!((share - 0.6).abs() < 0.06, "share {share}");
}

#[test]
fn tiles_never_touch() {
    let l = build(
        r#"
        type treemap { size: 400 300 gap: 4 }
        box a "A" 40
        box b "B" 35
        box c "C" 25
    "#,
    );

    for (i, x) in l.nodes.iter().enumerate() {
        for y in l.nodes.iter().skip(i + 1) {
            let overlap = x.rect.x < y.rect.right()
                && y.rect.x < x.rect.right()
                && x.rect.y < y.rect.bottom()
                && y.rect.y < x.rect.bottom();
            assert!(!overlap, "tiles overlap: {:?} {:?}", x.rect, y.rect);
        }
    }
}

#[test]
fn a_subtree_keeps_one_hue() {
    let l = build(
        r#"
        type treemap { size: 400 300 }
        group one { box a "A" 25; box b "B" 25 }
        group two { box c "C" 25; box d "D" 25 }
    "#,
    );

    // Two branches, two palette slots — and the children stay in the family.
    let hues: Vec<&str> = l
        .groups
        .iter()
        .filter_map(|g| g.style.fill.as_deref())
        .collect();
    assert_eq!(hues.len(), 2);
    assert_ne!(hues[0], hues[1], "each top-level branch takes its own hue");
    assert_eq!(hues[0], crate::props::PALETTE_LIGHT[0]);
    assert_eq!(hues[1], crate::props::PALETTE_LIGHT[1]);
}

#[test]
fn a_treemap_tile_needs_an_area() {
    assert!(err("type treemap\nbox a \"A\"").contains("has no value"));
    assert!(
        err("type treemap\ngroup g { value: 5\n box b 1\n}").contains("cannot declare a value")
    );
}

#[test]
fn a_label_that_does_not_fit_degrades_rather_than_lying() {
    // A wide tile carries name and value; a narrow one keeps the name only.
    let wide = build(
        r#"
        type treemap { size: 600 300 labels: both }
        box only "a-very-long-tile-name" 100
    "#,
    );
    assert!(wide.annotations.iter().any(|a| a.text.contains("100")));

    // Narrow enough that the pair genuinely does not fit. The threshold moved
    // when `W-040` replaced the flat 0.56 with real per-character advances —
    // this label is mostly thin letters, so it now needs less room than the
    // old estimate charged it for.
    let narrow = build(
        r#"
        type treemap { size: 120 300 labels: both }
        box only "a-very-long-tile-name" 100
    "#,
    );
    assert_eq!(
        narrow
            .annotations
            .iter()
            .map(|a| a.text.as_str())
            .collect::<Vec<_>>(),
        vec!["a-very-long-tile-name"],
        "the value should be dropped before the name"
    );
}

// -------------------------------------------------------------- quadrant

#[test]
fn a_quadrant_maps_data_space_onto_the_plot() {
    let l = build(
        r#"
        type quadrant { size: 400 400 }
        point a "A" 0 0
        point b "B" 1 1
        point c "C" 0.5 0.5
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Quadrant));

    let centre = |i: usize| {
        let r = l.nodes[i].rect;
        (r.cx(), r.cy())
    };
    // y grows upward on the page: (0,0) is bottom-left, (1,1) top-right.
    assert_eq!(centre(0), (0.0, 400.0));
    assert_eq!(centre(1), (400.0, 0.0));
    assert_eq!(centre(2), (200.0, 200.0));
}

#[test]
fn a_quadrant_honours_its_range() {
    let l = build(
        r#"
        type quadrant { size: 100 100 range: 0 10 }
        point a "A" 5 10
    "#,
    );
    let r = l.nodes[0].rect;
    assert_eq!((r.cx(), r.cy()), (50.0, 0.0));
}

#[test]
fn a_point_outside_the_range_is_pinned_and_reported() {
    let d = diags("type quadrant\npoint a \"A\" 1.4 0.5");
    assert!(
        d.iter().any(|x| x.message.contains("outside the range")),
        "{d:?}"
    );

    let l = build("type quadrant { size: 100 100 }\npoint a \"A\" 1.4 0.5");
    assert_eq!(
        l.nodes[0].rect.cx(),
        100.0,
        "clamped to the edge, not off it"
    );
}

#[test]
fn a_point_needs_both_coordinates() {
    let e = err("type quadrant\npoint a \"A\" 0.5");
    assert!(e.contains("needs two coordinates"), "{e}");
}

#[test]
fn the_frame_and_cross_are_drawn_as_guides() {
    let l = build("type quadrant { size: 100 100 }\npoint a \"A\" 0.5 0.5");
    // Four sides plus two dividers.
    assert_eq!(l.guides.len(), 6);
    assert_eq!(l.guides.iter().filter(|g| g.dash.is_some()).count(), 2);

    let svg = render_svg::render(&l);
    assert_eq!(svg.matches("<line").count(), 6);
    // Guides go under the marks.
    assert!(svg.find("<line").unwrap() < svg.find("<ellipse").unwrap());
}

#[test]
fn axis_and_region_names_are_counted() {
    assert!(
        err("type quadrant { x-axis: \"only one\" }\npoint a 0.5 0.5")
            .contains("takes exactly two labels")
    );
    assert!(
        err("type quadrant { quadrants: \"a\" \"b\" }\npoint a 0.5 0.5")
            .contains("takes four names")
    );
}

#[test]
fn a_point_label_turns_inward_near_the_right_edge() {
    let l = build(
        r#"
        type quadrant { size: 200 200 }
        point left "L" 0.1 0.5
        point right "R" 0.9 0.5
    "#,
    );

    let by_text = |t: &str| l.annotations.iter().find(|a| a.text == t).unwrap();
    assert_eq!(by_text("L").anchor, crate::layout::Anchor::Start);
    assert_eq!(by_text("R").anchor, crate::layout::Anchor::End);
}

#[test]
fn contrast_is_measured_against_what_each_archetype_actually_paints() {
    // A quadrant's labels sit on the canvas, not on a box fill. Comparing
    // them against `node_defaults`' white fill reported every one of them.
    let d = diags(
        "type quadrant\ngraph { background: #0b1120 }\nstyle point { font-color: #e2e8f0 }\npoint a \"A\" 0.5 0.5",
    );
    assert!(!d.iter().any(|x| x.message.contains("readable")), "{d:?}");
}

// ------------------------------------------------------------------ wave

fn wave_layout(source: &str) -> crate::layout::Layout {
    let l = build(source);
    assert!(matches!(l.chart, crate::layout::Chart::Wave));
    l
}

#[test]
fn a_restated_value_is_a_new_segment_but_a_dot_is_not() {
    // `=.=.` is two values held two periods each; `=...` is one held four.
    // Merging on equal levels alone silently joined them.
    let two = wave_layout(
        r#"
        type wave { period: 10 }
        signal bus "Bus" "=.=." { data: "a" "b" }
    "#,
    );
    let one = wave_layout(
        r#"
        type wave { period: 10 }
        signal bus "Bus" "=..." { data: "a" }
    "#,
    );

    let hexes = |l: &crate::layout::Layout| l.polylines.iter().filter(|p| p.closed).count();
    assert_eq!(hexes(&two), 2);
    assert_eq!(hexes(&one), 1);

    let labels: Vec<&str> = two.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(labels.contains(&"a") && labels.contains(&"b"), "{labels:?}");
}

#[test]
fn a_clock_has_two_edges_per_period() {
    let l = wave_layout(
        r#"
        type wave { period: 20 lane: 20 0 grid: off ruler: off }
        signal clk "clk" "pp"
    "#,
    );

    let line = l
        .polylines
        .iter()
        .find(|p| !p.closed)
        .expect("a clock line");
    // Two periods, each rising then falling, plus the closing point.
    let tops = line
        .points
        .iter()
        .filter(|(_, y)| *y == line.points[0].1)
        .count();
    assert!(tops >= 2, "{:?}", line.points);
    assert!(line.points.len() >= 8, "{:?}", line.points);
}

#[test]
fn levels_hold_and_transitions_slope() {
    let l = wave_layout(
        r#"
        type wave { period: 20 lane: 20 0 grid: off ruler: off }
        signal s "s" "0011"
    "#,
    );

    let line = l
        .polylines
        .iter()
        .find(|p| !p.closed)
        .expect("a level line");
    let ys: Vec<f64> = line.points.iter().map(|(_, y)| *y).collect();
    // Starts low, ends high, and the change is not a vertical jump.
    assert_eq!(ys.first(), ys.get(1));
    assert_ne!(ys.first(), ys.last());
    let xs: Vec<f64> = line.points.iter().map(|(x, _)| *x).collect();
    assert!(
        xs.windows(2).all(|w| w[1] >= w[0]),
        "x must advance: {xs:?}"
    );
}

#[test]
fn a_signal_needs_a_wave() {
    assert!(err("type wave\nsignal s \"S\"").contains("has no wave"));
}

#[test]
fn an_unknown_wave_character_names_the_alphabet() {
    let e = err("type wave\nsignal s \"S\" \"01Q1\"");
    assert!(e.contains("`Q` at position 2"), "{e}");
    assert!(e.contains(crate::props::WAVE_ALPHABET), "{e}");
}

#[test]
fn a_wave_cannot_open_with_a_repeat() {
    assert!(err("type wave\nsignal s \"S\" \".01\"").contains("there is none"));
}

#[test]
fn unnamed_data_segments_are_reported() {
    let d = diags("type wave\nsignal s \"S\" \"=.=.=.\" { data: \"only\" }");
    assert!(
        d.iter()
            .any(|x| x.message.contains("3 data segments but 1 name")),
        "{d:?}"
    );
}

#[test]
fn a_gap_breaks_the_record() {
    let l = wave_layout(
        r#"
        type wave { period: 20 lane: 20 0 grid: off ruler: off }
        signal s "s" "11|11"
    "#,
    );
    // Two runs of line, plus the two strokes that mark the break.
    let open = l.polylines.iter().filter(|p| !p.closed).count();
    assert_eq!(open, 4, "a gap splits the line and marks the seam");
}

// ---------------------------------------------------------------- sankey

#[test]
fn a_node_is_as_tall_as_what_flows_through_it() {
    let l = build(
        r#"
        type sankey { size: 300 200 gap: 0 width: 10 }
        node a
        node b
        node c
        a -> c 30
        b -> c 70
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Sankey));
    let h: Vec<f64> = l.nodes.iter().map(|n| n.rect.h).collect();
    // a and b together are exactly c.
    assert!((h[0] + h[1] - h[2]).abs() < 0.01, "{h:?}");
    assert!((h[0] / h[1] - 30.0 / 70.0).abs() < 0.01, "{h:?}");
    // `gap` has a floor of 2, so the busiest column plus its gaps fills the
    // height rather than the tallest bar alone.
    assert!((h[0] + h[1] + 2.0 - 200.0).abs() < 0.01, "{h:?}");
}

#[test]
fn layers_read_left_to_right() {
    let l = build(
        r#"
        type sankey { size: 300 100 }
        node a
        node b
        node c
        a -> b 10
        b -> c 10
    "#,
    );

    let x: Vec<f64> = l.nodes.iter().map(|n| n.rect.x).collect();
    assert!(x[0] < x[1] && x[1] < x[2], "{x:?}");
    // A node sits right of everything that feeds it, however it was declared.
    let l = build(
        r#"
        type sankey { size: 300 100 }
        node last
        node first
        first -> last 10
    "#,
    );
    assert!(l.nodes[1].rect.x < l.nodes[0].rect.x);
}

#[test]
fn a_flow_carries_its_value_in_position() {
    let terse = build("type sankey\nnode a\nnode b\na -> b 40");
    let verbose = build("type sankey\nnode a\nnode b\na -> b { value: 40 }");
    assert_eq!(terse.nodes[0].rect.h, verbose.nodes[0].rect.h);

    // The label still comes first when both are written.
    let labelled = build("type sankey\nnode a\nnode b\na -> b \"most of it\" 40");
    assert_eq!(labelled.nodes[0].rect.h, terse.nodes[0].rect.h);
}

#[test]
fn a_ribbon_takes_its_sources_hue() {
    // `edge_defaults` always sets `stroke`, so resolving on it made every
    // ribbon the same grey — the third time that trap has been sprung.
    let l = build(
        r#"
        type sankey { size: 300 100 }
        node a
        node b
        node c
        a -> c 10
        b -> c 10
    "#,
    );

    let fills: Vec<&str> = l
        .polylines
        .iter()
        .filter_map(|p| p.fill.as_deref())
        .collect();
    assert_eq!(fills.len(), 2);
    assert_ne!(fills[0], fills[1], "two sources, two hues");
    assert!(
        fills[0].starts_with(crate::props::PALETTE_LIGHT[0]),
        "{fills:?}"
    );
}

#[test]
fn a_flow_needs_a_magnitude() {
    assert!(err("type sankey\nnode a\nnode b\na -> b").contains("has no value"));
    assert!(
        err("type sankey\nnode a { value: 5 }\nnode b\na -> b 1")
            .contains("cannot declare a value")
    );
}

#[test]
fn a_flow_cycle_is_refused() {
    let e = err("type sankey\nnode a\nnode b\na -> b 1\nb -> a 1");
    assert!(e.contains("form a cycle"), "{e}");
    assert!(e.contains("reads left to right"), "{e}");
}

#[test]
fn a_node_with_no_flows_is_reported() {
    let d = diags("type sankey\nnode a\nnode b\nnode lonely\na -> b 1");
    assert!(
        d.iter()
            .any(|x| x.message.contains("`lonely` has no flows")),
        "{d:?}"
    );
}

// ------------------------------------------------------------------- bar

#[test]
fn a_value_axis_rounds_outward_to_human_numbers() {
    use crate::charts::nice_scale;

    let s = nice_scale(0.0, 310.0, 5);
    assert_eq!(s.min, 0.0);
    assert_eq!(s.max, 400.0);
    assert_eq!(s.ticks, vec![0.0, 100.0, 200.0, 300.0, 400.0]);

    // Negative data pulls the axis below zero rather than clipping.
    let s = nice_scale(-30.0, 90.0, 4);
    assert!(s.min <= -30.0 && s.max >= 90.0);
    assert!(
        s.ticks.contains(&0.0),
        "zero is always a tick: {:?}",
        s.ticks
    );

    // A flat series still produces a usable axis.
    let s = nice_scale(5.0, 5.0, 5);
    assert!(s.max > s.min);
}

#[test]
fn bars_stand_on_zero_and_scale_together() {
    let l = build(
        r#"
        type bar { size: 400 200 categories: "a" "b" legend: off }
        series only "Only" 50 100
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Bar));
    let bars: Vec<&crate::layout::PlacedNode> = l.nodes.iter().collect();
    assert_eq!(bars.len(), 2);
    // Both sit on the baseline, and twice the value is twice the height.
    assert!((bars[0].rect.bottom() - bars[1].rect.bottom()).abs() < 0.01);
    assert!((bars[1].rect.h / bars[0].rect.h - 2.0).abs() < 0.01);
}

#[test]
fn series_stand_side_by_side_or_stack() {
    let grouped = build(
        r#"
        type bar { size: 400 200 categories: "a" legend: off }
        series one "One" 10
        series two "Two" 20
    "#,
    );
    let g: Vec<f64> = grouped.nodes.iter().map(|n| n.rect.x).collect();
    assert_ne!(g[0], g[1], "grouped bars stand beside each other");

    let stacked = build(
        r#"
        type bar { size: 400 200 categories: "a" stack: on legend: off }
        series one "One" 10
        series two "Two" 20
    "#,
    );
    let s: Vec<&crate::layout::Rect> = stacked.nodes.iter().map(|n| &n.rect).collect();
    assert_eq!(s[0].x, s[1].x, "stacked bars share a column");
    assert!(s[1].bottom() < s[0].y, "and never overlap");
    // A 2px surface gap between fills, stacked segments included.
    assert!((s[0].y - s[1].bottom() - 2.0).abs() < 0.01, "{s:?}");
}

#[test]
fn a_stacked_axis_covers_the_totals() {
    let l = build(
        r#"
        type bar { size: 400 200 categories: "a" stack: on legend: off }
        series one "One" 60
        series two "Two" 60
    "#,
    );
    // 120 stacked, so the axis must reach past it, not past 60.
    let top = l.nodes.iter().map(|n| n.rect.y).fold(f64::MAX, f64::min);
    assert!(top > 0.0, "the stack must fit inside the plot");
}

#[test]
fn a_legend_appears_from_two_series_up() {
    let one = build(
        r#"
        type bar { categories: "a" }
        series only "Only" 1
    "#,
    );
    let two = build(
        r#"
        type bar { categories: "a" }
        series one "One" 1
        series two "Two" 2
    "#,
    );

    // Legend swatches are extra nodes beyond the bars themselves.
    assert_eq!(one.nodes.len(), 1, "one series names itself in the title");
    assert_eq!(two.nodes.len(), 4, "two bars plus two swatches");
    assert!(two.annotations.iter().any(|a| a.text == "One"));
}

#[test]
fn a_series_needs_numbers_and_matching_categories() {
    assert!(err("type bar { categories: \"a\" }\nseries s \"S\"").contains("has no numbers"));

    let d = diags("type bar { categories: \"a\" \"b\" \"c\" }\nseries s \"S\" 1 2");
    assert!(
        d.iter()
            .any(|x| x.message.contains("2 numbers for 3 categories")),
        "{d:?}"
    );

    let d = diags("type bar\nseries s \"S\" 1 2");
    assert!(
        d.iter()
            .any(|x| x.message.contains("stand along nothing named"))
    );
}

#[test]
fn the_axis_can_be_forced() {
    let l = build(
        r#"
        type bar { size: 400 200 categories: "a" y-axis: 0 1000 legend: off }
        series s "S" 100
    "#,
    );
    // A tenth of the axis, not the whole plot.
    assert!(
        (l.nodes[0].rect.h / 200.0 - 0.1).abs() < 0.01,
        "{:?}",
        l.nodes[0].rect
    );

    assert!(
        err("type bar { categories: \"a\" y-axis: 5 }\nseries s 1")
            .contains("takes a minimum and a maximum")
    );
}

// -------------------------------------------------------------------- xy

#[test]
fn a_line_reaches_both_edges_of_the_plot() {
    let l = build(
        r#"
        type xy { size: 400 200 categories: "a" "b" "c" legend: off }
        series s "S" 0 50 100
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Xy));
    let line = l.polylines.iter().find(|p| !p.closed).expect("a line");
    assert_eq!(line.points.len(), 3);

    // Points sit on the plot's edges, not in slot centres: a line that stops
    // short of the axis reads as clipped.
    let gutter = line.points[0].0;
    assert!(
        (line.points[2].0 - gutter - 400.0).abs() < 0.01,
        "{:?}",
        line.points
    );
    // And the values map onto the axis.
    assert!(line.points[0].1 > line.points[2].1);
}

#[test]
fn an_area_closes_on_the_baseline() {
    let l = build(
        r#"
        type xy { size: 400 200 categories: "a" "b" mark: area legend: off }
        series s "S" 40 80
    "#,
    );

    let band = l.polylines.iter().find(|p| p.closed).expect("an area");
    // Two points on the curve, two dropped to the baseline.
    assert_eq!(band.points.len(), 4);
    assert_eq!(band.points[2].1, band.points[3].1, "the base is level");
    assert!(
        band.fill.as_deref().is_some_and(|f| f.ends_with("40")),
        "translucent"
    );
    // The line is still drawn over it.
    assert!(l.polylines.iter().any(|p| !p.closed));
}

#[test]
fn dots_are_large_enough_to_aim_at() {
    let l = build(
        r#"
        type xy { size: 400 200 categories: "a" "b" mark: dot legend: off }
        series s "S" 40 80
    "#,
    );

    assert!(l.polylines.is_empty(), "a dot plot draws no line");
    assert_eq!(l.nodes.len(), 2);
    assert!(l.nodes[0].rect.w >= 8.0, "markers are at least 8px across");
}

#[test]
fn a_numeric_x_axis_replaces_the_categories() {
    let l = build(
        r#"
        type xy { size: 400 200 x-axis: 0 100 legend: off }
        series s "S" 1 2 3
    "#,
    );

    // Ticks come from the axis, not from category names.
    let labels: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(
        labels.contains(&"0") && labels.contains(&"100"),
        "{labels:?}"
    );

    assert!(err("type xy { x-axis: 5 }\nseries s 1").contains("takes a minimum and a maximum"));
}

// --------------------------------------------------------------- W-048, D-023

#[test]
fn the_written_keyword_chooses_the_positional_shape() {
    // The whole of D-023: the same archetype, two words, two shapes. Nothing
    // dispatches on how many values were written.
    let def = crate::props::archetype("xy").expect("xy exists");

    assert_eq!(def.slots_for("series")[0].prop, "points");
    assert_eq!(def.slots_for("scatter")[0].prop, "pairs");
    // `node` is a synonym, not a third form — it takes the primary shape.
    assert_eq!(def.slots_for("node")[0].prop, "points");

    assert!(def.accepts("series") && def.accepts("scatter") && def.accepts("node"));
    assert!(!def.accepts("slice"));
}

#[test]
fn a_scatter_point_lands_on_the_x_it_declares() {
    // An indexed series spreads evenly; a scatter does not. Declared x values
    // 0, 10 and 100 must land at the left edge, a tenth in, and the right —
    // never at 0, half and full, which is what evenly-spaced would give.
    let l = build(
        r#"
        type xy { size: 400 200 mark: dot legend: off }
        scatter s "S" 0 5 10 5 100 5
    "#,
    );

    let mut xs: Vec<f64> = l.nodes.iter().map(|n| n.rect.x + n.rect.w / 2.0).collect();
    xs.sort_by(f64::total_cmp);
    assert_eq!(xs.len(), 3, "three coordinate pairs, three marks");

    let span = xs[2] - xs[0];
    let tenth = (xs[1] - xs[0]) / span;
    assert!(
        (tenth - 0.1).abs() < 0.02,
        "the middle point sits a tenth along, not half: {tenth} from {xs:?}"
    );
}

#[test]
fn a_scatter_is_dots_until_told_otherwise() {
    // Joining scattered points by default would claim a continuity the data
    // does not have. `series` keeps its line.
    let dots = build("type xy { size: 400 200 legend: off }\nscatter s \"S\" 1 4 3 8");
    assert!(dots.polylines.is_empty(), "no line through a scatter");
    assert_eq!(dots.nodes.len(), 2);

    let lined = build("type xy { size: 400 200 legend: off }\nseries s \"S\" 4 8");
    assert!(!lined.polylines.is_empty(), "a series still draws its line");
}

#[test]
fn a_line_through_a_scatter_follows_x_not_declaration_order() {
    // Written out of order on purpose: a polyline in declaration order would
    // double back, which reads as data that goes backwards in x.
    let l = build(
        r#"
        type xy { size: 400 200 mark: line legend: off }
        scatter s "S" 90 1 10 9 50 5
    "#,
    );

    let path = &l.polylines[0].points;
    assert!(
        path.windows(2).all(|w| w[0].0 <= w[1].0),
        "the line runs left to right: {path:?}"
    );
}

#[test]
fn series_and_scatter_cannot_share_a_plot() {
    // Two meanings of x on one axis, and no reader could tell which mark used
    // which. Refused rather than blended.
    let e = err("type xy\nseries a \"A\" 1 2 3\nscatter b \"B\" 1 4 3 8");
    assert!(e.contains("cannot share a plot"), "{e}");
    assert!(e.contains("series") && e.contains("scatter"), "{e}");
}

#[test]
fn an_odd_count_of_coordinates_is_refused() {
    // The last x has no y. Silently dropping it would draw a plot missing a
    // point the author wrote.
    let e = err("type xy\nscatter s \"S\" 1 4 3");
    assert!(e.contains("two at a time"), "{e}");
    assert!(e.contains("last x has no y"), "{e}");
}

#[test]
fn a_scatter_needs_no_categories() {
    // It carries its own x, so the `no categories` warning would be noise.
    let diagram = parse_diagram("type xy\nscatter s \"S\" 1 4 3 8").expect("parses");
    let warnings: Vec<String> = crate::check::check(&diagram)
        .into_iter()
        .map(|d| d.message)
        .collect();
    assert!(
        !warnings.iter().any(|m| m.contains("categories")),
        "{warnings:?}"
    );
}

#[test]
fn the_second_keyword_belongs_to_its_own_archetype() {
    // `scatter` is xy's. Written under `bar` it is an unknown keyword, and the
    // message names the genre that does read it.
    let e = err("type bar { categories: \"a\" }\nscatter s \"S\" 1 4");
    assert!(e.contains("`type xy`"), "{e}");

    // And xy's own message names both of its words, not just the first.
    let e = err("type xy\nslice s \"S\" 1");
    assert!(e.contains("series") && e.contains("scatter"), "{e}");
}

#[test]
fn a_single_point_is_still_drawn() {
    let l = build(
        r#"
        type xy { size: 400 200 categories: "only" legend: off }
        series s "S" 42
    "#,
    );
    // No line to draw, so the point is marked instead of vanishing.
    assert_eq!(l.nodes.len(), 1);
}

#[test]
fn xy_and_bar_share_their_diagnostics() {
    assert!(err("type xy { categories: \"a\" }\nseries s \"S\"").contains("has no numbers"));
    let d = diags("type xy { categories: \"a\" \"b\" }\nseries s \"S\" 1");
    assert!(
        d.iter()
            .any(|x| x.message.contains("1 number for 2 categories")),
        "{d:?}"
    );
}

// -------------------------------------------------------------- sequence

#[test]
fn participants_head_their_own_lifelines() {
    let l = build(
        r#"
        type sequence { column: 100 row: 40 }
        participant a "A"
        participant b "B"
        a -> b "call"
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Sequence));
    assert_eq!(l.nodes.len(), 2);
    // One dashed lifeline per participant, under its box.
    assert_eq!(l.guides.len(), 2);
    assert!(l.guides.iter().all(|g| g.dash.is_some()));
    for (node, guide) in l.nodes.iter().zip(&l.guides) {
        assert!((guide.from.0 - node.rect.cx()).abs() < 0.01);
        assert!(guide.from.1 >= node.rect.bottom() - 0.01);
    }
}

#[test]
fn messages_run_down_the_page_in_source_order() {
    let l = build(
        r#"
        type sequence { column: 100 row: 40 }
        participant a "A"
        participant b "B"
        a -> b "first"
        b -> a "second"
        a -> b "third"
    "#,
    );

    let ys: Vec<f64> = l.polylines.iter().map(|p| p.points[0].1).collect();
    assert_eq!(ys.len(), 3);
    assert!(ys[0] < ys[1] && ys[1] < ys[2], "{ys:?}");
    // The second message runs the other way.
    assert!(l.polylines[0].points[0].0 < l.polylines[0].points[1].0);
    assert!(l.polylines[1].points[0].0 > l.polylines[1].points[1].0);
    // Every message carries an arrow.
    assert!(l.polylines.iter().all(|p| p.head.is_some()));
}

#[test]
fn a_message_to_oneself_loops_out_and_back() {
    let l = build(
        r#"
        type sequence { column: 100 row: 40 }
        participant a "A"
        a -> a "retry"
    "#,
    );

    let loop_ = &l.polylines[0];
    assert!(
        loop_.points.len() > 2,
        "a self-message is not a straight line"
    );
    // It leaves and returns to the same lifeline.
    let x = l.nodes[0].rect.cx();
    assert!((loop_.points[0].0 - x).abs() < 0.01);
    assert!(
        loop_.points.iter().any(|(px, _)| *px > x + 10.0),
        "it reaches out"
    );
    assert!(
        (loop_.points.last().unwrap().0 - x).abs() < 2.0,
        "and comes back"
    );
}

#[test]
fn a_group_frames_the_messages_it_holds() {
    let l = build(
        r#"
        type sequence { column: 100 row: 40 }
        participant a "A"
        participant b "B"
        a -> b "before"
        group retry "retry" {
          a -> b "inside"
          b -> a "also inside"
        }
        a -> b "after"
    "#,
    );

    assert_eq!(l.groups.len(), 1);
    let frame = l.groups[0].rect;
    let ys: Vec<f64> = l.polylines.iter().map(|p| p.points[0].1).collect();

    // It spans the two messages inside and neither of the ones outside.
    assert!(
        frame.y < ys[1] && frame.bottom() > ys[2],
        "{frame:?} {ys:?}"
    );
    assert!(frame.y > ys[0], "the message before must stay clear");
    assert!(frame.bottom() < ys[3], "and so must the one after");
    assert_eq!(l.groups[0].lines, vec!["retry"]);
}

#[test]
fn a_group_of_messages_is_not_empty() {
    // `members` holds items; a sequence frame holds edges, and counting only
    // the former called every `loop` block empty.
    let d = diags("type sequence\nparticipant a\nparticipant b\ngroup g {\n a -> b\n}");
    assert!(!d.iter().any(|x| x.message.contains("is empty")), "{d:?}");

    let d =
        diags("type sequence\nparticipant a\nparticipant b\na -> b\ngroup g {\n participant c\n}");
    assert!(
        d.iter().any(|x| x.message.contains("holds no messages")),
        "{d:?}"
    );
}

#[test]
fn messages_can_be_numbered() {
    let plain = build("type sequence\nparticipant a\nparticipant b\na -> b \"go\"");
    let numbered =
        build("type sequence { numbered: on }\nparticipant a\nparticipant b\na -> b \"go\"");

    assert!(plain.annotations.iter().any(|x| x.text == "go"));
    assert!(numbered.annotations.iter().any(|x| x.text == "1. go"));
}

#[test]
fn a_sequence_without_messages_says_so() {
    let d = diags("type sequence\nparticipant a \"A\"");
    assert!(d.iter().any(|x| x.message.contains("no messages")), "{d:?}");
}

// ---------------------------------------------------------------- state

#[test]
fn a_state_machine_keeps_its_declared_placement() {
    // Unlike the chart archetypes, `state` places nothing itself — the
    // anchors still decide, and the archetype only supplies the vocabulary.
    let l = build(
        r#"
        type state
        state a "A" at (0, 0) { size: 60 40 }
        state b "B" right-of a gap 30 { size: 60 40 }
        a -> b
    "#,
    );
    assert_eq!(l.nodes[0].rect.x, 0.0);
    assert_eq!(l.nodes[1].rect.x, 90.0);
}

#[test]
fn pseudo_states_are_marks_rather_than_boxes() {
    use crate::style::Shape;
    let l = build(
        r#"
        type state
        state begin { kind: initial }
        state s "S" right-of begin
        state stop { kind: final } right-of s
        state pick { kind: choice } below s
        begin -> s
        s -> stop
        s -> pick
    "#,
    );

    assert_eq!(l.nodes[0].style.shape, Some(Shape::Ellipse));
    assert_eq!(l.nodes[2].style.shape, Some(Shape::Ellipse));
    assert_eq!(l.nodes[3].style.shape, Some(Shape::Diamond));
    // An ordinary state is a rounded box; a mark carries no label.
    assert_eq!(l.nodes[1].style.shape, Some(Shape::Stadium));
    assert!(l.nodes[0].lines.is_empty());
    assert!(l.nodes[2].lines.is_empty());
    assert_eq!(l.nodes[1].lines, vec!["S"]);
    // Marks are small and square.
    assert_eq!(l.nodes[0].rect.w, l.nodes[0].rect.h);
    assert!(l.nodes[0].rect.w < 30.0);
}

#[test]
fn a_guard_reads_in_brackets_after_the_label() {
    let l = build(
        r#"
        type state
        state a
        state b right-of a
        a -> b "pay" { guard: "funds cleared" }
    "#,
    );
    assert_eq!(l.edges[0].label.as_deref(), Some("pay [funds cleared]"));

    let l = build("type state\nstate a\nstate b right-of a\na -> b { guard: \"ready\" }");
    assert_eq!(l.edges[0].label.as_deref(), Some("[ready]"));
}

#[test]
fn a_machine_reports_what_cannot_happen() {
    let e = err("type state\nstate begin { kind: initial }\nstate s\ns -> begin");
    assert!(e.contains("nothing transitions into it"), "{e}");

    let e = err("type state\nstate stop { kind: final }\nstate s\nstop -> s");
    assert!(e.contains("nothing transitions out of it"), "{e}");

    let d = diags("type state\nstate a\nstate b right-of a\na -> b");
    assert!(
        d.iter().any(|x| x.message.contains("no `kind: initial`")),
        "{d:?}"
    );

    let d = diags("type state\nstate begin { kind: initial }\nstate a\nstate orphan\nbegin -> a");
    assert!(
        d.iter()
            .any(|x| x.message.contains("nothing transitions into `orphan`"))
    );
}

#[test]
fn a_block_and_a_placement_may_come_in_either_order() {
    // `state stop { kind: final } right-of done` reads as naturally as the
    // other way round, and `{` cannot be mistaken for a placement keyword.
    for source in [
        "node a at (0, 0) { fill: #fff }",
        "node a { fill: #fff } at (0, 0)",
    ] {
        let l = build(source);
        assert_eq!(l.nodes[0].rect.x, 0.0, "{source}");
        assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#fff"), "{source}");
    }
}

#[test]
fn a_label_prefers_clipping_a_node_to_leaving_its_edge() {
    // Charging a five-pixel clip the same as sitting on top of a node exiled
    // labels three rows away, where they read as belonging to something else.
    let l = build(
        r#"
        node a "A" at (0, 0) { size: 100 40 }
        node b "B" at (130, 0) { size: 100 40 }
        a -> b "a long-ish label"
    "#,
    );

    let svg = render_svg::render(&l);
    let plate = svg.split(r#" rx="3""#).next().unwrap();
    let y: f64 = plate
        .rsplit(r#"y=""#)
        .next()
        .and_then(|s| s.split('"').next())
        .and_then(|s| s.parse().ok())
        .expect("the label plate declares a y");

    // The edge runs at y = 20 (plus the canvas translation); the label should
    // stay on it rather than fleeing the narrow gap.
    let translation = 32.0;
    assert!(
        (y + 10.0 - (20.0 + translation)).abs() < 14.0,
        "label at y={y}"
    );
}

/// Centres of the edge-label plates, in source order, in layout coordinates —
/// the canvas translation is applied by the wrapping `<g>`, not by these
/// numbers. A plate is the rect carrying `rx="3"`; its attributes are the ones
/// nearest it.
fn plate_centres(svg: &str) -> Vec<(f64, f64)> {
    svg.match_indices(r#" rx="3""#)
        .map(|(at, _)| {
            let head = &svg[..at];
            let attr = |name: &str| -> f64 {
                head.rsplit(&format!(r#"{name}=""#))
                    .next()
                    .and_then(|s| s.split('"').next())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or_else(|| panic!("the label plate declares {name}"))
            };
            (
                attr("x") + attr("width") / 2.0,
                attr("y") + attr("height") / 2.0,
            )
        })
        .collect()
}

#[test]
fn a_declared_offset_pins_the_edge_label_where_it_says() {
    // The same crossing that makes `an_edge_label_steps_off_a_line_it_would_sit_on`
    // move: `a -> b` runs at y=30, `c -> d` crosses it exactly at the midpoint.
    let crossing = |extra: &str| {
        format!(
            r#"
            node a at (0, 0) {{ size: 60 60 }}
            node b at (200, 0) {{ size: 60 60 }}
            node c at (120, -120) {{ size: 20 20 }}
            node d at (120, 120) {{ size: 20 20 }}
            a -> b "on the crossing" {{ {extra} }}
            c -> d
        "#
        )
    };

    // A zero offset is still an offset: it says "leave it alone", and the
    // avoidance that would otherwise have moved it stops.
    let pinned = render_svg::render(&build(&crossing("label-offset: 0 0")));
    let (x, y) = plate_centres(&pinned)[0];
    assert!(
        (x - 130.0).abs() < 0.5 && (y - 30.0).abs() < 0.5,
        "a pinned label must sit on the midpoint, not at ({x}, {y})"
    );

    // And a real offset is measured from that same midpoint, so it is read
    // straight off the source rather than off wherever the solver ran to.
    let moved = render_svg::render(&build(&crossing("label-offset: -24 40")));
    let (x, y) = plate_centres(&moved)[0];
    assert!(
        (x - 106.0).abs() < 0.5 && (y - 70.0).abs() < 0.5,
        "expected the label at (106, 70), found ({x}, {y})"
    );
}

#[test]
fn a_moved_edge_label_still_pushes_the_next_one_out_of_the_way() {
    // Pinning one label must not hide it from the solver. `first` is moved
    // down onto the exact spot `second` would take, so the automatic
    // placement has to see it there and go elsewhere.
    let l = build(
        r#"
        node a at (0, 0) { size: 40 40 }
        node b at (240, 0) { size: 40 40 }
        node c at (0, 100) { size: 40 40 }
        node d at (240, 100) { size: 40 40 }
        a -> b "first" { label-offset: 0 100 }
        c -> d "second"
    "#,
    );

    let centres = plate_centres(&render_svg::render(&l));
    assert_eq!(centres.len(), 2, "both labels are drawn");
    assert!(
        (centres[0].1 - 120.0).abs() < 0.5,
        "the pinned label moved: {:?}",
        centres[0]
    );
    assert!(
        (centres[1].1 - centres[0].1).abs() > 12.0,
        "the second label stacked on the pinned one: {centres:?}"
    );
}

#[test]
fn a_moved_node_label_takes_the_canvas_with_it() {
    let source = |extra: &str| {
        format!(
            r#"
            graph {{ padding: 0 }}
            node store "PostgreSQL" at (0, 0) {{ size: 60 40 {extra} }}
        "#
        )
    };

    let put = render_svg::render(&build(&source("")));
    let moved = render_svg::render(&build(&source("label-offset: 0 80")));

    // The label is drawn 80 below the centre of a box that is 40 tall…
    assert!(
        moved.contains(r#"<text x="30" y="100""#),
        "the node label did not move: {moved}"
    );
    // …so the drawing is taller by everything the label now sticks out by.
    let height = |svg: &str| {
        svg.split(r#"viewBox="0 0 "#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.split_whitespace().nth(1).map(str::to_string))
            .and_then(|s| s.parse::<f64>().ok())
            .expect("a viewBox")
    };
    assert!(
        height(&moved) > height(&put) + 60.0,
        "the label was left outside the canvas: {} vs {}",
        height(&moved),
        height(&put)
    );
}

#[test]
fn a_label_offset_cascades_like_any_other_property() {
    let l = build(
        r#"
        style nudged { label-offset: 12 -4 }
        node a "A" .nudged at (0, 0)
        node b "B" at (200, 0)
        a -> b "edge" .nudged
    "#,
    );

    assert_eq!(l.nodes[0].style.label_offset, Some((12.0, -4.0)));
    assert_eq!(l.edges[0].style.label_offset, Some((12.0, -4.0)));
    // A single number is a horizontal nudge; the vertical one stays zero.
    let l = build(r#"node a "A" { label-offset: 9 }"#);
    assert_eq!(l.nodes[0].style.label_offset, Some((9.0, 0.0)));
}

// ---------------------------------------------------------------- members

#[test]
fn a_member_line_opens_with_a_marker() {
    let l = build(
        r#"
        type class
        class user "User" {
          fill: #123456
          - id: Uuid
          + save() : void
        }
    "#,
    );

    // The marker is kept: `-` and `+` are the visibility UML readers expect.
    let text: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(text.contains(&"- id: Uuid"), "{text:?}");
    assert!(text.contains(&"+ save() : void"), "{text:?}");
    assert!(text.contains(&"User"), "the name heads the box");
    // Properties in the same block still work.
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#123456"));
}

#[test]
fn properties_may_still_share_a_line() {
    // The marker exists precisely so members need no separator rule, which
    // would have broken this — a documented shape with its own corpus case.
    let l = build("node a { fill: #111111 stroke: #222222 2 dashed }");
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#111111"));
    assert_eq!(l.nodes[0].style.stroke.as_deref(), Some("#222222"));
    assert_eq!(l.nodes[0].style.stroke_width, Some(2.0));
}

#[test]
fn a_negative_number_is_not_a_member() {
    // The space after the marker is what keeps `-40` a coordinate.
    let l = build("node a at (-40, -20) { size: 10 10 }");
    assert_eq!((l.nodes[0].rect.x, l.nodes[0].rect.y), (-40.0, -20.0));
}

#[test]
fn a_class_is_sized_by_its_widest_member() {
    let narrow = build("type class\nclass a \"A\" {\n - x: u8\n}");
    let wide = build("type class\nclass a \"A\" {\n - a_very_long_field_name_indeed: SomeType\n}");
    assert!(wide.nodes[0].rect.w > narrow.nodes[0].rect.w);

    let one = build("type class\nclass a \"A\" {\n - x: u8\n}");
    let three = build("type class\nclass a \"A\" {\n - x: u8\n - y: u8\n - z: u8\n}");
    assert!(three.nodes[0].rect.h > one.nodes[0].rect.h);
}

#[test]
fn a_compartment_rule_is_drawn_over_the_box() {
    // A `Guide` would have been drawn beneath the boxes and vanished under
    // the class's own fill.
    let l = build("type class\nclass a \"A\" {\n - x: u8\n}");
    assert!(l.guides.is_empty());
    assert_eq!(l.nodes.len(), 2, "the box, then the rule");
    let rule = &l.nodes[1];
    assert!(rule.rect.h < 4.0 && rule.rect.w == l.nodes[0].rect.w);

    let svg = render_svg::render(&l);
    let box_at = svg.find(r#"rx="8""#).or_else(|| svg.find("<rect")).unwrap();
    let rule_at = svg.rfind("<rect").unwrap();
    assert!(box_at < rule_at, "the rule must come after the box");
}

#[test]
fn members_outside_a_class_are_reported() {
    let d = diags("node a \"A\" {\n - x: u8\n}");
    assert!(
        d.iter()
            .any(|x| x.message.contains("only drawn by `type class`")),
        "{d:?}"
    );
    let d = diags("type class\nclass a \"A\"");
    assert!(
        d.iter().any(|x| x.message.contains("has no members")),
        "{d:?}"
    );
}

#[test]
fn a_diagnostic_spells_its_plurals() {
    // `classs` in a message about class diagrams undermines the rest of it.
    let e = err("type pie\nclass a \"A\" 1");
    assert!(e.contains("declares slices, not classes"), "{e}");
}

// ---------------------------------------------------- dates and durations

#[test]
fn a_date_is_one_value_not_three() {
    use crate::ast::{civil_from_days, days_from_civil};

    // `2026-07-31` would otherwise lex as 2026, -7 and -31.
    let d = parse_diagram("node a { start: 2026-07-31 3d }").expect("parses");
    let values = &d.nodes[0].props[0].values;
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].as_date(), Some(days_from_civil(2026, 7, 31)));
    assert_eq!(values[1].as_duration(), Some(3.0));

    // The civil-calendar round trip is the whole basis of the arithmetic.
    for (y, m, day) in [(1970, 1, 1), (2000, 2, 29), (2026, 12, 31), (1899, 3, 1)] {
        assert_eq!(
            civil_from_days(days_from_civil(y, m, day)),
            (y, m as u32, day as u32)
        );
    }
    assert_eq!(days_from_civil(1970, 1, 1), 0);
    assert_eq!(
        days_from_civil(2026, 8, 20) - days_from_civil(2026, 8, 1),
        19
    );
}

#[test]
fn durations_carry_their_unit() {
    let d = parse_diagram("node a { length: 6h }\nnode b { length: 2w }\nnode c { length: 3mo }")
        .expect("parses");
    assert_eq!(d.nodes[0].props[0].values[0].as_duration(), Some(0.25));
    assert_eq!(d.nodes[1].props[0].values[0].as_duration(), Some(14.0));
    assert_eq!(d.nodes[2].props[0].values[0].as_duration(), Some(90.0));
}

#[test]
fn a_negative_number_is_not_a_date() {
    let l = build("node a at (-40, -20) { size: 10 10 }");
    assert_eq!((l.nodes[0].rect.x, l.nodes[0].rect.y), (-40.0, -20.0));
}

// ----------------------------------------------------------------- gantt

#[test]
fn a_task_spans_from_its_start_to_its_end() {
    // `size` has a floor of 160; asking for less silently gets the floor.
    let l = build(
        r#"
        type gantt { size: 200 10 }
        task a "A" 2026-08-01 5d
        task b "B" 2026-08-06 5d
    "#,
    );

    assert!(matches!(l.chart, crate::layout::Chart::Gantt));
    // Ten days across two hundred units: each day is twenty wide.
    assert!(
        (l.nodes[0].rect.w - 100.0).abs() < 0.01,
        "{:?}",
        l.nodes[0].rect
    );
    assert!((l.nodes[1].rect.x - l.nodes[0].rect.right()).abs() < 0.01);
    assert!(l.nodes[1].rect.y > l.nodes[0].rect.y, "one row each");
}

#[test]
fn an_end_date_and_a_duration_say_the_same_thing() {
    let by_length = build("type gantt { size: 100 10 }\ntask a \"A\" 2026-08-01 10d");
    let by_end = build("type gantt { size: 100 10 }\ntask a \"A\" 2026-08-01 2026-08-11");
    assert_eq!(by_length.nodes[0].rect.w, by_end.nodes[0].rect.w);
}

#[test]
fn sections_group_their_tasks_and_keep_their_own_gutter() {
    let l = build(
        r#"
        type gantt { size: 200 10 }
        group one "First" { task a "A" 2026-08-01 2d }
        group two "Second" { task b "B" 2026-08-03 2d }
    "#,
    );

    assert_eq!(l.groups.len(), 2);
    // The frames stack and never overlap.
    assert!(l.groups[0].rect.bottom() <= l.groups[1].rect.y + 0.01);
    // A section label lives left of every frame, so it cannot land on a task
    // name — which is exactly what the first version did.
    let labels: Vec<&crate::layout::Annotation> = l
        .annotations
        .iter()
        .filter(|a| a.text == "First" || a.text == "Second")
        .collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.iter().all(|a| a.x < l.groups[0].rect.x + 0.01));
}

#[test]
fn a_today_line_is_drawn_where_it_falls() {
    let l = build(
        r#"
        type gantt { size: 200 10 today: 2026-08-06 }
        task a "A" 2026-08-01 10d
    "#,
    );
    // Halfway through a ten-day span.
    let today = l
        .guides
        .iter()
        .find(|g| g.dash.is_some())
        .expect("a today line");
    let start = l.nodes[0].rect.x;
    assert!(
        (today.from.0 - (start + 100.0)).abs() < 0.01,
        "{:?}",
        today.from
    );

    assert!(err("type gantt { today: soon }\ntask a 2026-08-01 1d").contains("takes a date"));
}

#[test]
fn a_task_needs_both_ends() {
    assert!(err("type gantt\ntask a \"A\"").contains("has no start"));
    assert!(err("type gantt\ntask a \"A\" 2026-08-01").contains("no end"));
    assert!(
        err("type gantt\ntask a \"A\" 2026-08-10 2026-08-01").contains("ends before it starts")
    );
}

// -------------------------------------------------------------- templates

use crate::template::{Bindings, Template, Value as Bound};

#[test]
fn a_hole_parses_so_a_template_stays_editable() {
    // Without this, every line of a template lights up in the editor.
    let source = "type pie\nslice a \"North\" {{north}}";
    assert!(parse_diagram(source).is_ok());
    let diagram = parse_diagram(source).unwrap();
    assert!(crate::check::first_error(&diagram).is_none());
    assert!(diagram.nodes[0].positional[0].is_hole());
}

#[test]
fn an_unbound_hole_does_not_read_as_a_missing_value() {
    // `slice a {{v}}` declares the datum; it is simply not bound yet.
    let d = diags("type pie\nslice a \"A\" {{v}}\nslice b \"B\" {{w}}");
    assert!(
        !d.iter().any(|x| x.message.contains("has no `value`")),
        "{d:?}"
    );

    // The same node with nothing at all is still an error.
    assert!(err("type pie\nslice a \"A\"").contains("has no `value`"));
}

#[test]
fn holes_are_listed_in_order_without_repeats() {
    let t = Template::parse("node {{a}} \"{{b}}\" { fill: {{a}} }").expect("parses");
    assert_eq!(t.holes(), ["a", "b"]);
}

#[test]
fn a_bound_value_is_escaped_for_where_it_lands() {
    let t = Template::parse("node a \"{{label}}\" { fill: {{colour}} }").expect("parses");
    let filled = t
        .fill(
            &Bindings::new()
                .set("label", "Americas (\"north\")")
                .set("colour", Bound::Raw("#38bdf8".into())),
        )
        .expect("fills");

    // Inside a string it is escaped, not quoted again.
    assert!(filled.contains(r#""Americas (\"north\")""#), "{filled}");
    assert!(filled.contains("fill: #38bdf8"), "{filled}");
    // And the result still parses, which is the whole point of the escaping.
    assert!(parse_diagram(&filled).is_ok(), "{filled}");
}

#[test]
fn text_in_a_value_position_is_quoted() {
    // A bare word there would have to be an identifier, and a two-word value
    // would silently become two values.
    let t = Template::parse("type pie\nslice a {{name}} 1").expect("parses");
    let filled = t
        .fill(&Bindings::new().set("name", "North America"))
        .unwrap();
    assert!(filled.contains(r#"slice a "North America" 1"#), "{filled}");
    assert!(parse_diagram(&filled).is_ok());
}

#[test]
fn a_number_is_written_bare_and_a_list_is_spread() {
    let t =
        Template::parse("type bar { categories: {{names}} }\nseries s {{points}}").expect("parses");
    let filled = t
        .fill(
            &Bindings::new()
                .set("names", vec!["Q1", "Q2"])
                .set("points", vec![10i64, 20, 30]),
        )
        .unwrap();

    assert!(filled.contains(r#"categories: "Q1" "Q2""#), "{filled}");
    assert!(filled.contains("series s 10 20 30"), "{filled}");
    let l = build(&filled);
    assert_eq!(l.nodes.len(), 3, "three bars");
}

#[test]
fn an_unbound_hole_is_refused_by_name() {
    let t = Template::parse("node a { fill: {{one}} }\nnode b { fill: {{two}} }").unwrap();
    let e = t
        .render(&Bindings::new().set("one", Bound::Raw("#fff".into())))
        .expect_err("should refuse");
    let text = format!("{e:#}");
    assert!(text.contains("nothing bound to `two`"), "{text}");
    assert!(!text.contains("`one`"), "{text}");
}

#[test]
fn a_template_that_cannot_parse_is_refused_up_front() {
    // Catching it at `parse` beats catching it at the first render, in
    // production, with a customer's data in hand.
    let e = Template::parse("node a { fill: }} }").expect_err("should refuse");
    assert!(format!("{e:#}").contains("does not parse"), "{e:#}");
}

#[test]
fn a_template_renders_to_svg() {
    let svg = crate::template::render(
        "type pie\nslice a \"North\" {{north}}\nslice b \"South\" {{south}}\nslice c \"East\" {{east}}",
        &Bindings::new().set("north", 50i64).set("south", 30i64).set("east", 20i64),
    )
    .expect("renders");
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("50%"), "the first slice is half the circle");
}

#[test]
fn a_hole_inside_a_comment_is_still_a_hole() {
    // Substitution is textual, so it reaches comments too — which is what
    // makes `// generated for {{year}}` work as a reader expects.
    let t = Template::parse("// for {{year}}\nnode a").expect("parses");
    assert_eq!(t.holes(), ["year"]);
    let filled = t.fill(&Bindings::new().set("year", 2026i64)).unwrap();
    assert!(filled.starts_with("// for 2026"), "{filled}");
}

// ---------------------------------------------------------------------------
// The embedding promise: hostile text produces an SVG or an error, never a
// panic and never an abort. A document generator holds this library inside its
// own process; a bad input file must not be able to take that process down.
// ---------------------------------------------------------------------------

/// Sources chosen to reach the places that can panic: division by zero, empty
/// collections, out-of-range indices, unbounded recursion, degenerate geometry.
fn hostile_corpus() -> Vec<(&'static str, String)> {
    let mut cases: Vec<(&'static str, String)> = vec![
        ("empty", String::new()),
        ("whitespace only", "\n\n   \t\n".into()),
        ("one brace", "{".into()),
        ("closing braces", "}}}}}}".into()),
        ("bare type", "type".into()),
        ("unknown type", "type nonsense\nnode a".into()),
        ("type with no items", "type pie".into()),
        (
            "pie of zeros",
            "type pie\nslice a \"A\" 0\nslice b \"B\" 0".into(),
        ),
        ("pie of one", "type pie\nslice a \"A\" 1".into()),
        (
            "negative slice",
            "type pie\nslice a \"A\" -5\nslice b \"B\" 3".into(),
        ),
        (
            "zero radius",
            "type pie { radius: 0 }\nslice a \"A\" 1".into(),
        ),
        ("treemap of zeros", "type treemap\nbox a \"A\" 0".into()),
        (
            "treemap gap past its box",
            "type treemap { size: 20 20 gap: 400 }\nbox a \"A\" 1\nbox b \"B\" 1".into(),
        ),
        (
            "flat axis",
            "type xy { y-axis: 5 5 }\nseries s 5 5 5".into(),
        ),
        ("empty series", "type xy\nseries s".into()),
        ("one point", "type xy { x-axis: 0 0 }\nseries s 1".into()),
        ("bar with no categories", "type bar\nseries s 1 2 3".into()),
        (
            "more points than categories",
            "type bar { categories: \"a\" }\nseries s 1 2 3 4 5".into(),
        ),
        ("wave with no wave", "type wave\nsignal s \"S\" \"\"".into()),
        (
            "wave of repeats only",
            "type wave\nsignal s \"S\" \"....\"".into(),
        ),
        (
            "wave period zero",
            "type wave { period: 0 }\nsignal s \"S\" \"01\"".into(),
        ),
        ("sankey with no flow", "type sankey\nnode a\nnode b".into()),
        (
            "sankey zero flow",
            "type sankey\nnode a\nnode b\na -> b 0".into(),
        ),
        (
            "sankey cycle",
            "type sankey\nnode a\nnode b\na -> b 1\nb -> a 1".into(),
        ),
        (
            "gantt zero duration",
            "type gantt\ntask a \"A\" 2026-01-01 0d".into(),
        ),
        (
            "gantt backwards",
            "type gantt\ntask a \"A\" 2026-01-01 2025-01-01".into(),
        ),
        ("leap day", "type gantt\ntask a \"A\" 2024-02-29 1d".into()),
        (
            "impossible date",
            "type gantt\ntask a \"A\" 2026-02-31 1d".into(),
        ),
        ("year zero", "type gantt\ntask a \"A\" 0000-01-01 1d".into()),
        ("packet zero bits", "type packet\nfield a \"A\" 0 0".into()),
        ("er with no relations", "type er\nentity a \"A\"".into()),
        ("timeline with no events", "type timeline".into()),
        (
            "timeline of one day",
            "type timeline\nevent a \"A\" 2026-03-01".into(),
        ),
        (
            "timeline of the same day",
            "type timeline\nevent a \"A\" 2026-03-01\nevent b \"B\" 2026-03-01".into(),
        ),
        (
            "timeline across two millennia",
            "type timeline\nevent a \"A\" 0001-01-01\nevent b \"B\" 2026-03-01".into(),
        ),
        (
            "timeline with a zero width",
            "type timeline { size: 0 0 }\nevent a \"A\" 2026-03-01".into(),
        ),
        (
            "er cardinality on a self relation",
            "type er\nentity a \"A\"\na -> a \"loops\" many many".into(),
        ),
        (
            "er cardinality on a free graph",
            "node a\nnode b\na -> b { from-card: many }".into(),
        ),
        ("packet backwards", "type packet\nfield a \"A\" 31 0".into()),
        (
            "quadrant off scale",
            "type quadrant\npoint a \"A\" 1e9 -1e9".into(),
        ),
        (
            "anchor cycle",
            "node a right-of b\nnode b right-of a".into(),
        ),
        ("self anchor", "node a right-of a".into()),
        ("unknown anchor", "node a right-of nowhere".into()),
        ("duplicate ids", "node a\nnode a".into()),
        ("edge to nothing", "a -> b".into()),
        ("negative geometry", "node a { size: -10 -10 }".into()),
        ("infinite size", "node a { size: 1e308 1e308 }".into()),
        ("subnormal size", "node a { size: 1e-308 1e-308 }".into()),
        ("bad colour", "node a { fill: #12345 }".into()),
        ("long colour", "node a { fill: #1234567890abcdef }".into()),
        ("unknown property", "node a { flibbertigibbet: 3 }".into()),
        ("unknown class", "node a .nonexistent".into()),
        ("style of nothing", "style".into()),
        ("empty group", "group g { }".into()),
        (
            "group holding itself",
            "group g { group g { node a } }".into(),
        ),
        ("unterminated string", "node a \"never closed".into()),
        ("lone backslash", "node a \"\\".into()),
        ("nul byte", "node a \"\0\"".into()),
        ("bom", "\u{feff}node a".into()),
        ("rtl label", "node a \"\u{202e}reversed\"".into()),
        (
            "combining marks",
            "node a \"e\u{301}\u{301}\u{301}\u{301}\"".into(),
        ),
        ("unpaired brace in label", "node a \"a { b\"".into()),
        ("empty hole", "node a \"{{}}\"".into()),
        ("hole as a keyword", "{{kind}} a".into()),
        ("unterminated hole", "node a \"{{never".into()),
    ];

    // Sizes chosen to sit either side of every bound that exists.
    cases.push((
        "very long label",
        format!("node a \"{}\"", "x".repeat(100_000)),
    ));
    cases.push((
        "very long identifier",
        format!("node {}", "a".repeat(100_000)),
    ));
    cases.push((
        "many nodes",
        (0..5_000).map(|i| format!("node n{i}\n")).collect(),
    ));
    cases.push((
        "many slices",
        std::iter::once("type pie\n".to_string())
            .chain((0..2_000).map(|i| format!("slice s{i} \"S\" 1\n")))
            .collect(),
    ));
    cases.push((
        "chain of anchors",
        std::iter::once("node n0\n".to_string())
            .chain((1..2_000).map(|i| format!("node n{i} right-of n{}\n", i - 1)))
            .collect(),
    ));
    for depth in [MAX_NESTING - 1, MAX_NESTING, MAX_NESTING + 1, 5_000] {
        cases.push((
            "nested groups",
            (0..depth)
                .map(|i| format!("group g{i} {{\n"))
                .chain(std::iter::once("node leaf\n".to_string()))
                .chain((0..depth).map(|_| "}\n".to_string()))
                .collect(),
        ));
    }

    cases
}

#[test]
fn hostile_input_never_panics() {
    for (name, source) in hostile_corpus() {
        // Not `catch_unwind`: a panic here should fail loudly with its own
        // message and location, which is what makes the failure diagnosable.
        // The assertion is simply that the call returns at all.
        match crate::render_source(&source) {
            Ok(svg) => assert!(
                svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"),
                "`{name}` produced something that is not an SVG document"
            ),
            Err(e) => assert!(
                !format!("{e:#}").is_empty(),
                "`{name}` failed without saying why"
            ),
        }
    }
}

#[test]
fn hostile_input_never_panics_in_the_language_server() {
    // The editor sees the same text and must survive the same way — it reports
    // every problem rather than the first, so it walks further into the checks.
    for (name, source) in hostile_corpus() {
        let _ = crate::parser::parse(&source).map(|d| {
            let diags = crate::check::check(&d);
            assert!(
                diags.iter().all(|d| !d.message.is_empty()),
                "`{name}` produced a diagnostic with no message"
            );
        });
    }
}

#[test]
fn a_bound_value_cannot_escape_its_template() {
    // The one place hostile input arrives as *data* rather than as source: a
    // program binding a customer's string into a document.
    let t = Template::parse("node a \"{{label}}\" { fill: #eee }").expect("parses");
    for hostile in [
        "plain",
        "with \"quotes\"",
        "with a \\ backslash",
        "\" } node injected \"",
        "\"\nnode injected \"",
        "{{label}}",
        "\u{0}\u{1}",
    ] {
        let filled = t
            .fill(&Bindings::new().set("label", hostile))
            .expect("fills");
        let d = parse_diagram(&filled)
            .unwrap_or_else(|e| panic!("`{hostile}` broke the template: {e}\n{filled}"));
        assert_eq!(d.nodes.len(), 1, "`{hostile}` injected a statement");
        assert_eq!(
            d.nodes[0].label.as_deref(),
            Some(hostile),
            "`{hostile}` did not survive the round trip"
        );
    }
}

#[test]
fn nesting_is_refused_before_the_stack_gives_out() {
    // Guarding this at all is the point: `pest` descends recursively, so deep
    // enough nesting aborts the process outright, which no caller can catch.
    let deep: String = (0..5_000)
        .map(|i| format!("group g{i} {{\n"))
        .chain(std::iter::once("node leaf\n".to_string()))
        .chain((0..5_000).map(|_| "}\n".to_string()))
        .collect();
    let e = parse_diagram(&deep).expect_err("should refuse");
    assert!(format!("{e:#}").contains("levels deep"), "{e:#}");

    // And the limit is not so tight that it refuses a real diagram.
    let fine: String = (0..MAX_NESTING - 1)
        .map(|i| format!("group g{i} {{\n"))
        .chain(std::iter::once("node leaf\n".to_string()))
        .chain((0..MAX_NESTING - 1).map(|_| "}\n".to_string()))
        .collect();
    parse_diagram(&fine).expect("just under the limit still parses");
}

#[test]
fn braces_that_are_not_blocks_do_not_count_toward_the_limit() {
    // The guard runs before the grammar, so it has to know the three places a
    // brace is not structure. Miscounting here would refuse a valid file.
    let source = format!(
        "// {}\nnode a \"{}\" {{ fill: #eee }}\nnode b {{ fill: {{{{c}}}} }}",
        "{".repeat(200),
        "{".repeat(200),
    );
    let filled = crate::template::Template::parse(&source)
        .expect("braces in comments, strings and holes are not depth")
        .fill(&Bindings::new().set("c", Bound::Raw("#111".into())))
        .expect("fills");
    let d = parse_diagram(&filled).expect("parses");
    assert_eq!(d.nodes.len(), 2);
}

// ---------------------------------------------------------------------------
// `er` — entities, columns, and cardinality at each end of a relation.
// ---------------------------------------------------------------------------

// A member runs to the end of its line, so it cannot share one with the
// closing brace — the same shape every real file has.
const ER: &str = r#"type er
entity customer "Customer" {
  - id: uuid
}
entity order "Order" right-of customer {
  - id: uuid
}
"#;

#[test]
fn cardinality_written_after_the_label_reaches_both_ends() {
    let svg = render_svg::render(&build(&format!(
        "{ER}customer -> order \"places\" one many"
    )));
    // Start is the tail of the arrow, end its head: `one` belongs to the
    // customer, `many` to the order.
    assert!(svg.contains("marker-start=\"url(#h-one-"), "{svg}");
    assert!(svg.contains("marker-end=\"url(#h-many-"), "{svg}");
    // The crow's foot spreads against the entity and meets back down the line.
    assert!(svg.contains(r#"L18,1 M"#), "the foot is drawn: {svg}");
}

#[test]
fn cardinality_can_be_written_as_a_property_instead() {
    let positional = render_svg::render(&build(&format!(
        "{ER}customer -> order \"places\" one many"
    )));
    let block = render_svg::render(&build(&format!(
        "{ER}customer -> order \"places\" {{ from-card: one to-card: many }}"
    )));
    assert_eq!(positional, block, "the two forms are the same statement");
}

#[test]
fn a_cardinality_replaces_the_arrowhead_at_that_end() {
    // Both would land on the same point and fight; the foot already says
    // which way the relation reads.
    let svg = render_svg::render(&build(&format!(
        "{ER}customer -> order \"places\" one many"
    )));
    assert!(!svg.contains("h-solid-"), "no arrowhead survives: {svg}");

    // The other end keeps its arrow when only one cardinality is given.
    let half = render_svg::render(&build(&format!(
        "{ER}customer -> order \"places\" {{ from-card: one }}"
    )));
    assert!(half.contains("marker-start=\"url(#h-one-"), "{half}");
    assert!(half.contains("marker-end=\"url(#h-solid-"), "{half}");
}

#[test]
fn an_unknown_cardinality_is_refused() {
    let e = err(&format!("{ER}customer -> order \"places\" one lots"));
    assert!(e.contains("lots"), "{e}");
}

#[test]
fn cardinality_at_one_end_only_is_flagged() {
    let d = diags(&format!("{ER}customer -> order \"places\" one"));
    assert!(
        d.iter().any(|d| d.message.contains("one end only")),
        "{d:#?}"
    );
    // Both ends, or neither, is silent.
    for both in ["one many", ""] {
        let d = diags(&format!("{ER}customer -> order \"places\" {both}"));
        assert!(
            !d.iter().any(|d| d.message.contains("one end only")),
            "`{both}` should not be flagged: {d:#?}"
        );
    }
}

#[test]
fn a_column_drops_the_member_marker_but_a_class_member_keeps_it() {
    // `- ` is UML visibility on a class and means something; on a column it is
    // only the syntax that marked the line.
    let er = build("type er\nentity t \"T\" {\n  - id: uuid\n}");
    assert!(
        er.annotations.iter().any(|a| a.text == "id: uuid"),
        "{:?}",
        er.annotations.iter().map(|a| &a.text).collect::<Vec<_>>()
    );

    let class = build("type class\nclass t \"T\" {\n  - id: Uuid\n}");
    assert!(
        class.annotations.iter().any(|a| a.text == "- id: Uuid"),
        "{:?}",
        class
            .annotations
            .iter()
            .map(|a| &a.text)
            .collect::<Vec<_>>()
    );
}

#[test]
fn the_zero_ring_is_filled_with_the_surface() {
    // Left transparent, the relation line runs through it and it reads as a
    // crossed-out circle rather than as "zero".
    let svg = render_svg::render(&build(&format!(
        "{ER}graph {{ background: #0b1120 }}\ncustomer -> order \"places\" one zero-many"
    )));
    let ring = svg
        .split("<circle")
        .nth(1)
        .expect("a ring is drawn")
        .split("/>")
        .next()
        .unwrap();
    assert!(ring.contains("#0b1120"), "{ring}");
}

#[test]
fn an_entity_with_no_columns_is_flagged() {
    let d = diags("type er\nentity t \"T\"");
    assert!(
        d.iter()
            .any(|d| d.message.contains("entity with none is a plain box")),
        "{d:#?}"
    );
}

// ---------------------------------------------------------------------------
// `block` — architecture views. Typed elements, a technology line, and a
// vocabulary that is this genre's alone.
// ---------------------------------------------------------------------------

#[test]
fn an_element_kind_decides_its_look_and_a_declared_value_still_wins() {
    let l = build("type block\nblock db \"Store\" database");
    assert_eq!(l.nodes[0].style.shape, Some(Shape::Cylinder));
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#438dd5"));

    // The base leaves everything the kind decides unset, so this is the
    // difference between declared and defaulted — the mistake this codebase
    // has made four times.
    let l = build("type block\nblock db \"Store\" database { fill: #111827 shape: rect }");
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#111827"));
    assert_eq!(l.nodes[0].style.shape, Some(Shape::Rect));
}

#[test]
fn the_element_kind_ramp_follows_the_surface() {
    let light = build("type block\nblock a \"A\" component");
    let dark = build("type block\ngraph { background: #0b1120 }\nblock a \"A\" component");
    assert_ne!(light.nodes[0].style.fill, dark.nodes[0].style.fill);
}

#[test]
fn a_block_draws_its_name_technology_and_description_as_separate_lines() {
    let l = build(
        "type block\nblock api \"API\" container { tech: \"Rust\" description: \"Serves the public API.\" }",
    );
    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"API"), "{texts:?}");
    assert!(texts.contains(&"[Container: Rust]"), "{texts:?}");
    assert!(
        texts.iter().any(|t| t.contains("Serves the public")),
        "{texts:?}"
    );

    // The name leads at full size; the rest is smaller.
    let name = l.annotations.iter().find(|a| a.text == "API").unwrap();
    let tech = l
        .annotations
        .iter()
        .find(|a| a.text.starts_with("[Container"))
        .unwrap();
    assert!(tech.size < name.size, "{} vs {}", tech.size, name.size);
}

#[test]
fn a_relation_carries_its_protocol_on_its_own_line() {
    // Declared and never drawn would be worse than refused.
    let l = build(
        "type block\nblock a \"A\"\nblock b \"B\" right-of a\na -> b \"calls\" { tech: \"gRPC\" }",
    );
    assert_eq!(l.edges[0].label.as_deref(), Some("calls\n[gRPC]"));
}

#[test]
fn a_description_is_wrapped_and_the_box_grows_around_it() {
    let short = build("type block\nblock a \"A\" container");
    let long = build(
        "type block\nblock a \"A\" container { description: \"A rather long sentence that has to wrap over several lines to fit.\" }",
    );
    assert!(
        long.nodes[0].rect.h > short.nodes[0].rect.h * 1.5,
        "{} vs {}",
        long.nodes[0].rect.h,
        short.nodes[0].rect.h
    );
    // Wrapped, not one runaway line.
    assert!(
        long.annotations.iter().filter(|a| a.size < 12.0).count() >= 2,
        "{:?}",
        long.annotations.iter().map(|a| &a.text).collect::<Vec<_>>()
    );
}

#[test]
fn an_external_element_is_the_one_with_a_visible_border() {
    let external = build("type block\nblock m \"Mainframe\" external");
    let s = &external.nodes[0].style;
    assert_eq!(s.dash.as_deref(), Some("dashed"));
    assert_ne!(s.stroke, s.fill, "a dash in the fill colour is invisible");

    let internal = build("type block\nblock m \"Mainframe\" system");
    assert_eq!(internal.nodes[0].style.dash, None);
}

#[test]
fn a_kind_from_the_wrong_genre_is_refused_in_both_directions() {
    // The point of letting an archetype refine a shared property.
    let e = err("type block\nblock a \"A\" { kind: initial }");
    assert!(e.contains("initial"), "{e}");

    let e = err("type state\nstate a \"A\" { kind: person }");
    assert!(e.contains("person"), "{e}");
}

#[test]
fn a_block_only_property_is_unknown_in_another_genre() {
    let e = err("node a \"A\" { tech: \"Rust\" }");
    assert!(e.contains("unknown property `tech`"), "{e}");
}

#[test]
fn a_non_rectangular_block_is_given_the_room_its_shape_takes() {
    // A second sizing path that forgets the shape allowance puts the label on
    // the edge of the cylinder — which is what this did until it was rendered
    // and looked at.
    let rect = build("type block\nblock a \"Metrics\" component { tech: \"Prometheus\" }");
    let hex = build("type block\nblock a \"Metrics\" queue { tech: \"Prometheus\" }");
    let cyl = build("type block\nblock a \"Metrics\" database { tech: \"Prometheus\" }");

    assert!(
        hex.nodes[0].rect.w > rect.nodes[0].rect.w,
        "the hexagon's slanted sides eat width"
    );
    assert!(
        cyl.nodes[0].rect.h > rect.nodes[0].rect.h,
        "the cylinder's caps eat height"
    );
}

// ---------------------------------------------------------------------------
// `timeline` — events on a date line.
// ---------------------------------------------------------------------------

/// Every event label, paired with the y its name sits at.
fn timeline_labels(source: &str) -> Vec<(String, f64)> {
    build(source)
        .annotations
        .iter()
        .filter(|a| a.weight == "600" && a.size < 16.0)
        .map(|a| (a.text.clone(), a.y))
        .collect()
}

#[test]
fn events_are_drawn_in_date_order_whatever_order_they_are_written() {
    let l = build(
        "type timeline\nevent c \"C\" 2026-03-09\nevent a \"A\" 2026-03-01\nevent b \"B\" 2026-03-05",
    );
    let xs: Vec<f64> = l.nodes.iter().map(|n| n.rect.cx()).collect();
    assert!(xs[0] < xs[1] && xs[1] < xs[2], "{xs:?}");
}

#[test]
fn neighbouring_labels_land_on_opposite_sides_of_the_line() {
    // The stagger is the point: dates cluster, and one side at one height is
    // unreadable exactly where a timeline matters most.
    let labels = timeline_labels(
        "type timeline\nevent a \"A\" 2026-03-01\nevent b \"B\" 2026-03-02\nevent c \"C\" 2026-03-03",
    );
    assert!(labels[0].1 < labels[1].1, "A is above, B below");
    assert!(labels[2].1 < labels[1].1, "C is back above");
}

#[test]
fn a_crowd_on_one_side_is_pushed_to_a_further_row() {
    // Four events one day apart: two per side, and on each side the second
    // cannot fit beside the first.
    let tight = timeline_labels(
        "type timeline { size: 200 30 }\n\
         event a \"A long enough label\" 2026-03-01\n\
         event b \"B long enough label\" 2026-03-02\n\
         event c \"C long enough label\" 2026-03-03\n\
         event d \"D long enough label\" 2026-03-04",
    );
    // `a` and `c` share the upper side and must not share a row.
    assert_ne!(tight[0].1, tight[2].1, "{tight:?}");
}

#[test]
fn a_roomy_line_keeps_every_label_on_one_row_per_side() {
    let roomy = timeline_labels(
        "type timeline { size: 900 30 }\n\
         event a \"A\" 2026-01-01\n\
         event b \"B\" 2026-04-01\n\
         event c \"C\" 2026-07-01\n\
         event d \"D\" 2026-10-01",
    );
    assert_eq!(roomy[0].1, roomy[2].1, "both upper labels fit one row");
    assert_eq!(roomy[1].1, roomy[3].1, "both lower labels fit one row");
}

#[test]
fn an_event_without_a_date_is_refused() {
    let e = err("type timeline\nevent a \"A\"");
    assert!(e.contains("no date"), "{e}");
}

#[test]
fn the_axis_step_coarsens_until_the_labels_fit() {
    // Ten years at a quarterly step is forty labels in 800px — a smear. The
    // step is chosen against the page, not against the calendar alone.
    let decade =
        build("type timeline { size: 800 30 }\nevent a \"A\" 2015-01-01\nevent b \"B\" 2025-01-01");
    let axis: Vec<&str> = decade
        .annotations
        .iter()
        .filter(|a| a.size <= 10.0)
        .map(|a| a.text.as_str())
        .collect();
    assert!(axis.len() <= 14, "{} labels: {axis:?}", axis.len());
    assert!(
        axis.iter().all(|t| t.len() == 4),
        "years, not days: {axis:?}"
    );

    let week =
        build("type timeline { size: 800 30 }\nevent a \"A\" 2026-03-01\nevent b \"B\" 2026-03-08");
    assert!(
        week.annotations
            .iter()
            .filter(|a| a.size <= 10.0)
            .any(|a| a.text.contains("03-0")),
        "a week is labelled by day"
    );
}

#[test]
fn a_stem_stops_at_the_edge_of_its_label_rather_than_through_it() {
    let l = build("type timeline\nevent a \"A\" 2026-03-01");
    let name = l
        .annotations
        .iter()
        .find(|a| a.text == "A")
        .expect("the name is drawn");
    let date = l
        .annotations
        .iter()
        .find(|a| a.text == "2026-03-01")
        .expect("the date is drawn");
    // Gridlines are vertical too; the stem is the one that starts on the axis.
    let axis_y = l
        .guides
        .iter()
        .find(|g| (g.from.1 - g.to.1).abs() < 0.01)
        .expect("the date line is drawn")
        .from
        .1;
    let stem = l
        .guides
        .iter()
        .find(|g| (g.from.0 - g.to.0).abs() < 0.01 && (g.from.1 - axis_y).abs() < 0.01)
        .expect("a stem is drawn");
    let top = stem.from.1.min(stem.to.1);
    assert!(
        top > date.y,
        "the stem ends below the date line: stem {top}, date {}",
        date.y
    );
    assert!(name.y < date.y, "the name leads");
}

#[test]
fn every_archetype_has_a_feature_and_every_feature_an_archetype() {
    // Two tables that must agree, in two files, in two languages. Adding an
    // archetype and forgetting its feature would ship one that cannot be
    // switched off; the reverse would ship a feature that gates nothing.
    let manifest = include_str!("../Cargo.toml");
    let features: Vec<&str> = manifest
        .lines()
        .skip_while(|l| l.trim() != "[features]")
        .skip(1)
        .take_while(|l| !l.starts_with('['))
        .filter_map(|l| l.split_once(" = []"))
        .map(|(name, _)| name.trim())
        // `pdf` gates an output format, `json` and `csv` input ones — not
        // genres.
        // They are named here rather
        // than filtered by a pattern, so a second non-archetype feature has to
        // be justified in this list instead of slipping past a wildcard.
        .filter(|name| !matches!(*name, "pdf" | "json" | "csv"))
        .collect();

    let mut archetypes: Vec<&str> = crate::props::ARCHETYPES
        .iter()
        .map(|a| a.name)
        .filter(|name| *name != "free")
        .collect();
    let mut features = features;
    archetypes.sort_unstable();
    features.sort_unstable();
    assert_eq!(
        archetypes, features,
        "`free` is the language itself and has no feature; every other \
         archetype has exactly one"
    );

    // And the umbrella turns them all on, or `default` would ship a partial
    // build to everyone who never thought about features.
    let umbrella: Vec<&str> = manifest
        .lines()
        .skip_while(|l| !l.starts_with("all-archetypes = ["))
        .skip(1)
        .take_while(|l| !l.starts_with(']'))
        .map(|l| l.trim().trim_matches(|c| c == '"' || c == ','))
        .collect();
    let mut umbrella = umbrella;
    umbrella.sort_unstable();
    assert_eq!(umbrella, archetypes);
}

// ---------------------------------------------------------------------------
// Templating: repetition. A chart whose *number* of items is not known when
// the template is written.
// ---------------------------------------------------------------------------

fn region(id: &str, name: &str, revenue: i64) -> Bindings {
    Bindings::new()
        .set("id", Bound::Raw(id.into()))
        .set("name", name)
        .set("revenue", revenue)
}

#[test]
fn a_line_with_a_row_hole_is_written_once_per_row() {
    let t = Template::parse("type pie\nslice {{r.id}} \"{{r.name}}\" {{r.value}}").expect("parses");
    let filled = t
        .fill(&Bindings::new().rows(
            "r",
            vec![
                Bindings::new()
                    .set("id", Bound::Raw("a".into()))
                    .set("name", "A")
                    .set("value", 1i64),
                Bindings::new()
                    .set("id", Bound::Raw("b".into()))
                    .set("name", "B")
                    .set("value", 2i64),
            ],
        ))
        .expect("fills");
    assert_eq!(
        filled, "type pie\nslice a \"A\" 1\nslice b \"B\" 2",
        "{filled}"
    );
}

#[test]
fn a_variable_number_of_slices_renders() {
    // The case `Q-004` was opened for: substitution alone cannot do this.
    for count in [2usize, 5, 8] {
        let rows: Vec<Bindings> = (0..count)
            .map(|i| region(&format!("r{i}"), &format!("Region {i}"), 100 + i as i64))
            .collect();
        let svg = crate::template::render(
            "type pie { title: \"Revenue\" }\nslice {{region.id}} \"{{region.name}}\" {{region.revenue}}",
            &Bindings::new().rows("region", rows),
        )
        .unwrap_or_else(|e| panic!("{count} slices: {e:#}"));
        assert_eq!(
            svg.matches("<path").count(),
            count,
            "{count} slices should draw {count} arcs"
        );
    }
}

#[test]
fn no_rows_means_no_line() {
    // A chart of nothing draws nothing, which beats a line full of holes.
    let t =
        Template::parse("type pie\nslice a \"A\" 1\nslice {{r.id}} \"x\" {{r.v}}").expect("parses");
    let filled = t
        .fill(&Bindings::new().rows("r", Vec::new()))
        .expect("fills");
    assert_eq!(filled, "type pie\nslice a \"A\" 1\n", "{filled}");
}

#[test]
fn a_row_hole_and_an_ordinary_hole_share_a_line() {
    let t = Template::parse("type bar { categories: {{cats}} }\nseries {{s.id}} {{s.points}}")
        .expect("parses");
    let filled = t
        .fill(&Bindings::new().set("cats", vec!["q1", "q2"]).rows(
            "s",
            vec![
                        Bindings::new()
                            .set("id", Bound::Raw("north".into()))
                            .set("points", vec![1i64, 2]),
                    ],
        ))
        .expect("fills");
    assert_eq!(
        filled, "type bar { categories: \"q1\" \"q2\" }\nseries north 1 2",
        "{filled}"
    );
}

#[test]
fn one_line_cannot_repeat_over_two_lists() {
    // How many times would it be written? There is no answer a reader could
    // predict. And the ambiguity is in the *template*, not in the binding, so
    // it is caught when the template is read rather than at the first render.
    let e =
        Template::parse("type pie\nslice {{a.id}} \"{{b.name}}\" 1").expect_err("should refuse");
    assert!(format!("{e:#}").contains("one list"), "{e:#}");
}

#[test]
fn rows_written_as_a_value_are_reported_by_name() {
    let t = Template::parse("type pie\nslice a \"A\" {{r}}").expect("parses");
    let e = t
        .render(&Bindings::new().rows("r", vec![Bindings::new().set("v", 1i64)]))
        .expect_err("should refuse");
    assert!(format!("{e:#}").contains("`r`"), "{e:#}");
    assert!(format!("{e:#}").contains(".field"), "{e:#}");
}

#[test]
fn an_unbound_row_is_reported_like_any_other_hole() {
    let t = Template::parse("type pie\nslice {{r.id}} \"A\" 1").expect("parses");
    let e = t.render(&Bindings::new()).expect_err("should refuse");
    assert!(format!("{e:#}").contains("r.id"), "{e:#}");
}

#[test]
fn a_row_value_is_escaped_for_where_it_lands() {
    // The same guarantee substitution already gave, now per row.
    let t = Template::parse("type pie\nslice {{r.id}} \"{{r.name}}\" 1").expect("parses");
    let filled = t
        .fill(&Bindings::new().rows(
            "r",
            vec![
                Bindings::new()
                    .set("id", Bound::Raw("a".into()))
                    .set("name", "\" } node injected \""),
            ],
        ))
        .expect("fills");
    let d = parse_diagram(&filled).unwrap_or_else(|e| panic!("{e}\n{filled}"));
    assert_eq!(d.nodes.len(), 1, "{filled}");
}

#[test]
fn every_checked_in_svg_is_the_one_the_renderer_produces() {
    // Thirty-three rendered files sat in the tree with nothing comparing them.
    // They all moved when `W-040` changed how text is measured, and only a
    // manual regeneration caught it — a checked-in artefact nobody checks is a
    // file that can quietly stop being true.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut seen = 0;

    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {dir}: {e}")) {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|e| e != "bd") {
            continue;
        }
        let source = std::fs::read_to_string(&path).expect("readable");
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        // A template renders once it has data. When a `.json` sits beside it,
        // that is the rendered form worth checking in — and leaving it out
        // would mean the one example of the templating flow was the one nobody
        // could see.
        let data = path.with_extension("json");
        let fresh = match (source.contains("{{"), data.exists()) {
            (true, false) => continue,
            (true, true) => {
                let raw = std::fs::read_to_string(&data).expect("readable");
                let bound = Bindings::from_json(&raw).unwrap_or_else(|e| panic!("{name}: {e:#}"));
                Template::parse(&source)
                    .and_then(|t| t.render(&bound))
                    .unwrap_or_else(|e| panic!("{name}: {e:#}"))
            }
            _ => crate::render_source(&source).unwrap_or_else(|e| panic!("{name}: {e:#}")),
        };

        let svg = path.with_extension("svg");
        let Ok(committed) = std::fs::read_to_string(&svg) else {
            panic!("{name} has no rendered .svg beside it");
        };
        assert_eq!(
            committed.trim_end(),
            fresh.trim_end(),
            "{name} is stale — regenerate it with `bgraph {name} -o {}`",
            svg.file_name().unwrap().to_string_lossy()
        );
        seen += 1;
    }

    assert!(seen >= 25, "only {seen} rendered examples found");
}

#[test]
fn every_example_still_works() {
    // The suite used to name two example files by hand, so the ones added
    // since were covered by nothing at all. A directory sweep cannot forget.
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/examples");
    let mut seen = 0;

    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {dir}: {e}")) {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|e| e != "bd") {
            continue;
        }
        seen += 1;
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let source = std::fs::read_to_string(&path).expect("readable");

        // A template is checked as a template: its holes are not bound here,
        // so rendering it is not the question — whether it is well-formed is.
        let template = Template::parse(&source)
            .unwrap_or_else(|e| panic!("{name} is not a well-formed template: {e:#}"));
        if !template.holes().is_empty() {
            continue;
        }

        // Through the composer, because that is the pipeline: an example
        // using `include` or a `component` is not a diagram until it has run.
        // Parsing the text directly would have said `web.app` is an unknown
        // node, which is true of the text and false of the diagram.
        let composed = crate::compose::load(&path).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        let errors: Vec<String> = crate::check::check(&composed.diagram)
            .into_iter()
            .filter(|d| d.severity == crate::check::Severity::Error)
            .map(|d| d.message)
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");

        let svg = crate::render_file(&path).unwrap_or_else(|e| panic!("{name}: {e:#}"));
        assert!(
            svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"),
            "{name}"
        );
    }

    assert!(
        seen >= 16,
        "only {seen} examples found — the sweep is not finding them"
    );
}

#[test]
fn every_diagram_in_the_documentation_still_works() {
    // A ```bgraph fence is a promise that the snippet is a complete, current
    // file. Prose rots quietly — `language.md` was still calling a slice a
    // `node` five archetypes after the word changed — so the snippets are run
    // rather than trusted.
    let root = env!("CARGO_MANIFEST_DIR");
    let mut files: Vec<std::path::PathBuf> = vec![std::path::Path::new(root).join("README.md")];
    for entry in std::fs::read_dir(std::path::Path::new(root).join("docs")).expect("docs/") {
        let path = entry.expect("entry").path();
        if path.extension().is_some_and(|e| e == "md") {
            files.push(path);
        }
    }

    let mut checked = 0;
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let text = std::fs::read_to_string(&path).expect("readable");

        for (i, block) in text.split("```bgraph\n").skip(1).enumerate() {
            let source = block.split("```").next().expect("a closing fence");
            checked += 1;

            let diagram = parse_diagram(source)
                .unwrap_or_else(|e| panic!("{name} block {i}: {e:#}\n---\n{source}"));
            let errors: Vec<String> = crate::check::check(&diagram)
                .into_iter()
                .filter(|d| d.severity == crate::check::Severity::Error)
                .map(|d| d.message)
                .collect();
            assert!(
                errors.is_empty(),
                "{name} block {i}: {errors:?}\n---\n{source}"
            );
        }
    }

    assert!(checked >= 8, "only {checked} documented diagrams found");
}

#[test]
fn every_archetype_shows_a_complete_file() {
    // The tables say what a property means; a reader still has to start from
    // something. A genre with no example is a genre you have to guess at.
    let reference = crate::docgen::reference();
    for def in crate::props::ARCHETYPES {
        let heading = format!("### `type {}`", def.name);
        let section = reference
            .split(&heading)
            .nth(1)
            .unwrap_or_else(|| panic!("`{}` has no section", def.name))
            .split("\n### ")
            .next()
            .unwrap();
        assert!(
            section.contains("A complete file"),
            "`type {}` shows no worked example",
            def.name
        );
    }
}

#[test]
fn the_highlighter_captures_every_kind_of_value() {
    // Five archetypes shipped between the queries being written and this test:
    // dates, durations, members, edge data and `{{holes}}` were all parsed by
    // the grammar and coloured by nothing. A query that compiles is not a
    // query that covers.
    let highlights = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/editors/zed/languages/bgraph/highlights.scm"
    ))
    .expect("the extension ships highlights.scm");

    // Every named node type that carries text a reader looks at. Structural
    // nodes are captured through their fields, not by name.
    for node in [
        "string",
        "number",
        "color",
        "date",
        "duration",
        "placeholder",
        "member",
        "identifier",
        "comment",
        "arrow",
        "item_keyword",
        "property_name",
        "alignment",
        "direction",
    ] {
        assert!(
            highlights.contains(&format!("({node}")),
            "`{node}` is parsed but never highlighted"
        );
    }

    // Positional data is captured on both statement kinds — an ER relation
    // carries its cardinality exactly where an item carries its value.
    for field in ["(item_declaration data:", "(edge_declaration data:"] {
        assert!(highlights.contains(field), "`{field}` is not captured");
    }
}

// ------------------------------------------------- `graph { title }` (I-053)

#[test]
fn any_genre_can_be_titled() {
    // The defect I-053 named: `title` lived only in each chart archetype's
    // own settings, so the genres that draw boxes had no way to say it at
    // all. One assertion per family, because the free path and the chart
    // paths reach the heading by different routes.
    for source in [
        r#"graph { title: "Heading" }
           node a "A""#,
        r#"type class
           graph { title: "Heading" }
           class a "A""#,
        r#"type er
           graph { title: "Heading" }
           entity a "A""#,
    ] {
        let l = build(source);
        assert!(
            l.annotations.iter().any(|a| a.text == "Heading"),
            "no heading drawn for:\n{source}"
        );
    }
}

#[test]
fn the_diagram_title_sits_above_every_box() {
    let l = build(
        r#"graph { title: "Heading" }
           node a "A"
           node b "B" below a"#,
    );
    let top = l.nodes.iter().map(|n| n.rect.y).fold(f64::MAX, f64::min);
    let heading = l
        .annotations
        .iter()
        .find(|a| a.text == "Heading")
        .expect("the heading should be placed");
    assert!(
        heading.y < top,
        "the heading sits at {} but the topmost box starts at {top}",
        heading.y
    );
}

#[test]
fn the_diagram_title_wears_the_diagram_font() {
    // An empty family falls through to the renderer's serif default, which
    // is never what the boxes are set in — the heading looked pasted on.
    let l = build(
        r#"graph { title: "Heading" }
           node a "A""#,
    );
    let heading = l.annotations.iter().find(|a| a.text == "Heading").unwrap();
    assert_eq!(
        Some(heading.family.as_str()),
        l.nodes[0].style.font_family.as_deref(),
        "the heading is set in a different font from the boxes"
    );
}

#[test]
fn an_archetypes_own_title_beats_the_graph_one() {
    // Both are legal, and the more specific one wins. Stated as a test
    // because the fallback is written once per chart function.
    let l = build(
        r#"type pie { title: "Specific" }
           graph { title: "General" }
           slice a "A" 40"#,
    );
    assert!(l.annotations.iter().any(|a| a.text == "Specific"));
    assert!(
        !l.annotations.iter().any(|a| a.text == "General"),
        "the graph-level title should have been overridden, not drawn as well"
    );
}

#[test]
fn a_chart_falls_back_to_the_graph_title() {
    let l = build(
        r#"type pie
           graph { title: "General" }
           slice a "A" 40"#,
    );
    assert!(l.annotations.iter().any(|a| a.text == "General"));
}

// ------------------------------------------------- completions (I-056)

#[test]
fn completions_are_on_by_default_and_cost_the_core_nothing() {
    // The opposite call from `window`, and it needs stating for the same
    // reason. `clap_complete` adds no dependency beyond the `clap` already
    // here and `clap_mangen` brings one small crate, both binary-only — so
    // this is on by default, where the window cannot be. What must stay
    // true is that they are optional, so `--no-default-features` still
    // resolves to the five dependencies this crate promises.
    let manifest = include_str!("../Cargo.toml");

    let defaults = manifest
        .split_once("default = [")
        .and_then(|(_, tail)| tail.split_once(']'))
        .expect("a default feature list")
        .0;
    assert!(
        defaults.contains("completions"),
        "completions are not on by default: {defaults}"
    );

    for crate_name in ["clap_complete", "clap_mangen"] {
        let line = manifest
            .lines()
            .find(|l| l.trim_start().starts_with(crate_name))
            .unwrap_or_else(|| panic!("`{crate_name}` is not in the manifest"));
        assert!(
            line.contains("optional = true"),
            "`{crate_name}` is not optional: {line}"
        );
    }
}

#[test]
fn the_generated_reference_lists_the_completions_command() {
    // The reference's command list is hand-written prose in `docgen.rs`,
    // unlike the property tables — so it is the one part that can fall
    // behind a subcommand, and only a test notices.
    let manual = crate::docgen::reference();
    assert!(
        manual.contains("bgraph completions"),
        "the reference does not mention `completions`"
    );
}

// ------------------------------------------------- `bgraph init` (I-049)

#[test]
fn every_genre_starts_from_a_file_that_renders() {
    // The promise of the command: what comes out is not a sketch to repair
    // but a diagram. Anything less and the first thing a newcomer meets is
    // a diagnostic.
    for def in crate::props::ARCHETYPES {
        let source = crate::init::skeleton(def.name)
            .unwrap_or_else(|e| panic!("no skeleton for `{}`: {e:#}", def.name));

        let diagram = parse_diagram(&source)
            .unwrap_or_else(|e| panic!("`{}` does not parse: {e:#}", def.name));

        let errors: Vec<_> = crate::check::check(&diagram)
            .into_iter()
            .filter(|d| d.severity == crate::check::Severity::Error)
            .collect();
        assert!(
            errors.is_empty(),
            "`{}` starts with an error: {errors:?}",
            def.name
        );

        layout::solve(&diagram).unwrap_or_else(|e| panic!("`{}` does not draw: {e:#}", def.name));
    }
}

#[test]
fn the_skeleton_states_the_genres_own_shape() {
    // The header is generated from the archetype tables rather than written
    // per genre, so it cannot describe a keyword that was renamed. Asserted
    // over every genre, because one hand-written header would pass a test
    // that named only `pie`.
    for def in crate::props::ARCHETYPES {
        let source = crate::init::skeleton(def.name).unwrap();
        let header: String = source.lines().take_while(|l| l.starts_with("//")).collect();

        for item in def.items() {
            assert!(
                header.contains(&format!("{item} <id>")),
                "`{}` never shows how to write a `{item}`",
                def.name
            );
        }
        for slot in def.slots {
            assert!(
                header.contains(&format!("<{}>", slot.prop)),
                "`{}` never shows its `{}` slot",
                def.name,
                slot.prop
            );
        }
    }
}

#[test]
fn an_unknown_genre_names_the_ones_that_exist() {
    // The person typing a wrong genre is the person who does not know the
    // list, so the error is the list.
    let e = crate::init::skeleton("piechart").expect_err("`piechart` is not a genre");
    let message = format!("{e:#}");
    assert!(message.contains("piechart"), "{message}");
    assert!(message.contains("pie"), "{message}");
    assert!(message.contains("gantt"), "{message}");
}

// ------------------------------------------------- diagnostics (I-051)

/// The first diagnostic a source produces, with its position resolved.
fn first_diag(source: &str) -> (crate::check::Diag, usize, usize) {
    let diag = diags(source).into_iter().next().expect("a diagnostic");
    let (line, col) = crate::lsp::LineIndex::new(source).line_col(diag.span.start);
    (diag, line, col)
}

#[test]
fn the_flat_form_has_not_moved() {
    // Pinned byte for byte, and deliberately so. Editors parse this, CI logs
    // are grepped for it, and readers have it in a shell alias — it is an
    // interchange format, not a rendering, and the rich form beside it is
    // not permission to drift.
    let source = r#"node a "A" { fil: #fff }"#;
    let (diag, line, col) = first_diag(source);

    assert_eq!(
        crate::report::flat("diagram.bd", line, col, &diag),
        "diagram.bd:1:14: error: unknown property `fil` — did you mean `fill`?"
    );
}

#[test]
fn the_caret_lands_under_what_is_wrong() {
    // The whole value of the rich form. An off-by-one here is worse than no
    // caret at all: it points confidently at the wrong token.
    let source = "node a \"A\"\nnode b \"B\" right-of a\nb -> nowhere\n";
    let (diag, line, col) = first_diag(source);
    let rendered = crate::report::rich("d.bd", source, line, col, &diag, false);

    let lines: Vec<&str> = rendered.lines().collect();
    let text = lines[3];
    let carets = lines[4];

    let at = carets.find('^').expect("a caret");
    assert_eq!(
        text[at..at + 7].to_string(),
        "nowhere",
        "the caret sits over `{}`",
        &text[at..]
    );
    assert_eq!(
        carets[at..].trim_end(),
        "^".repeat(7),
        "the caret does not cover the whole name"
    );
}

#[test]
fn a_span_running_past_its_line_is_not_underlined_to_the_end_of_the_file() {
    // An unclosed block reaches to end of file. Underlining all of it would
    // claim the rest of the diagram is the mistake.
    // Built rather than provoked: which parser reports an unclosed block is
    // beside the point, and the clamp is what is being tested.
    let source = "node a \"A\" {\nnode b \"B\"\nnode c \"C\"\n";
    let diag = crate::check::Diag {
        severity: crate::check::Severity::Error,
        message: "unclosed block".into(),
        span: crate::ast::Span {
            start: 11,
            end: source.len(),
        },
    };

    let rendered = crate::report::rich("d.bd", source, 0, 11, &diag, false);
    let carets = rendered.lines().nth(4).expect("a caret line");
    assert_eq!(
        carets.chars().filter(|c| *c == '^').count(),
        1,
        "the underline runs past its own line:\n{rendered}"
    );
}

#[test]
fn colour_is_absent_when_it_is_not_wanted() {
    // Escape sequences in a redirected stream are noise a reader has to strip.
    let source = r#"node a "A" { fil: #fff }"#;
    let (diag, line, col) = first_diag(source);

    let plain = crate::report::rich("d.bd", source, line, col, &diag, false);
    assert!(!plain.contains('\x1b'), "uncoloured output has escapes");

    let tinted = crate::report::rich("d.bd", source, line, col, &diag, true);
    assert!(tinted.contains('\x1b'), "coloured output has none");
}

// ------------------------------------------------- watch a directory (I-052)

/// A scratch directory of its own per test, emptied first so a previous run
/// cannot make this one pass.
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("bgraph-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

#[test]
fn a_directory_builds_every_diagram_in_it() {
    let dir = scratch("build-all");
    std::fs::write(dir.join("one.bd"), "node a \"A\"\n").unwrap();
    std::fs::write(dir.join("two.bd"), "node b \"B\"\n").unwrap();
    // Not a source, and must not become one: the loop renders `.bd`, not
    // whatever else happens to share the directory.
    std::fs::write(dir.join("notes.md"), "# notes\n").unwrap();

    crate::watch::build_dir(&dir, None, true).expect("the directory should build");

    assert!(dir.join("one.svg").is_file(), "one.svg was not written");
    assert!(dir.join("two.svg").is_file(), "two.svg was not written");
    assert!(!dir.join("notes.svg").exists(), "a non-source was rendered");
}

#[test]
fn one_broken_file_does_not_stop_the_others() {
    // Mid-edit, a broken file is the normal state of the one being worked on.
    // Taking the loop down with it would end the session.
    let dir = scratch("build-broken");
    std::fs::write(dir.join("good.bd"), "node a \"A\"\n").unwrap();
    std::fs::write(dir.join("bad.bd"), "node a \"A\" right-of ghost\n").unwrap();

    let outcome = crate::watch::build_dir(&dir, None, true);

    assert!(
        dir.join("good.svg").is_file(),
        "the sound file was not rendered"
    );
    assert!(
        !dir.join("bad.svg").exists(),
        "the broken file was rendered"
    );
    // …and the *command* still fails, or a build step would pass on a
    // directory half of which does not draw.
    assert!(outcome.is_err(), "a failed render exited zero");
}

#[test]
fn a_picture_already_current_is_not_rewritten() {
    let dir = scratch("build-current");
    std::fs::write(dir.join("one.bd"), "node a \"A\"\n").unwrap();

    crate::watch::build_dir(&dir, None, true).unwrap();
    let first = std::fs::metadata(dir.join("one.svg"))
        .and_then(|m| m.modified())
        .unwrap();

    crate::watch::build_dir(&dir, None, true).unwrap();
    let second = std::fs::metadata(dir.join("one.svg"))
        .and_then(|m| m.modified())
        .unwrap();

    assert_eq!(
        first, second,
        "an up-to-date picture was written again — a build loop that rewrites \
         everything every pass is one that never settles"
    );
}

#[test]
fn the_pictures_can_go_somewhere_else() {
    let dir = scratch("build-out-src");
    let out = scratch("build-out-dst");
    std::fs::write(dir.join("one.bd"), "node a \"A\"\n").unwrap();

    crate::watch::build_dir(&dir, Some(&out), true).unwrap();

    assert!(out.join("one.svg").is_file(), "nothing landed in --out");
    assert!(
        !dir.join("one.svg").exists(),
        "the source directory was written to as well"
    );
}

#[test]
fn sources_come_back_in_a_stable_order() {
    // The filesystem's order is not one, and a build whose log reorders
    // itself each run is a log that cannot be compared with the last.
    let dir = scratch("build-order");
    for name in ["zebra.bd", "alpha.bd", "mid.bd"] {
        std::fs::write(dir.join(name), "node a \"A\"\n").unwrap();
    }

    let names: Vec<String> = crate::watch::sources(&dir)
        .unwrap()
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, ["alpha.bd", "mid.bd", "zebra.bd"]);
}

// ------------------------------------------------- PNG output (I-041)

#[cfg(feature = "png")]
#[test]
fn the_raster_is_the_same_picture_as_the_vector() {
    // One renderer, two encodings. A raster drawn by its own path would be a
    // second renderer, and the two would drift the way a preview and an
    // export always do when they are drawn separately.
    let l = build("node a \"A\"\nnode b \"B\" right-of a\na -> b");
    let (_, _, w, h) = crate::render_svg::canvas(&l);

    let png = crate::render_png::render(&l, 1.0).expect("a raster");
    assert_eq!(&png[1..4], b"PNG", "not a PNG");

    // The IHDR width and height are big-endian at a fixed offset, which is
    // enough to assert the size without decoding the image.
    let width = u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    let height = u32::from_be_bytes([png[20], png[21], png[22], png[23]]);
    assert_eq!(width, w.round() as u32, "the raster is not the canvas wide");
    assert_eq!(
        height,
        h.round() as u32,
        "the raster is not the canvas tall"
    );
}

#[cfg(feature = "png")]
#[test]
fn scale_multiplies_the_diagrams_own_size() {
    let l = build("node a \"A\"");
    let one = crate::render_png::render(&l, 1.0).unwrap();
    let two = crate::render_png::render(&l, 2.0).unwrap();

    let width = |png: &[u8]| u32::from_be_bytes([png[16], png[17], png[18], png[19]]);
    // Each dimension rounds on its own, so twice the size is within a pixel
    // rather than exactly double.
    assert!(
        width(&two).abs_diff(width(&one) * 2) <= 1,
        "{} is not twice {}",
        width(&two),
        width(&one)
    );
}

#[cfg(feature = "png")]
#[test]
fn a_nonsense_scale_does_not_bring_the_renderer_down() {
    // `--scale 0` and `--scale -1` are things a person types. Neither may
    // reach `Pixmap::new` as a zero or a negative.
    let l = build("node a \"A\"");
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            crate::render_png::render(&l, scale).is_ok(),
            "scale {scale} was not handled"
        );
    }
}

#[test]
fn the_rasteriser_is_opt_in_and_the_window_implies_it() {
    // `X-017` narrows here rather than retiring: raster becomes optional, not
    // present. What must stay true is the direction of the implication — a
    // window already needs the rasteriser, and someone rendering images in a
    // build wants no windowing library.
    let manifest = include_str!("../Cargo.toml");

    let defaults = manifest
        .split_once("default = [")
        .and_then(|(_, tail)| tail.split_once(']'))
        .expect("a default feature list")
        .0;
    assert!(!defaults.contains("png"), "the rasteriser is on by default");

    let window = manifest
        .lines()
        .find(|l| l.starts_with("window = "))
        .expect("a window feature");
    assert!(
        window.contains("\"png\""),
        "the window does not imply the rasteriser: {window}"
    );
    assert!(
        !window.contains("resvg"),
        "the window names the rasteriser directly instead of going through \
         `png`, so the two can drift: {window}"
    );
}

// ------------------------------------------------- layout as JSON (I-045)

fn geometry(source: &str) -> serde_json::Value {
    serde_json::from_str(&crate::render_json::render(&build(source))).expect("valid JSON")
}

#[test]
fn the_json_geometry_is_in_the_same_space_as_the_svg() {
    // The whole reason the export is worth having. Emitting the layout's own
    // untranslated coordinates would hand every consumer the same
    // off-by-a-padding bug to find on their own.
    let l = build("node a \"A\"\nnode b \"B\" right-of a\na -> b");
    let (dx, dy, _, _) = crate::render_svg::canvas(&l);
    let json = geometry("node a \"A\"\nnode b \"B\" right-of a\na -> b");

    let node = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "a")
        .expect("`a` should be in the export");

    assert_eq!(node["rect"]["x"].as_f64().unwrap(), l.nodes[0].rect.x + dx);
    assert_eq!(node["rect"]["y"].as_f64().unwrap(), l.nodes[0].rect.y + dy);
}

#[test]
fn every_box_the_author_named_is_addressable() {
    // Geometry without names is a picture, not data.
    let json = geometry(
        r#"group core "Core" {
             node api "API"
             node db "DB" below api
           }
           node ui "UI" right-of core
           api -> db"#,
    );

    let ids: Vec<&str> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["id"].as_str())
        .filter(|id| !id.is_empty())
        .collect();
    assert!(ids.contains(&"api"), "{ids:?}");
    assert!(ids.contains(&"db"), "{ids:?}");
    assert!(ids.contains(&"ui"), "{ids:?}");

    let groups: Vec<&str> = json["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|g| g["id"].as_str())
        .collect();
    assert_eq!(groups, ["core"]);

    let edge = &json["edges"][0];
    assert_eq!(edge["from"], "api");
    assert_eq!(edge["to"], "db");
}

#[test]
fn an_edge_carries_its_elbows_and_not_just_its_ends() {
    // A consumer drawing a straight line between the endpoints of an
    // orthogonal edge would be drawing a different diagram.
    let json = geometry(
        r#"node a "A"
           node b "B" below a
           a -> b { router: ortho }"#,
    );
    let points = json["edges"][0]["points"].as_array().unwrap();
    assert!(
        points.len() >= 2,
        "an edge came back with fewer than two points"
    );
}

#[test]
fn the_export_declares_its_schema_version() {
    // The shape is promised and the coordinates are not, so a consumer needs
    // to be able to tell which half changed.
    let json = geometry("node a \"A\"");
    assert_eq!(
        json["version"].as_u64().unwrap() as u32,
        crate::render_json::VERSION
    );
}

#[test]
fn the_canvas_in_the_export_is_the_canvas_in_the_svg() {
    let l = build("node a \"A\"\nnode b \"B\" right-of a");
    let (_, _, w, h) = crate::render_svg::canvas(&l);
    let json = geometry("node a \"A\"\nnode b \"B\" right-of a");

    assert_eq!(json["canvas"]["width"].as_f64().unwrap(), w);
    assert_eq!(json["canvas"]["height"].as_f64().unwrap(), h);
}

// ------------------------------------------------- editor setup (I-057)

#[test]
fn the_editor_guide_points_at_paths_that_exist() {
    // Written because the first draft of that guide cited
    // `tree-sitter-bgraph/queries/`, which has never existed — the queries
    // live beside the Zed extension. A setup guide with a wrong path is worse
    // than none: it costs the reader the time to discover it is wrong.
    let guide = include_str!("../editors/README.md");
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let mut checked = 0;
    for word in guide.split(|c: char| c.is_whitespace() || c == '`' || c == '*') {
        let word = word.trim_end_matches(['.', ',', ':', ';', ')']);
        if !word.starts_with("editors/")
            && !word.starts_with("tree-sitter-bgraph/")
            && !word.starts_with("src/")
        {
            continue;
        }
        // A path with a placeholder in it is an instruction, not a claim.
        if word.contains('<') || word.contains('*') {
            continue;
        }
        assert!(
            root.join(word).exists(),
            "the editor guide cites `{word}`, which does not exist"
        );
        checked += 1;
    }
    assert!(
        checked >= 4,
        "only {checked} paths were checked — the guide's \
                           shape changed and this test stopped looking"
    );
}

#[test]
fn every_editor_the_repository_ships_is_in_the_guide() {
    // A packaged editor that the guide does not mention is one nobody finds.
    let guide = include_str!("../editors/README.md");
    let editors = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/editors"));

    for entry in std::fs::read_dir(editors).expect("an editors directory") {
        let path = entry.expect("a directory entry").path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            guide.to_lowercase().contains(&name.to_lowercase()),
            "`editors/{name}` is packaged but the guide never mentions it"
        );
    }
}

// ------------------------------------------------- error recovery (I-055)

#[test]
fn a_mistake_on_line_three_does_not_cost_line_fifty() {
    // The whole point. Before this, one bad separator threw away the
    // diagnostics and the highlighting of everything after it — at exactly
    // the moment both were wanted.
    let source = "node a \"A\"\nnode b \"B\" right-of a\nnode ??? bad\nnode d \"D\" below b\n";
    let (diagram, errors) = crate::parser::parse_recovering(source);

    assert_eq!(errors.len(), 1, "{errors:?}");
    let diagram = diagram.expect("the sound statements should survive");
    let ids: Vec<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(
        ids,
        ["a", "b", "d"],
        "a statement past the mistake was lost"
    );
}

#[test]
fn the_recovered_error_points_where_the_mistake_is() {
    // An error reported at the top of the file is an error nobody can act on.
    let source = "node a \"A\"\nnode ??? bad\n";
    let (_, errors) = crate::parser::parse_recovering(source);

    let span = errors.first().expect("an error").span;
    assert!(
        span.start >= 11,
        "the error is reported at byte {} — before the second line even \
         starts",
        span.start
    );
}

#[test]
fn several_mistakes_are_all_reported() {
    let source = "node ?? one\nnode b \"B\"\nnode ?? two\n";
    let (diagram, errors) = crate::parser::parse_recovering(source);

    assert_eq!(errors.len(), 2, "{errors:?}");
    assert_eq!(diagram.expect("`b` should survive").nodes.len(), 1);
}

#[test]
fn a_sound_file_pays_nothing_and_reports_nothing() {
    let source = "node a \"A\"\nnode b \"B\" right-of a\na -> b\n";
    let (diagram, errors) = crate::parser::parse_recovering(source);

    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(diagram.expect("it parses").nodes.len(), 2);
}

#[test]
fn a_block_spanning_lines_is_one_statement() {
    // A statement ends at a newline *at depth zero*. Treating every line as a
    // statement would cut a block in half and report three mistakes where
    // there are none.
    let source = "node a \"A\" {\n  fill: #fff\n  stroke: #000\n}\nnode ?? bad\n";
    let (diagram, errors) = crate::parser::parse_recovering(source);

    assert_eq!(errors.len(), 1, "the block was cut up: {errors:?}");
    let diagram = diagram.expect("the block should survive");
    assert_eq!(diagram.nodes.len(), 1);
    assert_eq!(
        diagram.nodes[0].props.len(),
        2,
        "the block lost its content"
    );
}

#[test]
fn a_trailing_comment_does_not_drag_the_next_statement_down_with_it() {
    // The comment scanner must leave the newline behind, or the two lines
    // become one chunk and a mistake in either blanks both.
    let source = "node a \"A\" // fine\nnode ?? bad\n";
    let (diagram, errors) = crate::parser::parse_recovering(source);

    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(
        diagram
            .expect("`a` should survive its own comment")
            .nodes
            .len(),
        1
    );
}

#[test]
fn an_unclosed_block_costs_only_what_is_inside_it() {
    // A block that never closes swallows the rest of the file — depth never
    // returns to zero, so everything after the `{` is one statement. That is
    // the right answer rather than a shortcoming: what precedes it is still
    // sound and still reported, and nothing inside is guessed at.
    let source = "node a \"A\"\nnode b \"B\" {\nnode c \"C\"\n";
    let (diagram, errors) = crate::parser::parse_recovering(source);

    assert_eq!(errors.len(), 1, "{errors:?}");
    let diagram = diagram.expect("the statement before the block should survive");
    let ids: Vec<&str> = diagram.nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, ["a"]);
}

#[test]
fn nesting_past_the_cap_is_reported_rather_than_recovered_from() {
    // The one case where recovery is refused: past `MAX_NESTING` every
    // boundary is a guess, and the guard exists to stop a stack overflow
    // rather than to improve a message.
    let source = "node a ".to_string() + &"{ x: ".repeat(MAX_NESTING + 2);
    let (diagram, errors) = crate::parser::parse_recovering(&source);

    assert!(diagram.is_none(), "a file past the nesting cap was parsed");
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(
        errors[0].message.contains("nested"),
        "{}",
        errors[0].message
    );
}

#[test]
fn a_syntax_error_no_longer_hides_the_rest_of_the_file() {
    // The reason recovery was worth building. Three different kinds of
    // problem, on three different lines, all reported in one pass — where
    // before the first one silenced the other two until it was fixed.
    let source = concat!(
        "node a \"A\"\n",
        "node ??? bad\n",
        "node c \"C\" { fil: #fff }\n",
        "c -> ghost\n",
    );
    let reported = crate::lsp::diagnostics(source);
    let messages: Vec<String> = reported
        .iter()
        .filter_map(|d| d["message"].as_str().map(String::from))
        .collect();

    assert!(
        messages.iter().any(|m| m.contains("syntax error")),
        "{messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("`fil`")),
        "a property mistake after a syntax error went unreported: {messages:?}"
    );
    assert!(
        messages.iter().any(|m| m.contains("ghost")),
        "an unknown name after a syntax error went unreported: {messages:?}"
    );
}

// ------------------------------------------------- composition (I-028, I-032)

/// Write a set of files into a scratch directory and return the entry point.
fn compose_files(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = scratch(name);
    for (file, source) in files {
        std::fs::write(dir.join(file), source).expect("a scratch file");
    }
    dir.join(files[0].0)
}

#[test]
fn an_include_brings_a_file_in_whole() {
    let entry = compose_files(
        "compose-plain",
        &[
            (
                "main.bd",
                "include \"core.bd\"\nnode ui \"UI\"\nui -> api\n",
            ),
            ("core.bd", "node api \"API\"\n"),
        ],
    );
    let composed = crate::compose::load(&entry).expect("it should compose");
    let ids: Vec<&str> = composed
        .diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert!(ids.contains(&"api"), "{ids:?}");
    assert!(ids.contains(&"ui"), "{ids:?}");
    crate::layout::solve(&composed.diagram).expect("and it should draw");
}

#[test]
fn a_prefix_makes_a_clash_impossible() {
    // Both files declare `api`. Without `as` that is an error; with it the
    // two are simply different names.
    let entry = compose_files(
        "compose-prefix",
        &[
            (
                "main.bd",
                "include \"core.bd\" as core\nnode api \"Mine\"\napi -> core.api\n",
            ),
            ("core.bd", "node api \"Theirs\"\n"),
        ],
    );
    let composed = crate::compose::load(&entry).expect("it should compose");
    let ids: Vec<&str> = composed
        .diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert!(ids.contains(&"core.api"), "{ids:?}");
    assert!(ids.contains(&"api"), "{ids:?}");
    crate::layout::solve(&composed.diagram).expect("the qualified edge should resolve");
}

#[test]
fn an_included_files_own_references_follow_it_under_the_prefix() {
    // The included file's internal edge and anchor must be renamed with the
    // names they point at, or the prefix would break the file it prefixes.
    let entry = compose_files(
        "compose-internal",
        &[
            ("main.bd", "include \"core.bd\" as core\nnode ui \"UI\"\n"),
            (
                "core.bd",
                "node api \"API\"\nnode db \"DB\" below api\napi -> db\n",
            ),
        ],
    );
    let composed = crate::compose::load(&entry).expect("it should compose");
    let edge = &composed.diagram.edges[0];
    assert_eq!(edge.from.as_str(), "core.api");
    assert_eq!(edge.to.as_str(), "core.db");
    crate::layout::solve(&composed.diagram).expect("the anchor should still resolve");
}

#[test]
fn a_name_in_two_files_is_an_error_naming_both() {
    // `D-005` applied to a second file rather than a second spelling: no
    // silent shadowing, and the message has to say where the other one is or
    // it cannot be acted on.
    let entry = compose_files(
        "compose-clash",
        &[
            ("main.bd", "include \"core.bd\"\nnode api \"Mine\"\n"),
            ("core.bd", "node api \"Theirs\"\n"),
        ],
    );
    let message = format!("{:#}", crate::compose::load(&entry).expect_err("a clash"));
    assert!(message.contains("api"), "{message}");
    assert!(message.contains("main.bd"), "{message}");
    assert!(message.contains("core.bd"), "{message}");
}

#[test]
fn a_circular_include_names_the_chain() {
    let entry = compose_files(
        "compose-cycle",
        &[
            ("a.bd", "include \"b.bd\"\nnode a \"A\"\n"),
            ("b.bd", "include \"a.bd\"\nnode b \"B\"\n"),
        ],
    );
    let message = format!("{:#}", crate::compose::load(&entry).expect_err("a cycle"));
    assert!(message.contains("circle"), "{message}");
    assert!(message.contains("a.bd"), "{message}");
    assert!(message.contains("b.bd"), "{message}");
}

#[test]
fn a_bds_holds_styles_and_nothing_else() {
    let entry = compose_files(
        "compose-bds",
        &[
            ("main.bd", "include \"stray.bds\"\nnode a \"A\"\n"),
            ("stray.bds", "style x { fill: #fff }\nnode b \"B\"\n"),
        ],
    );
    let message = format!(
        "{:#}",
        crate::compose::load(&entry).expect_err("a stray item")
    );
    assert!(message.contains(".bds"), "{message}");

    // …and one that holds only styles is fine, and its styles arrive.
    let entry = compose_files(
        "compose-bds-good",
        &[
            ("main.bd", "include \"house.bds\"\nnode a \"A\" .card\n"),
            ("house.bds", "style card { fill: #1e293b }\n"),
        ],
    );
    let composed = crate::compose::load(&entry).expect("styles should come in");
    assert_eq!(composed.diagram.styles.len(), 1);
    let l = crate::layout::solve(&composed.diagram).expect("it draws");
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#1e293b"));
}

#[test]
fn a_diagnostic_points_at_the_file_that_wrote_it() {
    // The whole reason the composer keeps a source map. A warning about a
    // statement in an included file reported against the including file
    // would send the reader to the wrong line of the wrong document.
    let entry = compose_files(
        "compose-spans",
        &[
            ("main.bd", "include \"core.bd\"\nnode ui \"UI\"\n"),
            ("core.bd", "node api \"API\"\nnode db \"DB\" below api\n"),
        ],
    );
    let composed = crate::compose::load(&entry).expect("it composes");

    let api = composed
        .diagram
        .nodes
        .iter()
        .find(|n| n.id.as_str() == "api")
        .expect("`api` is there");
    let (path, line, _) = composed.map.locate(api.id.span.start);
    assert_eq!(path.file_name().unwrap(), "core.bd");
    assert_eq!(line, 0, "`api` is on the first line of core.bd");

    let ui = composed
        .diagram
        .nodes
        .iter()
        .find(|n| n.id.as_str() == "ui")
        .expect("`ui` is there");
    let (path, line, _) = composed.map.locate(ui.id.span.start);
    assert_eq!(path.file_name().unwrap(), "main.bd");
    assert_eq!(line, 1, "`ui` is on the second line of main.bd");
}

#[test]
fn render_source_refuses_an_include_rather_than_reading_the_disk() {
    // It is the semver surface and the wasm build rests on it, so it cannot
    // start opening files because a statement asked it to (`D-029`).
    let message = format!(
        "{:#}",
        crate::render_source("include \"other.bd\"\nnode a \"A\"\n").expect_err("it must refuse")
    );
    assert!(message.contains("render_file"), "{message}");
}

#[test]
fn a_qualified_name_does_not_swallow_a_class_reference() {
    // The conflict `I-032` flagged. `core.api` is one token by maximal munch,
    // so `.card` after it is still a class and not the tail of a name.
    let l = build("style card { fill: #123456 }\nnode a \"A\" .card\n");
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#123456"));
}

#[test]
fn the_composed_example_set_still_draws() {
    // `examples/compose/` is what a bgraph-tagged fence cannot be: three
    // files that are one diagram. That fence promises a complete current
    // file, so the multi-file case is verified here rather than documented
    // and never run.
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/compose"));

    let composed = crate::compose::load(&dir.join("main.bd")).expect("main.bd composes");
    let ids: Vec<&str> = composed
        .diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert!(ids.contains(&"core.api"), "{ids:?}");
    assert!(ids.contains(&"ui"), "{ids:?}");
    crate::layout::solve(&composed.diagram).expect("and it draws");

    // …and the included file is a diagram in its own right, which is the
    // claim its own comment makes.
    let alone = crate::compose::load(&dir.join("core.bd")).expect("core.bd composes alone");
    crate::layout::solve(&alone.diagram).expect("and draws alone");
}

// ------------------------------------------------- named themes (I-029)

#[test]
fn every_theme_is_a_style_file_that_parses_and_resolves() {
    // A theme *is* a `.bds`, run through the same parser and registry as one
    // somebody writes. If that stopped being true, this is where it shows.
    for theme in crate::themes::THEMES {
        let parsed = parse_diagram(theme.styles)
            .unwrap_or_else(|e| panic!("theme `{}` does not parse: {e:#}", theme.name));
        crate::style::Registry::build(&parsed.styles)
            .unwrap_or_else(|e| panic!("theme `{}` does not resolve: {e:#}", theme.name));
        assert!(
            parsed.nodes.is_empty() && parsed.edges.is_empty(),
            "theme `{}` declares more than styles",
            theme.name
        );
    }
}

#[test]
fn every_theme_is_readable() {
    // Validated for contrast the way the categorical palette already is: a
    // theme that ships text nobody can read is worse than no theme.
    for theme in crate::themes::THEMES {
        let mut diagram = parse_diagram("node a \"A\"\nnode b \"B\" right-of a\na -> b\n").unwrap();
        crate::themes::apply(&mut diagram, theme.name).expect("it applies");

        let unreadable: Vec<String> = crate::check::check(&diagram)
            .into_iter()
            .filter(|d| d.message.contains("contrast"))
            .map(|d| d.message)
            .collect();
        assert!(
            unreadable.is_empty(),
            "theme `{}`: {unreadable:?}",
            theme.name
        );
    }
}

#[test]
fn a_theme_goes_underneath_the_file_rather_than_over_it() {
    // A theme that overrode the file would make `--theme` destructive rather
    // than useful: it would silently discard the one box the author coloured
    // on purpose.
    let mut diagram = parse_diagram("node a \"A\" { fill: #f43f5e }\n").unwrap();
    crate::themes::apply(&mut diagram, "dark").unwrap();
    let l = crate::layout::solve(&diagram).unwrap();
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#f43f5e"));

    // …and a named style the file declares wins over the theme's of the same
    // name, which is the same rule one level up.
    let mut diagram = parse_diagram("style node { fill: #ffffff }\nnode a \"A\"\n").unwrap();
    crate::themes::apply(&mut diagram, "dark").unwrap();
    let l = crate::layout::solve(&diagram).unwrap();
    assert_eq!(l.nodes[0].style.fill.as_deref(), Some("#ffffff"));
}

#[test]
fn a_declared_background_beats_the_themes() {
    let mut diagram = parse_diagram("graph { background: #123456 }\nnode a \"A\"\n").unwrap();
    crate::themes::apply(&mut diagram, "dark").unwrap();
    let l = crate::layout::solve(&diagram).unwrap();
    assert_eq!(l.opts.background.as_deref(), Some("#123456"));

    // …and without one, the theme supplies it.
    let mut diagram = parse_diagram("node a \"A\"\n").unwrap();
    crate::themes::apply(&mut diagram, "dark").unwrap();
    let l = crate::layout::solve(&diagram).unwrap();
    assert_eq!(l.opts.background.as_deref(), Some("#0b1120"));
}

#[test]
fn an_unknown_theme_names_the_ones_that_exist() {
    let mut diagram = parse_diagram("node a \"A\"\n").unwrap();
    let message = format!(
        "{:#}",
        crate::themes::apply(&mut diagram, "neon").expect_err("`neon` is not a theme")
    );
    for name in ["neon", "light", "dark", "print"] {
        assert!(message.contains(name), "{message}");
    }
}

// ------------------------------------------------- components (I-030)

fn compose_one(name: &str, source: &str) -> crate::compose::Composed {
    let dir = scratch(name);
    let file = dir.join("main.bd");
    std::fs::write(&file, source).expect("a scratch file");
    crate::compose::load(&file).expect("it should compose")
}

#[test]
fn a_component_placed_twice_declares_two_of_everything() {
    let composed = compose_one(
        "component-twice",
        "component service {\n  node app \"{{label}}\"\n  node db \"DB\" below app\n  app -> db\n}\n\
         use service as web { label: \"Web\" }\n\
         use service as jobs { label: \"Jobs\" }\n",
    );
    let ids: Vec<&str> = composed
        .diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    for expected in ["web.app", "web.db", "jobs.app", "jobs.db"] {
        assert!(ids.contains(&expected), "{ids:?}");
    }
    assert_eq!(
        composed.diagram.edges.len(),
        2,
        "each instance has its own edge"
    );
    crate::layout::solve(&composed.diagram).expect("and it draws");
}

#[test]
fn an_instance_is_a_frame_so_it_can_be_placed() {
    // What makes the feature usable rather than a curiosity: without this an
    // author can name the boxes inside an instance but cannot say where the
    // instance goes.
    let composed = compose_one(
        "component-placed",
        "component service {\n  node app \"A\"\n}\n\
         use service as web\n\
         use service as jobs right-of web gap 90\n",
    );
    let frames: Vec<&str> = composed
        .diagram
        .groups
        .iter()
        .map(|g| g.id.as_str())
        .collect();
    assert_eq!(frames, ["web", "jobs"]);

    let l = crate::layout::solve(&composed.diagram).expect("it draws");
    let web = l.groups.iter().find(|g| g.id == "web").unwrap();
    let jobs = l.groups.iter().find(|g| g.id == "jobs").unwrap();
    assert!(
        jobs.rect.x > web.rect.right(),
        "the second instance did not land to the right of the first"
    );
}

#[test]
fn a_binding_lands_where_the_hole_is_and_is_escaped_for_it() {
    // The component body goes through the template engine, so a label with a
    // quote in it cannot become a statement.
    let composed = compose_one(
        "component-escape",
        "component box {\n  node b \"{{label}}\"\n}\n\
         use box as one { label: \"a \\\" quote\" }\n",
    );
    let l = crate::layout::solve(&composed.diagram).expect("it draws");
    assert_eq!(l.nodes[0].lines, vec!["a \" quote"]);
}

#[test]
fn a_components_holes_are_not_the_files_holes() {
    // A component's holes are bound by each `use`, not by whoever renders the
    // file. Counting them would make every file with a component a template
    // that can never be satisfied.
    let source = "component box {\n  node b \"{{label}}\"\n}\nuse box as one { label: \"A\" }\n";
    let template = crate::Template::parse(source).expect("it parses");
    assert!(
        template.holes().is_empty(),
        "the file claims holes that belong to a component: {:?}",
        template.holes()
    );
}

#[test]
fn placing_an_unknown_component_says_which_exist() {
    let dir = scratch("component-unknown");
    let file = dir.join("main.bd");
    std::fs::write(
        &file,
        "component service {\n  node a \"A\"\n}\nuse serivce as web\n",
    )
    .unwrap();
    let message = format!("{:#}", crate::compose::load(&file).expect_err("a typo"));
    assert!(message.contains("serivce"), "{message}");
    assert!(message.contains("service"), "{message}");
}

// ------------------------------------- grid and constraints (I-033…I-035)

#[test]
fn a_grid_cell_is_an_absolute_position_in_declared_units() {
    // `D-031`: a coordinate system, not a solver. `at (col 3, row 2)` is
    // still `at`, which is why it cannot break `D-001`.
    let l = build(
        r#"graph { grid: 160 90 }
           node a "A" at (col 0, row 0)
           node b "B" at (col 2, row 1)"#,
    );
    assert_eq!((l.nodes[0].rect.x, l.nodes[0].rect.y), (0.0, 0.0));
    assert_eq!((l.nodes[1].rect.x, l.nodes[1].rect.y), (320.0, 90.0));
}

#[test]
fn one_grid_number_is_a_square_cell() {
    let l = build(
        r#"graph { grid: 120 }
           node a "A" at (col 1, row 1)"#,
    );
    assert_eq!((l.nodes[0].rect.x, l.nodes[0].rect.y), (120.0, 120.0));
}

#[test]
fn a_box_in_pixels_is_untouched_by_the_grid() {
    // Nothing is snapped to it. A grid that quietly moved a declared
    // position would be exactly the solver `D-001` refuses.
    let l = build(
        r#"graph { grid: 160 90 }
           node a "A" at (37, 41)"#,
    );
    assert_eq!((l.nodes[0].rect.x, l.nodes[0].rect.y), (37.0, 41.0));
}

#[test]
fn same_width_resizes_before_placement_so_nothing_overlaps() {
    // Found by looking at the picture: widening a box *after* it is placed
    // does not push its neighbours along, it lands on top of them. A size
    // constraint belongs before placement, because placement already knows
    // how to lay out boxes of a given size.
    let l = build(
        r#"node a "A"
           node b "A much longer label" right-of a gap 40
           node c "C" right-of b gap 40
           same-width a b c"#,
    );
    let widths: Vec<f64> = l.nodes.iter().map(|n| n.rect.w).collect();
    assert!(
        widths.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01),
        "widths differ: {widths:?}"
    );
    for pair in l.nodes.windows(2) {
        assert!(
            pair[1].rect.x >= pair[0].rect.right(),
            "boxes overlap after being widened: {} then {}",
            pair[0].rect.right(),
            pair[1].rect.x
        );
    }
}

#[test]
fn the_largest_wins_a_size_constraint() {
    // Shrinking to match a narrower neighbour clips the label, and a
    // constraint that makes text unreadable is worse than no constraint.
    let l = build(
        r#"node a "A"
           node b "A much longer label" below a
           same-width a b"#,
    );
    assert!(l.nodes[0].rect.w > 100.0, "the short box was not widened");
}

#[test]
fn align_group_moves_the_others_onto_the_first() {
    // The first named box decides, not an average: an average moves every
    // box including the one the others were being lined up against.
    let l = build(
        r#"node a "A" at (0, 0) { size: 80 40 }
           node b "B" at (120, 33) { size: 80 60 }
           node c "C" at (240, 71) { size: 80 40 }
           align-group top a b c"#,
    );
    assert_eq!(l.nodes[0].rect.y, 0.0, "the lead moved");
    assert_eq!(l.nodes[1].rect.y, 0.0);
    assert_eq!(l.nodes[2].rect.y, 0.0);
}

#[test]
fn a_constraint_never_reorders() {
    // The line `D-001` draws, and the whole reason this is a pass rather
    // than a solver.
    let l = build(
        r#"node a "A"
           node b "Long one" right-of a gap 40
           node c "C" right-of b gap 40
           same-width a b c
           align-group center a b c"#,
    );
    let xs: Vec<f64> = l.nodes.iter().map(|n| n.rect.x).collect();
    assert!(
        xs.windows(2).all(|w| w[0] < w[1]),
        "the declared order changed: {xs:?}"
    );
}

#[test]
fn a_constraint_needs_two_names() {
    let message = err("align-group a");
    assert!(message.contains("two names"), "{message}");
}

#[test]
fn constraints_survive_composition() {
    // Found the hard way: `merge` copied every other list and silently
    // dropped this one, so a constraint written in a file did nothing at all
    // and said nothing about it.
    let composed = compose_one(
        "constraints-compose",
        "node a \"A\"\nnode b \"A much longer label\" below a\nsame-width a b\n",
    );
    assert_eq!(composed.diagram.constraints.len(), 1);
    let l = crate::layout::solve(&composed.diagram).unwrap();
    assert!(
        l.nodes[0].rect.w > 100.0,
        "the constraint was lost on the way"
    );
}

// ------------------------------------------------- accessibility (I-040)

#[test]
fn a_described_box_carries_its_description_into_the_svg() {
    // `<title>` on the shape rather than a wrapping group: valid on any SVG
    // element, shown as a tooltip by a browser, read by a screen reader. One
    // line serves the reader with a mouse and the reader without sight, which
    // is why `I-040` was widened past accessibility.
    let svg = crate::render_source(
        r#"node cart "Cart" { description: "What the customer has chosen." }"#,
    )
    .unwrap();
    assert!(
        svg.contains("<title>What the customer has chosen.</title>"),
        "{svg}"
    );
}

#[test]
fn a_description_is_escaped_like_every_other_text() {
    let svg = crate::render_source(r#"node a "A" { description: "R&D <ops>" }"#).unwrap();
    assert!(svg.contains("R&amp;D &lt;ops&gt;"), "{svg}");
    assert!(!svg.contains("<ops>"), "unescaped markup reached the file");
}

#[test]
fn the_root_says_what_the_diagram_is() {
    // Without `role="img"` and a label an SVG is announced as a group of
    // anonymous shapes, or skipped entirely.
    let svg = crate::render_source(
        r#"graph { description: "How an order reaches the warehouse." }
           node a "A""#,
    )
    .unwrap();
    assert!(svg.contains(r#"role="img""#), "{svg}");
    assert!(
        svg.contains(r#"aria-label="How an order reaches the warehouse.""#),
        "{svg}"
    );
}

#[test]
fn the_label_falls_back_to_the_title_rather_than_asking_twice() {
    let svg = crate::render_source("graph { title: \"Checkout\" }\nnode a \"A\"").unwrap();
    assert!(svg.contains(r#"aria-label="Checkout""#), "{svg}");
}

#[test]
fn a_chart_is_labelled_by_its_own_title() {
    // A screen reader on a pie chart is the reader who needs the name most,
    // and a chart names itself in its own settings rather than in `graph`.
    let svg =
        crate::render_source("type pie { title: \"Traffic sources\" }\nslice a \"A\" 40").unwrap();
    assert!(svg.contains(r#"aria-label="Traffic sources""#), "{svg}");
}

#[test]
fn an_undescribed_diagram_gains_no_empty_attributes() {
    // A `role="img"` with nothing to say is worse than none: it promises a
    // screen reader a label and then gives it an empty string.
    let svg = crate::render_source("node a \"A\"").unwrap();
    assert!(!svg.contains("role="), "{svg}");
    assert!(!svg.contains("<title>"), "{svg}");
}
