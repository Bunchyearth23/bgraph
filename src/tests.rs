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

    // One corner, not two. With nothing in the way both are 400 points long,
    // and `W-105`'s per-corner price is what settles a tie the old mid-point
    // split decided by construction.
    let svg = render_svg::render(&l);
    assert!(svg.contains(r#"points="50,100 50,250 300,250""#), "{svg}");
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

/// The manual is illustrated by the compiler it documents.
///
/// There were nine `.svg` files beside their sources, and on the day the
/// renderer changed eight of the nine were quietly out of date — a document
/// describing one language and illustrating another, with nothing to say so.
/// It reads the `.bd` through bgraph's own Typst plugin now, so a diagram that
/// stops parsing stops the build (`W-138`).
#[test]
fn the_manual_draws_its_diagrams_rather_than_embedding_them() {
    let manual = include_str!("../docs/manual.typ");

    assert!(
        manual.contains(r#"#import "../typst/bgraph.typ""#),
        "the manual does not use the plugin"
    );
    assert!(
        !manual.contains(".svg\""),
        "the manual embeds a rendered picture again"
    );

    // Every source it reads has to be there, and every source there has to be
    // read — an orphan `.bd` in that directory is one nobody renders, so
    // nobody notices when it breaks.
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/diagrams"));
    let mut on_disk: Vec<String> = std::fs::read_dir(dir)
        .expect("docs/diagrams")
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();

    assert!(
        on_disk.iter().all(|n| n.ends_with(".bd")),
        "a rendered picture is checked in beside its source: {on_disk:?}"
    );
    for name in &on_disk {
        assert!(
            manual.contains(&format!("diagrams/{name}")),
            "`{name}` is in docs/diagrams and the manual never draws it"
        );
    }
    assert_eq!(
        manual.matches("read(\"diagrams/").count(),
        on_disk.len(),
        "the manual reads a source that is not there"
    );
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
    assert_eq!(
        genres, 29,
        "the manual says 29 specialised genres plus `free`"
    );
    assert!(
        manual.contains("29 specialised genres, plus `free`"),
        "the prose count"
    );
}

// ---------------------------------------------------------------------- W-102

#[cfg(feature = "bpmn")]
const PROCESS: &str = "type bpmn\n\
    event got \"Got it\" start message\n\
    task check \"Check\" service right-of got\n\
    gateway ok \"Ok?\" exclusive right-of check\n\
    task fix \"Fix\" send below ok\n\
    data form \"Form\" object below check\n\
    data book \"Ledger\" store below fix\n\
    event done \"Done\" end right-of ok\n\
    got -> check\ncheck -> ok\nok -> done\nok -> fix\nfix -> check\n\
    check -> form association\n";

#[cfg(feature = "bpmn")]
#[test]
fn each_bpmn_word_reads_its_own_positional_shape() {
    // `D-023`, and the defect finding it turned up: the free path resolved
    // positional values against the archetype's **primary** slots rather than
    // against the word the author wrote. `xy` was the only genre with a second
    // word and it is a chart, so it never came through there — `bpmn` has four
    // and is the first to prove the rule on the free path.
    let l = build(PROCESS);
    let by = |id: &str| {
        l.nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("`{id}` is placed"))
    };

    assert_eq!(by("got").style.stage.as_deref(), Some("start"));
    assert_eq!(by("got").style.trigger.as_deref(), Some("message"));
    assert_eq!(by("check").style.task_kind.as_deref(), Some("service"));
    assert_eq!(by("ok").style.gate.as_deref(), Some("exclusive"));
    assert_eq!(by("book").style.artifact.as_deref(), Some("store"));
    // Not read as the *task* slot, which is what the defect did.
    assert_eq!(by("ok").style.task_kind, None);
}

#[cfg(feature = "bpmn")]
#[test]
fn a_bpmn_shape_is_its_meaning_and_the_two_smallest_have_a_fixed_size() {
    let l = build(PROCESS);
    let by = |id: &str| l.nodes.iter().find(|n| n.id == id).expect("placed");

    assert_eq!(by("got").style.shape, Some(crate::style::Shape::Ellipse));
    assert_eq!(by("ok").style.shape, Some(crate::style::Shape::Diamond));
    assert_eq!(by("book").style.shape, Some(crate::style::Shape::Cylinder));
    assert_eq!(by("form").style.shape, Some(crate::style::Shape::Rect));

    // A ring and a diamond are sized by the notation, never by their label —
    // otherwise two lengths of text would read as two importances.
    assert_eq!(by("got").rect.w, crate::bpmn::EVENT);
    assert_eq!(by("got").rect.h, crate::bpmn::EVENT);
    assert_eq!(by("ok").rect.w, crate::bpmn::GATE);

    // And the label they cannot hold is drawn under them instead.
    let captions: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(captions.contains(&"Got it"), "{captions:?}");
    assert!(captions.contains(&"Ok?"), "{captions:?}");
    assert!(!captions.contains(&"Check"), "a task keeps its own label");
}

#[cfg(feature = "bpmn")]
#[test]
fn a_bpmn_mark_is_drawn_over_the_shape_it_is_inside() {
    // The whole point of the `over` flag. Under the box, a gateway's cross is
    // behind an opaque fill and simply is not there — which is how the first
    // draft of this rendered, and it looked like a blank diamond.
    let l = build(PROCESS);
    assert!(
        l.polylines.iter().any(|p| p.over),
        "no mark was queued to draw over the boxes"
    );
    assert!(
        l.polylines.iter().filter(|p| p.over).count() >= 6,
        "an event, a task, a gateway and a datum each carry one"
    );

    let svg = render_svg::render(&l);
    let boxes = svg.find("<rect").expect("boxes are drawn");
    let last_mark = svg.rfind("<polyline").expect("marks are drawn");
    assert!(
        last_mark > boxes,
        "a mark was emitted before the boxes, so it is behind them"
    );
}

#[cfg(feature = "bpmn")]
#[test]
fn a_boundary_event_sits_on_the_task_it_names() {
    // Declared placement (`D-001`): the author says which box, and the rule
    // for where on it is fixed rather than solved. Two on one task spread
    // along the edge in the order they are written, so the picture follows
    // the file.
    let l = build(
        "type bpmn\n\
         task pay \"Pay\" service at (0, 0) { size: 200 80 }\n\
         event late \"14 days\" boundary timer { attach: pay }\n\
         event err \"Declined\" boundary error { attach: pay }\n\
         task chase \"Chase\" at (0, 300)\n\
         late -> chase\n",
    );
    let by = |id: &str| l.nodes.iter().find(|n| n.id == id).expect("placed");
    let host = by("pay").rect;

    for id in ["late", "err"] {
        let e = by(id).rect;
        assert!(
            (e.cy() - host.bottom()).abs() < 0.01,
            "`{id}` is not on the bottom edge: {} vs {}",
            e.cy(),
            host.bottom()
        );
        assert!(
            e.cx() > host.x && e.cx() < host.right(),
            "`{id}` is off the box"
        );
    }
    assert!(
        by("late").rect.cx() < by("err").rect.cx(),
        "they spread in declaration order"
    );

    // And the host is not an obstacle for the event's own flow: the event sits
    // *on* it, so every route out of one starts inside a box. Routing round it
    // would be a detour around the very thing the event belongs to, and
    // reporting the crossing would be the validator complaining about the
    // notation working.
    assert_eq!(
        crate::check::after_layout(&l)
            .into_iter()
            .map(|d| d.message)
            .collect::<Vec<_>>(),
        Vec::<String>::new()
    );
}

#[cfg(feature = "bpmn")]
#[test]
fn a_bpmn_connector_says_what_it_means_with_its_line() {
    let l = build(PROCESS);
    let edge = |from: &str, to: &str| {
        let (a, b) = (
            l.nodes.iter().position(|n| n.id == from).expect("from"),
            l.nodes.iter().position(|n| n.id == to).expect("to"),
        );
        l.edges
            .iter()
            .find(|e| e.from == a && e.to == b)
            .expect("edge")
    };

    // A sequence flow is orthogonal and solid, because a process is read as a
    // circuit; an association is dotted, headless and straight, because it
    // crosses the page rather than running through the process.
    let seq = edge("got", "check");
    assert_eq!(seq.style.router, Some(crate::style::Router::Ortho));
    assert_eq!(seq.style.head, Some(crate::style::Head::Solid));
    assert_eq!(seq.style.dash, None);

    let assoc = edge("check", "form");
    assert_eq!(assoc.style.router, Some(crate::style::Router::Straight));
    assert_eq!(assoc.style.head, Some(crate::style::Head::None));
    assert_eq!(assoc.style.dash.as_deref(), Some("dotted"));
}

#[cfg(feature = "bpmn")]
#[test]
fn a_bpmn_property_under_the_wrong_word_is_refused_and_named() {
    // `D-005` applied between the four words rather than between genres: the
    // element that would be *drawn* is the one the properties describe, so a
    // `stage` on a task would silently make it an event. The message names the
    // word that does read it.
    let said =
        |source: &str| -> Vec<String> { diags(source).into_iter().map(|d| d.message).collect() };
    let has = |source: &str, want: &str| {
        let found = said(source);
        assert!(
            found.iter().any(|m| m.contains(want)),
            "expected {want:?} among {found:?}"
        );
    };

    has(
        "type bpmn\ntask t \"Do it\" { stage: start }\n",
        "`stage` belongs to `event`, and this is a `task`",
    );
    has(
        "type bpmn\ntask a \"A\"\ngateway g \"Where?\" exclusive { attach: a }\n",
        "`attach` belongs to `event`, and this is a `gateway`",
    );

    // A boundary event with nowhere to sit, and one sitting on the wrong kind
    // of thing.
    has(
        "type bpmn\nevent b \"Timeout\" boundary timer\n",
        "boundary event with no `attach`",
    );
    has(
        "type bpmn\ngateway g \"Where?\" exclusive\n\
         event b \"Timeout\" boundary timer { attach: g }\n",
        "which is a `gateway`",
    );
}

#[cfg(feature = "bpmn")]
#[test]
fn a_process_that_runs_backwards_is_a_warning_and_not_an_error() {
    // A fragment of a process is a legitimate thing to draw, so these are the
    // one family here that does not refuse — but a start event with something
    // flowing into it is still worth saying out loud.
    let said =
        |source: &str| -> Vec<String> { diags(source).into_iter().map(|d| d.message).collect() };
    let found = said(
        "type bpmn\nevent s \"Go\" start\ntask t \"Do\"\nevent e \"Stop\" end\n\
         s -> t\nt -> e\ne -> s\n",
    );
    assert!(
        found
            .iter()
            .any(|m| m.contains("start event with something flowing into it")),
        "{found:?}"
    );
    assert!(
        found
            .iter()
            .any(|m| m.contains("end event with something flowing out")),
        "{found:?}"
    );

    // A gateway that branches nothing is one too — it is a task wearing a
    // diamond, and the reader will look for a decision that is not there.
    let idle =
        said("type bpmn\ntask a \"A\"\ngateway g \"G?\" exclusive\ntask b \"B\"\na -> g\ng -> b\n");
    assert!(
        idle.iter().any(|m| m.contains("branches nothing")),
        "{idle:?}"
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
fn a_class_diagram_can_draw_the_three_relations_it_exists_for() {
    // `W-125`. `type class` could not draw a generalisation, an aggregation or
    // a composition — the three marks UML is *for* — and nobody had noticed,
    // because the word `triangle` was quietly a third spelling of the solid
    // arrow rather than the hollow head it names everywhere else.
    let l = build(
        "type class\nclass a \"A\"\nclass b \"B\" right-of a\n\
         a -> b { head: triangle }\n\
         a -> b { head: aggregation }\n\
         a -> b { head: composition }\n",
    );
    use crate::style::Head;
    assert_eq!(l.edges[0].style.head, Some(Head::Hollow));
    assert_eq!(l.edges[1].style.head, Some(Head::Diamond));
    assert_eq!(l.edges[2].style.head, Some(Head::Composed));

    // Three distinct markers reach the document, and the two hollow ones are
    // not painted in the line's own colour — a filled triangle is a different
    // relation from a hollow one, so they may not render alike.
    let svg = render_svg::render(&l);
    for id in ["hollow", "diamond", "composed"] {
        assert!(svg.contains(id), "no `{id}` marker in the document");
    }

    // And a mark can be put at the *tail*, which is where UML's diamond
    // belongs: the whole owns the part, not the other way round.
    let tail = build(
        "type class\nclass a \"A\"\nclass b \"B\" right-of a\n\
         a -> b { from-head: composition }\n",
    );
    assert_eq!(tail.edges[0].style.from_head, Some(Head::Composed));
}

#[test]
fn a_bend_rounds_an_elbow_and_a_curve_is_a_polyline_like_everything_else() {
    let square = build(
        "node a \"A\" at (0, 0) { size: 60 40 }\n\
         node b \"B\" at (300, 200) { size: 60 40 }\n\
         a -> b { router: ortho }\n",
    );
    let rounded = build(
        "node a \"A\" at (0, 0) { size: 60 40 }\n\
         node b \"B\" at (300, 200) { size: 60 40 }\n\
         a -> b { router: ortho bend: 16 }\n",
    );
    assert!(
        render_svg::route_all(&rounded)[0].points.len()
            > render_svg::route_all(&square)[0].points.len(),
        "a rounded corner is sampled into the line"
    );

    // `curved` is sampled too, and that is the point: the bounds, the label
    // solver, the PDF writer and the JSON export all keep working on the one
    // shape they already understand.
    let curved = build(
        "node a \"A\" at (0, 0) { size: 60 40 }\n\
         node b \"B\" at (300, 0) { size: 60 40 }\n\
         a -> b { router: curved }\n",
    );
    let path = &render_svg::route_all(&curved)[0].points;
    assert!(path.len() > 4, "a curve is many points: {}", path.len());
    let straight = path.iter().all(|(_, y)| (y - path[0].1).abs() < 0.5);
    assert!(!straight, "and it bows off the straight line");
}

#[test]
fn a_heading_goes_where_the_file_says() {
    // `W-128`. One setting for every genre: a chart names itself in its own
    // block and every other genre in `graph`, but *where* the name goes is the
    // same question either way, so it is asked once.
    let at = |align: &str| -> (f64, crate::layout::Anchor) {
        let l = build(&format!(
            "type pie {{ title: \"T\" }}\ngraph {{ title-align: {align} }}\n\
             slice a \"Organic search\" 4820\nslice b \"Direct\" 2410\n"
        ));
        let t = l
            .annotations
            .iter()
            .find(|a| a.title)
            .expect("the heading is marked, not guessed at");
        (t.x, t.anchor)
    };

    let (lx, la) = at("left");
    let (cx, ca) = at("center");
    let (rx, ra) = at("right");
    assert_eq!(la, crate::layout::Anchor::Start);
    assert_eq!(ca, crate::layout::Anchor::Middle);
    assert_eq!(ra, crate::layout::Anchor::End);
    assert!(lx < cx && cx < rx, "{lx} {cx} {rx}");

    // Left is the default *and* a no-op, which is why no checked-in drawing
    // moved when this arrived.
    let quiet = build("type pie { title: \"T\" }\nslice a \"A\" 1\n");
    let plain = quiet
        .annotations
        .iter()
        .find(|a| a.title)
        .expect("a heading");
    assert_eq!(plain.anchor, crate::layout::Anchor::Start);

    // And it works on a genre that is not a chart, since the whole point is
    // that the question is asked in one place.
    let free = build(
        "graph { title: \"T\" title-align: center }\n\
         node a \"A\" at (0, 0)\nnode b \"B\" at (400, 0)\n",
    );
    let t = free
        .annotations
        .iter()
        .find(|a| a.title)
        .expect("a heading");
    assert_eq!(t.anchor, crate::layout::Anchor::Middle);
}

#[test]
fn a_centred_heading_is_centred_on_everything_drawn() {
    // `W-133`. Boxes and text are not the drawing. A rack's rails and unit
    // rules are `Guide`s, so a bank whose right-hand racks are still empty
    // put its heading over the fourth rack: the box only knew about the two
    // racks holding equipment.
    let l = build(
        "type rack { title: \"Row A\" units: 8 cell: 120 16 }\n\
         graph { title-align: center }\n\
         group r1 \"R1\" {\n  device a \"A\" 1 2\n}\n\
         group r2 \"R2\" {\n}\n\
         group r3 \"R3\" {\n}\n",
    );
    let t = l.annotations.iter().find(|a| a.title).expect("a heading");
    let right = l
        .guides
        .iter()
        .map(|g| g.from.0.max(g.to.0))
        .fold(f64::MIN, f64::max);
    let left = l
        .guides
        .iter()
        .map(|g| g.from.0.min(g.to.0))
        .fold(f64::MAX, f64::min);

    // Not exactly the rails' midpoint — the U numbers hang to the left of the
    // leftmost rail — but well past the middle of the boxes, which is the
    // whole failure.
    let boxes = l.nodes.iter().map(|n| n.rect.cx()).fold(f64::MIN, f64::max);
    assert!(
        t.x > boxes && t.x > (left + right) * 0.5 - 20.0 && t.x < right,
        "heading at {t_x}, rails {left}..{right}, boxes end {boxes}",
        t_x = t.x
    );
}

#[test]
fn two_ortho_lines_do_not_run_along_each_other() {
    // `X-018`. The router avoided boxes and not other lines, so two edges whose
    // tidiest routes crossed the same tall obstacle both took the one channel
    // over it and a reader saw a single line for the length of the overlap.
    //
    // Three edges, one obstacle they must all get past, and a floor below so
    // the space is genuinely contended.
    let source = "node a1 \"A1\" at (0, 0) { size: 90 50 }\n\
                  node a2 \"A2\" at (0, 90) { size: 90 50 }\n\
                  node a3 \"A3\" at (0, 180) { size: 90 50 }\n\
                  node wall \"W\" at (220, -60) { size: 90 360 }\n\
                  node b1 \"B1\" at (440, 0) { size: 90 50 }\n\
                  node b2 \"B2\" at (440, 90) { size: 90 50 }\n\
                  node b3 \"B3\" at (440, 180) { size: 90 50 }\n\
                  node floor \"F\" at (0, 340) { size: 530 40 }\n\
                  style edge { router: ortho }\n\
                  a1 -> b1\na2 -> b2\na3 -> b3\n";
    let l = build(source);
    let routes = render_svg::route_all(&l);

    // How much any two of them run along each other, collinear.
    let mut shared = 0.0_f64;
    for i in 0..routes.len() {
        for j in (i + 1)..routes.len() {
            for p in routes[i].points.windows(2) {
                for q in routes[j].points.windows(2) {
                    let span = |a: f64, b: f64, c: f64, d: f64| {
                        (a.max(b).min(c.max(d)) - a.min(b).max(c.min(d))).max(0.0)
                    };
                    if (p[0].1 - p[1].1).abs() < 0.01
                        && (q[0].1 - q[1].1).abs() < 0.01
                        && (p[0].1 - q[0].1).abs() < 0.01
                    {
                        shared += span(p[0].0, p[1].0, q[0].0, q[1].0);
                    }
                    if (p[0].0 - p[1].0).abs() < 0.01
                        && (q[0].0 - q[1].0).abs() < 0.01
                        && (p[0].0 - q[0].0).abs() < 0.01
                    {
                        shared += span(p[0].1, p[1].1, q[0].1, q[1].1);
                    }
                }
            }
        }
    }
    assert!(
        shared < 1.0,
        "{shared} points of line run along another line"
    );

    // And no run passes nearer a box than the clearance — which is what the
    // clearance was always meant to mean, and what the lanes this fix creates
    // would otherwise have quietly broken.
    for route in &routes {
        for seg in route.points.windows(2) {
            for node in l.nodes.iter().filter(|n| n.id == "wall" || n.id == "floor") {
                let r = node.rect;
                let m = 10.0;
                let hit = seg[0].0.max(seg[1].0) > r.x - m
                    && seg[0].0.min(seg[1].0) < r.right() + m
                    && seg[0].1.max(seg[1].1) > r.y - m
                    && seg[0].1.min(seg[1].1) < r.bottom() + m;
                assert!(!hit, "a run hugs `{}`: {seq:?}", node.id, seq = seg);
            }
        }
    }
}

#[test]
fn two_lines_between_the_same_pair_do_not_lie_on_each_other() {
    // `X-018`, narrowed. Direction is ignored: `a -> b` and `b -> a` share a
    // corridor, and two lines drawn on top of each other are two lines a
    // reader sees as one however they were written.
    let l = build(
        "node a \"A\" at (0, 0) { size: 60 40 }\n\
         node b \"B\" at (300, 0) { size: 60 40 }\n\
         a -> b \"there\"\nb -> a \"back\"\n",
    );
    let routes = render_svg::route_all(&l);
    let mid = |r: &render_svg::Routed| r.points[r.points.len() / 2].1;
    assert!(
        (mid(&routes[0]) - mid(&routes[1])).abs() > 8.0,
        "they bow apart: {} and {}",
        mid(&routes[0]),
        mid(&routes[1])
    );

    // One line to itself keeps the straight run it always had.
    let alone = build(
        "node a \"A\" at (0, 0) { size: 60 40 }\n\
         node b \"B\" at (300, 0) { size: 60 40 }\n\
         a -> b\n",
    );
    assert_eq!(render_svg::route_all(&alone)[0].points.len(), 2);
}

#[test]
fn a_line_goes_through_every_place_it_is_told_to() {
    // `D-034`. A list, not a point — and each stop is parenthesised, which is
    // what lets the list need no separator, since `,` already separates
    // statements.
    let l = build(
        "node a \"A\" at (0, 0) { size: 80 60 }\n\
         node b \"B\" at (500, 0) { size: 80 60 }\n\
         node c \"C\" at (240, 300) { size: 80 60 }\n\
         a -> b { router: ortho } via (240, 200) (left-of c gap 20)\n",
    );
    let path = &render_svg::route_all(&l)[0].points;
    let near = |p: (f64, f64)| {
        path.iter()
            .any(|q| (q.0 - p.0).abs() < 0.5 && (q.1 - p.1).abs() < 0.5)
    };

    assert!(near((240.0, 200.0)), "the absolute stop: {path:?}");
    let c = l.nodes[2].rect;
    assert!(
        near((c.x - 20.0, c.cy())),
        "and the one beside a box: {path:?}"
    );

    // A grid cell is the third form, and it is free: `at` already reads one.
    let g = build(
        "graph { grid: 100 50 }\n\
         node a \"A\" at (0, 0) { size: 40 30 }\n\
         node b \"B\" at (500, 0) { size: 40 30 }\n\
         a -> b via (col 3, row 4)\n",
    );
    let path = &render_svg::route_all(&g)[0].points;
    assert!(
        path.iter()
            .any(|q| (q.0 - 300.0).abs() < 0.5 && (q.1 - 200.0).abs() < 0.5),
        "the cell is multiplied by the declared step: {path:?}"
    );
}

#[test]
fn a_waypoint_is_a_passage_and_the_engine_says_when_it_cannot_be_one() {
    // The passage is not negotiable, so what the engine owes the author is to
    // say when the place he named is inside something — repairing it would
    // move the line he asked for, which is the bargain `X-004` struck.
    let inside = after(
        "node a \"A\" at (0, 0) { size: 60 40 }\n\
         node m \"M\" at (200, 0) { size: 100 100 }\n\
         node b \"B\" at (400, 0) { size: 60 40 }\n\
         a -> b via (230, 30)\n",
    );
    assert!(
        inside.iter().any(|m| m.contains("is inside `m`")),
        "{inside:?}"
    );

    // And an anchor that does not exist is an error, not a line through the
    // origin — the silent no-op `D-005` refuses.
    let e = err("node a \"A\" at (0, 0)\nnode b \"B\" at (300, 0)\na -> b via (left-of nope)\n");
    assert!(e.contains("unknown node `nope` in a waypoint"), "{e}");
}

#[test]
fn an_edge_attaches_where_it_is_told_to() {
    // `D-033`. Placement was declared and attachment was not, so an author
    // could say where every box goes and still not stop two heads landing on
    // the same three pixels.
    let l = build(
        "node a \"A\" at (0, 0) { size: 100 100 }\n\
         node b \"B\" at (300, 300) { size: 100 100 }\n\
         a -> b { router: ortho from: bottom to: left }\n",
    );
    let path = &render_svg::route_all(&l)[0].points;
    let (a, b) = (l.nodes[0].rect, l.nodes[1].rect);

    let start = path[0];
    let end = *path.last().expect("a routed edge has an end");
    assert!(
        (start.1 - a.bottom()).abs() < 0.5,
        "it leaves by the bottom: {start:?}"
    );
    assert!(
        (end.0 - b.x).abs() < 0.5,
        "and arrives at the left: {end:?}"
    );
}

#[test]
fn lines_arriving_at_one_side_share_it_instead_of_stacking() {
    // The half of `D-033` nobody has to write a word for. Two heads at one
    // point is what the author reported; two *tails* at one point is the same
    // ambiguity seen from the other end, so both spread.
    let l = build(
        "node hub \"Hub\" at (400, 0) { size: 120 200 }\n\
         node p \"P\" at (0, 0) { size: 80 60 }\n\
         node q \"Q\" at (0, 100) { size: 80 60 }\n\
         p -> hub\nq -> hub\n",
    );
    let routes = render_svg::route_all(&l);
    let (first, second) = (
        *routes[0].points.last().unwrap(),
        *routes[1].points.last().unwrap(),
    );
    assert!(
        (first.1 - second.1).abs() > 10.0,
        "two arrivals on one side are apart: {first:?} and {second:?}"
    );

    // A line arriving and a line leaving are **not** in each other's way — a
    // head and a tail at one point read perfectly well — so separating them
    // would only bend two lines that were straight.
    let l = build(
        "node a \"A\" at (0, 0) { size: 80 60 }\n\
         node m \"M\" at (200, 0) { size: 80 60 }\n\
         node z \"Z\" at (400, 0) { size: 80 60 }\n\
         a -> m\nm -> z\n",
    );
    let routes = render_svg::route_all(&l);
    let into_m = *routes[0].points.last().unwrap();
    let out_of_m = routes[1].points[0];
    let m = l.nodes[1].rect;
    assert!((into_m.1 - m.cy()).abs() < 0.5, "{into_m:?}");
    assert!((out_of_m.1 - m.cy()).abs() < 0.5, "{out_of_m:?}");
}

#[test]
fn an_attachment_lands_on_the_outline_and_not_on_the_bounding_box() {
    // A hexagon's widest point is at mid-height, so a port two thirds of the
    // way down its right-hand side sits *outside* the shape and the line
    // starts in mid-air — which is what `type network` drew the first time
    // spreading ran.
    let l = build(
        "node hex \"Hex\" at (0, 0) { size: 200 120 shape: hexagon }\n\
         node p \"P\" at (400, -80) { size: 80 60 }\n\
         node q \"Q\" at (400, 80) { size: 80 60 }\n\
         hex -> p\nhex -> q\n",
    );
    let hexagon = l.nodes[0].rect;
    for route in render_svg::route_all(&l) {
        let start = route.points[0];
        assert!(
            start.0 < hexagon.right() - 1.0,
            "a hexagon is narrower than its box away from mid-height: {start:?}"
        );
    }
}

#[test]
fn a_label_wider_than_its_gap_is_reported_rather_than_repaired() {
    // `W-113`. Nodes are painted after labels, so a label that does not fit
    // between the two boxes it runs between is not truncated — it is
    // overpainted, and the text is simply gone. That is what `[funds cleared]`
    // looked like in the `state` example.
    //
    // The engine says so instead of fixing it, because both fixes are the
    // author's: widening a gap **moves a box**, which `D-001` forbids it to do.
    // Tall boxes, because the label solver *does* slide a label off a short
    // one — this is the case it cannot escape, and the case that reaches a
    // reader.
    let tight = after(
        "node a \"A\" at (0, 0) { size: 60 200 }\n\
         node b \"B\" right-of a gap 20 { size: 60 200 }\n\
         a -> b \"a rather long transition label\"\n",
    );
    assert!(
        tight.iter().any(|m| m.contains("is drawn over")),
        "{tight:?}"
    );
    assert!(
        tight.iter().any(|m| m.contains("label-offset")),
        "the message names a way out: {tight:?}"
    );

    // Room for it, and nothing is said. The diagnostic fires on one example of
    // thirty and on none of the other twenty-nine, which is what makes it worth
    // having rather than worth turning off.
    let roomy = after(
        "node a \"A\" at (0, 0) { size: 60 200 }\n\
         node b \"B\" right-of a gap 320 { size: 60 200 }\n\
         a -> b \"a rather long transition label\"\n",
    );
    assert_eq!(roomy, Vec::<String>::new());
}

#[test]
fn an_ortho_edge_goes_round_the_box_it_used_to_go_through() {
    // `X-004`, both halves. This case is what `W-064` added the diagnostic for
    // — a line drawn straight through `mid` and saying nothing about it — and
    // since `W-105` the router goes over the top instead, so there is nothing
    // left to report. The diagnostic did not become wrong; it became rare.
    let source = "node a \"A\" at (0, 0)\nnode mid \"in the way\" right-of a gap 40\n\
                  node b \"B\" right-of mid gap 40\na -> b { router: ortho }";
    assert_eq!(after(source), Vec::<String>::new());

    let l = build(source);
    let path = &render_svg::route_all(&l)[0].points;
    let wall = l.nodes[1].rect;
    for seg in path.windows(2) {
        let (p, q) = (seg[0], seg[1]);
        assert!(
            !(p.0.max(q.0) > wall.x
                && p.0.min(q.0) < wall.right()
                && p.1.max(q.1) > wall.y
                && p.1.min(q.1) < wall.bottom()),
            "the route still enters `mid`: {path:?}"
        );
    }
    assert!(path.len() > 2, "going round takes a corner: {path:?}");
}

#[test]
fn a_routed_path_is_the_same_every_time_it_is_asked_for() {
    // The determinism the golden test rests on, asserted where it is decided
    // rather than three files downstream. `W-065` shipped a diagram that
    // rendered to different bytes run to run because one chart iterated a
    // `HashMap`; the search in `ortho.rs` has integer costs and settles a tie
    // by state index for exactly that reason, and nothing here may reintroduce
    // an unordered container without this failing.
    let source = "node a \"A\" at (0, 0)\nnode w \"wall\" at (200, -40) { size: 60 200 }\n\
                  node b \"B\" at (400, 0)\nnode c \"C\" at (400, 300)\n\
                  a -> b { router: ortho }\na -> c { router: ortho }";
    let first = render_svg::render(&build(source));
    for _ in 0..8 {
        assert_eq!(render_svg::render(&build(source)), first);
    }

    // And the obstacles are read as a set, not a sequence: the same boxes
    // declared in another order give the same line.
    let swapped = "node a \"A\" at (0, 0)\nnode b \"B\" at (400, 0)\n\
                   node c \"C\" at (400, 300)\nnode w \"wall\" at (200, -40) { size: 60 200 }\n\
                   a -> b { router: ortho }\na -> c { router: ortho }";
    let l = build(swapped);
    let path: Vec<_> = render_svg::route_all(&l)[0].points.clone();
    let l0 = build(source);
    assert_eq!(path, render_svg::route_all(&l0)[0].points);
}

#[test]
fn a_route_that_cannot_be_found_keeps_the_elbow_and_says_so() {
    // The router declines past `MAX_OBSTACLES`, because the grid grows as the
    // square of the box count and is built once per edge. What matters is what
    // happens *then*: the old elbow is kept and `check::after_layout` reports
    // the crossing, so the drawing admits it is wrong. Dropping the edge would
    // be a drawing that lies, which is the worse of the two.
    let mut source = String::from("node a \"A\" at (0, 0)\n");
    source.push_str("node mid \"in the way\" at (200, 0) { size: 120 60 }\n");
    source.push_str("node b \"B\" at (500, 0)\n");
    // Well clear of the line from `a` to `b`, so the only box in the way is
    // `mid` and the count is the only thing that changed.
    for i in 0..70 {
        source.push_str(&format!("node f{i} \"f\" at (0, {})\n", 400 + i * 40));
    }
    source.push_str("a -> b { router: ortho }\n");

    let found = after(&source);
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
    for (genre, source) in crate::examples::EXAMPLES {
        assert!(after(source).is_empty(), "{genre}: {:?}", after(source));
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

// ---------------------------------------------------------------------- W-104

#[test]
fn the_migration_moves_opacity_onto_every_paint_in_its_block() {
    // `X-008`. The whole point is that this is surgical: the alpha digits go
    // into the colour where it stands, the `opacity` line is cut out, and the
    // comment and the hand alignment either side of it are untouched.
    let before = "style card {\n  fill:    #1e293b      // aligned by hand\n  \
                  stroke:  #334155 1\n  opacity: 0.5\n}\n\
                  node a \"A\" { fill: #38bdf8 opacity: 0.25 }\n";
    let after = crate::fmt::migrate(before).expect("migrates");

    assert_eq!(
        after,
        "style card {\n  fill:    #1e293b80      // aligned by hand\n  \
         stroke:  #33415580 1\n}\n\
         node a \"A\" { fill: #38bdf840 }\n",
        "{after:?}"
    );

    // And the file it produces is one the language accepts, where the file it
    // was given is not — which is the only reason the command exists.
    assert!(err(before).contains("unknown property `opacity`"));
    let migrated = parse_diagram(&after).expect("parses");
    for diag in crate::check::check(&migrated) {
        assert!(
            !diag.message.contains("unknown property"),
            "the migrated file still has an unknown property: {}",
            diag.message
        );
    }
}

#[test]
fn the_migration_multiplies_an_alpha_it_finds_and_drops_one_it_does_not_need() {
    // An element opacity and a paint alpha composited, so a colour that
    // already carried one keeps what it meant rather than losing it.
    let half = crate::fmt::migrate("node a { fill: #11223380 opacity: 0.5 }\n").expect("migrates");
    assert!(half.contains("#11223340"), "{half}");

    // Fully opaque is written as no alpha at all: `ff` is the default, and
    // `split_alpha` reads the two forms back the same way.
    let whole = crate::fmt::migrate("node a { fill: #112233 opacity: 1 }\n").expect("migrates");
    assert_eq!(whole, "node a { fill: #112233 }\n", "{whole:?}");

    // A short colour is expanded rather than given a one-digit alpha, because
    // one hex digit cannot hold 0.5 and a migration may not change the value
    // it was asked to preserve.
    let short = crate::fmt::migrate("node a { fill: #123 opacity: 0.5 }\n").expect("migrates");
    assert_eq!(short, "node a { fill: #11223380 }\n", "{short:?}");
}

#[test]
fn the_migration_is_idempotent_and_leaves_a_clean_file_alone() {
    let done = "node a { fill: #11223380 }\n";
    assert_eq!(crate::fmt::migrate(done).expect("migrates"), done);

    let once = crate::fmt::migrate("node a { fill: #112233 opacity: 0.5 }\n").expect("migrates");
    assert_eq!(crate::fmt::migrate(&once).expect("migrates"), once);
}

#[test]
fn the_migration_declines_rather_than_inventing_a_colour() {
    // Both refusals are the same principle: this command may move a value, it
    // may not decide one. A `#ffffff` conjured to carry an alpha would have
    // changed the drawing while claiming to preserve it.
    let none = crate::fmt::migrate("node a \"A\" {\n  size: 10 10\n  opacity: 0.5\n}\n")
        .expect_err("nothing to ride on");
    let none = format!("{none:#}");
    assert!(none.contains("line 3"), "{none}");
    assert!(none.contains("nothing to ride on"), "{none}");

    let named = crate::fmt::migrate("node a \"A\" {\n  fill: tomato\n  opacity: 0.5\n}\n")
        .expect_err("a name has no fourth channel");
    let named = format!("{named:#}");
    assert!(named.contains("line 2"), "{named}");
    assert!(named.contains("tomato"), "{named}");

    let silly =
        crate::fmt::migrate("node a { fill: #112233 opacity: 50 }\n").expect_err("not a fraction");
    assert!(format!("{silly:#}").contains("not a fraction"), "{silly:#}");
}

#[test]
fn the_migration_reads_the_source_and_not_a_label_that_looks_like_one() {
    // The scan is lexical, so it has to survive the two things a lexical scan
    // always gets wrong: a `#` inside a string and a property inside a
    // comment. Neither is a declaration, and treating one as such would edit
    // a label or delete a note.
    let source = "node a \"fill: #ffffff opacity: 0.5\" { fill: #112233 opacity: 0.5 }\n\
                  // opacity: 0.9 — a note about the old property\n";
    let after = crate::fmt::migrate(source).expect("migrates");
    assert_eq!(
        after,
        "node a \"fill: #ffffff opacity: 0.5\" { fill: #11223380 }\n\
         // opacity: 0.9 — a note about the old property\n",
        "{after:?}"
    );
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
    for (genre, source) in crate::examples::EXAMPLES {
        assert_eq!(
            crate::fmt::format(source).unwrap_or_else(|e| panic!("{genre}: {e:#}")),
            *source,
            "the `{genre}` example is not in canonical shape — it is what \
             `bgraph init {genre}` prints, so it has to be"
        );
    }
}

// ---------------------------------------------------------------------- W-066

#[test]
fn the_viewer_shows_a_picture_it_did_not_draw() {
    // An `.svg` is already the picture. Serving it as it is means the viewer
    // works on a file bgraph never wrote — and since the reload is driven by
    // the file's timestamp, it follows another tool regenerating it.
    // The picture is one this test writes, not one borrowed from a directory
    // of samples — and it is deliberately *not* something bgraph would emit,
    // which is the whole claim being made (`W-116`).
    let dir = scratch("viewer");
    let hand_drawn = "<svg xmlns=\"http://www.w3.org/2000/svg\"><!-- drawn elsewhere --></svg>\n";
    let svg = dir.join("hand.svg");
    std::fs::write(&svg, hand_drawn).unwrap();

    let served = crate::watch::render_source(&svg, None).expect("serves");
    assert_eq!(served, hand_drawn, "verbatim, not re-rendered");

    // And a source is still rendered, so one command covers both.
    let source = dir.join("a.bd");
    std::fs::write(&source, "node a \"A\"\n").unwrap();
    let rendered = crate::watch::render_source(&source, None).expect("renders");
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
fn the_raster_is_the_only_dependency_and_it_is_opt_in() {
    // The whole reason the SVG writer, the HTTP preview and the PDF writer are
    // hand-rolled: each is a format with a short honest subset, and a raster
    // is the one that is not. So it is the one thing behind a feature, and a
    // manifest that quietly turned it on would undo every one of those
    // decisions at once.
    //
    // There was a `window` feature beside it once, a native viewer on
    // `minifb`. It is gone, and so is that crate — a browser is on every
    // machine this runs on (`W-136`).
    let manifest = include_str!("../Cargo.toml");

    let defaults = manifest
        .split_once("default = [")
        .and_then(|(_, tail)| tail.split_once(']'))
        .expect("a default feature list")
        .0;
    for gate in ["window", "png"] {
        assert!(
            !defaults.contains(gate),
            "`{gate}` is on by default: {defaults}"
        );
    }
    assert!(
        !manifest
            .lines()
            .any(|l| l.trim_start().starts_with("minifb")),
        "the windowing dependency is back"
    );
    assert!(
        !manifest.lines().any(|l| l.starts_with("window = ")),
        "the window feature is back"
    );

    for crate_name in ["resvg", "tiny-skia"] {
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
    let dir = scratch("preview-data");
    std::fs::write(
        dir.join("report.bd"),
        "type pie { title: \"{{quarter}}\" }\n\
         slice {{source.id}} \"{{source.name}}\" {{source.value}}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("report.json"),
        r#"{"quarter": "Q3", "source": [{"id": "organic", "name": "Organic search", "value": 4820}]}"#,
    )
    .unwrap();
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
    for (genre, source) in crate::examples::EXAMPLES {
        let pdf = crate::render_source_pdf(source).unwrap_or_else(|e| panic!("{genre}: {e:#}"));
        assert!(pdf.starts_with(b"%PDF"), "{genre}");
    }
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
fn a_treeview_guide_is_never_drawn_behind_a_box() {
    // `W-103`. The links used to be diagonals from the parent's right edge to
    // the child's left, which on indented rows crossed every row between them
    // and ran *behind* the boxes — nodes are painted after polylines, so the
    // structure of the tree was hidden by the tree.
    //
    // The fix is a property rather than a nicer picture: the indent reserves a
    // column for the vertical guide, so a link cannot land behind a box because
    // the space is not the boxes' to occupy. This asserts the property, on a
    // deep tree rather than the shared fixture — the failure needs a row
    // between a parent and its last child to appear at all.
    let deep = "node a \"A\"\nnode b \"B\"\nnode c \"C\"\nnode d \"D\"\nnode e \"Eeeeeeee\"\n\
                a -> b\nb -> c\nc -> d\na -> e\n";
    let l = build(&format!("type treeview {{ row: 24 spacing: 12 }}\n{deep}"));

    for line in &l.polylines {
        for pair in line.points.windows(2) {
            let (x0, y0) = pair[0];
            let (x1, y1) = pair[1];
            for node in &l.nodes {
                let r = node.rect;
                // Interior overlap only: an elbow is *meant* to touch the left
                // edge of the row it points at, and a trunk leaves from under
                // its parent. A tolerance keeps a shared boundary from reading
                // as a crossing.
                let t = 0.01;
                let hit = x0.min(x1) < r.right() - t
                    && x0.max(x1) > r.x + t
                    && y0.min(y1) < r.bottom() - t
                    && y0.max(y1) > r.y + t;
                assert!(
                    !hit,
                    "a guide from ({x0}, {y0}) to ({x1}, {y1}) runs through {} at ({}, {}) {}×{}",
                    node.id, r.x, r.y, r.w, r.h
                );
            }
        }
    }

    // And the trunk stops at the last child rather than running on: without
    // this the property above would hold trivially for a tree whose guides all
    // ran off the bottom of the drawing.
    let lowest = l
        .nodes
        .iter()
        .map(|n| n.rect.bottom())
        .fold(0.0_f64, f64::max);
    let deepest = l
        .polylines
        .iter()
        .flat_map(|p| p.points.iter().map(|(_, y)| *y))
        .fold(0.0_f64, f64::max);
    assert!(
        deepest < lowest,
        "a guide reaches {deepest}, past the last row at {lowest}"
    );
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
    // A device spans the full width between the rails, so no unit rule shows
    // past it, and is inset vertically so one unit reads apart from the next
    // (`W-123`). The height it occupies is still the units it spans.
    const GAP: f64 = 2.0;
    assert_eq!(l.nodes[0].rect.h, 20.0 - GAP * 2.0);
    assert_eq!(
        l.nodes[1].rect.h,
        80.0 - GAP * 2.0,
        "U5 through U8 inclusive is four"
    );
    assert_eq!(
        l.nodes[0].rect.w, 200.0,
        "rail to rail, so it hides the rules"
    );
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

// ---------------------------------------------------------------------- W-132

#[test]
fn a_group_is_a_rack_and_a_bank_is_a_row_of_them() {
    let l = build(
        "type rack { units: 10 cell: 200 20 ruler: off }\n\
         group a1 \"A1\" {\n  device x \"X\" 10 10\n}\n\
         group a2 \"A2\" {\n  device y \"Y\" 10 10\n}\n",
    );

    assert_eq!(l.nodes.len(), 2);
    let (x, y) = (&l.nodes[0], &l.nodes[1]);
    assert_eq!(x.rect.y, y.rect.y, "U10 is U10 in both racks");
    assert!(
        x.rect.x + x.rect.w <= y.rect.x,
        "the second rack stands beside the first, not on it: {x:?} {y:?}",
        x = x.rect,
        y = y.rect
    );

    // Each rack is named over its own rails, not in a caption somewhere.
    let named: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(named.contains(&"A1") && named.contains(&"A2"), "{named:?}");
}

#[test]
fn two_racks_may_both_have_a_device_in_the_same_unit() {
    // The ordinary case for a row of racks, and an error for one rack. The
    // checker can only tell them apart by reading the frames.
    let d = diags(
        "type rack { units: 10 }\n\
         group a1 \"A1\" {\n  device x \"X\" 10 10\n}\n\
         group a2 \"A2\" {\n  device y \"Y\" 10 10\n}\n",
    );
    assert!(!d.iter().any(|x| x.message.contains("overlap")), "{d:?}");

    // Inside one rack it is still a collision, and the message still names it.
    let e = err("type rack { units: 10 }\n\
         group a1 \"A1\" {\n  device x \"X\" 9 10\n  device y \"Y\" 10 10\n}\n");
    assert!(e.contains("cannot share a unit"), "{e}");
}

#[test]
fn a_rack_may_be_shorter_than_the_bank_and_stands_on_the_same_floor() {
    let l = build(
        "type rack { units: 10 cell: 200 20 ruler: off }\n\
         group tall \"Tall\" {\n  device t \"T\" 1 1\n}\n\
         group short \"Short\" { units: 4\n  device s \"S\" 1 1\n}\n",
    );
    let (t, s) = (&l.nodes[0], &l.nodes[1]);
    assert_eq!(t.rect.y, s.rect.y, "U1 is the floor, and racks stand on it");

    // And U5 does not exist in a 4U rack.
    let e = err("type rack { units: 10 }\n\
         group short \"Short\" { units: 4\n  device s \"S\" 5 5\n}\n");
    assert!(e.contains("past the top of `Short` of a 4U frame"), "{e}");
}

#[test]
fn a_rack_file_with_no_group_is_the_single_rack_it_always_was() {
    // The compatibility hinge: the bank is drawn by the same code, and a file
    // that declares no rack is one unnamed rack holding everything.
    let l = build(
        "type rack { units: 10 cell: 200 20 ruler: off }\n\
         device a \"A\" 1 1\ndevice b \"B\" 5 5\n",
    );
    assert_eq!(l.nodes.len(), 2);
    assert_eq!(l.nodes[0].rect.x, l.nodes[1].rect.x, "one frame");
    assert!(
        !l.annotations.iter().any(|a| !a.text.parse::<u32>().is_ok()),
        "nothing is captioned over a rack nobody named"
    );

    // A device left outside the groups gets a frame of its own rather than
    // being dropped on the floor.
    let l = build(
        "type rack { units: 10 cell: 200 20 ruler: off }\n\
         device loose \"Loose\" 1 1\n\
         group a1 \"A1\" {\n  device x \"X\" 1 1\n}\n",
    );
    assert_eq!(l.nodes.len(), 2);
    assert_ne!(l.nodes[0].rect.x, l.nodes[1].rect.x);
}

#[test]
fn units_on_a_device_says_so_rather_than_looking_like_it_works() {
    let d = diags("type rack { units: 10 }\ndevice a \"A\" 1 1 { units: 4 }\n");
    assert!(
        d.iter()
            .any(|x| x.message.contains("it is the height of a rack")),
        "{d:?}"
    );
}

// ---------------------------------------------------------------------- W-134

#[test]
fn a_chart_s_own_furniture_is_styled_by_the_diagram_not_by_its_items() {
    // The heading used to take the colour of whatever the genre calls its
    // items — in a rack, `style device`, which is the one thing on the drawing
    // it is certainly not.
    let l = build(
        "type rack { title: \"T\" units: 4 cell: 120 16 }\n\
         style device { font: 11 #ff0000 }\n\
         device a \"A\" 1 1\n",
    );
    let head = l.annotations.iter().find(|a| a.title).expect("a heading");
    assert_ne!(head.color, "#ff0000", "the heading is not a device");

    // And `style title` is what does colour it.
    let l = build(
        "type rack { title: \"T\" units: 4 cell: 120 16 }\n\
         style device { font: 11 #ff0000 }\n\
         style title { font: 20 #00ff00 }\n\
         device a \"A\" 1 1\n",
    );
    let head = l.annotations.iter().find(|a| a.title).expect("a heading");
    assert_eq!(head.color, "#00ff00");
    assert_eq!(head.size, 20.0);
}

#[test]
fn style_grid_paints_the_rules_and_the_numbers_along_them() {
    // There was no way to recolour a rack's unit rules at all: they were the
    // theme's, and the theme is decided by the background.
    let plain = build("type rack { units: 4 cell: 120 16 }\ndevice a \"A\" 1 1\n");
    let styled = build(
        "type rack { units: 4 cell: 120 16 }\n\
         style grid { stroke: #123456 2 dash: dotted font: 7 #654321 }\n\
         device a \"A\" 1 1\n",
    );

    assert!(plain.guides.iter().all(|g| g.color != "#123456"));
    let rules: Vec<&crate::layout::Guide> = styled
        .guides
        .iter()
        .filter(|g| g.color == "#123456")
        .collect();
    assert!(!rules.is_empty(), "the unit rules take the grid's stroke");
    assert!(rules.iter().all(|g| g.width == 2.0));
    assert!(rules.iter().all(|g| g.dash.as_deref() == Some("dotted")));

    let ticks: Vec<&crate::layout::Annotation> = styled
        .annotations
        .iter()
        .filter(|a| a.text.parse::<u32>().is_ok())
        .collect();
    assert!(!ticks.is_empty());
    assert!(ticks.iter().all(|a| a.color == "#654321" && a.size == 7.0));
}

#[test]
fn a_theme_says_what_it_does_not_read() {
    let d = diags("type rack { units: 4 }\nstyle grid { fill: #f00 }\ndevice a \"A\" 1 1\n");
    assert!(
        d.iter()
            .any(|x| x.message.contains("no effect on `style grid`")),
        "{d:?}"
    );

    // And what it does read is accepted in silence.
    let d = diags("type rack { units: 4 }\nstyle grid { stroke: #f00 }\ndevice a \"A\" 1 1\n");
    assert!(!d.iter().any(|x| x.message.contains("no effect")), "{d:?}");
}

#[test]
fn a_style_that_paints_nothing_says_so() {
    // `style device` is a theme in a rack and dead text in a pie, and the
    // difference was invisible.
    let d = diags(
        "type pie\nstyle device { fill: #f00 }\nslice a \"A\" 1\nslice b \"B\" 2\nslice c \"C\" 3\n",
    );
    let m = d
        .iter()
        .find(|x| x.message.contains("paints nothing here"))
        .unwrap_or_else(|| panic!("{d:?}"));
    assert!(m.message.contains("themes `type rack`"), "{}", m.message);
    assert!(
        m.message.contains("calls its items `slice`"),
        "{}",
        m.message
    );

    // A class nobody applies is the milder version of the same thing.
    let d = diags("style warn { fill: #f00 }\nnode a \"A\"\n");
    assert!(
        d.iter().any(|x| x.message.contains("never applied")),
        "{d:?}"
    );

    // Applied, extended, or a theme — all silent.
    for quiet in [
        "style warn { fill: #f00 }\nnode a \"A\" .warn\n",
        "style warn { fill: #f00 }\nstyle worse extends warn { }\nnode a \"A\" .worse\n",
        "style node { fill: #f00 }\nnode a \"A\"\n",
        "style grid { stroke: #f00 }\nnode a \"A\"\n",
        "type rack { units: 4 }\nstyle device { fill: #f00 }\ndevice a \"A\" 1 1\n",
    ] {
        let d = diags(quiet);
        assert!(
            !d.iter().any(
                |x| x.message.contains("paints nothing") || x.message.contains("never applied")
            ),
            "{quiet:?} → {d:?}"
        );
    }
}

#[test]
fn every_theme_reads_properties_that_exist() {
    // The narrowing is a list of names matched against what somebody wrote, so
    // a renamed property would silently make a theme read nothing.
    for t in crate::props::THEME_STYLES {
        for name in t.only {
            assert!(
                crate::props::lookup(name, crate::props::PROPS).is_some(),
                "`style {}` claims to read `{name}`, which is not a property",
                t.name
            );
        }
    }
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

    // A **band**, not a group frame (`W-115`): a bordered rounded box read as
    // an object floating over the plot rather than as a stretch of the axis.
    assert!(l.groups.is_empty(), "a section is no longer a frame");
    let band = l
        .polylines
        .iter()
        .find(|p| p.closed && p.fill.is_some())
        .expect("the section is drawn as a filled strip");
    let (left, right) = band
        .points
        .iter()
        .fold((f64::MAX, f64::MIN), |(a, b), (x, _)| {
            (a.min(*x), b.max(*x))
        });

    assert!(
        l.annotations.iter().any(|a| a.text == "Early"),
        "the section is named in the band"
    );
    // It covers the first two steps — past their centres, since a band is over
    // the steps and not over the line between them — and stops short of the
    // third.
    let mut xs: Vec<f64> = l.nodes.iter().map(|n| n.rect.cx()).collect();
    xs.sort_by(|a, b| a.partial_cmp(b).expect("finite"));
    let (first, second, third) = (xs[0], xs[1], xs[2]);
    // Clamped to the plot on the left, since the first step sits on its edge.
    assert!(left <= first, "the band starts at the first step: {left}");
    assert!(
        right > second,
        "and reaches past the second, not merely to its centre: {right} vs {second}"
    );
    assert!(
        right < third,
        "and stops before the third: {right} vs {third}"
    );
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
        // Three genres now, and `D-019` is what makes that safe: `kind` reads
        // the word list of the archetype in force, so `service` is a BPMN task
        // and never an option while writing a state machine.
        ("kind", &["block", "bpmn", "state"]),
        ("stage", &["bpmn"]),
        ("trigger", &["bpmn"]),
        ("attach", &["bpmn"]),
        ("gate", &["bpmn"]),
        ("artifact", &["bpmn"]),
        ("flow", &["bpmn"]),
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
    for (_, source) in crate::examples::EXAMPLES
        .iter()
        .filter(|(genre, _)| matches!(*genre, "pie" | "packet"))
    {
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

    // Every archetype's example, laid out on disk because the cross-check
    // shells out to the `tree-sitter` CLI and a CLI takes a path. They come
    // from the table rather than from a directory, so the set cannot shrink
    // without a genre disappearing with it (`W-116`).
    let dir = scratch("grammar-examples");
    let mut examples: Vec<std::path::PathBuf> = crate::examples::EXAMPLES
        .iter()
        .map(|(genre, source)| {
            let path = dir.join(format!("{genre}.bd"));
            std::fs::write(&path, source).expect("a scratch example");
            path
        })
        .collect();
    examples.sort();
    assert_eq!(examples.len(), crate::props::ARCHETYPES.len());
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
fn every_pair_of_braces_is_indented_by_the_editors() {
    // `W-129`. The query beside it proves a `.scm` is *valid*; nothing proved
    // one was *complete*. `indents.scm` named `(block)` and the grammar has
    // two block types, so `group { … }`, `component { … }` and `use … { … }`
    // — all `group_block` — indented not at all, while `node a { … }` beside
    // them did. Pressing Enter did different things in two places that look
    // the same, which is what "annoying" meant.
    //
    // So this counts: every `{` in a sample must be inside something the query
    // captures. A list of node types written by hand is a list that stops
    // being true, and the query is written against the brackets for that
    // reason — this is what holds it to it.
    let dir = scratch("indent-query");
    let sample = dir.join("sample.bd");
    let text = "node a \"A\" {\n  fill: #123456\n}\n\
                group g \"G\" {\n  node b \"B\"\n}\n\
                component c {\n  node d \"D\"\n}\n\
                use c as e {\n  label: \"E\"\n}\n\
                type pie {\n  title: \"T\"\n}\n";
    std::fs::write(&sample, text).expect("a scratch sample");

    let query = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/editors/zed/languages/bgraph/indents.scm"
    );
    let out = {
        let _serial = tree_sitter_lock();
        std::process::Command::new("tree-sitter")
            .arg("query")
            .arg(query)
            .arg(&sample)
            .current_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/tree-sitter-bgraph"))
            .output()
    };
    let Ok(out) = out else {
        println!(
            "SKIPPED: `tree-sitter` is not on PATH, so the indent query was not \
             checked for completeness."
        );
        return;
    };

    // Distinct *positions*, not occurrences: a query may match one node with
    // several patterns, and counting hits would make a redundant pattern look
    // like coverage. The sample deliberately contains no parentheses, so every
    // capture here is a brace pair.
    let printed = String::from_utf8_lossy(&out.stdout);
    let mut spots: Vec<&str> = printed
        .lines()
        .filter_map(|l| l.trim().strip_prefix("capture: indent, start: "))
        .collect();
    spots.sort_unstable();
    spots.dedup();

    let braces = text.matches('{').count();
    assert_eq!(
        spots.len(),
        braces,
        "{} of {braces} brace pairs are indented — a block the query does not \
         reach is a block Enter behaves differently in:\n{printed}",
        spots.len()
    );
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
    // A file for the CLI to run the queries over. Written here from the table,
    // so no directory of samples has to exist for the guard to run.
    let sample_dir = scratch("query-sample");
    let sample = sample_dir.join("sample.bd");
    let (_, packet) = crate::examples::EXAMPLES
        .iter()
        .find(|(genre, _)| *genre == "packet")
        .expect("`packet` is an archetype");
    std::fs::write(&sample, packet).expect("a scratch sample");

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
            .arg(&sample)
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
fn cycle_check_uses_the_same_scoped_flow_as_the_solver() {
    let l = build(
        r#"
        node outside right-of first { size: 20 20 }
        group g {
          padding: 0
          node first { size: 20 20 }
        }
    "#,
    );

    assert_eq!(l.nodes[1].rect.x, 0.0);
    assert!(l.nodes[0].rect.x > l.nodes[1].rect.right());
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

    // A style registry that will not build, **crossed with every genre**.
    //
    // The corpus had broken styles and it had every genre; it never had the
    // two together, and that is where the hole was. A cycle makes
    // `Registry::build` fail, `resolved_nodes` then returned nothing, and the
    // seventeen checks that walk it alongside `diagram.nodes` were handed a
    // list of the wrong length — most silently, `check_state` by panicking on
    // `kinds[i]` (`W-130`). Crossing them is what makes this a guard rather
    // than a memory of one bug.
    for def in crate::props::ARCHETYPES {
        cases.push((
            "style cycle in a genre",
            format!(
                "type {}\nstyle a extends b {{ }}\nstyle b extends a {{ }}\n\
                 {} one \"One\" .a\n{} two \"Two\" .b\none -> two\n",
                def.name, def.item, def.item
            ),
        ));
        cases.push((
            "unknown class in a genre",
            format!(
                "type {}\n{} one \"One\" .nope\n{} two \"Two\"\none -> two\n",
                def.name, def.item, def.item
            ),
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

/// Inputs found in the wild (or reduced from a fuzz case) live as files so a
/// regression is reviewable, reproducible and shared by every pipeline.
fn versioned_hostile_corpus() -> Vec<(String, String)> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hostile");
    let mut cases = Vec::new();
    for entry in std::fs::read_dir(root).expect("the hostile corpus exists") {
        let path = entry.expect("read hostile corpus entry").path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("bd") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(&path).expect("read hostile corpus case");
        cases.push((name, source));
    }
    cases.sort_by(|left, right| left.0.cmp(&right.0));
    assert!(!cases.is_empty(), "the hostile corpus must not be empty");
    cases
}

#[test]
fn versioned_hostile_inputs_cover_every_renderer() {
    for (name, source) in versioned_hostile_corpus() {
        let Ok(diagram) = crate::parser::parse(&source) else {
            continue;
        };
        let diagnostics = crate::check::check(&diagram);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.is_empty()),
            "{name}"
        );
        let Ok(layout) = crate::layout::solve(&diagram) else {
            continue;
        };
        let svg = crate::render_svg::render(&layout);
        assert!(svg.starts_with("<svg"), "{name}: invalid SVG root");
        let pdf = crate::render_pdf::render(&layout);
        assert!(pdf.starts_with(b"%PDF-"), "{name}: invalid PDF header");
        let json = crate::render_json::render(&layout);
        serde_json::from_str::<serde_json::Value>(&json)
            .unwrap_or_else(|error| panic!("{name}: invalid JSON: {error}"));
    }
}

#[test]
fn generated_hostile_inputs_do_not_panic_in_any_front_end_stage() {
    let alphabet = b"abcdefghijklmnopqrstuvwxyz{}[]()#:.-/\\\" \n0123456789";
    let mut state = 0x9e37_79b9_u32;
    for case in 0..512 {
        let len = 1 + (state as usize % 384);
        let mut source = String::with_capacity(len);
        for _ in 0..len {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            source.push(alphabet[state as usize % alphabet.len()] as char);
        }
        if let Ok(diagram) = crate::parser::parse(&source) {
            let _ = crate::check::check(&diagram);
            if let Ok(layout) = crate::layout::solve(&diagram) {
                let _ = crate::render_svg::render(&layout);
                let _ = crate::render_pdf::render(&layout);
                let json = crate::render_json::render(&layout);
                assert!(
                    serde_json::from_str::<serde_json::Value>(&json).is_ok(),
                    "case {case}"
                );
            }
        }
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
        // `pdf` gates an output format, `json` and `csv` input ones, `wasm`
        // the four entry points a browser calls — none of them genres.
        // They are named here rather
        // than filtered by a pattern, so a second non-archetype feature has to
        // be justified in this list instead of slipping past a wildcard.
        .filter(|name| !matches!(*name, "pdf" | "json" | "csv" | "wasm"))
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
    //
    // **It is a manifest of hashes now, not a directory of pictures**
    // (`W-116`). The pictures went with `examples/`, and with them the only
    // thing that could say "this commit moved a drawing". A hash per genre
    // gives that back for two kilobytes instead of five hundred. What it
    // cannot do is show you *what* moved: it says which genre and leaves you
    // to render it and look, which is what you had to do anyway.
    let manifest = include_str!("../docs/goldens.txt");
    let want: std::collections::HashMap<&str, &str> = manifest
        .lines()
        .filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty())
        .filter_map(|l| l.split_once(char::is_whitespace))
        .map(|(genre, hash)| (genre.trim(), hash.trim()))
        .collect();

    let mut stale = Vec::new();
    for (genre, source) in crate::examples::EXAMPLES {
        let svg = crate::render_source(source).unwrap_or_else(|e| panic!("{genre}: {e:#}"));
        let got = format!("{:016x}", fnv1a(svg.trim_end()));
        match want.get(genre) {
            Some(pinned) if *pinned == got => {}
            Some(_) => stale.push(format!("{genre} {got}")),
            None => panic!("`{genre}` is not pinned in docs/goldens.txt — add `{genre} {got}`"),
        }
    }

    assert!(
        stale.is_empty(),
        "these drawings moved. Render each and look at it, then write the new \
         line into docs/goldens.txt:\n{}",
        stale.join("\n")
    );
    assert_eq!(
        want.len(),
        crate::examples::EXAMPLES.len(),
        "the manifest has a stray line"
    );
}

/// FNV-1a over the bytes, for the golden manifest.
///
/// Hand-rolled and non-cryptographic on purpose: the question is "did this
/// change" and not "can this be forged", and it keeps the crate's dependency
/// list where it is.
#[cfg(test)]
fn fnv1a(text: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[cfg(feature = "png")]
fn png_snapshot_fixtures() -> [(&'static str, &'static str); 3] {
    [
        (
            "shapes",
            "node a \"\" at (0, 0) { shape: cylinder size: 100 60 }\nnode b \"\" right-of a { shape: hexagon size: 100 60 }\na -> b { router: ortho }\n",
        ),
        (
            "labels",
            "node api \"Public API\" at (0, 0) { size: 130 64 }\nnode queue \"Job queue\" right-of api gap 80\napi -> queue \"enqueue request\" { router: ortho }\n",
        ),
        (
            "charts",
            "type pie { title: \"Traffic sources\" }\nslice organic \"Organic\" 4820\nslice direct \"Direct\" 2310\nslice referral \"Referral\" 970\n",
        ),
    ]
}

#[cfg(feature = "png")]
#[test]
fn representative_png_snapshots_are_byte_stable() {
    let manifest = include_str!("../docs/png-goldens.txt");
    for (name, source) in png_snapshot_fixtures() {
        let expected = manifest
            .lines()
            .find_map(|line| line.split_once(' ').filter(|(found, _)| *found == name))
            .and_then(|(_, hash)| u64::from_str_radix(hash, 16).ok())
            .unwrap_or_else(|| panic!("no PNG snapshot for `{name}`"));
        let png = crate::render_png::render(&build(source), 1.0).expect("PNG snapshot renders");
        let got = png.iter().fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
        assert_ne!(
            expected, 0,
            "add `{name} {got:016x}` to docs/png-goldens.txt"
        );
        assert_eq!(got, expected, "{name}: PNG changed (threshold: 0 bytes)");
    }
}

/// Whether a document's tags open and close in order, and nothing is left open.
///
/// Hand-rolled, like the base64 writer and the nesting guard, and for the same
/// reason: it is forty lines against a dependency, in a crate whose whole shape
/// is that it writes its own SVG. Returns the first mismatch as a sentence,
/// because "invalid" without a place is not a diagnosis.
#[cfg(test)]
fn ill_formed(doc: &str) -> Option<String> {
    let b = doc.as_bytes();
    let mut open: Vec<&str> = Vec::new();
    let mut i = 0;

    while let Some(lt) = doc[i..].find('<').map(|k| i + k) {
        i = lt + 1;
        if doc[i..].starts_with("!--") {
            i = doc[i..].find("-->").map_or(b.len(), |k| i + k + 3);
            continue;
        }
        if doc[i..].starts_with('!') || doc[i..].starts_with('?') {
            i = doc[i..].find('>').map_or(b.len(), |k| i + k + 1);
            continue;
        }

        let closing = doc[i..].starts_with('/');
        if closing {
            i += 1;
        }
        let name_end = doc[i..]
            .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
            .map_or(b.len(), |k| i + k);
        let name = &doc[i..name_end];
        i = name_end;

        // Past the attributes, minding that a `>` inside a quoted value is not
        // the end of the tag — which it is, in half the paths this emits.
        let mut quote = None::<u8>;
        while i < b.len() {
            match (quote, b[i]) {
                (Some(q), c) if c == q => quote = None,
                (None, c @ (b'"' | b'\'')) => quote = Some(c),
                (None, b'>') => break,
                _ => {}
            }
            i += 1;
        }
        let self_closing = i > 0 && b[i - 1] == b'/';
        i += 1;

        if closing {
            match open.pop() {
                Some(want) if want == name => {}
                Some(want) => return Some(format!("`</{name}>` closes an open `<{want}>`")),
                None => return Some(format!("`</{name}>` closes nothing")),
            }
        } else if !self_closing {
            open.push(name);
        }
    }

    open.last()
        .map(|name| format!("`<{name}>` is never closed"))
}

#[test]
fn every_rendered_example_is_well_formed_xml() {
    // The golden manifest hashes what we emit and says nothing about whether
    // it parses, so an SVG with an `<ellipse>` closed by a `</path>` would hash
    // consistently and still be broken. A browser draws that; resvg refuses it,
    // which is what made `--window` and `--png` fail on `type block` and
    // nothing else notice (`W-107`).
    for (genre, source) in crate::examples::EXAMPLES {
        let svg = crate::render_source(source).unwrap_or_else(|e| panic!("{genre}: {e:#}"));
        assert_eq!(ill_formed(&svg), None, "{genre}");
    }

    // And the scanner is worth believing: it catches the exact shape of the
    // defect it exists for.
    assert!(ill_formed("<svg><ellipse cx=\"1\"><title>x</title></path></svg>").is_some());
    assert!(
        ill_formed("<svg><rect a=\"1 > 0\"/></svg>").is_none(),
        "a `>` in a value"
    );
    assert_eq!(ill_formed("<svg><g><rect/></g></svg>"), None);
}

#[test]
fn every_example_still_works() {
    // The suite used to name two example files by hand, so the ones added
    // since were covered by nothing at all. A directory sweep cannot forget.
    // The table, not a directory. A sweep over `examples/` could only ever
    // check the files that happened to be there, and the day the directory
    // went it checked nothing at all while still passing (`W-116`).
    for (genre, source) in crate::examples::EXAMPLES {
        // A template is checked as a template: its holes are not bound here,
        // so rendering it is not the question — whether it is well-formed is.
        let template = Template::parse(source)
            .unwrap_or_else(|e| panic!("{genre} is not a well-formed template: {e:#}"));
        assert!(
            template.holes().is_empty(),
            "{genre}: an archetype's example must be complete, not a template"
        );

        let errors: Vec<String> = diags(source)
            .into_iter()
            .filter(|d| d.severity == crate::check::Severity::Error)
            .map(|d| d.message)
            .collect();
        assert!(errors.is_empty(), "{genre}: {errors:?}");

        let svg = crate::render_source(source).unwrap_or_else(|e| panic!("{genre}: {e:#}"));
        assert!(
            svg.starts_with("<svg") && svg.trim_end().ends_with("</svg>"),
            "{genre}"
        );
    }

    // One per archetype, and the count is asserted so a genre cannot be added
    // without a file that shows how to write it.
    assert_eq!(
        crate::examples::EXAMPLES.len(),
        crate::props::ARCHETYPES.len(),
        "every archetype has exactly one worked example"
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
fn the_json_layout_names_residual_label_conflicts() {
    let json =
        geometry("node a \"A\" at (0, 0)\nnode b \"B\" right-of a\na -> b \"one\"\nb -> a \"two\"");
    assert!(json["label_conflicts"].is_array(), "{json}");
    for conflict in json["label_conflicts"].as_array().unwrap() {
        assert!(conflict["kind"].is_string(), "{conflict}");
        assert!(conflict["edge"].is_u64(), "{conflict}");
    }
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
    // Three files that are one diagram — what a bgraph-tagged fence cannot be,
    // since that fence promises a complete current file. So the multi-file case
    // is verified here rather than documented and never run.
    //
    // Written into a scratch directory rather than read from `examples/`: the
    // whole point of `W-116` is that the suite does not need a directory of
    // sample files to be there, and a fixture a test writes itself is one that
    // cannot go missing.
    let dir = scratch("compose");
    std::fs::write(
        dir.join("house.bds"),
        "// A shared palette. A `.bds` holds `style` declarations and nothing else.\n\
         style card { fill: #1e293b stroke: #38bdf8 1.5 font: 13 #e2e8f0 radius: 6 padding: 12 }\n\
         style hot extends card { stroke: #f43f5e 2 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("core.bd"),
        "// A diagram in its own right — render this file on its own and it draws.\n\
         include \"house.bds\"\n\
         node api \"API\"      .card\n\
         node db  \"Database\" .card below api\n\
         api -> db \"reads\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.bd"),
        "include \"house.bds\"\n\
         include \"core.bd\" as core\n\
         graph { background: #0b1120 title: \"Composed from three files\" }\n\
         node ui \"Browser\" .hot\n\
         ui -> core.api \"HTTPS\"\n",
    )
    .unwrap();

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

// ------------------------------------------------- the note statement (I-054)

#[test]
fn a_note_is_drawn_where_its_anchor_puts_it() {
    let l = build(
        r#"node api "API"
           note "Figures are March." right-of api gap 30"#,
    );
    let note = l
        .annotations
        .iter()
        .find(|a| a.text == "Figures are March.")
        .expect("the note should be drawn");
    assert!(
        note.x > l.nodes[0].rect.right(),
        "the note sits at {} but `api` ends at {}",
        note.x,
        l.nodes[0].rect.right()
    );
}

#[test]
fn a_note_is_not_a_box_and_cannot_be_pointed_at() {
    // The whole difference between a note and a box with no border: it
    // declares no name, so there is nothing for an edge to land on.
    let l = build(r#"node a "A""#);
    let before = l.nodes.len();
    let l = build(
        r#"node a "A"
           note "An aside." below a"#,
    );
    assert_eq!(l.nodes.len(), before, "the note became a box");

    // …and naming it is an unknown-name error, not a silent link.
    let message = err(r#"node a "A"
           note "An aside." below a
           a -> aside"#);
    assert!(message.contains("aside"), "{message}");
}

#[test]
fn a_note_takes_a_style_class() {
    let l = build(
        r#"style caption { font: 11 #b45309 }
           node a "A"
           note "An aside." .caption below a"#,
    );
    let note = l
        .annotations
        .iter()
        .find(|a| a.text == "An aside.")
        .unwrap();
    assert_eq!(note.color, "#b45309");
    assert_eq!(note.size, 11.0);
}

#[test]
fn a_note_anchored_to_nothing_is_left_out_rather_than_drawn_at_the_origin() {
    // `check` has already reported the unknown name; drawing the note in the
    // corner would add a second wrong thing to look at.
    let source = r#"node a "A"
                    note "An aside." below ghost"#;
    let diagram = parse_diagram(source).expect("it parses");
    let l = layout::solve(&diagram);
    // The unknown anchor is an error, so this does not solve at all — which
    // is the stronger form of the same answer.
    assert!(l.is_err(), "an unknown anchor should be refused");
}

#[test]
fn a_note_follows_a_box_a_constraint_moved() {
    // Solved last, against the boxes as they finally sit.
    let l = build(
        r#"node a "A" at (0, 0) { size: 80 40 }
           node b "B" at (200, 90) { size: 80 40 }
           align-group top a b
           note "Beside B." right-of b gap 20"#,
    );
    let b = l.nodes[1].rect;
    let note = l
        .annotations
        .iter()
        .find(|a| a.text == "Beside B.")
        .unwrap();
    assert_eq!(b.y, 0.0, "the constraint did not run");
    assert!(
        (note.y - b.cy()).abs() < b.h,
        "the note stayed beside where `b` used to be"
    );
}

#[test]
fn every_style_field_is_inherited() {
    // `overlay!` names its fields by hand, so a field added to `Style` and
    // forgotten here is silently never inherited — a class sets it, the
    // cascade drops it, and nothing says a word. That is exactly how the
    // `legend` property arrived born broken.
    let source = include_str!("style.rs");
    let fields: Vec<&str> = source
        .split_once("pub struct Style {")
        .expect("the Style struct")
        .1
        .split_once("\n}")
        .expect("its closing brace")
        .0
        .lines()
        .filter_map(|l| l.trim().strip_prefix("pub "))
        .filter_map(|l| l.split_once(':'))
        .map(|(name, _)| name.trim())
        .collect();

    let listed = source
        .split_once("overlay!(")
        .expect("the overlay call")
        .1
        .split_once(");")
        .expect("its close")
        .0;

    for field in &fields {
        assert!(
            listed
                .lines()
                .any(|l| l.trim().trim_end_matches(',') == *field),
            "`Style::{field}` is never inherited — add it to `overlay!`"
        );
    }
    assert!(fields.len() > 20, "only {} fields found", fields.len());
}

// ------------------------------------------------- a general legend (I-060)

#[test]
fn a_legend_lists_only_the_classes_that_asked_to_be_in_it() {
    // Derived from what the file says rather than from the colours on the
    // page: a legend built from every class would list the ones that only
    // round a corner, and one built from the colours in use would have
    // nothing to call them.
    let l = build(
        r#"style healthy { fill: #dcfce7 stroke: #16a34a legend: "Healthy" }
           style down    { fill: #fee2e2 stroke: #dc2626 legend: "Down" }
           style plain   { radius: 2 }
           node a "A" .healthy
           node b "B" .down below a
           legend right-of a gap 40"#,
    );
    let texts: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(texts.contains(&"Healthy"), "{texts:?}");
    assert!(texts.contains(&"Down"), "{texts:?}");
    assert_eq!(
        texts.len(),
        2,
        "something unasked-for got listed: {texts:?}"
    );
}

#[test]
fn a_swatch_carries_the_stroke_as_well_as_the_fill() {
    // A status colour is usually a pale tint with the meaning in the border,
    // so a swatch drawn from the fill alone is a key nobody can read.
    let l = build(
        r#"style healthy { fill: #dcfce7 stroke: #16a34a legend: "Healthy" }
           node a "A" .healthy
           legend right-of a gap 40"#,
    );
    let swatch = l
        .nodes
        .iter()
        .find(|n| n.style.fill.as_deref() == Some("#dcfce7") && n.lines.is_empty())
        .expect("a swatch should be drawn");
    assert_eq!(swatch.style.stroke.as_deref(), Some("#16a34a"));
}

#[test]
fn no_class_asks_for_a_legend_so_none_is_drawn() {
    let l = build(
        r#"style plain { radius: 2 }
           node a "A" .plain
           legend right-of a gap 40"#,
    );
    assert!(l.annotations.is_empty(), "an empty legend was drawn");
}

// ------------------------------------------------- `top n` (I-047)

#[test]
fn top_keeps_the_largest_and_folds_the_rest() {
    // `check` already advises exactly this when a pie exhausts the palette.
    // Before, the advice could only be taken by doing the arithmetic by hand
    // outside the tool.
    let l = build(
        r#"type pie { top: 3 }
           slice a "A" 50
           slice b "B" 30
           slice c "C" 10
           slice d "D" 6
           slice e "E" 4"#,
    );
    let labels: Vec<&str> = l
        .annotations
        .iter()
        .map(|a| a.text.as_str())
        .filter(|t| t.starts_with(['A', 'B', 'C', 'D', 'E', 'O']))
        .collect();
    assert!(labels.iter().any(|t| t.starts_with("Other")), "{labels:?}");
    assert!(
        !labels
            .iter()
            .any(|t| t.starts_with('D') || t.starts_with('E')),
        "a folded slice is still listed: {labels:?}"
    );
}

#[test]
fn the_folded_slice_keeps_the_total_honest() {
    // Folding must not lose a number: the whole is still the whole.
    let l = build(
        r#"type pie { top: 2 labels: value }
           slice a "A" 50
           slice b "B" 30
           slice c "C" 12
           slice d "D" 8"#,
    );
    let values: Vec<&str> = l.annotations.iter().map(|a| a.text.as_str()).collect();
    assert!(
        values.iter().any(|t| t.contains("20")),
        "12 and 8 did not add up to 20: {values:?}"
    );
}

#[test]
fn the_folded_slice_can_be_named() {
    let l = build(
        r#"type pie { top: 1 other: "Everything else" }
           slice a "A" 50
           slice b "B" 30"#,
    );
    assert!(
        l.annotations
            .iter()
            .any(|a| a.text.starts_with("Everything else")),
        "{:?}",
        l.annotations.iter().map(|a| &a.text).collect::<Vec<_>>()
    );
}

#[test]
fn folding_is_the_same_every_run() {
    // A chart that reordered itself between runs would defeat the golden
    // test and, before that, the reader.
    let source = r#"type pie { top: 2 }
                    slice a "A" 10
                    slice b "B" 10
                    slice c "C" 10
                    slice d "D" 10"#;
    let first = crate::render_source(source).unwrap();
    let second = crate::render_source(source).unwrap();
    assert_eq!(first, second);
}

#[test]
fn every_public_render_is_byte_stable() {
    let source = r#"graph { title: "Stable" }
        node a "A" at (0, 0)
        node b "B" right-of a gap 40
        a -> b "route" { router: ortho }
    "#;
    let svg = crate::render_source(source).unwrap();
    let pdf = crate::render_source_pdf(source).unwrap();
    let json = crate::render_json::render(&build(source));
    for _ in 0..5 {
        assert_eq!(crate::render_source(source).unwrap(), svg);
        assert_eq!(crate::render_source_pdf(source).unwrap(), pdf);
        assert_eq!(crate::render_json::render(&build(source)), json);
    }
}

#[test]
fn top_larger_than_the_data_changes_nothing() {
    let l = build(
        r#"type pie { top: 10 }
           slice a "A" 50
           slice b "B" 30"#,
    );
    assert!(
        !l.annotations.iter().any(|a| a.text.starts_with("Other")),
        "a fold happened with nothing to fold"
    );
}

// ------------------------------------------------- multi-page print (I-061)

#[cfg(feature = "pdf")]
#[test]
fn a_diagram_that_fits_is_still_one_page() {
    // The default must not change: a diagram smaller than a sheet has always
    // been one page sized to itself, and that is still the right answer.
    let l = build("node a \"A\"");
    let pdf = String::from_utf8_lossy(&crate::render_pdf::render(&l)).into_owned();
    assert_eq!(
        pdf.matches("/Type /Page\n").count() + pdf.matches("/Type /Page ").count(),
        1
    );
    assert!(pdf.contains("/Count 1"), "not a single-page document");
}

#[cfg(feature = "pdf")]
#[test]
fn a_wide_diagram_is_tiled_across_sheets() {
    let mut source = String::from("graph { flow: right, gap: 30 }\n");
    for i in 0..14 {
        source.push_str(&format!("node n{i} \"Stage {i}\"\n"));
    }
    let l = build(&source);
    let pdf = String::from_utf8_lossy(&crate::render_pdf::render_paged(
        &l,
        crate::render_pdf::Page::A4,
    ))
    .into_owned();

    assert!(
        !pdf.contains("/Count 1"),
        "a diagram much wider than A4 came out on one sheet"
    );
    // One content stream, several pages: a sheet is a window onto the drawing,
    // so nothing is re-rendered and there is no second code path to disagree
    // with the single-page one.
    // `>>\nstream` rather than `stream`, because `endstream` contains the
    // word too and counting that would always answer two.
    assert_eq!(
        pdf.matches(">>\nstream").count(),
        1,
        "the drawing was written more than once"
    );
}

#[cfg(feature = "pdf")]
#[test]
fn a_page_size_is_named_or_measured() {
    use crate::render_pdf::Page;
    assert!((Page::parse("a4").unwrap().width - 595.28).abs() < 0.01);
    assert!((Page::parse("A4").unwrap().height - 841.89).abs() < 0.01);
    assert_eq!(Page::parse("400x300").unwrap().width, 400.0);
    assert_eq!(Page::parse("400 x 300").unwrap().height, 300.0);
    assert!(Page::parse("foolscap").is_none());
}

#[cfg(feature = "pdf")]
#[test]
fn sheets_overlap_so_a_seam_is_readable() {
    // A box cut exactly in half at a seam is unreadable on both sheets, so
    // adjacent tiles have to share a strip. Read off the produced document
    // rather than off the constant, or the test would only be restating the
    // number and not that it is used.
    let mut source = String::from("graph { flow: right, gap: 30 }\n");
    for i in 0..14 {
        source.push_str(&format!("node n{i} \"Stage {i}\"\n"));
    }
    let l = build(&source);
    let pdf = String::from_utf8_lossy(&crate::render_pdf::render_paged(
        &l,
        crate::render_pdf::Page::A4,
    ))
    .into_owned();

    let lefts: Vec<f64> = pdf
        .match_indices("/MediaBox [")
        .filter_map(|(i, _)| {
            let rest = &pdf[i + "/MediaBox [".len()..];
            let inside = rest.split(']').next()?;
            let mut numbers = inside.split_whitespace();
            let x0: f64 = numbers.next()?.parse().ok()?;
            let x1: f64 = numbers.nth(1)?.parse().ok()?;
            Some((x0, x1))
        })
        .map(|(x0, x1)| {
            assert!(x1 > x0, "a sheet with no width");
            x0
        })
        .collect();

    assert!(lefts.len() >= 2, "only {} sheets", lefts.len());
    let step = lefts[1] - lefts[0];
    assert!(
        step > 0.0 && step < crate::render_pdf::Page::A4.width,
        "sheets advance by {step}, which leaves no overlap"
    );
}

// ------------------------------------------------- visual diff (I-050)

fn diff_of(before: &str, after: &str) -> Layout {
    crate::diff::compare(&build(before), &build(after))
}

#[test]
fn an_added_box_is_green_and_a_removed_one_is_red() {
    let l = diff_of(
        "node a \"A\"\nnode gone \"Gone\" right-of a",
        "node a \"A\"\nnode fresh \"Fresh\" right-of a",
    );
    let fresh = l.nodes.iter().find(|n| n.lines == ["Fresh"]).unwrap();
    let gone = l.nodes.iter().find(|n| n.lines == ["Gone"]).unwrap();
    assert_eq!(fresh.style.stroke.as_deref(), Some("#16a34a"));
    assert_eq!(gone.style.stroke.as_deref(), Some("#dc2626"));
}

#[test]
fn a_removed_box_goes_below_rather_than_under_what_replaced_it() {
    // The first attempt drew removals in place. Something new usually takes
    // the space, and the two labels then sat on top of each other — the test
    // render read "Qaeuhee". A gutter never collides.
    let l = diff_of(
        "node a \"A\"\nnode cache \"Cache\" right-of a",
        "node a \"A\"\nnode queue \"Queue\" right-of a",
    );
    let queue = l.nodes.iter().find(|n| n.lines == ["Queue"]).unwrap();
    let cache = l.nodes.iter().find(|n| n.lines == ["Cache"]).unwrap();
    assert!(
        cache.rect.y > queue.rect.bottom(),
        "the removed box is still on top of its replacement"
    );
}

#[test]
fn a_moved_box_keeps_a_ghost_of_where_it_was() {
    // The pair is what says "moved" — either alone reads as removed or added.
    let l = diff_of(
        "node a \"A\"\nnode b \"B\" below a gap 40",
        "node a \"A\"\nnode b \"B\" below a gap 120",
    );
    let b = l.nodes.iter().find(|n| n.lines == ["B"]).unwrap();
    assert_eq!(b.style.stroke.as_deref(), Some("#d97706"));
    assert!(
        l.nodes
            .iter()
            .any(|n| n.lines.is_empty() && n.style.stroke.as_deref() == Some("#d97706")),
        "no ghost was left where the box used to be"
    );
}

#[test]
fn an_unchanged_diagram_shows_nothing_but_itself() {
    let source = "node a \"A\"\nnode b \"B\" right-of a\na -> b";
    let l = diff_of(source, source);
    assert!(
        l.nodes
            .iter()
            .all(|n| n.style.stroke.as_deref() == Some("#94a3b8")),
        "an unchanged diagram reported a change"
    );
}

#[test]
fn a_box_nudged_by_a_neighbours_label_is_not_called_moved() {
    // Half a pixel, not zero: reporting every fractional shift would make
    // each diff say everything moved, and nobody reads a diff like that.
    let l = diff_of(
        "node a \"A\"\nnode b \"B\" right-of a",
        "node a \"A\"\nnode b \"B\" right-of a",
    );
    assert!(
        !l.nodes
            .iter()
            .any(|n| n.style.stroke.as_deref() == Some("#d97706")),
        "a still diagram reported a move"
    );
}

// ------------------------------------------------- reading a table (I-048)

#[cfg(feature = "csv")]
#[test]
fn a_chart_can_read_its_rows_from_a_table() {
    let dir = scratch("from-csv");
    std::fs::write(
        dir.join("sales.csv"),
        "name,value\nOrganic search,4820\nDirect,2410\nReferral,1180\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.bd"),
        "type pie { title: \"From a table\" from: \"sales.csv\" }\n",
    )
    .unwrap();

    let composed = crate::compose::load(&dir.join("main.bd")).expect("it composes");
    assert_eq!(composed.diagram.nodes.len(), 3);
    let l = crate::layout::solve(&composed.diagram).expect("it draws");
    assert!(
        l.annotations.iter().any(|a| a.text.starts_with("Organic")),
        "the labels did not come through"
    );
}

#[cfg(feature = "csv")]
#[test]
fn columns_are_matched_by_name_and_not_by_position() {
    // A table whose columns are in a different order still reads correctly,
    // which is the whole reason the header is the contract.
    let dir = scratch("from-order");
    std::fs::write(dir.join("t.csv"), "value,name\n40,A\n60,B\n").unwrap();
    std::fs::write(dir.join("main.bd"), "type pie { from: \"t.csv\" }\n").unwrap();

    let composed = crate::compose::load(&dir.join("main.bd")).expect("it composes");
    assert_eq!(composed.diagram.nodes[0].label.as_deref(), Some("A"));
}

#[cfg(feature = "csv")]
#[test]
fn a_missing_column_says_which_and_lists_what_is_there() {
    let dir = scratch("from-missing");
    std::fs::write(dir.join("t.csv"), "label,amount\nA,1\n").unwrap();
    std::fs::write(dir.join("main.bd"), "type pie { from: \"t.csv\" }\n").unwrap();

    let message = format!(
        "{:#}",
        crate::compose::load(&dir.join("main.bd")).expect_err("no `value` column")
    );
    assert!(message.contains("value"), "{message}");
    assert!(message.contains("amount"), "{message}");
}

#[cfg(feature = "csv")]
#[test]
fn a_missing_table_names_the_file_that_asked_for_it() {
    // The consequence this feature accepts: a diagram that reads the disk at
    // render time is no longer autonomous, so the diagnostic owes the reader
    // both names.
    let dir = scratch("from-gone");
    std::fs::write(dir.join("main.bd"), "type pie { from: \"nope.csv\" }\n").unwrap();

    let message = format!(
        "{:#}",
        crate::compose::load(&dir.join("main.bd")).expect_err("no such table")
    );
    assert!(message.contains("nope.csv"), "{message}");
    assert!(message.contains("main.bd"), "{message}");
}

#[cfg(feature = "csv")]
#[test]
fn rows_are_numbered_rather_than_named_after_their_labels() {
    // A label may repeat, contain spaces or be empty. None of those may
    // become an identifier collision.
    let dir = scratch("from-ids");
    std::fs::write(dir.join("t.csv"), "name,value\nA,1\nA,2\n").unwrap();
    std::fs::write(dir.join("main.bd"), "type pie { from: \"t.csv\" }\n").unwrap();

    let composed = crate::compose::load(&dir.join("main.bd")).expect("duplicate labels are fine");
    let ids: Vec<&str> = composed
        .diagram
        .nodes
        .iter()
        .map(|n| n.id.as_str())
        .collect();
    assert_eq!(ids, ["row1", "row2"]);
}

// ------------------------------------------------- sketch mode (I-043)

#[test]
fn sketch_mode_draws_paths_instead_of_rectangles() {
    let plain = crate::render_source("node a \"A\"").unwrap();
    let drawn = crate::render_source("graph { sketch: on }\nnode a \"A\"").unwrap();
    assert!(plain.contains("<rect"), "{plain}");
    // `rx=` rather than `<rect`: the canvas background is a rectangle too,
    // and it is not a shape the author drew.
    assert!(
        !drawn.contains("rx="),
        "a crisp box survived sketch mode: {drawn}"
    );
    assert!(drawn.contains("<path d=\"M"), "{drawn}");
}

#[test]
fn a_sketch_is_the_same_file_every_time() {
    // The whole difficulty of the feature. The golden test asserts the output
    // is byte-identical run to run — and more to the point, a diagram that
    // redrew itself differently on every save would be unreviewable.
    let source = "graph { sketch: on }\nnode a \"A\"\nnode b \"B\" right-of a\na -> b";
    let first = crate::render_source(source).unwrap();
    for _ in 0..5 {
        assert_eq!(crate::render_source(source).unwrap(), first);
    }
}

#[test]
fn two_boxes_in_different_places_wobble_differently() {
    // Otherwise it is a stamp rather than a drawing: every box deformed
    // identically reads as a font, not as a hand.
    let svg =
        crate::render_source("graph { sketch: on }\nnode a \"A\"\nnode b \"B\" right-of a gap 40")
            .unwrap();
    let paths: Vec<&str> = svg
        .match_indices("<path d=\"M")
        .map(|(i, _)| &svg[i..i + 40])
        .collect();
    assert!(paths.len() >= 2, "{paths:?}");
    // Compare the fractional parts: the two boxes are at different x, so the
    // *offsets* are what must differ rather than the coordinates.
    assert_ne!(
        paths[0].split_whitespace().nth(2),
        paths[1].split_whitespace().nth(2),
        "both boxes were deformed identically"
    );
}

#[test]
fn a_shape_whose_meaning_is_its_outline_is_left_alone() {
    // A diamond drawn freehand stops reading as a decision, and a circle
    // stops reading as a circle. Only the rectangles are wobbled.
    let svg =
        crate::render_source("graph { sketch: on }\nnode a \"A\" { shape: diamond }").unwrap();
    assert!(svg.contains("<polygon") || svg.contains("<path"), "{svg}");
    assert!(
        !svg.contains("Q "),
        "a diamond was drawn with the freehand bow"
    );
}

// ------------------------------------------------- `explain` (I-037)

fn explained(source: &str, id: &str) -> String {
    let diagram = parse_diagram(source).expect("it parses");
    let solved = layout::solve(&diagram).expect("it draws");
    crate::explain::explain(&diagram, &solved, id).expect("it explains")
}

#[test]
fn explain_says_what_the_file_declared_and_what_filled_in() {
    // `D-001` makes placement the author's job, so a declared layout deserves
    // a declared explanation: every line is a statement in the file or a
    // default that stood in for one, and both are named.
    let out = explained(
        r#"graph { gap: 60 }
           node ui "Browser"
           node api "API" right-of ui gap 90"#,
        "api",
    );
    assert!(out.contains("right-of ui"), "{out}");
    assert!(out.contains("gap 90"), "{out}");
    assert!(out.contains("declared"), "{out}");
    // The gap was declared, so the graph default must not be claimed.
    assert!(!out.contains("gap 60"), "{out}");
}

#[test]
fn explain_names_the_default_when_the_file_was_silent() {
    let out = explained(
        r#"graph { gap: 60 }
           node ui "Browser"
           node api "API" right-of ui"#,
        "api",
    );
    assert!(out.contains("gap 60"), "{out}");
    assert!(out.contains("default"), "{out}");
}

#[test]
fn explain_names_the_rule_an_author_forgets() {
    // An unplaced box flowing from the statement before it is the single
    // most common surprise, and the hardest thing to see in the source.
    let out = explained("node a \"A\"\nnode b \"B\"", "b");
    assert!(out.contains("flows"), "{out}");
    assert!(out.contains('a'), "{out}");
}

#[test]
fn explain_names_the_scoped_flow_root() {
    let out = explained(
        "node outside at (100, 0)\ngroup g {\n node first\n node second\n}",
        "first",
    );
    assert!(out.contains("first box in its container"), "{out}");
    assert!(!out.contains("`outside`"), "{out}");
}

#[test]
fn explain_says_when_a_cell_has_no_grid_to_sit_on() {
    // A cell with no declared grid puts everything on the origin — a
    // question nobody thinks to ask, which is exactly when to volunteer it.
    let out = explained("node a \"A\" at (col 2, row 1)", "a");
    assert!(out.contains("NOT DECLARED"), "{out}");
}

#[test]
fn explain_attributes_each_colour_to_the_layer_that_set_it() {
    let out = explained(
        r#"style card { fill: #1e293b stroke: #38bdf8 }
           style hot extends card { stroke: #f43f5e }
           node a "A" .hot { fill: #000000 }"#,
        "a",
    );
    assert!(out.contains("#f43f5e"), "{out}");
    assert!(
        out.contains("#000000") && out.contains("own block"),
        "the inline block did not win: {out}"
    );
}

#[test]
fn explain_mentions_a_constraint_acting_on_the_box() {
    // Easy to forget three screens away from the statement it acts on.
    let out = explained(
        r#"node a "A"
           node b "B" right-of a
           same-width a b"#,
        "a",
    );
    assert!(out.contains("same-width"), "{out}");
    assert!(out.contains('b'), "{out}");
}

#[test]
fn explaining_an_unknown_box_lists_the_ones_there_are() {
    let diagram = parse_diagram("node a \"A\"\nnode b \"B\"").unwrap();
    let solved = layout::solve(&diagram).unwrap();
    let message = format!(
        "{:#}",
        crate::explain::explain(&diagram, &solved, "ghost").expect_err("no such box")
    );
    assert!(message.contains("ghost"), "{message}");
    assert!(message.contains('a') && message.contains('b'), "{message}");
}

// ------------------------------------------------- `bgraph layout` (I-033)

/// Fill in the placements and hand back both texts.
fn frozen(source: &str) -> (String, String) {
    let diagram = parse_diagram(source).expect("it parses");
    let filled = crate::relayout::fill_in(source, &diagram).expect("it fills in");
    (source.to_string(), filled)
}

#[test]
fn freezing_a_layout_does_not_change_the_picture() {
    // The whole promise, and the only guard that makes the command
    // trustworthy: it edits the author's file, so the diagram must come out
    // byte-identical.
    for source in [
        "node a \"A\"\nnode b \"B\"\nnode c \"C\"\na -> b",
        "graph { flow: down, gap: 30 }\nnode a \"A\"\nnode b \"B\"",
        "node a \"A\"\nnode b \"B\" { fill: #eef3ff }\nnode c \"C\"",
        "node a \"A\" at (0, 0)\nnode b \"B\" right-of a\nnode c \"C\"",
        "node outside at (100, 0)\ngroup g {\n node first\n node second\n}",
    ] {
        let (before, after) = frozen(source);
        let one = crate::render_source(&before).expect("the original draws");
        let two = crate::render_source(&after).expect("the frozen one draws");
        assert_eq!(
            one, two,
            "the picture moved:\n--- before\n{before}\n--- after\n{after}"
        );
    }
}

#[test]
fn freezing_uses_the_previous_sibling_inside_a_group() {
    let (_, after) = frozen("node outside at (100, 0)\ngroup g {\n node first\n node second\n}");
    assert!(!after.contains("first right-of outside"), "{after}");
    assert!(after.contains("second right-of first"), "{after}");
}

#[test]
fn every_box_says_where_it_goes_afterwards() {
    let (_, after) = frozen("node a \"A\"\nnode b \"B\"\nnode c \"C\"");
    let diagram = parse_diagram(&after).unwrap();
    assert!(
        crate::relayout::unplaced(&diagram).is_empty(),
        "still loose: {after}"
    );
}

#[test]
fn a_comment_and_the_author_s_spacing_survive() {
    // `fmt` leaves interior alignment alone; this has to go further, because
    // a comment and a column lined up by hand are things the author put
    // there on purpose.
    let source = "node ui   \"Browser\"          // the reader\nnode api  \"API\"\n";
    let (_, after) = frozen(source);
    assert!(after.contains("// the reader"), "{after}");
    assert!(after.contains("node ui   \"Browser\""), "{after}");
    assert!(after.contains("node api  \"API\" right-of ui"), "{after}");
}

#[test]
fn a_placement_lands_before_a_block_and_not_against_it() {
    // Both orders parse, but `right-of api{ … }` is not something a person
    // would have typed — and that was the first output.
    let (_, after) = frozen("node a \"A\"\nnode b \"B\" { fill: #eef3ff }\n");
    assert!(after.contains("right-of a { fill:"), "{after}");
}

#[test]
fn a_box_that_already_says_where_it_goes_is_left_alone() {
    let source = "node a \"A\"\nnode b \"B\" below a gap 90\n";
    let (_, after) = frozen(source);
    assert_eq!(after, source, "a declared placement was rewritten");
}

#[test]
fn freezing_twice_changes_nothing_the_second_time() {
    let (_, once) = frozen("node a \"A\"\nnode b \"B\"\nnode c \"C\"");
    let (_, twice) = frozen(&once);
    assert_eq!(once, twice);
}

#[test]
fn a_brace_inside_a_label_is_not_a_block() {
    let (_, after) = frozen("node a \"A\"\nnode b \"a { brace\"\n");
    assert!(after.contains("\"a { brace\" right-of a"), "{after}");
}

// ------------------------------------------------- embedded font (I-058)

#[test]
fn base64_round_trips_through_a_decoder() {
    // Written out rather than pulled in, so it is worth proving against the
    // real rules: padded to a multiple of four, and the tail correct for a
    // length of one, two and three over.
    fn decode(text: &str) -> Vec<u8> {
        const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let bits: Vec<u32> = text
            .bytes()
            .filter(|b| *b != b'=')
            .map(|b| A.iter().position(|a| *a == b).expect("in the alphabet") as u32)
            .collect();
        let mut out = Vec::new();
        for chunk in bits.chunks(4) {
            let mut n = 0u32;
            for (i, v) in chunk.iter().enumerate() {
                n |= v << (18 - 6 * i);
            }
            let bytes = [(n >> 16) as u8, (n >> 8) as u8, n as u8];
            out.extend_from_slice(&bytes[..chunk.len() - 1]);
        }
        out
    }

    for original in [
        vec![],
        vec![0x00],
        vec![0x00, 0x01],
        vec![0x00, 0x01, 0x00],
        b"the quick brown fox".to_vec(),
        (0..=255u8).collect::<Vec<u8>>(),
    ] {
        let encoded = crate::render_svg::base64_for_test(&original);
        assert_eq!(encoded.len() % 4, 0, "not padded: {encoded}");
        assert_eq!(decode(&encoded), original, "{encoded}");
    }
}

#[test]
fn an_embedded_font_becomes_a_font_face_the_diagram_can_ask_for() {
    let l = {
        let mut l = build(r#"node a "A" { font: 14 "Whatever" }"#);
        // A TrueType magic number and nothing else: this is about the wiring,
        // not about the font.
        l.opts.embed_font = Some((
            "Whatever".into(),
            std::sync::Arc::new(vec![0x00, 0x01, 0x00, 0x00]),
        ));
        l
    };
    let svg = crate::render_svg::render(&l);
    assert!(svg.contains("@font-face"), "{svg}");
    assert!(svg.contains(r#"font-family:"Whatever""#), "{svg}");
    assert!(svg.contains("data:font/ttf;base64,AAEAAA"), "{svg}");
}

#[test]
fn a_diagram_with_no_embedded_font_carries_no_style_block() {
    // Off unless asked for: the file grows by the size of the font, and that
    // is a price only the person handing the file over should pay.
    let svg = crate::render_source(r#"node a "A""#).unwrap();
    assert!(!svg.contains("@font-face"), "{svg}");
}

// ------------------------------------------------- the wasm build (I-024)

#[test]
fn the_library_needs_no_argument_parser() {
    // What makes a wasm build sane: a page has no argv, and a library
    // consumer should not compile a parser it will never call. The binaries
    // declare `cli` in `required-features`, so `cargo build` still builds
    // them and `--no-default-features` resolves to four dependencies.
    let manifest = include_str!("../Cargo.toml");

    let clap = manifest
        .lines()
        .find(|l| l.starts_with("clap = "))
        .expect("clap is in the manifest");
    assert!(clap.contains("optional = true"), "{clap}");

    assert_eq!(
        manifest.matches("required-features = [\"cli\"]").count(),
        2,
        "both binaries must declare the feature that gives them their parser"
    );

    // And the library must emit a module at all, or there is nothing to load.
    assert!(
        manifest.contains("crate-type = [\"cdylib\", \"rlib\"]"),
        "the library cannot produce a wasm module"
    );
}

#[test]
fn the_playground_calls_the_module_the_build_script_writes() {
    // Two files that have to agree, in two languages, and neither would fail
    // loudly — the page would simply show nothing. The wrapper is the only
    // place in the app that knows a pointer exists, so it is the only place
    // this has to check.
    let wrapper = include_str!("../playground/src/lib/bgraph.js");
    let script = include_str!("../playground/build.sh");

    assert!(wrapper.contains("bgraph.wasm"), "the page loads no module");
    assert!(
        script.contains("playground/public/bgraph.wasm"),
        "the build script writes somewhere Vite does not serve"
    );

    for name in [
        "bgraph_alloc",
        "bgraph_free",
        "bgraph_render",
        "bgraph_check",
        "bgraph_examples",
        "bgraph_result_len",
    ] {
        assert!(wrapper.contains(name), "the page never calls `{name}`");
        assert!(
            include_str!("wasm.rs").contains(name),
            "`{name}` is not exported"
        );
    }
}

/// The single-file page is gone, and it has to stay gone in both directions:
/// a build script that still wrote one, or a manifest that still shipped one,
/// would leave half of `W-135` behind.
#[test]
fn the_playground_is_an_application_rather_than_one_file() {
    let script = include_str!("../playground/build.sh");
    assert!(!script.contains("--inline"), "the standalone page is back");
    assert!(!script.contains("standalone"), "{script}");

    let manifest = include_str!("../playground/package.json");
    for command in ["\"dev\"", "\"build\"", "\"wasm\""] {
        assert!(manifest.contains(command), "no {command} script");
    }
    assert!(manifest.contains("svelte"), "{manifest}");
    assert!(manifest.contains("packageManager"), "pnpm is not pinned");

    // The genre menu is the crate's own table, not a copy kept beside it.
    assert!(
        include_str!("../playground/src/App.svelte").contains("wasm.examples()"),
        "the playground carries its own list of genres"
    );

    // CodeMirror paints its own gutter, and for a light editor unless told
    // otherwise — which is how the line numbers came out brighter than the
    // code they number (`W-137`). Said through a theme, because a theme is
    // injected above the base and a stylesheet is a specificity race the
    // build order decides.
    let editor = include_str!("../playground/src/lib/Editor.svelte");
    assert!(
        editor.contains("cm-lineNumbers .cm-gutterElement"),
        "the line numbers are left to CodeMirror's own colour"
    );
    assert!(
        editor.contains("EditorView.theme") && editor.contains("{ dark },"),
        "the editor never tells CodeMirror which way round it is"
    );
}

// ------------------------------------------------- releases and CI (I-025, I-027)

#[test]
fn the_release_ships_everything_and_publishes_only_here() {
    // `D-035` reversed the second half of `D-032`: the artefacts go out as a
    // GitHub Release. What did not change is *where* — one place, this
    // repository's own releases page. crates.io and the two extension
    // marketplaces are a separate decision and nobody has taken it, so they
    // are named here rather than left to a future edit nobody reviews.
    let source = include_str!("../.github/workflows/release.yml");
    // Commands, not prose. This file *explains* which registries it stays off
    // — naming them is the point — so a search over the comments would find
    // every one of them and prove nothing.
    let release: String = source
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        release.contains("gh release create"),
        "the workflow builds a release and never makes one"
    );
    for forbidden in [
        "cargo publish",
        "vsce publish",
        "ovsx publish",
        "npm publish",
    ] {
        assert!(
            !release.contains(forbidden),
            "the workflow publishes outward: `{forbidden}`"
        );
    }
    assert!(
        include_str!("../Cargo.toml").contains("publish = false"),
        "crates.io would refuse on a comment rather than on a line"
    );

    // Test, then build, then release — and the last one only on a tag, so a
    // `workflow_dispatch` run is a full rehearsal that publishes nothing.
    for job in [
        "  test:",
        "  binaries:",
        "  extensions:",
        "  wasm:",
        "  release:",
    ] {
        assert!(release.contains(job), "no `{job}` job");
    }
    assert!(
        release.contains("needs: [test, binaries, extensions, wasm]"),
        "the release does not wait for everything that feeds it"
    );
    assert!(
        release.contains("if: github.ref_type == 'tag'"),
        "a manual run would publish"
    );

    // Every artefact the notes promise. A release that lists five things and
    // ships four is worse than one that never claimed the fifth.
    for artefact in [
        "bgraph-lsp",
        "bgraph-vscode-",
        "bgraph-zed-",
        "bgraph-typst-",
        "bgraph-playground-",
    ] {
        assert!(release.contains(artefact), "`{artefact}` is not built");
    }

    // The Zed manifest pins its grammar at a path on the author's disk, which
    // is right for `install.sh` and useless to anybody else.
    assert!(
        release.contains("the released manifest still points at a local path"),
        "the Zed extension would ship pinned to a `file://` path"
    );

    // One build per platform: the native window that made it two is gone.
    assert!(!release.contains("--features window"), "{release}");
    assert!(!release.contains("libwayland-dev"), "{release}");
}

/// One licence, in one file, and five manifests that name it.
///
/// `D-038`: written for this project after the standard texts were tried and
/// each one gave up something the author wanted. Two properties are worth a
/// test rather than a reading. The four grants have to still be there — use,
/// passing on a build, and the two refusals that pay for them — because a
/// licence loses a clause the way any other text does, in an edit that looked
/// small. And it must not claim to be somebody else's: it is modelled on the
/// PolyForm shape and says so, and PolyForm's own terms are that a text
/// derived from theirs stops using their name.
#[test]
fn every_manifest_names_the_licence_the_repository_carries() {
    const ID: &str = "LicenseRef-bgraph-noncommercial-1.0";

    let licence = include_str!("../LICENSE");

    assert!(
        licence.starts_with("bgraph Noncommercial License 1.0\n\nCopyright 2026"),
        "LICENSE does not name itself, or carries no copyright line"
    );

    // The rights holder, in one form, everywhere. `D-039` put a pseudonym in
    // the notice; a manifest still carrying a different name would undo that
    // in the one file a package manager reads and a reader never opens.
    //
    // Asserted by agreement rather than by naming what must be absent: a test
    // that spelled out the name being replaced would put it straight back
    // into the tree it was taken out of.
    let holder = licence
        .lines()
        .find_map(|l| l.strip_prefix("Copyright 2026 "))
        .map(|l| l.trim_end_matches('.').to_string())
        .expect("a copyright line naming a holder");
    for (what, text) in [
        ("the crate", include_str!("../Cargo.toml")),
        ("the Typst package", include_str!("../typst/typst.toml")),
        (
            "the Zed extension",
            include_str!("../editors/zed/extension.toml"),
        ),
    ] {
        let authors = text
            .lines()
            .find(|l| l.starts_with("authors = "))
            .unwrap_or_else(|| panic!("{what} names no author"));
        assert!(
            authors.contains(&holder),
            "{what} says {authors}, and LICENSE says {holder}"
        );
    }

    // What it grants, and what it holds back. Each of these was somebody's
    // answer to a question, and a heading going missing is that answer being
    // dropped without anyone deciding to.
    for clause in [
        "## Copyright Licence",
        "## Permitted Purposes",
        "## Running It For Others",
        "## Passing On A Build",
        "## The Source Is Confidential",
        "## No Changes",
        "## Patent Licence",
        "## Violations",
        "## No Liability",
        "## Definitions",
    ] {
        assert!(
            licence.contains(clause),
            "`{clause}` is missing from LICENSE"
        );
    }
    for said in [
        "Commercial use is not permitted",
        "unchanged",
        "personal to",
        "You may not modify the software",
    ] {
        assert!(licence.contains(said), "LICENSE no longer says `{said}`");
    }

    // Modelled on PolyForm's shape, and saying so — their terms are that a
    // text derived from theirs stops using their name, so it must not read as
    // one of theirs anywhere else.
    assert!(
        licence.contains("in the shape of the PolyForm licences")
            && licence.contains("it is not endorsed by them"),
        "the licence borrows a shape without saying whose, or without disclaiming it"
    );
    assert!(
        !licence.contains("PolyForm Noncommercial License")
            && !licence.contains("PolyForm Strict License")
            && !licence.contains("polyformproject.org"),
        "the licence presents itself as a PolyForm licence"
    );

    for (what, text) in [
        ("the crate", include_str!("../Cargo.toml")),
        ("the Typst package", include_str!("../typst/typst.toml")),
        (
            "the Zed extension's crate",
            include_str!("../editors/zed/Cargo.toml"),
        ),
        (
            "the VS Code extension",
            include_str!("../editors/vscode/package.json"),
        ),
    ] {
        assert!(text.contains(ID), "{what} does not name `{ID}`");
    }

    // It travels with everything the release hands out — a build passed on
    // without it is a build passed on outside the clause that allows it.
    let release = include_str!("../.github/workflows/release.yml");
    assert_eq!(
        release.matches("cp LICENSE").count(),
        3,
        "an archive ships without the licence"
    );
    assert!(
        include_str!("../editors/vscode/package.sh").contains("LICENSE"),
        "the VSIX ships without the licence"
    );

    // crates.io and the marketplaces are still a separate decision.
    assert!(
        include_str!("../Cargo.toml").contains("publish = false"),
        "a licence is not a decision to publish"
    );

    let notes = std::fs::read_to_string(
        std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/release-notes"))
            .join(format!("{}.md", env!("CARGO_PKG_VERSION"))),
    )
    .expect("release notes");
    assert!(
        notes.contains("bgraph Noncommercial License 1.0"),
        "the notes do not say what licence the artefacts come under"
    );
    for said in ["separate agreement", "pass on a build", "confidential"] {
        assert!(notes.contains(said), "the notes never mention `{said}`");
    }
}

/// One version, in five files.
///
/// A version written in five places is a version that is wrong in four of
/// them, and the one that is wrong is the one somebody reads.
#[test]
fn every_manifest_agrees_on_the_version() {
    let crate_version = env!("CARGO_PKG_VERSION");

    let vscode: serde_json::Value =
        serde_json::from_str(include_str!("../editors/vscode/package.json")).expect("valid JSON");
    assert_eq!(
        vscode["version"], crate_version,
        "the VS Code extension is a different version from the compiler in it"
    );

    for (what, text) in [
        (
            "the Zed extension",
            include_str!("../editors/zed/extension.toml"),
        ),
        ("the Typst package", include_str!("../typst/typst.toml")),
        (
            "the Zed extension's crate",
            include_str!("../editors/zed/Cargo.toml"),
        ),
    ] {
        let line = text
            .lines()
            .find(|l| l.starts_with("version = "))
            .unwrap_or_else(|| panic!("{what} declares no version"));
        assert!(
            line.contains(crate_version),
            "{what} says {line}, the compiler says {crate_version}"
        );
    }

    // And the notes, which are what a reader is handed.
    let notes = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/release-notes"))
        .join(format!("{crate_version}.md"));
    assert!(
        notes.exists(),
        "no release notes for {crate_version} — write docs/release-notes/{crate_version}.md"
    );
    let notes = std::fs::read_to_string(&notes).expect("readable");
    for artefact in ["vsix", "zed", "typst", "playground", "bgraph-lsp"] {
        assert!(
            notes.contains(artefact),
            "the notes never mention `{artefact}`"
        );
    }
    // The count in the prose, which is the sentence a reader trusts.
    assert!(
        notes.contains("29 specialised genres, plus `free`")
            && crate::props::ARCHETYPES.len() - 1 == 29,
        "the notes give a genre count the compiler does not"
    );
}

#[test]
fn every_public_document_uses_the_same_genre_count() {
    let phrase = "29 specialised genres, plus `free`";
    for (name, text) in [
        ("README", include_str!("../README.md")),
        ("reference", include_str!("../docs/reference.md")),
        ("manual", include_str!("../docs/manual.typ")),
        ("vocabulary", include_str!("../docs/vocabulary.md")),
        (
            "release notes",
            include_str!("../docs/release-notes/0.1.0.md"),
        ),
    ] {
        assert!(
            text.contains(phrase),
            "{name} does not use the canonical count"
        );
    }
    assert_eq!(
        crate::props::ARCHETYPES.len(),
        30,
        "one `free` archetype plus 29 specialised genres"
    );
}

#[test]
fn the_embedding_api_boundary_is_documented_and_exercised() {
    let api = include_str!("../docs/API.md");
    for name in [
        "render_source",
        "render_file",
        "Template",
        "Bindings",
        "parser",
        "check",
    ] {
        assert!(api.contains(name), "the embedding guide omits `{name}`");
    }
    let library = include_str!("lib.rs");
    assert!(library.contains("# What semver covers"));
    assert!(library.contains("public to be inspected, not\n//! relied on"));
}

#[test]
fn the_readme_onboarding_uses_supported_commands_and_examples() {
    let readme = include_str!("../README.md");
    for command in [
        "bgraph init free",
        "bgraph architecture.bd -o architecture.svg",
        "bgraph watch architecture.bd",
        "bgraph check architecture.bd",
        "--theme light|dark|print",
        "type pie",
    ] {
        assert!(
            readme.contains(command),
            "onboarding command `{command}` is missing"
        );
    }
    for source in [
        "node browser \"Browser\"",
        "node api \"API\"",
        "browser -> api",
    ] {
        assert!(
            parse_diagram(source).is_ok(),
            "README example no longer parses: {source}"
        );
    }
}

#[test]
fn the_action_uses_the_flat_diagnostics_a_parser_wants() {
    // A caret and a colour are for a terminal. An annotation parser wants
    // `file:line:col`, which is exactly what `--ci` pins.
    let action = include_str!("../.github/actions/bgraph/action.yml");
    assert!(action.contains("check"), "the action never checks anything");
    assert!(
        action.contains("--ci"),
        "the action reads the rich form, which nothing parses"
    );
    assert!(action.contains("fmt --check"), "{action}");
}

#[test]
fn the_manifest_still_refuses_to_publish_itself() {
    // `Q-005` is unanswered, and until it is, this is the line that keeps an
    // accident from becoming a distribution.
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("publish = false"), "{manifest}");
}

#[test]
fn ci_builds_the_small_feature_surfaces_the_library_promises() {
    let ci = include_str!("../.github/workflows/ci.yml");
    assert!(ci.contains("feature-builds"), "CI has no feature-build job");
    assert!(
        ci.contains("cargo build --no-default-features"),
        "CI skips the minimal build"
    );
    for features in [
        "pie",
        "packet",
        "packet,json,csv,pdf",
        "wasm",
        "png",
        "cli,completions",
    ] {
        assert!(ci.contains(features), "CI does not build `{features}`");
    }
}

#[test]
fn the_generated_vocabulary_schema_matches_the_compiler_table() {
    let schema: serde_json::Value =
        serde_json::from_str(&crate::docgen::schema()).expect("schema is JSON");
    assert_eq!(schema["schemaVersion"], 1);
    assert_eq!(schema["compilerVersion"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        schema["archetypes"].as_array().map(Vec::len),
        Some(crate::props::ARCHETYPES.len())
    );
    assert_eq!(
        schema["properties"].as_array().map(Vec::len),
        Some(crate::props::PROPS.len())
    );
}

// ------------------------------------------------- Markdown preview (I-023)

#[test]
fn the_vs_code_extension_renders_a_bgraph_fence() {
    // The single largest reason Mermaid gets used instead of something
    // better: a diagram you can only see by leaving the document is one you
    // stop keeping current.
    let manifest = include_str!("../editors/vscode/package.json");
    let extension = include_str!("../editors/vscode/src/extension.js");

    assert!(
        manifest.contains("\"markdown.markdownItPlugins\": true"),
        "the extension does not offer itself to the Markdown preview"
    );
    assert!(
        manifest.contains("onLanguage:markdown"),
        "it would never activate on a Markdown file"
    );
    assert!(
        extension.contains("extendMarkdownIt"),
        "nothing is exported for the preview to call"
    );
}

#[test]
fn the_preview_renders_through_the_bundled_module_not_a_binary() {
    // A preview that only worked for people who had already installed the CLI
    // would miss exactly the readers it is for.
    let extension = include_str!("../editors/vscode/src/extension.js");
    let packaging = include_str!("../editors/vscode/package.sh");

    assert!(extension.contains("bgraph.wasm"), "{extension}");
    assert!(
        packaging.contains("bgraph.wasm"),
        "the VSIX would ship without the renderer"
    );
    // And a missing module must degrade to a code block rather than take the
    // document's rendering down with it.
    assert!(
        extension.contains("fallback(tokens, index, options, env, self)"),
        "there is no fallback for a fence it cannot draw"
    );
}

// ------------------------------------------------- the Typst plugin (I-023)

#[test]
fn the_typst_module_is_built_apart_from_the_general_one() {
    // Typst's protocol works through *imports* the host supplies, so a module
    // built with them cannot be instantiated by anything else — the
    // playground and the Markdown preview both pass an empty import object
    // and would simply fail to load it. Two modules, and the manifest has to
    // keep them apart.
    let manifest = include_str!("../Cargo.toml");
    assert!(manifest.contains("typst = [\"wasm\"]"), "{manifest}");

    let general = include_str!("../playground/build.sh");
    let for_typst = include_str!("../typst/build.sh");
    assert!(
        !general.contains("typst"),
        "the playground's module would carry imports nothing supplies"
    );
    assert!(
        for_typst.contains("features all-archetypes,json,typst"),
        "{for_typst}"
    );
}

#[test]
fn the_typst_package_calls_the_entry_point_that_exists() {
    // Two files in two languages that have to agree on a symbol, and neither
    // fails loudly — Typst would say "plugin function takes 2 arguments",
    // which is what the first attempt said.
    let package = include_str!("../typst/bgraph.typ");
    let wasm = include_str!("wasm.rs");

    assert!(package.contains("bgraph_typst_render"), "{package}");
    assert!(wasm.contains("bgraph_typst_render"), "{wasm}");
    assert!(
        package.contains("plugin(\"bgraph.wasm\")"),
        "the package loads a module its build script does not write"
    );
}

// ------------------------------------------------- font subsetting (I-058)

/// Any TrueType font on this machine. Every platform this builds on ships
/// some; a machine with none is a machine this test cannot speak about, and
/// saying so beats passing silently.
fn some_font() -> (std::path::PathBuf, Vec<u8>) {
    let roots = [
        "/usr/share/fonts",
        "/usr/local/share/fonts",
        "/System/Library/Fonts",
        "C:\\Windows\\Fonts",
    ];
    let mut stack: Vec<std::path::PathBuf> = roots.iter().map(std::path::PathBuf::from).collect();
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "ttf")
                && let Ok(bytes) = std::fs::read(&path)
                // A collection and a CFF font are both `.ttf` sometimes, and
                // neither is what this subsets.
                && bytes.starts_with(&[0x00, 0x01, 0x00, 0x00])
            {
                return (path, bytes);
            }
        }
    }
    panic!("no TrueType font found under {roots:?} — this test cannot run here");
}

#[test]
fn a_subset_font_is_smaller_and_still_a_font() {
    let (path, font) = some_font();
    let subset = crate::subset::keep_glyphs_for(&font, "Ligature: office");

    assert!(
        subset.len() < font.len(),
        "{} did not shrink: {} → {}",
        path.display(),
        font.len(),
        subset.len()
    );
    assert_eq!(&subset[..4], &font[..4], "the version was rewritten");

    // The table directory must still be sorted by tag and point inside the
    // file. A reader may find a table by binary search, and one that walks off
    // the end reads whatever follows.
    let count = u16::from_be_bytes([subset[4], subset[5]]) as usize;
    let mut previous = [0u8; 4];
    for i in 0..count {
        let at = 12 + i * 16;
        let tag: [u8; 4] = subset[at..at + 4].try_into().unwrap();
        assert!(tag > previous, "the directory is out of order at {i}");
        previous = tag;
        let offset = u32::from_be_bytes(subset[at + 8..at + 12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(subset[at + 12..at + 16].try_into().unwrap()) as usize;
        assert!(
            offset + length <= subset.len(),
            "table {} runs past the end of the file",
            String::from_utf8_lossy(&tag)
        );
    }
}

#[test]
fn every_table_the_reader_needs_survives() {
    // Nothing is renumbered, so nothing may be dropped either: `cmap` still
    // maps characters to the ids `hmtx` still gives widths for.
    let (_, font) = some_font();
    let subset = crate::subset::keep_glyphs_for(&font, "abc");

    let tags = |bytes: &[u8]| -> Vec<String> {
        let count = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
        (0..count)
            .map(|i| String::from_utf8_lossy(&bytes[12 + i * 16..16 + i * 16]).into_owned())
            .collect()
    };
    let mut before = tags(&font);
    let mut after = tags(&subset);
    before.sort();
    after.sort();
    assert_eq!(before, after, "a table went missing");
}

#[test]
fn anything_it_does_not_understand_comes_back_whole() {
    // A font that is a little too large is a nuisance. A font that renders
    // the wrong glyph is a lie, so anything unfamiliar is handed back.
    for bytes in [
        b"OTTO\x00\x00\x00\x00".to_vec(), // CFF outlines
        b"ttcf\x00\x01\x00\x00".to_vec(), // a collection
        b"not a font at all".to_vec(),
        Vec::new(),
    ] {
        assert_eq!(
            crate::subset::keep_glyphs_for(&bytes, "abc"),
            bytes,
            "something unfamiliar was rewritten"
        );
    }
}

// ------------------------------------------------- the server and `include`

#[test]
fn the_server_does_not_underline_a_name_borrowed_from_another_file() {
    // Without composing, a file using `include` shows an error on every name
    // it borrows: the composed diagram is valid and the buffer alone is not.
    // An editor that underlines correct code is one people turn off.
    let dir = scratch("lsp-include");
    std::fs::write(dir.join("core.bd"), "node api \"API\"\n").unwrap();
    let entry = dir.join("main.bd");
    let text = "include \"core.bd\" as core\nnode ui \"UI\"\nui -> core.api\n";
    std::fs::write(&entry, text).unwrap();

    let alone = crate::lsp::diagnostics(text);
    assert!(
        alone.iter().any(|d| d["message"]
            .as_str()
            .is_some_and(|m| m.contains("core.api"))),
        "the buffer on its own really is invalid, which is the whole point"
    );

    let composed = crate::lsp::diagnostics_for(text, Some(&entry));
    assert!(
        !composed.iter().any(|d| d["message"]
            .as_str()
            .is_some_and(|m| m.contains("core.api"))),
        "still underlined after composing: {composed:?}"
    );
}

#[test]
fn a_problem_in_an_included_file_is_not_reported_against_this_one() {
    // LSP publishes per document. A diagnostic from another file shown here
    // would underline the wrong line of the wrong buffer.
    let dir = scratch("lsp-include-other");
    std::fs::write(dir.join("core.bd"), "node api \"API\" { fil: #fff }\n").unwrap();
    let entry = dir.join("main.bd");
    let text = "include \"core.bd\"\nnode ui \"UI\"\n";
    std::fs::write(&entry, text).unwrap();

    let reported = crate::lsp::diagnostics_for(text, Some(&entry));
    assert!(
        !reported
            .iter()
            .any(|d| d["message"].as_str().is_some_and(|m| m.contains("fil"))),
        "somebody else's mistake was published here: {reported:?}"
    );
}

#[test]
fn the_buffer_wins_over_what_is_on_disk() {
    // The file on disk is a version of the buffer from before the last
    // keystroke. Composing from disk would report against text nobody sees.
    let dir = scratch("lsp-include-buffer");
    std::fs::write(dir.join("core.bd"), "node api \"API\"\n").unwrap();
    let entry = dir.join("main.bd");
    std::fs::write(&entry, "include \"core.bd\"\n").unwrap();

    // Unsaved: the buffer names something that does not exist.
    let text = "include \"core.bd\"\nnode ui \"UI\"\nui -> ghost\n";
    let reported = crate::lsp::diagnostics_for(text, Some(&entry));
    assert!(
        reported
            .iter()
            .any(|d| d["message"].as_str().is_some_and(|m| m.contains("ghost"))),
        "the server read the disk instead of the buffer: {reported:?}"
    );
}

#[test]
fn a_file_uri_becomes_a_path() {
    assert_eq!(
        crate::lsp::path_of("file:///home/a/b.bd"),
        Some(std::path::PathBuf::from("/home/a/b.bd"))
    );
    // A path with a space in it is ordinary.
    assert_eq!(
        crate::lsp::path_of("file:///home/a%20b/c.bd"),
        Some(std::path::PathBuf::from("/home/a b/c.bd"))
    );
    // Anything else means the buffer is checked on its own, which is what
    // every editor did before `include` existed.
    assert_eq!(crate::lsp::path_of("untitled:Untitled-1"), None);
}
