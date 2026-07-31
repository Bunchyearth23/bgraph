//! The non-`free` archetypes.
//!
//! Both reinterpret the same `node` statements as chart data, and both reuse
//! the style cascade — a slice or a bitfield is styled exactly like a box.
//! `packet` needs no renderer support at all: it emits ordinary rects.

use std::collections::HashMap;

use anyhow::Result;

use crate::ast::{Diagram, Prop, Value};
use crate::layout::{
    Anchor, Annotation, Chart, Guide, Layout, Pie, PlacedGroup, PlacedNode, Polyline, Rect, Slice,
    text_width,
};
use crate::props;
use crate::style::{GraphOpts, Registry, Shape, Style};

/// Chart text inherits the theme without inheriting a box: no fill, no
/// border, and crucially **no font color** — so a declared one is
/// distinguishable from a default, both here and in `check`.
pub fn chart_defaults() -> Style {
    Style {
        font_size: Some(13.0),
        font_family: Some("Inter, ui-sans-serif, system-ui, sans-serif".into()),
        font_weight: Some("500".into()),
        ..Style::default()
    }
}

/// sRGB channels of a hex color. `None` for a named color, which we cannot
/// read without a lookup table — callers treat that as "no opinion".
fn rgb(color: &str) -> Option<(f64, f64, f64)> {
    // The alpha channel is not a color; contrast and the palette choice read
    // the opaque part.
    let (opaque, _) = crate::render_svg::split_alpha(color);
    let hex = opaque.strip_prefix('#')?;
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| f64::from(v) / 255.0);

    Some((
        channel(&hex[0..2])?,
        channel(&hex[2..4])?,
        channel(&hex[4..6])?,
    ))
}

/// WCAG relative luminance.
pub fn luminance(color: &str) -> Option<f64> {
    let (r, g, b) = rgb(color)?;
    let linear = |c: f64| {
        if c <= 0.03928 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    Some(0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b))
}

/// Flattens a translucent color onto what sits behind it. Contrast is a
/// property of what the eye receives, not of what was written down: a fill at
/// alpha 20 is mostly its backdrop.
pub fn composite(over: &str, under: &str) -> String {
    let (opaque, alpha) = crate::render_svg::split_alpha(over);
    let Some(a) = alpha else {
        return opaque;
    };
    let (Some(f), Some(b)) = (rgb(&opaque), rgb(under)) else {
        return opaque;
    };

    let mix = |f: f64, b: f64| ((a * f + (1.0 - a) * b) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        mix(f.0, b.0),
        mix(f.1, b.1),
        mix(f.2, b.2)
    )
}

/// WCAG contrast ratio, 1.0 (invisible) to 21.0 (black on white).
pub fn contrast(a: &str, b: &str) -> Option<f64> {
    let (la, lb) = (luminance(a)?, luminance(b)?);
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    Some((hi + 0.05) / (lo + 0.05))
}

/// Which palette column this surface needs.
fn is_dark(color: &str) -> bool {
    luminance(color).is_some_and(|l| l < 0.18)
}

struct Theme {
    dark: bool,
    surface: String,
    ink: String,
    muted: String,
}

impl Theme {
    fn of(opts: &GraphOpts) -> Theme {
        let dark = opts.background.as_deref().is_some_and(is_dark);
        Theme {
            dark,
            surface: opts.background.clone().unwrap_or_else(|| "#ffffff".into()),
            ink: if dark { "#f1f5f9" } else { "#0b0b0b" }.into(),
            muted: if dark { "#94a3b8" } else { "#52514e" }.into(),
        }
    }

    fn palette(&self) -> &'static [&'static str] {
        if self.dark {
            props::PALETTE_DARK
        } else {
            props::PALETTE_LIGHT
        }
    }
}

fn setting<'a>(props: &'a [Prop], name: &str) -> Option<&'a [Value]> {
    props
        .iter()
        .find(|p| p.name == name)
        .map(|p| p.values.as_slice())
}

fn number(props: &[Prop], name: &str, fallback: f64) -> f64 {
    setting(props, name)
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(fallback)
}

fn text(props: &[Prop], name: &str) -> Option<String> {
    setting(props, name)
        .and_then(|v| v.first())
        .and_then(|v| v.as_text())
        .map(String::from)
}

fn toggle(props: &[Prop], name: &str, fallback: bool) -> bool {
    match setting(props, name)
        .and_then(|v| v.first())
        .and_then(|v| v.as_word())
    {
        Some("off" | "false" | "none") => false,
        Some(_) => true,
        None => fallback,
    }
}

fn word(props: &[Prop], name: &str, fallback: &str) -> String {
    setting(props, name)
        .and_then(|v| v.first())
        .and_then(|v| v.as_word())
        .unwrap_or(fallback)
        .to_string()
}

/// Text color: what the style says, or the theme's ink.
fn ink(style: &Style, theme: &Theme) -> String {
    style
        .font_color
        .clone()
        .unwrap_or_else(|| theme.ink.clone())
}

fn family(style: &Style) -> String {
    style
        .font_family
        .clone()
        .or_else(|| chart_defaults().font_family)
        .unwrap_or_default()
}

// ------------------------------------------------------------------- pie

const TITLE_SIZE: f64 = 16.0;
const SWATCH: f64 = 13.0;
const LEGEND_STEP: f64 = 24.0;
const LEGEND_MARGIN: f64 = 28.0;

