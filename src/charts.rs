//! The non-`free` archetypes.
//!
//! Both reinterpret the same `node` statements as chart data, and both reuse
//! the style cascade — a slice or a bitfield is styled exactly like a box.
//! `packet` needs no renderer support at all: it emits ordinary rects.

use std::collections::HashMap;

use anyhow::Result;

use crate::ast::{Diagram, Prop, Value};
use crate::layout::{Anchor, Annotation, Chart, Layout, Pie, PlacedNode, Rect, Slice, text_width};
use crate::props;
use crate::style::{GraphOpts, Registry, Shape, Style};

/// Chart text inherits the theme without inheriting a box: no fill, no
/// border, and crucially **no font color** — so a declared one is
/// distinguishable from a default, both here and in `check`.
pub fn chart_defaults() -> Style {
    Style {
        opacity: Some(1.0),
        font_size: Some(13.0),
        font_family: Some("Inter, ui-sans-serif, system-ui, sans-serif".into()),
        font_weight: Some("500".into()),
        ..Style::default()
    }
}

/// sRGB channels of a hex color. `None` for a named color, which we cannot
/// read without a lookup table — callers treat that as "no opinion".
fn rgb(color: &str) -> Option<(f64, f64, f64)> {
    let hex = color.trim_start_matches('#');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |s: &str| u8::from_str_radix(s, 16).ok().map(|v| v as f64 / 255.0);

    match hex.len() {
        3 => Some((
            channel(&hex[0..1].repeat(2))?,
            channel(&hex[1..2].repeat(2))?,
            channel(&hex[2..3].repeat(2))?,
        )),
        6 | 8 => Some((
            channel(&hex[0..2])?,
            channel(&hex[2..4])?,
            channel(&hex[4..6])?,
        )),
        _ => None,
    }
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
    let base = registry.resolve(chart_defaults(), "node", &[], &[])?;

    let mut entries = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(chart_defaults(), "node", &node.classes, &node.props)?;
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
                    opacity: Some(1.0),
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

    let mut fields = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(Style::node_defaults(), "node", &node.classes, &node.props)?;
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
    let base = registry.resolve(Style::node_defaults(), "node", &[], &[])?;

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
