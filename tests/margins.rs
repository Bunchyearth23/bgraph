//! W-166: spacing contracts checked on the geometry shared by every writer.

use bgraph::layout::Layout;

fn drawing(source: &str) -> Layout {
    let analysis = bgraph::analyze_source(source);
    assert!(
        !analysis.has_errors(),
        "{source}\n{:?}",
        analysis.diagnostics
    );
    analysis.layout.unwrap()
}

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-7, "{actual} != {expected}");
}

fn sides(style: &bgraph::style::Style) -> [f64; 4] {
    [
        style.margin_top.unwrap_or(0.0),
        style.margin_right.unwrap_or(0.0),
        style.margin_bottom.unwrap_or(0.0),
        style.margin_left.unwrap_or(0.0),
    ]
}

#[test]
fn margin_shorthand_and_side_overrides_follow_the_cascade() {
    for (value, expected) in [
        ("5", [5.0, 5.0, 5.0, 5.0]),
        ("5 7", [5.0, 7.0, 5.0, 7.0]),
        ("5 7 11", [5.0, 7.0, 11.0, 7.0]),
        ("5 7 11 13", [5.0, 7.0, 11.0, 13.0]),
        ("5px 7% 11px 13", [5.0, 7.0, 11.0, 13.0]),
    ] {
        let layout = drawing(&format!("node a {{ margin: {value} }}"));
        assert_eq!(sides(&layout.nodes[0].style), expected, "{value}");
    }
    let layout = drawing(
        "style node { margin: 1 2 3 4 }\n\
         style base { margin-top: 10 }\n\
         style chosen extends base { margin-right: 20 }\n\
         style last { margin-bottom: 30 }\n\
         node a .chosen .last { margin-left: 40 }",
    );
    assert_eq!(sides(&layout.nodes[0].style), [10.0, 20.0, 30.0, 40.0]);
    let layout = drawing(
        "style first { margin: 1 2 3 4 }\n\
         node a .first { margin: 9 margin-left: 12 }\n\
         node b .first { margin-left: 12 margin: 9 }",
    );
    assert_eq!(sides(&layout.nodes[0].style), [9.0, 9.0, 9.0, 12.0]);
    assert_eq!(sides(&layout.nodes[1].style), [9.0; 4]);
}

#[test]
fn invalid_margins_are_diagnostics_and_managed_genres_refuse_them() {
    for prop in [
        "margin: -1",
        "margin: 1 2 3 4 5",
        "margin: auto",
        "margin-right: 1 2",
        "margin-bottom: -4",
    ] {
        let source = format!("node a {{ {prop} }}");
        let analysis = bgraph::analyze_source(&source);
        assert!(analysis.has_errors(), "{prop}");
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|d| d.message.contains("finite nonnegative")),
            "{prop}: {:?}",
            analysis.diagnostics
        );
    }
    for def in bgraph::props::ARCHETYPES
        .iter()
        .filter(|def| !def.layout.accepts_layout_block() && def.name != "sequence")
    {
        let source = format!("type {}\nstyle node {{ margin: 4 }}", def.name);
        let diagram = bgraph::parser::parse(&source).unwrap();
        let diagnostics = bgraph::check::check(&diagram);
        assert!(
            diagnostics.iter().any(|d| d
                .message
                .contains("supported only on declared-layout boxes/frames")),
            "{}: {diagnostics:?}",
            def.name
        );
    }
}

#[test]
fn declared_genres_add_adjacent_margins_to_relative_gaps() {
    for def in bgraph::props::ARCHETYPES
        .iter()
        .filter(|def| def.layout.accepts_layout_block())
    {
        for (direction, alignment, dx, dy) in [
            ("right-of", "top", 40.0 + 10.0 + 7.0 + 23.0, 0.0),
            ("left-of", "top", -(20.0 + 10.0 + 13.0 + 19.0), 0.0),
            ("below", "left", 0.0, 30.0 + 10.0 + 11.0 + 17.0),
            ("above", "left", 0.0, -(20.0 + 10.0 + 5.0 + 29.0)),
        ] {
            let source = format!(
                "type {}\n\
                 {} a {{ size: 40 30 margin: 5 7 11 13 }}\n\
                 {} b {{ size: 20 20 margin: 17 19 29 23 }}\n\
                 layout {{\n\
                   place a at (100, 100)\n\
                   place b {direction} a gap 10 align {alignment}\n\
                 }}",
                def.name, def.item, def.item
            );
            let layout = drawing(&source);
            let a = layout.nodes.iter().find(|n| n.id == "a").unwrap().rect;
            let b = layout.nodes.iter().find(|n| n.id == "b").unwrap().rect;
            near(a.x, 100.0);
            near(a.y, 100.0);
            near(b.x, a.x + dx);
            near(b.y, a.y + dy);
        }
    }
}

#[test]
fn rows_columns_and_implicit_flow_include_adjacent_margins() {
    let nodes = "style node { size: 40 30 margin: 5 7 11 13 }\n\
                 node a\nnode b\nnode c\n";
    let layout = drawing(&format!(
        "{nodes}layout {{\nplace a at (0, 0)\nrow a b gap 24\ncolumn b c gap 18\n}}"
    ));
    near(layout.nodes[1].rect.x, 40.0 + 7.0 + 13.0 + 24.0);
    near(layout.nodes[2].rect.y, 30.0 + 11.0 + 5.0 + 18.0);
    let base = drawing("style node { size: 40 30 }\nnode a\nnode b\n");
    let margin = drawing(nodes);
    near(
        margin.nodes[1].rect.x - margin.nodes[0].rect.right(),
        base.nodes[1].rect.x - base.nodes[0].rect.right() + 7.0 + 13.0,
    );
}

