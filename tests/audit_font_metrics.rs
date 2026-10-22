use bgraph::layout::{Anchor, Layout};
use std::sync::Arc;

const FONT: &[u8] = include_bytes!("fixtures/fonts/DejaVuSans.ttf");
const FAMILY: &str = "DejaVuSans";
const TITLE: &str = "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW";

fn width(size: f64) -> f64 {
    // Fixed DejaVu Sans 2.37 fixture: W advances 2025 units; one em is 2048.
    TITLE.len() as f64 * 2025.0 * size / 2048.0
}

fn drawing(source: &str) -> Layout {
    bgraph::analyze_source(source).into_layout().unwrap()
}

fn embed(layout: &mut Layout) {
    layout.opts.embed_font = Some((FAMILY.into(), Arc::new(FONT.to_vec())));
}

fn plate(svg: &str) -> &str {
    let text_end = svg.find(&format!(">{TITLE}</text>")).unwrap();
    let start = svg[..text_end].rfind("<rect ").unwrap();
    &svg[start..start + svg[start..].find('>').unwrap() + 1]
}

fn attribute(tag: &str, name: &str) -> f64 {
    tag.split_once(&format!("{name}=\""))
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn embedded_group_title_plates_follow_the_selected_face_without_moving_nodes() {
    let mut layout = drawing(&format!(
        "style heading {{ font: 20 \"{FAMILY}\" font-weight: 400 }}\ngroup frame \"{TITLE}\" .heading {{ node a }}\n"
    ));
    let initial_nodes: Vec<_> = layout
        .nodes
        .iter()
        .map(|n| (n.rect.x, n.rect.y, n.rect.w, n.rect.h))
        .collect();
    let initial_group = layout.groups[0].rect;
    let estimated_svg = bgraph::render_svg::render(&layout);
    let estimated = attribute(plate(&estimated_svg), "width");
    embed(&mut layout);
    let svg = bgraph::render_svg::render(&layout);
    let plate = plate(&svg);
    let measured = attribute(plate, "width");
    assert!((measured - (width(20.0) + 6.0)).abs() < 0.011, "{plate}");
    assert!(measured > estimated + 20.0);
    let (dx, _, canvas_width, _) = bgraph::render_svg::canvas(&layout);
    assert!(dx + attribute(plate, "x") + measured <= canvas_width - layout.opts.padding + 0.02);
    assert_eq!(
        initial_nodes,
        layout
            .nodes
            .iter()
            .map(|n| (n.rect.x, n.rect.y, n.rect.w, n.rect.h))
            .collect::<Vec<_>>()
    );
    let group = layout.groups[0].rect;
    assert_eq!(
        (
            initial_group.x,
            initial_group.y,
            initial_group.w,
            initial_group.h
        ),
        (group.x, group.y, group.w, group.h)
    );
}

#[test]
fn heading_bounds_include_embedded_advances_for_each_alignment() {
    for align in ["left", "center", "right"] {
        let mut layout = drawing(&format!(
            "graph {{ title: \"{TITLE}\" title-align: {align} }}\nstyle node {{ font-family: \"{FAMILY}\" }}\nnode a\n"
        ));
        embed(&mut layout);
        let title = layout.annotations.iter().find(|a| a.title).unwrap();
        let w = width(title.size);
        let left = match title.anchor {
            Anchor::Start => title.x,
            Anchor::Middle => title.x - w / 2.0,
            Anchor::End => title.x - w,
        };
        let (dx, _, canvas_width, _) = bgraph::render_svg::canvas(&layout);
        assert!(left + dx >= layout.opts.padding - 0.01, "{align}");
        assert!(
            left + w + dx <= canvas_width - layout.opts.padding + 0.01,
            "{align}"
        );
        let svg = bgraph::render_svg::render(&layout);
        assert!((attribute(&svg[..svg.find('>').unwrap()], "width") - canvas_width).abs() < 0.011);
    }
}

#[test]
fn unrelated_or_unreadable_embedded_fonts_keep_the_existing_estimate() {
    let mut layout = drawing(&format!("group frame \"{TITLE}\" {{ node a }}\n"));
    let baseline = bgraph::render_svg::render(&layout);
    let baseline_plate = plate(&baseline);
    let baseline_canvas = bgraph::render_svg::canvas(&layout);
    embed(&mut layout);
    let other_family = bgraph::render_svg::render(&layout);
    assert_eq!(baseline_plate, plate(&other_family));
    assert_eq!(baseline_canvas, bgraph::render_svg::canvas(&layout));
    layout.groups[0].style.font_family = Some(FAMILY.into());
    layout.opts.embed_font = Some((FAMILY.into(), Arc::new(vec![0, 1, 0, 0])));
    let invalid = bgraph::render_svg::render(&layout);
    assert_eq!(baseline_plate, plate(&invalid));
    assert_eq!(baseline_canvas, bgraph::render_svg::canvas(&layout));
}