pub fn pie(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let palette = theme.palette();

    let radius = number(cfg, "radius", 130.0).max(20.0);
    let donut = number(cfg, "donut", 0.0).clamp(0.0, 0.95);
    let start_at = number(cfg, "start", 0.0);
    let legend = toggle(cfg, "legend", true);
    let labels = word(cfg, "labels", "percent");
    let title = text(cfg, "title");

    // Resolve on a bare base so a declared `fill` is distinguishable from
    // the default one, and the palette only fills in what was left unsaid.
    // `style node { … }` is the only text hook a pie has, so it governs the
    // title too — an author who sets a font color means all of the chart.
    let def = crate::check::archetype_of(diagram);
    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    let mut entries = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.effective_props(def.slots),
        )?;
        let Some(value) = style.value.filter(|v| *v > 0.0) else {
            continue;
        };
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        entries.push((label, value, style));
    }

    let total: f64 = entries.iter().map(|(_, v, _)| *v).sum();
    let title_h = if title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let cx = radius;
    let cy = title_h + radius;

    let mut slices = Vec::new();
    let mut annotations = Vec::new();
    let mut nodes = Vec::new();

    if let Some(title) = title {
        annotations.push(Annotation {
            text: title,
            x: 0.0,
            y: TITLE_SIZE,
            anchor: Anchor::Start,
            size: TITLE_SIZE,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    let mut angle = start_at;
    for (i, (label, value, style)) in entries.iter().enumerate() {
        let sweep = if total > 0.0 {
            value / total * 360.0
        } else {
            0.0
        };
        let fill = style
            .fill
            .clone()
            .unwrap_or_else(|| palette[i % palette.len()].to_string());

        slices.push(Slice {
            start: angle,
            end: angle + sweep,
            fill: fill.clone(),
        });

        // A slice narrower than this cannot hold readable text; the legend
        // carries its identity instead.
        if labels != "none" && sweep >= 22.0 {
            let mid = (angle + sweep / 2.0).to_radians();
            let ring = (radius * donut + radius) / 2.0;
            let r = if donut > 0.0 { ring } else { radius * 0.62 };
            let percent = if total > 0.0 {
                value / total * 100.0
            } else {
                0.0
            };

            annotations.push(Annotation {
                text: match labels.as_str() {
                    "value" => trim_number(*value),
                    "both" => format!("{} · {:.0}%", trim_number(*value), percent),
                    _ => format!("{percent:.0}%"),
                },
                x: cx + r * mid.sin(),
                y: cy - r * mid.cos(),
                anchor: Anchor::Middle,
                size: style.font_size.unwrap_or(12.0),
                // This text sits on the slice, not on the surface: white reads
                // on all eight palette hues. A declared color still wins.
                color: style.font_color.clone().unwrap_or_else(|| "#ffffff".into()),
                weight: style.font_weight.clone().unwrap_or_else(|| "600".into()),
                family: family(style),
            });
        }

        // Legend: identity must never rest on color alone.
        if legend {
            let x = radius * 2.0 + LEGEND_MARGIN;
            let y = title_h + i as f64 * LEGEND_STEP;

            nodes.push(PlacedNode {
                lines: Vec::new(),
                rect: Rect {
                    x,
                    y,
                    w: SWATCH,
                    h: SWATCH,
                },
                style: Style {
                    fill: Some(fill),
                    stroke: Some("none".into()),
                    stroke_width: Some(0.0),
                    radius: Some(3.0),
                    shape: Some(Shape::Rect),
                    ..Style::default()
                },
            });

            let percent = if total > 0.0 {
                value / total * 100.0
            } else {
                0.0
            };
            annotations.push(Annotation {
                // The legend sits on the surface, so it takes the node's own
                // text style directly.
                text: format!("{label}  {percent:.0}%"),
                x: x + SWATCH + 9.0,
                y: y + SWATCH / 2.0,
                anchor: Anchor::Start,
                size: style.font_size.unwrap_or(13.0),
                color: ink(style, &theme),
                weight: style.font_weight.clone().unwrap_or_else(|| "400".into()),
                family: family(style),
            });
        }

        angle += sweep;
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Pie(Pie {
            cx,
            cy,
            radius,
            inner: radius * donut,
            slices,
            gap_color: theme.surface,
        }),
        annotations,
    })
}

fn trim_number(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

// ---------------------------------------------------------------- packet

pub fn packet(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);

    let per_row = number(cfg, "width", 32.0).max(1.0) as u32;
    let bit_w = setting(cfg, "cell")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(20.0)
        .max(2.0);
    let row_h = setting(cfg, "cell")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(46.0)
        .max(12.0);
    let ruler = toggle(cfg, "ruler", true);
    let title = text(cfg, "title");

    let def = crate::check::archetype_of(diagram);
    let mut fields = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            Style::node_defaults(),
            def.item,
            &node.classes,
            &node.effective_props(def.slots),
        )?;
        let Some((first, last)) = style.bits else {
            continue;
        };
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        fields.push((first, last, label, style));
    }
    fields.sort_by_key(|f| f.0);

    let ruler_h = if ruler { 16.0 } else { 0.0 };
    let title_h = if title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let row_step = row_h + ruler_h + 10.0;

    let mut nodes = Vec::new();
    let mut annotations = Vec::new();
    let mut rows_used: HashMap<u32, ()> = HashMap::new();
    let def = crate::check::archetype_of(diagram);
    let base = registry.resolve(Style::node_defaults(), def.item, &[], &[])?;

    if let Some(title) = title {
        annotations.push(Annotation {
            text: title,
            x: 0.0,
            y: TITLE_SIZE,
            anchor: Anchor::Start,
            size: TITLE_SIZE,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    for (first, last, label, style) in &fields {
        // A field crossing a row boundary is drawn once per row it touches;
        // only the first segment is named.
        let mut bit = *first;
        let mut named = false;

        while bit <= *last {
            let row = bit / per_row;
            let row_end = (row + 1) * per_row - 1;
            let segment_end = (*last).min(row_end);
            rows_used.insert(row, ());

            let x = (bit % per_row) as f64 * bit_w;
            let y = title_h + row as f64 * row_step + ruler_h;
            let w = (segment_end - bit + 1) as f64 * bit_w;

            nodes.push(PlacedNode {
                lines: if named {
                    Vec::new()
                } else {
                    vec![label.clone()]
                },
                rect: Rect { x, y, w, h: row_h },
                style: style.clone(),
            });

            // The exact range, small, in the corner — the label says what it
            // is, this says where it lives.
            annotations.push(Annotation {
                text: if bit == segment_end {
                    format!("{bit}")
                } else {
                    format!("{bit}–{segment_end}")
                },
                x: x + 4.0,
                y: y + 9.0,
                anchor: Anchor::Start,
                size: 9.0,
                color: style
                    .font_color
                    .clone()
                    .unwrap_or_else(|| theme.muted.clone()),
                weight: "400".into(),
                family: chart_defaults().font_family.unwrap_or_default(),
            });

            named = true;
            bit = segment_end + 1;
        }
    }

    if ruler {
        let step = if per_row.is_multiple_of(8) {
            8
        } else {
            per_row
        };
        let mut rows: Vec<u32> = rows_used.into_keys().collect();
        rows.sort_unstable();

        for row in rows {
            let y = title_h + row as f64 * row_step + ruler_h - 5.0;
            for tick in (0..per_row).step_by(step as usize) {
                annotations.push(Annotation {
                    text: format!("{}", row * per_row + tick),
                    x: tick as f64 * bit_w,
                    y,
                    anchor: Anchor::Start,
                    size: 9.0,
                    color: theme.muted.clone(),
                    weight: "400".into(),
                    family: chart_defaults().font_family.unwrap_or_default(),
                });
            }
        }
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Packet,
        annotations,
    })
}

/// Estimated box of an annotation, so the canvas grows to fit its text.
pub fn annotation_bounds(a: &Annotation) -> Rect {
    let lines = vec![a.text.clone()];
    let w = text_width(&lines, a.size);
    let x = match a.anchor {
        Anchor::Start => a.x,
        Anchor::Middle => a.x - w / 2.0,
        Anchor::End => a.x - w,
    };
    Rect {
        x,
        y: a.y - a.size,
        w,
        h: a.size * 1.4,
    }
}

// --------------------------------------------------------------- treemap

/// One entry in the hierarchy: a leaf box, or a group and what it holds.
struct Tile {
    label: String,
    value: f64,
    style: Style,
    children: Vec<Tile>,
    /// Groups get a header band and a frame; leaves get neither.
    is_group: bool,
}

pub fn treemap(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(640.0)
        .max(40.0);
    let height = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(400.0)
        .max(40.0);
    // Adjacent fills must never touch; a gap of zero is not on offer.
    let gap = number(cfg, "gap", 2.0).max(1.0);
    let header = number(cfg, "header", 20.0).max(0.0);
    let labels = word(cfg, "labels", "name");
    let title = text(cfg, "title");

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;
    let roots = build_tiles(diagram, registry, def, None)?;

    let title_h = if title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let frame = Rect {
        x: 0.0,
        y: title_h,
        w: width,
        h: height,
    };

    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    let mut annotations = Vec::new();

    if let Some(title) = title {
        annotations.push(Annotation {
            text: title,
            x: 0.0,
            y: TITLE_SIZE,
            anchor: Anchor::Start,
            size: TITLE_SIZE,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    let palette = theme.palette();
    emit_tiles(
        &roots,
        frame,
        gap,
        header,
        &labels,
        palette,
        None,
        0,
        &mut nodes,
        &mut groups,
        &mut annotations,
    );

    Ok(Layout {
        guides: Vec::new(),
        polylines: Vec::new(),
        groups,
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Treemap,
        annotations,
    })
}

/// Reads the hierarchy out of group nesting. A group's value is the sum of
/// what it holds — the whole point of a treemap is that the parts add up.
fn build_tiles(
    diagram: &Diagram,
    registry: &Registry,
    def: &'static props::ArchetypeDef,
    parent: Option<usize>,
) -> Result<Vec<Tile>> {
    let members = match parent {
        Some(g) => diagram.groups[g]
            .members
            .iter()
            .map(|m| m.value.clone())
            .collect::<Vec<_>>(),
        None => roots_of(diagram),
    };

    let mut tiles = Vec::new();
    for name in members {
        if let Some(g) = diagram.groups.iter().position(|x| x.id.as_str() == name) {
            let group = &diagram.groups[g];
            let children = build_tiles(diagram, registry, def, Some(g))?;
            let style =
                registry.resolve(chart_defaults(), "group", &group.classes, &group.props)?;
            tiles.push(Tile {
                label: group
                    .label
                    .clone()
                    .unwrap_or_else(|| group.id.value.clone()),
                value: children.iter().map(|c| c.value).sum(),
                style,
                children,
                is_group: true,
            });
        } else if let Some(i) = diagram.nodes.iter().position(|x| x.id.as_str() == name) {
            let node = &diagram.nodes[i];
            let style = registry.resolve(
                chart_defaults(),
                def.item,
                &node.classes,
                &node.effective_props(def.slots),
            )?;
            tiles.push(Tile {
                label: node.label.clone().unwrap_or_else(|| node.id.value.clone()),
                value: style.value.unwrap_or(0.0).max(0.0),
                style,
                children: Vec::new(),
                is_group: false,
            });
        }
    }

    tiles.retain(|t| t.value > 0.0);
    Ok(tiles)
}

/// Names declared at the top level — everything no group claims.
fn roots_of(diagram: &Diagram) -> Vec<String> {
    let owned: std::collections::HashSet<&str> = diagram
        .groups
        .iter()
        .flat_map(|g| g.members.iter().map(|m| m.as_str()))
        .collect();

    let mut out = Vec::new();
    for node in &diagram.nodes {
        if !owned.contains(node.id.as_str()) {
            out.push(node.id.value.clone());
        }
    }
    for group in &diagram.groups {
        if !owned.contains(group.id.as_str()) {
            out.push(group.id.value.clone());
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn emit_tiles(
    tiles: &[Tile],
    frame: Rect,
    gap: f64,
    header: f64,
    labels: &str,
    palette: &'static [&'static str],
    inherited: Option<&str>,
    depth: usize,
    nodes: &mut Vec<PlacedNode>,
    groups: &mut Vec<PlacedGroup>,
    annotations: &mut Vec<Annotation>,
) {
    let values: Vec<f64> = tiles.iter().map(|t| t.value).collect();
    let rects = squarify(&values, frame);

    for (slot, (tile, rect)) in tiles.iter().zip(rects).enumerate() {
        // Colour identifies the top-level branch and nothing else: a subtree
        // inherits its root's hue and varies only in depth, so it always
        // reads as one family. Slots are taken in order, never cycled — past
        // eight, `check` says to merge the tail.
        let hue = tile
            .style
            .fill
            .clone()
            .or_else(|| inherited.map(String::from))
            .unwrap_or_else(|| palette[slot.min(palette.len() - 1)].to_string());

        let inset = Rect {
            x: rect.x + gap / 2.0,
            y: rect.y + gap / 2.0,
            w: (rect.w - gap).max(1.0),
            h: (rect.h - gap).max(1.0),
        };

        if tile.is_group {
            groups.push(PlacedGroup {
                lines: Vec::new(),
                rect: inset,
                style: Style {
                    fill: Some(shade(&hue, depth)),
                    stroke: Some("none".into()),
                    stroke_width: Some(0.0),
                    dash: Some("solid".into()),
                    radius: Some(3.0),
                    ..tile.style.clone()
                },
            });

            let band = header.min(inset.h * 0.5);
            if band >= 11.0 && inset.w > 34.0 {
                annotations.push(Annotation {
                    text: tile.label.clone(),
                    x: inset.x + 6.0,
                    y: inset.y + band / 2.0,
                    anchor: Anchor::Start,
                    size: 11.0,
                    color: "#ffffff".into(),
                    weight: "600".into(),
                    family: family(&tile.style),
                });
            }

            emit_tiles(
                &tile.children,
                Rect {
                    x: inset.x + gap,
                    y: inset.y + band + gap / 2.0,
                    w: (inset.w - gap * 2.0).max(1.0),
                    h: (inset.h - band - gap * 1.5).max(1.0),
                },
                gap,
                header,
                labels,
                palette,
                Some(&hue),
                depth + 1,
                nodes,
                groups,
                annotations,
            );
            continue;
        }

        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: inset,
            style: Style {
                fill: Some(shade(&hue, depth)),
                stroke: Some("none".into()),
                stroke_width: Some(0.0),
                radius: Some(3.0),
                shape: Some(Shape::Rect),
                ..tile.style.clone()
            },
        });

        // A label that does not fit is worse than none: it spills over its
        // neighbours and reads as belonging to the wrong tile. So try the
        // full form, then the shorter one, then say nothing.
        let candidates: Vec<String> = match labels {
            "none" => Vec::new(),
            "value" => vec![trim_number(tile.value)],
            // A lone number identifies nothing, so it is not a fallback.
            "both" => vec![
                format!("{}  {}", tile.label, trim_number(tile.value)),
                tile.label.clone(),
            ],
            _ => vec![tile.label.clone()],
        };

        let size = 11.0;
        let Some(text) = candidates.into_iter().find(|t| {
            inset.h >= size * 1.8 && inset.w >= text_width(std::slice::from_ref(t), size) + 10.0
        }) else {
            continue;
        };

        annotations.push(Annotation {
            text,
            x: inset.x + 6.0,
            y: inset.y + size,
            anchor: Anchor::Start,
            size,
            color: "#ffffff".into(),
            weight: "500".into(),
            family: family(&tile.style),
        });
    }
}

/// Depth darkens the family so a nested tile reads as part of its parent
/// without inventing a hue that would claim to be a different category.
fn shade(color: &str, depth: usize) -> String {
    if depth == 0 {
        return color.to_string();
    }
    let alpha = 255 - (depth.min(4) * 40);
    let (opaque, _) = crate::render_svg::split_alpha(color);
    composite(&format!("{opaque}{alpha:02x}"), "#000000")
}

/// Squarified treemap (Bruls, Huizing & van Wijk): fill along the shorter
/// side, growing each row while the worst aspect ratio keeps improving.
fn squarify(values: &[f64], frame: Rect) -> Vec<Rect> {
    let mut out = vec![
        Rect {
            x: frame.x,
            y: frame.y,
            w: 0.0,
            h: 0.0
        };
        values.len()
    ];
    let total: f64 = values.iter().sum();
    if total <= 0.0 {
        return out;
    }

    let mut order: Vec<usize> = (0..values.len()).collect();
    order.sort_by(|a, b| values[*b].total_cmp(&values[*a]));

    let scale = frame.w * frame.h / total;
    let mut rect = frame;
    let mut i = 0;

    while i < order.len() {
        let short = rect.w.min(rect.h).max(1e-6);
        let mut end = i;
        let mut sum = 0.0;
        let mut best = f64::MAX;

        while end < order.len() {
            let candidate = sum + values[order[end]] * scale;
            let areas: Vec<f64> = order[i..=end].iter().map(|k| values[*k] * scale).collect();
            let ratio = worst_ratio(&areas, candidate, short);
            if ratio > best {
                break;
            }
            best = ratio;
            sum = candidate;
            end += 1;
        }

        let depth = (sum / short).min(rect.w.max(rect.h));
        let mut offset = 0.0;
        for &k in &order[i..end] {
            let area = values[k] * scale;
            let along = if sum > 0.0 { area / depth } else { 0.0 };
            out[k] = if rect.w <= rect.h {
                Rect {
                    x: rect.x + offset,
                    y: rect.y,
                    w: along,
                    h: depth,
                }
            } else {
                Rect {
                    x: rect.x,
                    y: rect.y + offset,
                    w: depth,
                    h: along,
                }
            };
            offset += along;
        }

        if rect.w <= rect.h {
            rect.y += depth;
            rect.h = (rect.h - depth).max(0.0);
        } else {
            rect.x += depth;
            rect.w = (rect.w - depth).max(0.0);
        }
        i = end;
    }

    out
}

fn worst_ratio(areas: &[f64], sum: f64, short: f64) -> f64 {
    if sum <= 0.0 || areas.is_empty() {
        return f64::MAX;
    }
    let max = areas.iter().copied().fold(f64::MIN, f64::max);
    let min = areas.iter().copied().fold(f64::MAX, f64::min);
    let s2 = sum * sum;
    let w2 = short * short;
    (w2 * max / s2).max(s2 / (w2 * min))
}

// -------------------------------------------------------------- quadrant

pub fn quadrant(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(460.0)
        .max(80.0);
    let height = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(460.0)
        .max(80.0);
    let (lo, hi) = (
        setting(cfg, "range")
            .and_then(|v| v.first())
            .and_then(|v| v.as_num())
            .unwrap_or(0.0),
        setting(cfg, "range")
            .and_then(|v| v.get(1))
            .and_then(|v| v.as_num())
            .unwrap_or(1.0),
    );
    let span = (hi - lo).abs().max(f64::EPSILON);
    let named = |key: &str| -> Vec<String> {
        setting(cfg, key)
            .map(|v| {
                v.iter()
                    .filter_map(|x| x.as_text())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    };
    let x_axis = named("x-axis");
    let y_axis = named("y-axis");
    let regions = named("quadrants");
    let show_names = word(cfg, "labels", "name") != "none";
    let title = text(cfg, "title");

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;
    let title_h = if title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let plot = Rect {
        x: 0.0,
        y: title_h,
        w: width,
        h: height,
    };

    let mut guides = Vec::new();
    let mut nodes = Vec::new();
    let mut annotations = Vec::new();

    if let Some(title) = title {
        annotations.push(Annotation {
            text: title,
            x: 0.0,
            y: TITLE_SIZE,
            anchor: Anchor::Start,
            size: TITLE_SIZE,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    // The frame, then the cross. Both recede: the data is the subject.
    let frame = if theme.dark { "#334155" } else { "#d4d4d8" };
    for (from, to) in [
        ((plot.x, plot.y), (plot.right(), plot.y)),
        ((plot.right(), plot.y), (plot.right(), plot.bottom())),
        ((plot.right(), plot.bottom()), (plot.x, plot.bottom())),
        ((plot.x, plot.bottom()), (plot.x, plot.y)),
    ] {
        guides.push(Guide {
            from,
            to,
            color: frame.into(),
            width: 1.0,
            dash: None,
        });
    }
    for (from, to) in [
        ((plot.cx(), plot.y), (plot.cx(), plot.bottom())),
        ((plot.x, plot.cy()), (plot.right(), plot.cy())),
    ] {
        guides.push(Guide {
            from,
            to,
            color: frame.into(),
            width: 1.0,
            dash: Some("6 5".into()),
        });
    }

    // Region names sit in their corners, quiet enough to stay background.
    let inset = 12.0;
    let corners: [(f64, f64, Anchor); 4] = [
        (plot.right() - inset, plot.y + inset + 4.0, Anchor::End),
        (plot.x + inset, plot.y + inset + 4.0, Anchor::Start),
        (plot.x + inset, plot.bottom() - inset, Anchor::Start),
        (plot.right() - inset, plot.bottom() - inset, Anchor::End),
    ];
    for (name, (x, y, anchor)) in regions.iter().zip(corners) {
        annotations.push(Annotation {
            text: name.clone(),
            x,
            y,
            anchor,
            size: 12.0,
            color: theme.muted.clone(),
            weight: "600".into(),
            family: family(&base),
        });
    }

    // Axis ends, outside the frame. Each pair reads low-end first.
    type Spot = (f64, f64, Anchor);
    let axis_pairs: [(&[String], [Spot; 2]); 2] = [
        (
            &x_axis,
            [
                (plot.x, plot.bottom() + 18.0, Anchor::Start),
                (plot.right(), plot.bottom() + 18.0, Anchor::End),
            ],
        ),
        (
            &y_axis,
            [
                (plot.x - 8.0, plot.bottom(), Anchor::End),
                (plot.x - 8.0, plot.y, Anchor::End),
            ],
        ),
    ];
    for (labels, spots) in axis_pairs {
        for (text, (x, y, anchor)) in labels.iter().zip(spots) {
            annotations.push(Annotation {
                text: text.clone(),
                x,
                y,
                anchor,
                size: 12.0,
                color: theme.muted.clone(),
                weight: "500".into(),
                family: family(&base),
            });
        }
    }

    const DOT: f64 = 11.0;
    for (slot, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.effective_props(def.slots),
        )?;
        let (Some(x), Some(y)) = (style.x, style.y) else {
            continue;
        };

        // Data space to plot space; y grows upward on the page.
        let px = plot.x + ((x - lo) / span).clamp(0.0, 1.0) * plot.w;
        let py = plot.bottom() - ((y - lo) / span).clamp(0.0, 1.0) * plot.h;
        let fill = style
            .fill
            .clone()
            .unwrap_or_else(|| palette[slot.min(palette.len() - 1)].to_string());

        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x: px - DOT / 2.0,
                y: py - DOT / 2.0,
                w: DOT,
                h: DOT,
            },
            style: Style {
                fill: Some(fill),
                stroke: Some(theme.surface.clone()),
                stroke_width: Some(2.0),
                shape: Some(Shape::Ellipse),
                ..style.clone()
            },
        });

        if show_names {
            let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
            // Points near the right edge label leftwards, so nothing spills.
            let right_half = px > plot.cx();
            annotations.push(Annotation {
                text: label,
                x: if right_half { px - DOT } else { px + DOT },
                y: py,
                anchor: if right_half {
                    Anchor::End
                } else {
                    Anchor::Start
                },
                size: style.font_size.unwrap_or(12.0),
                color: ink(&style, &theme),
                weight: style.font_weight.clone().unwrap_or_else(|| "500".into()),
                family: family(&style),
            });
        }
    }

    Ok(Layout {
        guides,
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Quadrant,
        annotations,
    })
}

// ------------------------------------------------------------------ wave

/// What one wave character means. `.` never reaches here — it is resolved
/// into a repeat of whatever came before.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Low,
    High,
    Clock { positive: bool },
    Unknown,
    HighImpedance,
    Data(u8),
    Gap,
}

fn level_of(c: char) -> Option<Level> {
    Some(match c {
        '0' | 'l' => Level::Low,
        '1' | 'h' => Level::High,
        'p' | 'P' => Level::Clock { positive: true },
        'n' | 'N' => Level::Clock { positive: false },
        'x' => Level::Unknown,
        'z' => Level::HighImpedance,
        '=' => Level::Data(0),
        d @ '2'..='9' => Level::Data(d as u8 - b'1'),
        '|' => Level::Gap,
        _ => return None,
    })
}

pub fn wave(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let period = number(cfg, "period", 40.0).max(8.0);
    let lane_h = setting(cfg, "lane")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(28.0)
        .max(10.0);
    let lane_gap = setting(cfg, "lane")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(14.0)
        .max(0.0);
    let ruler = toggle(cfg, "ruler", true);
    let grid = toggle(cfg, "grid", true);
    let every = number(cfg, "every", 4.0).max(1.0) as usize;
    let title = text(cfg, "title");

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    // Resolve every signal first: the name column and the canvas both need
    // to know the widest name and the longest wave before anything is drawn.
    let mut signals = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.effective_props(def.slots),
        )?;
        let Some(spec) = style.wave.clone() else {
            continue;
        };
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        signals.push((name, spec, style));
    }

    let name_size = base.font_size.unwrap_or(13.0);
    let names_w = signals
        .iter()
        .map(|(name, _, _)| text_width(std::slice::from_ref(name), name_size))
        .fold(0.0_f64, f64::max)
        + 14.0;
    let periods = signals
        .iter()
        .map(|(_, spec, _)| spec.chars().count())
        .max()
        .unwrap_or(0);

    let title_h = if title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let ruler_h = if ruler { 16.0 } else { 0.0 };
    let top = title_h + ruler_h;
    let plot_w = periods as f64 * period;

    let mut guides = Vec::new();
    let mut polylines = Vec::new();
    let mut annotations = Vec::new();

    if let Some(title) = title {
        annotations.push(Annotation {
            text: title,
            x: 0.0,
            y: TITLE_SIZE,
            anchor: Anchor::Start,
            size: TITLE_SIZE,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    let lanes_h = signals.len() as f64 * (lane_h + lane_gap);
    let rule = if theme.dark { "#1e293b" } else { "#e4e4e7" };

    if grid {
        for p in 0..=periods {
            let x = names_w + p as f64 * period;
            guides.push(Guide {
                from: (x, top),
                to: (x, top + lanes_h - lane_gap),
                color: rule.into(),
                width: 1.0,
                dash: None,
            });
        }
    }

    if ruler {
        for p in (0..periods).step_by(every) {
            annotations.push(Annotation {
                text: format!("{p}"),
                x: names_w + p as f64 * period + 3.0,
                y: title_h + ruler_h / 2.0,
                anchor: Anchor::Start,
                size: 10.0,
                color: theme.muted.clone(),
                weight: "400".into(),
                family: family(&base),
            });
        }
    }

    for (row, (name, spec, style)) in signals.iter().enumerate() {
        let y = top + row as f64 * (lane_h + lane_gap);
        let stroke = style
            .stroke
            .clone()
            .unwrap_or_else(|| if theme.dark { "#7dd3fc" } else { "#0369a1" }.into());

        annotations.push(Annotation {
            text: name.clone(),
            x: names_w - 12.0,
            y: y + lane_h / 2.0,
            anchor: Anchor::End,
            size: name_size,
            color: ink(style, &theme),
            weight: style.font_weight.clone().unwrap_or_else(|| "500".into()),
            family: family(style),
        });

        draw_lane(
            spec,
            style.data.as_deref().unwrap_or(&[]),
            names_w,
            y,
            period,
            lane_h,
            &stroke,
            &theme,
            palette,
            style,
            &mut polylines,
            &mut annotations,
        );
    }

    let _ = plot_w;

    Ok(Layout {
        guides,
        polylines,
        groups: Vec::new(),
        nodes: Vec::new(),
        edges: Vec::new(),
        opts,
        chart: Chart::Wave,
        annotations,
    })
}

#[allow(clippy::too_many_arguments)]
fn draw_lane(
    spec: &str,
    data: &[String],
    x0: f64,
    y: f64,
    period: f64,
    lane_h: f64,
    stroke: &str,
    theme: &Theme,
    palette: &'static [&'static str],
    style: &Style,
    polylines: &mut Vec<Polyline>,
    annotations: &mut Vec<Annotation>,
) {
    let (top, bottom, mid) = (y, y + lane_h, y + lane_h / 2.0);
    // A transition is drawn as a slope, not a vertical jump: it reads as an
    // edge rather than as a seam between two unrelated segments.
    let slew = (period * 0.12).min(6.0);

    // Expand `.` into a repeat, keeping the fact that it *was* a repeat: two
    // explicit `=` are two data segments, while `=.` is one held for two
    // periods. Merging on equal levels alone would silently join them.
    let mut chars: Vec<(Level, bool)> = Vec::new();
    for c in spec.chars() {
        match c {
            '.' => {
                if let Some((previous, _)) = chars.last().copied() {
                    chars.push((previous, true));
                }
            }
            other => {
                if let Some(level) = level_of(other) {
                    chars.push((level, false));
                }
            }
        }
    }

    let height_of = |l: Level| match l {
        Level::Low => bottom,
        Level::High => top,
        Level::HighImpedance => mid,
        _ => mid,
    };

    let mut line: Vec<(f64, f64)> = Vec::new();
    let mut data_index = 0usize;

    // A transition straddles the period boundary: pull the previous point
    // back by half a slew rather than adding one behind it, or the line
    // doubles back on itself.
    fn open_at(line: &mut Vec<(f64, f64)>, x: f64, height: f64, slew: f64) {
        match line.last_mut() {
            None => line.push((x, height)),
            Some(last) => {
                last.0 -= slew / 2.0;
                line.push((x + slew / 2.0, height));
            }
        }
    }

    let flush_line = |line: &mut Vec<(f64, f64)>, polylines: &mut Vec<Polyline>| {
        if line.len() >= 2 {
            polylines.push(Polyline {
                points: std::mem::take(line),
                closed: false,
                fill: None,
                stroke: Some(stroke.to_string()),
                width: 1.6,
            });
        } else {
            line.clear();
        }
    };

    let mut i = 0;
    while i < chars.len() {
        let (level, _) = chars[i];
        let x = x0 + i as f64 * period;

        match level {
            Level::Clock { positive } => {
                let (a, b) = if positive {
                    (top, bottom)
                } else {
                    (bottom, top)
                };
                open_at(&mut line, x, a, slew);
                line.push((x + period / 2.0 - slew / 2.0, a));
                line.push((x + period / 2.0 + slew / 2.0, b));
                line.push((x + period, b));
                i += 1;
            }
            Level::Low | Level::High | Level::HighImpedance => {
                // A held level looks the same whether it was repeated or
                // restated, so either extends the run.
                let mut end = i;
                while end + 1 < chars.len() && chars[end + 1].0 == level {
                    end += 1;
                }
                let h = height_of(level);
                let stop = x0 + (end + 1) as f64 * period;
                open_at(&mut line, x, h, slew);
                line.push((stop, h));
                i = end + 1;
            }
            Level::Unknown | Level::Data(_) => {
                // Only a `.` continues a segment: a restated `=` is a new
                // value, and drawing it as one run would lose a transition.
                let mut end = i;
                while end + 1 < chars.len() && chars[end + 1] == (level, true) {
                    end += 1;
                }
                let stop = x0 + (end + 1) as f64 * period;
                flush_line(&mut line, polylines);

                let fill = match level {
                    Level::Data(0) => {
                        if theme.dark {
                            "#334155".to_string()
                        } else {
                            "#e4e4e7".to_string()
                        }
                    }
                    Level::Data(k) => palette[(k as usize).min(palette.len() - 1)].to_string(),
                    _ => {
                        if theme.dark {
                            "#475569".to_string()
                        } else {
                            "#a1a1aa".to_string()
                        }
                    }
                };

                // A bus segment is a stretched hexagon: it opens and closes on
                // a slope, so two adjacent values read as two values.
                polylines.push(Polyline {
                    points: vec![
                        (x, mid),
                        (x + slew, top),
                        (stop - slew, top),
                        (stop, mid),
                        (stop - slew, bottom),
                        (x + slew, bottom),
                    ],
                    closed: true,
                    fill: Some(fill),
                    stroke: Some(stroke.to_string()),
                    width: 1.6,
                });

                if let Level::Data(_) = level {
                    let label = data.get(data_index).cloned();
                    data_index += 1;
                    if let Some(label) = label {
                        let room = stop - x - slew * 2.0;
                        if room >= text_width(std::slice::from_ref(&label), 11.0) + 6.0 {
                            annotations.push(Annotation {
                                text: label,
                                x: (x + stop) / 2.0,
                                y: mid,
                                anchor: Anchor::Middle,
                                size: 11.0,
                                color: ink(style, theme),
                                weight: "500".into(),
                                family: family(style),
                            });
                        }
                    }
                }

                i = end + 1;
            }
            Level::Gap => {
                // A break in the record: stop the line and mark the seam.
                flush_line(&mut line, polylines);
                for offset in [-3.0, 3.0] {
                    polylines.push(Polyline {
                        points: vec![
                            (x + period / 2.0 + offset - 4.0, bottom + 4.0),
                            (x + period / 2.0 + offset + 4.0, top - 4.0),
                        ],
                        closed: false,
                        fill: None,
                        stroke: Some(theme.muted.clone()),
                        width: 1.2,
                    });
                }
                i += 1;
            }
        }
    }

    flush_line(&mut line, polylines);
}

// ---------------------------------------------------------------- sankey

pub fn sankey(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(680.0)
        .max(120.0);
    let height = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(400.0)
        .max(80.0);
    let gap = number(cfg, "gap", 14.0).max(2.0);
    let bar = number(cfg, "width", 16.0).max(2.0);
    let labels = word(cfg, "labels", "name");
    let title = text(cfg, "title");

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    // Links first: a node's size is whatever passes through it.
    let mut links = Vec::new();
    for edge in &diagram.edges {
        // A bare base, so a declared colour is distinguishable from a
        // default one — `edge_defaults` always sets `stroke`, which would
        // make every ribbon the same grey.
        let style = registry.resolve(
            chart_defaults(),
            "edge",
            &edge.classes,
            &edge.effective_props(def.edge_slots),
        )?;
        let (Some(&from), Some(&to)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        let Some(value) = style.value.filter(|v| *v > 0.0) else {
            continue;
        };
        links.push((from, to, value, style));
    }

    let layers = layer_nodes(diagram.nodes.len(), &links);
    let depth = layers.iter().copied().max().unwrap_or(0);

    // A node is as tall as the larger of what enters and what leaves it.
    let mut incoming = vec![0.0; diagram.nodes.len()];
    let mut outgoing = vec![0.0; diagram.nodes.len()];
    for (from, to, value, _) in &links {
        outgoing[*from] += value;
        incoming[*to] += value;
    }
    let weight: Vec<f64> = incoming
        .iter()
        .zip(&outgoing)
        .map(|(a, b)| a.max(*b))
        .collect();

    // Scale so the busiest column just fills the height.
    let mut column_total = vec![0.0; depth + 1];
    let mut column_count = vec![0usize; depth + 1];
    for (i, layer) in layers.iter().enumerate() {
        if weight[i] > 0.0 {
            column_total[*layer] += weight[i];
            column_count[*layer] += 1;
        }
    }
    let scale = column_total
        .iter()
        .zip(&column_count)
        .map(|(total, count)| {
            let free = height - gap * count.saturating_sub(1) as f64;
            if *total > 0.0 { free / total } else { 0.0 }
        })
        .fold(f64::MAX, f64::min)
        .max(0.0);

    let title_h = if title.is_some() {
        TITLE_SIZE * 2.0
    } else {
        0.0
    };
    let step = if depth > 0 {
        (width - bar) / depth as f64
    } else {
        0.0
    };

    let mut nodes = Vec::new();
    let mut polylines = Vec::new();
    let mut annotations = Vec::new();
    let mut rect_of = vec![
        Rect {
            x: 0.0,
            y: 0.0,
            w: 0.0,
            h: 0.0
        };
        diagram.nodes.len()
    ];

    if let Some(title) = title {
        annotations.push(Annotation {
            text: title,
            x: 0.0,
            y: TITLE_SIZE,
            anchor: Anchor::Start,
            size: TITLE_SIZE,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    // Stack each column in declaration order — predictable beats clever.
    let mut cursor = vec![0.0; depth + 1];
    for (i, node) in diagram.nodes.iter().enumerate() {
        if weight[i] <= 0.0 {
            continue;
        }
        let layer = layers[i];
        let h = (weight[i] * scale).max(2.0);
        let rect = Rect {
            x: layer as f64 * step,
            y: title_h + cursor[layer],
            w: bar,
            h,
        };
        cursor[layer] += h + gap;
        rect_of[i] = rect;

        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.effective_props(def.slots),
        )?;
        let fill = style
            .fill
            .clone()
            .unwrap_or_else(|| palette[i.min(palette.len() - 1)].to_string());

        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect,
            style: Style {
                fill: Some(fill.clone()),
                stroke: Some("none".into()),
                stroke_width: Some(0.0),
                radius: Some(2.0),
                shape: Some(Shape::Rect),
                ..style.clone()
            },
        });

        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        let text = match labels.as_str() {
            "none" => None,
            "value" => Some(trim_number(weight[i])),
            "both" => Some(format!("{name}  {}", trim_number(weight[i]))),
            _ => Some(name),
        };
        if let Some(text) = text {
            // The last column labels leftward so nothing spills off the plot.
            let last = layer == depth;
            annotations.push(Annotation {
                text,
                x: if last {
                    rect.x - 8.0
                } else {
                    rect.right() + 8.0
                },
                y: rect.cy(),
                anchor: if last { Anchor::End } else { Anchor::Start },
                size: style.font_size.unwrap_or(12.0),
                color: ink(&style, &theme),
                weight: style.font_weight.clone().unwrap_or_else(|| "500".into()),
                family: family(&style),
            });
        }
    }

    // Ribbons, allocated down each node's face in declaration order.
    let mut out_at = vec![0.0; diagram.nodes.len()];
    let mut in_at = vec![0.0; diagram.nodes.len()];
    for (from, to, value, style) in &links {
        let thickness = value * scale;
        let a = rect_of[*from];
        let b = rect_of[*to];
        let y0 = a.y + out_at[*from];
        let y1 = b.y + in_at[*to];
        out_at[*from] += thickness;
        in_at[*to] += thickness;

        // A ribbon takes its source's colour, at an alpha that lets crossings
        // stay legible.
        let hue = style
            .stroke
            .clone()
            .unwrap_or_else(|| palette[(*from).min(palette.len() - 1)].to_string());
        let (opaque, _) = crate::render_svg::split_alpha(&hue);

        polylines.push(Polyline {
            points: ribbon(a.right(), y0, b.x, y1, thickness),
            closed: true,
            fill: Some(format!("{opaque}66")),
            stroke: None,
            width: 0.0,
        });
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines,
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Sankey,
        annotations,
    })
}

/// Longest path from a source, so a node sits to the right of everything that
/// feeds it. A cycle leaves nodes unassigned; `check` reports it.
fn layer_nodes(count: usize, links: &[(usize, usize, f64, Style)]) -> Vec<usize> {
    let mut layer = vec![0usize; count];
    // `count` relaxations settle any acyclic graph; a cycle stops improving.
    for _ in 0..count {
        let mut moved = false;
        for (from, to, _, _) in links {
            if layer[*to] < layer[*from] + 1 {
                layer[*to] = layer[*from] + 1;
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    layer
}

/// A flow band: two cubic curves sampled into a closed polygon, so the ribbon
/// needs no primitive beyond the polyline everything else already uses.
fn ribbon(x0: f64, y0: f64, x1: f64, y1: f64, thickness: f64) -> Vec<(f64, f64)> {
    const STEPS: usize = 24;
    let curve = |a: f64, b: f64| {
        (0..=STEPS).map(move |i| {
            let t = i as f64 / STEPS as f64;
            // Horizontal control points: the band leaves and arrives flat.
            let ease = t * t * (3.0 - 2.0 * t);
            (x0 + (x1 - x0) * t, a + (b - a) * ease)
        })
    };

    let mut points: Vec<(f64, f64)> = curve(y0, y1).collect();
    points.extend(
        curve(y0 + thickness, y1 + thickness)
            .collect::<Vec<_>>()
            .into_iter()
            .rev(),
    );
    points
}