#[test]
fn containers_and_canvas_include_asymmetric_margins() {
    let layout = drawing(
        "graph { padding: 0 }\n\
         group outer {\n\
           padding: 10\nmargin: 17 19 23 29\n\
           node a { size: 40 30 margin: 5 7 11 13 }\n\
           layout { place a at (100, 100) }\n\
         }",
    );
    let node = layout.nodes[0].rect;
    let group = layout.groups[0].rect;
    near(node.x - group.x, 10.0 + 13.0);
    near(node.y - group.y, 10.0 + 5.0);
    near(group.right() - node.right(), 10.0 + 7.0);
    near(group.bottom() - node.bottom(), 10.0 + 11.0);
    let (tx, ty, w, h) = bgraph::render_svg::canvas(&layout);
    near(group.x + tx, 29.0);
    near(group.y + ty, 17.0);
    near(w, group.w + 29.0 + 19.0);
    near(h, group.h + 17.0 + 23.0);
}

#[cfg(feature = "sequence")]
fn frame<'a>(layout: &'a Layout, label: &str) -> &'a bgraph::layout::PlacedGroup {
    layout
        .groups
        .iter()
        .find(|group| group.lines == [label])
        .unwrap()
}

#[cfg(feature = "sequence")]
const PARTICIPANTS: &str = "type sequence\nparticipant a \"A\"\nparticipant b \"B\"\n";

#[cfg(feature = "sequence")]
#[test]
fn the_first_sequence_frame_clears_headers_and_both_margins() {
    for margin in [0.0, 20.0] {
        let layout = drawing(&format!(
            "type sequence\nstyle participant {{ margin-bottom: 13 }}\n\
             participant a \"A\"\nparticipant b \"B\"\n\
             group demands \"Demands\" {{\nmargin-top: {margin}\na -> b\n}}"
        ));
        let header_bottom = layout.nodes[..2]
            .iter()
            .map(|node| node.rect.bottom() + 13.0)
            .fold(0.0, f64::max);
        assert!(frame(&layout, "Demands").rect.y >= header_bottom + margin);
    }
}

#[cfg(feature = "sequence")]
#[test]
fn sequence_participant_columns_and_headers_fit_multiline_labels_and_margins() {
    let layout = drawing(
        "type sequence { column: 60 }\n\
         style participant { margin: 7 19 23 31 }\n\
         participant a \"A participant with a very long name\\nSecond line\"\n\
         participant b \"Another long participant name\"\n\
         a -> b \"Message\"",
    );
    let (a, b) = (layout.nodes[0].rect, layout.nodes[1].rect);
    assert!(b.x - a.right() >= 19.0 + 31.0);
    assert!(a.h >= 2.0 * layout.nodes[0].style.font_size.unwrap_or(13.0) * 1.2);
    assert!(layout.polylines[0].points[0].1 >= a.bottom().max(b.bottom()) + 23.0);
    near(a.y, 7.0);
}

#[cfg(feature = "sequence")]
#[test]
fn sequence_sibling_frames_reserve_both_external_margins() {
    let layout = drawing(&format!(
        "{PARTICIPANTS}\
         group first \"First\" {{\nmargin-bottom: 37\na -> b\n}}\n\
         group second \"Second\" {{\nmargin-top: 29\nb -> a\n}}\n"
    ));
    assert!(frame(&layout, "Second").rect.y - frame(&layout, "First").rect.bottom() >= 37.0 + 29.0);
}

#[cfg(feature = "sequence")]
#[test]
fn sequence_parent_frames_enclose_descendants_and_their_margins() {
    let layout = drawing(&format!(
        "{PARTICIPANTS}\
         group parent \"Parent\" {{\npadding: 17\n\
           group child \"Child\" {{\nmargin: 19 23 29 31\na -> b\n}}\n\
         }}"
    ));
    let parent = frame(&layout, "Parent").rect;
    let child = frame(&layout, "Child").rect;
    assert!(child.x - parent.x >= 31.0 + 17.0);
    assert!(child.y - parent.y >= 19.0 + 17.0);
    assert!(parent.right() - child.right() >= 23.0 + 17.0);
    assert!(parent.bottom() - child.bottom() >= 29.0 + 17.0);
}

#[cfg(feature = "sequence")]
#[test]
fn sequence_activation_extents_clear_following_frames() {
    let layout = drawing(&format!(
        "{PARTICIPANTS}\
         style participant {{ margin: 100 }}\n\
         activate b {{\na -> b\n}}\n\
         group next \"Next\" {{\nmargin-top: 37\nb -> a\n}}"
    ));
    let bar = layout.nodes.iter().find(|node| node.id.is_empty()).unwrap();
    assert_eq!(sides(&bar.style), [0.0; 4]);
    assert!(frame(&layout, "Next").rect.y >= bar.rect.bottom() + 37.0);
}

#[cfg(feature = "sequence")]
#[test]
fn sequence_frame_titles_clear_large_message_labels() {
    for (row, title, font_size) in [
        (46, "\"Frame title\"", 60),
        (20, "\"Frame title\"", 12),
        (20, "", 12),
    ] {
        let layout = drawing(&format!(
            "type sequence {{ row: {row} }}\nparticipant a\nparticipant b\n\
             group outer {title} {{\npadding: 18\na -> b \"Label\" {{ font-size: {font_size} }}\n}}"
        ));
        let frame = &layout.groups[0];
        let label = &layout.annotations[0];
        let label_top = label.y - label.size / 2.0;
        assert!(
            label_top > frame.rect.y + frame.style.font_size.unwrap_or(12.0) * 1.8,
            "message label starts at {label_top}, frame starts at {}",
            frame.rect.y
        );
        assert!(label.y + label.size / 2.0 < frame.rect.bottom());
    }
}
