//! The non-`free` archetypes.
//!
//! Both reinterpret the same `node` statements as chart data, and both reuse
//! the style cascade — a slice or a bitfield is styled exactly like a box.
//! `packet` needs no renderer support at all: it emits ordinary rects.

// The chart helpers — the theme, the axis scale, the frame, the legend — are
// shared by several archetypes. Cfg-ing each against the set of features that
// might use it would be a maintenance trap for no gain: they are small, and
// what a reduced build actually saves is the chart bodies themselves.
#![cfg_attr(not(feature = "all-archetypes"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use anyhow::Result;

use crate::ast::{Diagram, Prop, Value};
use crate::layout::{
    Anchor, Annotation, Chart, Guide, Layout, Pie, PlacedGroup, PlacedNode, Polyline, Rect, Slice,
    text_width,
};
use crate::props;
use crate::style::{GraphOpts, Head, Registry, Shape, Style};

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
pub fn is_dark(color: &str) -> bool {
    luminance(color).is_some_and(|l| l < 0.18)
}

pub(crate) struct Theme {
    dark: bool,
    surface: String,
    pub(crate) ink: String,
    muted: String,
}

impl Theme {
    pub(crate) fn of(opts: &GraphOpts) -> Theme {
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

pub(crate) const TITLE_SIZE: f64 = 16.0;
const SWATCH: f64 = 13.0;
const LEGEND_STEP: f64 = 24.0;
const LEGEND_MARGIN: f64 = 28.0;

#[cfg(feature = "pie")]
pub fn pie(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let palette = theme.palette();

    let radius = number(cfg, "radius", 130.0).max(20.0);
    let donut = number(cfg, "donut", 0.0).clamp(0.0, 0.95);
    let start_at = number(cfg, "start", 0.0);
    let legend = toggle(cfg, "legend", true);
    let labels = word(cfg, "labels", "percent");
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

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
            &node.props_for(def),
        )?;
        let Some(value) = style.value.filter(|v| *v > 0.0) else {
            continue;
        };
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        entries.push((label, value, style));
    }

    let total: f64 = entries.iter().map(|(_, v, _)| *v).sum();
    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
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

pub(crate) fn trim_number(v: f64) -> String {
    if v == v.trunc() {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

// ---------------------------------------------------------------- packet

// Both genres need it, so it survives as long as either does. Gating it on
// `packet` alone left `bytefield` — and the featureless build — referring to
// a struct that was no longer compiled.
#[cfg(any(feature = "packet", feature = "bytefield"))]
/// `packet` and `bytefield` are one layout at two granularities (`W-039`).
/// Only the unit changes: the default row width, the ruler step, and which
/// property carries the range. Everything below — splitting a field across a
/// row boundary, naming only its first segment, the corner range — is the
/// same picture read at a different scale, so it is written once.
struct Granularity {
    /// Cells per row when the file does not say. A packet is read 32 bits
    /// wide; a byte layout is read 16 bytes wide, as a hex dump is.
    per_row: f64,
    /// Ruler step, when the row divides by it. Bits group by eight, bytes by
    /// four — that is where a reader's eye already puts the marks.
    step: u32,
}

/// Commits on rails, with the lines between them.
///
/// Parentage is the edges — `c1 -> c2` makes c1 the parent — and a commit with
/// two parents is a merge. The **rails** are the archetype's to assign, which
/// is the only auto-placement here: the author still declares every commit,
/// its branch and what came before it. That is the line `D-001` draws, and it
/// is why this is not `I-004` in disguise.
#[cfg(feature = "gitgraph")]
pub fn gitgraph(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    use std::collections::HashMap;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let along = setting(cfg, "spacing")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(78.0)
        .max(24.0);
    let between = setting(cfg, "spacing")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(58.0)
        .max(20.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let declared: Vec<String> = setting(cfg, "branches")
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_text())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    // Rails: the declared order first, then any branch met along the way, in
    // the order it is first seen — so a file that names none still reads in
    // the order it was written.
    let mut rails: Vec<String> = declared.clone();
    let mut commits = Vec::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let branch = style.branch.clone().unwrap_or_else(|| "main".to_string());
        if !rails.contains(&branch) {
            rails.push(branch.clone());
        }
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        commits.push((node.id.value.clone(), label, branch, style, i));
    }

    let rail_of: HashMap<&str, usize> = rails
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_str(), i))
        .collect();
    let index: HashMap<&str, usize> = commits
        .iter()
        .enumerate()
        .map(|(i, (id, _, _, _, _))| (id.as_str(), i))
        .collect();

    // Topological order along the rail: a commit sits one step past its
    // latest parent, so a merge lands after both sides of it.
    let mut parents: Vec<Vec<usize>> = vec![Vec::new(); commits.len()];
    for edge in &diagram.edges {
        if let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str())) {
            parents[b].push(a);
        }
    }
    let mut step = vec![0usize; commits.len()];
    // Declaration order is a valid topological order for any graph `check`
    // accepts, since it refuses a parent declared after its child.
    for i in 0..commits.len() {
        step[i] = parents[i].iter().map(|p| step[*p] + 1).max().unwrap_or(0);
    }

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    let gutter = rails
        .iter()
        .map(|r| text_width(std::slice::from_ref(r), 11.0))
        .fold(0.0_f64, f64::max)
        + 16.0;
    const DOT: f64 = 13.0;

    let at = |i: usize| -> (f64, f64) {
        let rail = rail_of.get(commits[i].2.as_str()).copied().unwrap_or(0);
        (
            gutter + step[i] as f64 * along + DOT / 2.0,
            title_h + 18.0 + rail as f64 * between,
        )
    };

    let mut polylines = Vec::new();
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

    // The rail names, down the left.
    for (r, name) in rails.iter().enumerate() {
        annotations.push(Annotation {
            text: name.clone(),
            x: gutter - 10.0,
            y: title_h + 22.0 + r as f64 * between,
            anchor: Anchor::End,
            size: 11.0,
            color: palette[r.min(palette.len() - 1)].to_string(),
            weight: "600".into(),
            family: family(&base),
        });
    }

    // Parent lines first, so a dot always sits on top of the line into it.
    for (child, ps) in parents.iter().enumerate() {
        for parent in ps {
            let (px, py) = at(*parent);
            let (cx, cy) = at(child);
            let rail = rail_of.get(commits[child].2.as_str()).copied().unwrap_or(0);
            let colour = palette[rail.min(palette.len() - 1)].to_string();
            // A right angle, not a diagonal into the dot: the message sits
            // above its commit, and a line rising into it crosses the text.
            // The turn happens between the two commits, where nothing is.
            let points = if (py - cy).abs() < 0.5 {
                vec![(px, py), (cx, cy)]
            } else {
                let turn = cx - along * 0.45;
                vec![(px, py), (turn, py), (turn, cy), (cx, cy)]
            };
            polylines.push(Polyline {
                points,
                closed: false,
                fill: None,
                stroke: Some(colour),
                width: 1.6,
                head: None,
                dash: None,
            });
        }
    }

    for (i, (_, label, branch, style, _)) in commits.iter().enumerate() {
        let (x, y) = at(i);
        let rail = rail_of.get(branch.as_str()).copied().unwrap_or(0);
        let colour = style
            .fill
            .clone()
            .unwrap_or_else(|| palette[rail.min(palette.len() - 1)].to_string());

        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x: x - DOT / 2.0,
                y: y - DOT / 2.0,
                w: DOT,
                h: DOT,
            },
            style: Style {
                fill: Some(colour.clone()),
                stroke: Some(theme.surface.clone()),
                stroke_width: Some(2.0),
                shape: Some(Shape::Ellipse),
                ..style.clone()
            },
        });

        // The message above the dot, any tag below it — two lines that never
        // compete for the same place.
        annotations.push(Annotation {
            text: label.clone(),
            x,
            y: y - DOT,
            anchor: Anchor::Middle,
            size: 11.0,
            color: ink(&base, &theme),
            weight: "400".into(),
            family: family(&base),
        });
        if let Some(tag) = &style.tag {
            annotations.push(Annotation {
                text: format!("⌾ {tag}"),
                x,
                y: y + DOT + 6.0,
                anchor: Anchor::Middle,
                size: 10.0,
                color: colour,
                weight: "600".into(),
                family: family(&base),
            });
        }
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines,
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

/// Which arrangement a tree is drawn in. The engine below is one; only the
/// axes and the starting point change (`W-041`).
#[cfg(any(feature = "orgchart", feature = "mindmap", feature = "treeview"))]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TreeKind {
    /// Layered top-down, parents centred over their children.
    Org,
    /// A root with branches either side, growing outward.
    Mind,
    /// Indented rows, one per line.
    View,
}

/// A tree, from the edges.
///
/// The hierarchy is the edges rather than group nesting: an org chart is
/// written `ceo -> cto`, and asking an author to nest a manager inside a group
/// *and* declare them as a box would be two statements for one fact.
///
/// The engine is the classic subtree-width pass — measure every subtree
/// bottom-up, then hand each child its slice of the parent's span. No threads
/// or contours: siblings are placed in disjoint spans by construction, so they
/// cannot overlap and there is nothing to repair afterwards.
#[cfg(any(feature = "orgchart", feature = "mindmap", feature = "treeview"))]
pub fn tree(
    diagram: &Diagram,
    opts: GraphOpts,
    registry: &Registry,
    kind: TreeKind,
) -> Result<Layout> {
    use std::collections::HashMap;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);

    let row = number(cfg, "row", if kind == TreeKind::View { 26.0 } else { 92.0 }).max(12.0);
    let spacing = number(cfg, "spacing", 24.0).max(0.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let base = registry.resolve(Style::node_defaults(), def.item, &[], &[])?;

    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut styles = Vec::new();
    let mut sizes = Vec::new();
    let mut labels = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            Style::node_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        let font = style.font_size.unwrap_or(14.0);
        let pad = style.padding.unwrap_or(14.0);
        let w = style
            .width
            .unwrap_or(text_width(std::slice::from_ref(&label), font) + pad * 2.0);
        let h = style.height.unwrap_or(font * 1.45 + pad * 1.4);
        sizes.push((w, h));
        labels.push(label);
        styles.push(style);
    }

    // Children in declaration order, so the picture reads in the order the
    // file does.
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); diagram.nodes.len()];
    let mut has_parent = vec![false; diagram.nodes.len()];
    for edge in &diagram.edges {
        if let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
            && !has_parent[b]
        {
            children[a].push(b);
            has_parent[b] = true;
        }
    }
    let roots: Vec<usize> = (0..diagram.nodes.len())
        .filter(|i| !has_parent[*i])
        .collect();

    // Bottom-up: a subtree is as wide as its children laid side by side, or as
    // wide as its own box, whichever is more.
    fn measure(
        i: usize,
        children: &[Vec<usize>],
        sizes: &[(f64, f64)],
        spacing: f64,
        out: &mut [f64],
    ) -> f64 {
        let kids = &children[i];
        let own = sizes[i].0;
        let span = if kids.is_empty() {
            own
        } else {
            let total: f64 = kids
                .iter()
                .map(|k| measure(*k, children, sizes, spacing, out))
                .sum::<f64>()
                + spacing * (kids.len() - 1) as f64;
            total.max(own)
        };
        out[i] = span;
        span
    }

    let mut span = vec![0.0; diagram.nodes.len()];
    for r in &roots {
        measure(*r, &children, &sizes, spacing, &mut span);
    }

    // Top-down: each child takes its slice of the parent's span, and the
    // parent sits over the middle of what it holds.
    #[allow(clippy::too_many_arguments)]
    fn place(
        i: usize,
        left: f64,
        depth: usize,
        children: &[Vec<usize>],
        sizes: &[(f64, f64)],
        span: &[f64],
        spacing: f64,
        row: f64,
        pos: &mut [(f64, f64)],
    ) {
        let mut cursor = left;
        for k in &children[i] {
            place(
                *k,
                cursor,
                depth + 1,
                children,
                sizes,
                span,
                spacing,
                row,
                pos,
            );
            cursor += span[*k] + spacing;
        }
        let centre = left + span[i] / 2.0;
        pos[i] = (centre - sizes[i].0 / 2.0, depth as f64 * row);
    }

    let mut pos = vec![(0.0, 0.0); diagram.nodes.len()];
    let mut cursor = 0.0;
    for r in &roots {
        place(
            *r, cursor, 0, &children, &sizes, &span, spacing, row, &mut pos,
        );
        cursor += span[*r] + spacing * 2.0;
    }

    // The three arrangements are the same tree read on different axes.
    match kind {
        TreeKind::Org => {}
        TreeKind::View => {
            // Depth is the indent, order is pre-order, one row each.
            fn walk(
                i: usize,
                depth: usize,
                children: &[Vec<usize>],
                order: &mut Vec<(usize, usize)>,
            ) {
                order.push((i, depth));
                for k in &children[i] {
                    walk(*k, depth + 1, children, order);
                }
            }
            let mut order = Vec::new();
            for r in &roots {
                walk(*r, 0, &children, &mut order);
            }
            // A row has to clear the box that sits on it. `row` is a
            // *minimum*: asking for 26 with a 29-tall box drew every entry
            // over the one above it, which is illegible rather than tight.
            let tallest = sizes.iter().map(|(_, h)| *h).fold(0.0_f64, f64::max);
            let step = row.max(tallest + 6.0);
            for (line, (i, depth)) in order.iter().enumerate() {
                pos[*i] = (*depth as f64 * (spacing + 14.0), line as f64 * step);
            }
        }
        TreeKind::Mind => {
            // Half the root's branches each way. The root stays central
            // however lopsided the tree is, which is the point of the shape.
            for r in &roots {
                let kids = children[*r].clone();
                let half = kids.len().div_ceil(2);
                for (n, k) in kids.iter().enumerate() {
                    let side = if n < half { 1.0 } else { -1.0 };
                    mirror(*k, side, &children, &mut pos);
                }
                let _ = half;
            }
            // Swap the axes: depth runs horizontally, siblings vertically.
            for p in pos.iter_mut() {
                *p = (p.1 * 1.4, p.0 * 0.55);
            }
        }
    }

    #[cfg(any(feature = "orgchart", feature = "mindmap", feature = "treeview"))]
    fn mirror(i: usize, side: f64, children: &[Vec<usize>], pos: &mut [(f64, f64)]) {
        pos[i].1 *= side;
        for k in &children[i] {
            mirror(*k, side, children, pos);
        }
    }

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    let min_x = pos.iter().map(|(x, _)| *x).fold(f64::MAX, f64::min);
    let min_y = pos.iter().map(|(_, y)| *y).fold(f64::MAX, f64::min);
    let shift = |(x, y): (f64, f64)| (x - min_x, y - min_y + title_h);

    let mut nodes = Vec::new();
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

    for (i, node) in diagram.nodes.iter().enumerate() {
        let _ = node;
        let (x, y) = shift(pos[i]);
        nodes.push(PlacedNode {
            lines: vec![labels[i].clone()],
            rect: Rect {
                x,
                y,
                w: sizes[i].0,
                h: sizes[i].1,
            },
            style: styles[i].clone(),
        });
    }

    // Connectors, drawn as plain lines rather than routed edges: a tree's
    // links are structure, and an arrowhead on every one of them is noise.
    let link = if theme.dark { "#334155" } else { "#d4d4d8" };
    for (parent, kids) in children.iter().enumerate() {
        for k in kids {
            let a = &nodes[parent].rect;
            let b = &nodes[*k].rect;
            let (from, to) = match kind {
                TreeKind::Org => ((a.x + a.w / 2.0, a.bottom()), (b.x + b.w / 2.0, b.y)),
                _ if b.x >= a.x => ((a.right(), a.y + a.h / 2.0), (b.x, b.y + b.h / 2.0)),
                _ => ((a.x, a.y + a.h / 2.0), (b.right(), b.y + b.h / 2.0)),
            };
            polylines.push(Polyline {
                points: vec![from, to],
                closed: false,
                fill: None,
                stroke: Some(link.to_string()),
                width: 1.4,
                head: None,
                dash: None,
            });
        }
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines,
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

/// The area of the lens between two circles of radii `a` and `b` whose centres
/// are `d` apart. The standard two-circle formula; it is what makes the
/// two-set case *solvable* rather than merely arranged.
#[cfg(feature = "venn")]
pub(crate) fn lens_area(a: f64, b: f64, d: f64) -> f64 {
    if d >= a + b {
        return 0.0;
    }
    if d <= (a - b).abs() {
        return std::f64::consts::PI * a.min(b).powi(2);
    }
    let c1 = ((d * d + a * a - b * b) / (2.0 * d * a)).clamp(-1.0, 1.0);
    let c2 = ((d * d + b * b - a * a) / (2.0 * d * b)).clamp(-1.0, 1.0);
    a * a * (c1.acos() - c1 * (1.0 - c1 * c1).max(0.0).sqrt())
        + b * b * (c2.acos() - c2 * (1.0 - c2 * c2).max(0.0).sqrt())
}

/// The centre distance that makes the lens exactly `want`. Bisection rather
/// than a closed form, because there is none: the area is monotone in `d`,
/// which is all a bisection needs.
#[cfg(feature = "venn")]
fn distance_for(a: f64, b: f64, want: f64) -> f64 {
    let (mut lo, mut hi) = ((a - b).abs(), a + b);
    for _ in 0..60 {
        let mid = (lo + hi) / 2.0;
        if lens_area(a, b, mid) > want {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) / 2.0
}

/// Overlapping sets.
///
/// With **two** sets the lens is solved so its area is the number written —
/// a Venn diagram whose overlap is drawn by eye is a chart that lies about the
/// one thing it exists to show. With **three** the overlaps are labelled but
/// arranged rather than solved, because three circles cannot in general be
/// placed to honour all seven regions at once; saying so is better than
/// pretending. Four is refused in `check`.
#[cfg(feature = "venn")]
pub fn venn(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    use std::collections::HashMap;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let biggest = number(cfg, "radius", 110.0).max(30.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    let mut sets = Vec::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let count = style.set_size.unwrap_or(0.0).max(0.0);
        let colour = style
            .fill
            .clone()
            .unwrap_or_else(|| palette[i.min(palette.len() - 1)].to_string());
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        sets.push((node.id.value.clone(), name, count, colour, style));
    }

    // Area, not radius: a circle read as twice the size is one with twice the
    // ink in it. Radius would exaggerate every difference by squaring it.
    let largest = sets
        .iter()
        .map(|(_, _, c, _, _)| *c)
        .fold(0.0_f64, f64::max)
        .max(1.0);
    let radius = |count: f64| biggest * (count / largest).max(0.0).sqrt();

    let index: HashMap<&str, usize> = sets
        .iter()
        .enumerate()
        .map(|(i, (id, _, _, _, _))| (id.as_str(), i))
        .collect();
    // Ordered, not hashed: iterating a `HashMap` emitted the overlap labels in
    // a different order on different runs, so the same file rendered to
    // different bytes. The golden test caught it; nothing else would have.
    let mut shared: std::collections::BTreeMap<(usize, usize), f64> =
        std::collections::BTreeMap::new();
    for edge in &diagram.edges {
        let (Some(&a), Some(&b)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        let style = registry.resolve(
            chart_defaults(),
            "edge",
            &edge.classes,
            &edge.effective_props(def.edge_slots),
        )?;
        if let Some(n) = style.shared {
            shared.insert((a.min(b), a.max(b)), n.max(0.0));
        }
    }

    let scale = biggest * biggest / largest * std::f64::consts::PI;
    let centres: Vec<(f64, f64)> = match sets.len() {
        0 | 1 => vec![(0.0, 0.0)],
        2 => {
            let (ra, rb) = (radius(sets[0].2), radius(sets[1].2));
            let want = shared.get(&(0, 1)).copied().unwrap_or(0.0) * scale;
            let d = distance_for(ra, rb, want);
            vec![(0.0, 0.0), (d, 0.0)]
        }
        _ => {
            // Three on an equilateral triangle. The radius is set by the
            // *tightest* pair, not the largest circle: three points at 120°
            // sit √3·R apart, so R has to be under (rᵢ + rⱼ)/√3 for every pair
            // or the smallest two never meet — which is a Venn diagram with no
            // intersections in it. Arranged, not solved; the docs say so.
            let radii: Vec<f64> = sets.iter().map(|(_, _, c, _, _)| radius(*c)).collect();
            let tightest = (0..radii.len())
                .flat_map(|i| ((i + 1)..radii.len()).map(move |j| (i, j)))
                .map(|(i, j)| radii[i] + radii[j])
                .fold(f64::MAX, f64::min);
            let d = tightest / 3.0_f64.sqrt() * 0.72;
            (0..sets.len())
                .map(|i| {
                    let a = -std::f64::consts::FRAC_PI_2
                        + i as f64 * std::f64::consts::TAU / sets.len() as f64;
                    (d * a.cos(), d * a.sin())
                })
                .collect()
        }
    };

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    let min_x = centres
        .iter()
        .zip(&sets)
        .map(|((x, _), (_, _, c, _, _))| x - radius(*c))
        .fold(f64::MAX, f64::min);
    let min_y = centres
        .iter()
        .zip(&sets)
        .map(|((_, y), (_, _, c, _, _))| y - radius(*c))
        .fold(f64::MAX, f64::min);

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

    let place = |i: usize| -> (f64, f64) {
        let (x, y) = centres.get(i).copied().unwrap_or((0.0, 0.0));
        (x - min_x, y - min_y + title_h + 18.0)
    };

    for (i, (_, name, count, colour, style)) in sets.iter().enumerate() {
        let r = radius(*count);
        let (cx, cy) = place(i);
        let (opaque, _) = crate::render_svg::split_alpha(colour);
        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x: cx - r,
                y: cy - r,
                w: r * 2.0,
                h: r * 2.0,
            },
            style: Style {
                fill: Some(format!("{opaque}44")),
                stroke: Some(colour.clone()),
                stroke_width: Some(style.stroke_width.unwrap_or(1.5)),
                shape: Some(Shape::Ellipse),
                ..style.clone()
            },
        });

        // The name sits above its circle rather than inside it: the inside is
        // where the overlaps are, and a name there would compete with them.
        annotations.push(Annotation {
            text: format!("{name}  {}", trim_number(*count)),
            x: cx,
            y: cy - r - 8.0,
            anchor: Anchor::Middle,
            size: 12.0,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    // The shared count goes in the lens, on the line between the two centres.
    for ((a, b), n) in &shared {
        let (ax, ay) = place(*a);
        let (bx, by) = place(*b);
        annotations.push(Annotation {
            text: trim_number(*n),
            x: (ax + bx) / 2.0,
            y: (ay + by) / 2.0 + 4.0,
            anchor: Anchor::Middle,
            size: 12.0,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

/// A rack elevation: a numbered frame, devices occupying spans of units.
///
/// Units count from **1 at the bottom**, which is how a rack is labelled and
/// how anyone standing in front of one reads it. Getting that inverted would
/// produce a diagram that is right about everything except where to put your
/// hands.
#[cfg(feature = "rack")]
pub fn rack(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);

    let units = number(cfg, "units", 42.0).max(1.0) as u32;
    let frame_w = setting(cfg, "cell")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(260.0)
        .max(60.0);
    let unit_h = setting(cfg, "cell")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(18.0)
        .max(6.0);
    let ruler = toggle(cfg, "ruler", true);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(Style::node_defaults(), def.item, &[], &[])?;
    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    let gutter = if ruler {
        // Room for the widest number *and* air between it and the rail. Four
        // pixels put the digits under the frame's own border.
        text_width(&[format!("{units}")], 9.0) + 20.0
    } else {
        0.0
    };
    let top = title_h;

    // U1 is the bottom row, so a unit's y grows as its number shrinks.
    let row_y = |u: u32| -> f64 { top + (units.saturating_sub(u)) as f64 * unit_h };

    let mut nodes = Vec::new();
    let mut groups = Vec::new();
    let mut guides = Vec::new();
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

    groups.push(PlacedGroup {
        lines: Vec::new(),
        rect: Rect {
            x: gutter,
            y: top,
            w: frame_w,
            h: units as f64 * unit_h,
        },
        style: registry.resolve(Style::group_defaults(), "group", &[], &[])?,
    });

    if ruler {
        let rail = if theme.dark { "#1e293b" } else { "#e4e4e7" };
        for u in 1..=units {
            let y = row_y(u);
            // Every unit is numbered, as a real rail is — the number is what
            // you count against when you are holding a server.
            annotations.push(Annotation {
                text: format!("{u}"),
                x: gutter - 9.0,
                y: y + unit_h * 0.5 + 3.0,
                anchor: Anchor::End,
                size: 9.0,
                color: theme.muted.clone(),
                weight: "400".into(),
                family: family(&base),
            });
            guides.push(Guide {
                from: (gutter, y),
                to: (gutter + frame_w, y),
                color: rail.to_string(),
                width: 0.5,
                dash: None,
            });
        }
    }

    for node in &diagram.nodes {
        let style = registry.resolve(
            Style::node_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let Some((low, high)) = style.u else { continue };
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        let span = high.saturating_sub(low) + 1;

        nodes.push(PlacedNode {
            lines: vec![label],
            rect: Rect {
                x: gutter + 2.0,
                y: row_y(high),
                w: frame_w - 4.0,
                h: span as f64 * unit_h,
            },
            style,
        });
    }

    Ok(Layout {
        guides,
        polylines: Vec::new(),
        groups,
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

/// Axes radiating from a centre, one closed polygon per series.
///
/// Every axis shares one scale. A radar whose axes each normalised themselves
/// would draw a shape nobody could read: the whole device is a *shape* the eye
/// compares, and that only means something if a ring means the same all the
/// way round.
#[cfg(feature = "radar")]
pub fn radar(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let radius = number(cfg, "radius", 140.0).max(40.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let axes: Vec<String> = setting(cfg, "categories")
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_text())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    let mut series = Vec::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let Some(points) = style.points.clone().filter(|p| !p.is_empty()) else {
            continue;
        };
        let colour = style
            .fill
            .clone()
            .or_else(|| style.stroke.clone())
            .unwrap_or_else(|| palette[i.min(palette.len() - 1)].to_string());
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        series.push((name, points, colour, style));
    }

    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (_, points, _, _) in &series {
        for v in points {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
    }
    if series.is_empty() {
        (lo, hi) = (0.0, 1.0);
    }
    // A radar is read from the centre outward, so the floor is 0 unless the
    // author says otherwise — starting the rings at the smallest value would
    // exaggerate every difference.
    let scale = match setting(cfg, "range") {
        Some(v) if v.len() == 2 => {
            nice_scale(v[0].as_num().unwrap_or(0.0), v[1].as_num().unwrap_or(hi), 4)
        }
        _ => nice_scale(lo.min(0.0), hi, 4),
    };

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    // Room for the axis names, which sit outside the rim.
    let margin = axes
        .iter()
        .map(|name| text_width(std::slice::from_ref(name), 11.0))
        .fold(0.0_f64, f64::max)
        + 12.0;
    let cx = margin + radius;
    let cy = title_h + margin.min(28.0) + radius;

    let n = axes
        .len()
        .max(series.iter().map(|(_, p, _, _)| p.len()).max().unwrap_or(0));
    // Clockwise from the top: the order the axes are written is the order they
    // are read, and twelve o'clock is where a reader starts.
    let angle = |i: usize| -> f64 {
        -std::f64::consts::FRAC_PI_2 + (i as f64) * std::f64::consts::TAU / (n.max(1) as f64)
    };
    let point = |i: usize, v: f64| -> (f64, f64) {
        let r = scale.fraction(v).clamp(0.0, 1.0) * radius;
        let a = angle(i);
        (cx + r * a.cos(), cy + r * a.sin())
    };

    let mut guides = Vec::new();
    let mut polylines = Vec::new();
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

    let rule = if theme.dark { "#1e293b" } else { "#e4e4e7" };
    let axis = if theme.dark { "#334155" } else { "#d4d4d8" };

    // A ring per tick, drawn as the same polygon the data uses — a circle
    // would suggest a continuum between axes that does not exist.
    for tick in &scale.ticks {
        let ring: Vec<(f64, f64)> = (0..n).map(|i| point(i, *tick)).collect();
        for pair in 0..ring.len() {
            let from = ring[pair];
            let to = ring[(pair + 1) % ring.len()];
            guides.push(Guide {
                from,
                to,
                color: rule.to_string(),
                width: 1.0,
                dash: None,
            });
        }
    }

    for i in 0..n {
        let rim = point(i, scale.max);
        guides.push(Guide {
            from: (cx, cy),
            to: rim,
            color: axis.to_string(),
            width: 1.0,
            dash: None,
        });

        if let Some(name) = axes.get(i) {
            let a = angle(i);
            let (lx, ly) = (
                cx + (radius + 10.0) * a.cos(),
                cy + (radius + 10.0) * a.sin(),
            );
            // The label sits outside its spoke and leans away from the centre,
            // or the names on the left overlap the shape.
            let anchor = if a.cos() > 0.2 {
                Anchor::Start
            } else if a.cos() < -0.2 {
                Anchor::End
            } else {
                Anchor::Middle
            };
            annotations.push(Annotation {
                text: name.clone(),
                x: lx,
                y: ly + 4.0,
                anchor,
                size: 11.0,
                color: ink(&base, &theme),
                weight: "400".into(),
                family: family(&base),
            });
        }
    }

    for (_, points, colour, style) in &series {
        let shape: Vec<(f64, f64)> = points
            .iter()
            .enumerate()
            .map(|(i, v)| point(i, *v))
            .collect();
        if shape.len() < 2 {
            continue;
        }
        let (opaque, _) = crate::render_svg::split_alpha(colour);
        polylines.push(Polyline {
            points: shape.clone(),
            closed: true,
            fill: Some(format!("{opaque}33")),
            stroke: Some(colour.clone()),
            width: style.stroke_width.unwrap_or(2.0),
            head: None,
            dash: None,
        });
        for (x, y) in shape {
            nodes.push(PlacedNode {
                lines: Vec::new(),
                rect: Rect {
                    x: x - 3.5,
                    y: y - 3.5,
                    w: 7.0,
                    h: 7.0,
                },
                style: Style {
                    fill: Some(colour.clone()),
                    stroke: Some(theme.surface.clone()),
                    stroke_width: Some(1.0),
                    shape: Some(Shape::Ellipse),
                    ..base.clone()
                },
            });
        }
    }

    if toggle(cfg, "legend", series.len() > 1) && !series.is_empty() {
        let names: Vec<String> = series.iter().map(|(n, _, _, _)| n.clone()).collect();
        let colours: Vec<String> = series.iter().map(|(_, _, c, _)| c.clone()).collect();
        draw_legend(
            &names,
            &colours,
            cx + radius + margin,
            cy - radius,
            &theme,
            &base,
            &mut nodes,
            &mut annotations,
        );
    }

    Ok(Layout {
        guides,
        polylines,
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

/// A user journey: steps in the order they are taken, one line per actor.
/// `timeline` supplies the idea of a run of things along an axis and `xy` the
/// idea of a series over it; what this adds is that the two are read together
/// — the point of a journey is the step where two people diverge.
#[cfg(feature = "journey")]
pub fn journey(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    use std::collections::HashMap;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(620.0)
        .max(160.0);
    let height = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(220.0)
        .max(80.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let actors: Vec<String> = setting(cfg, "actors")
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_text())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let (lo, hi) = match setting(cfg, "range") {
        Some(v) if v.len() == 2 => (v[0].as_num().unwrap_or(1.0), v[1].as_num().unwrap_or(5.0)),
        _ => (1.0, 5.0),
    };

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    let mut steps = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        let scores = style.scores.clone().unwrap_or_default();
        steps.push((node.id.value.clone(), label, scores));
    }

    // A satisfaction score has no meaning between two whole steps, so the axis
    // is labelled at each one rather than at a "nice" interval.
    let scale = Scale {
        min: lo.min(hi),
        max: hi.max(lo + 1.0),
        ticks: {
            let (a, b) = (lo.min(hi).ceil() as i64, hi.max(lo).floor() as i64);
            (a..=b).map(|n| n as f64).collect()
        },
    };

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    const SECTION_H: f64 = 24.0;
    let band = if diagram.groups.is_empty() {
        0.0
    } else {
        SECTION_H
    };
    let gutter = scale
        .ticks
        .iter()
        .map(|t| text_width(&[trim_number(*t)], 11.0))
        .fold(0.0_f64, f64::max)
        + 14.0;
    let plot = Rect {
        x: gutter,
        y: title_h + band,
        w: width,
        h: height,
    };

    // Steps sit on the plot's edges, as an `xy` series does: a line has to
    // reach both ends or it reads as clipped.
    let at = |i: usize| -> f64 {
        if steps.len() <= 1 {
            0.5
        } else {
            i as f64 / (steps.len() - 1) as f64
        }
    };

    let mut guides = Vec::new();
    let mut polylines = Vec::new();
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

    let x_labels: Vec<(f64, String)> = steps
        .iter()
        .enumerate()
        .map(|(i, (_, label, _))| (at(i), label.clone()))
        .collect();
    draw_frame(
        plot,
        &scale,
        &x_labels,
        &theme,
        &base,
        &mut guides,
        &mut annotations,
    );

    // One line per actor. The count comes from `actors`, so a step that wrote
    // too few scores simply has no point on the lines it left out — `check`
    // is what says so.
    for (a, name) in actors.iter().enumerate() {
        let colour = palette[a.min(palette.len() - 1)].to_string();
        let path: Vec<(f64, f64)> = steps
            .iter()
            .enumerate()
            .filter_map(|(i, (_, _, scores))| {
                scores.get(a).map(|s| {
                    (
                        plot.x + at(i) * plot.w,
                        plot.bottom() - scale.fraction(*s) * plot.h,
                    )
                })
            })
            .collect();

        if path.len() >= 2 {
            polylines.push(Polyline {
                points: path.clone(),
                closed: false,
                fill: None,
                stroke: Some(colour.clone()),
                width: 2.0,
                head: None,
                dash: None,
            });
        }
        for (x, y) in path {
            nodes.push(PlacedNode {
                lines: Vec::new(),
                rect: Rect {
                    x: x - 4.5,
                    y: y - 4.5,
                    w: 9.0,
                    h: 9.0,
                },
                style: Style {
                    fill: Some(colour.clone()),
                    stroke: Some(theme.surface.clone()),
                    stroke_width: Some(1.5),
                    shape: Some(Shape::Ellipse),
                    ..base.clone()
                },
            });
        }
        let _ = name;
    }

    // Sections, as a band above the steps they hold — the same `group`
    // machinery kanban uses for its columns.
    let index: HashMap<&str, usize> = steps
        .iter()
        .enumerate()
        .map(|(i, (id, _, _))| (id.as_str(), i))
        .collect();
    for group in &diagram.groups {
        let held: Vec<usize> = group
            .members
            .iter()
            .filter_map(|m| index.get(m.as_str()).copied())
            .collect();
        let (Some(first), Some(last)) = (held.iter().min(), held.iter().max()) else {
            continue;
        };
        let x0 = plot.x + at(*first) * plot.w;
        let x1 = plot.x + at(*last) * plot.w;
        let style = registry.resolve(
            Style::group_defaults(),
            "group",
            &group.classes,
            &group.props,
        )?;
        groups.push(PlacedGroup {
            lines: vec![
                group
                    .label
                    .clone()
                    .unwrap_or_else(|| group.id.value.clone()),
            ],
            rect: Rect {
                x: x0 - 8.0,
                y: title_h,
                w: (x1 - x0) + 16.0,
                h: SECTION_H - 6.0,
            },
            style,
        });
    }

    if toggle(cfg, "legend", actors.len() > 1) && !actors.is_empty() {
        let colours: Vec<String> = (0..actors.len())
            .map(|a| palette[a.min(palette.len() - 1)].to_string())
            .collect();
        draw_legend(
            &actors,
            &colours,
            plot.right() + 18.0,
            plot.y,
            &theme,
            &base,
            &mut nodes,
            &mut annotations,
        );
    }

    Ok(Layout {
        guides,
        polylines,
        groups,
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

/// A board. Columns are `group`s in declaration order and cards stack inside
/// them in declaration order, so the archetype places everything — a kanban
/// whose columns had to be anchored by hand would be busywork with no reading
/// gained. The `group` machinery (`W-011`) supplies the membership; what this
/// adds is the arrangement and the header.
#[cfg(feature = "kanban")]
pub fn kanban(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    use std::collections::HashMap;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);

    let col_w = number(cfg, "column", 220.0).max(80.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    const PAD: f64 = 12.0;
    const CARD_GAP: f64 = 10.0;
    const COL_GAP: f64 = 18.0;
    const HEADER: f64 = 30.0;

    let by_id: HashMap<&str, &crate::ast::NodeDecl> =
        diagram.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
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

    let mut tallest: f64 = 0.0;
    for (col, group) in diagram.groups.iter().enumerate() {
        let x = col as f64 * (col_w + COL_GAP);
        let top = title_h + HEADER;
        let mut y = top + PAD;
        let mut held = 0usize;

        for member in &group.members {
            let Some(node) = by_id.get(member.as_str()) else {
                continue;
            };
            let style = registry.resolve(
                chart_defaults(),
                def.item,
                &node.classes,
                &node.props_for(def),
            )?;
            let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
            let size = style.font_size.unwrap_or(13.0);
            // A fixed column count, like every other wrap here: the box is
            // sized from the lines, so measuring against a width the lines
            // have not produced yet would be circular.
            // The wrap column is still a character count, so it uses the
            // average advance rather than a per-character measure — the
            // lines it produces are then sized properly by `advance`.
            let lines = crate::layout::wrap(&label, ((col_w - 2.0 * PAD) / (size * 0.52)) as usize);
            let h = lines.len() as f64 * size * 1.35 + 2.0 * PAD;

            nodes.push(PlacedNode {
                lines,
                rect: Rect {
                    x: x + PAD,
                    y,
                    w: col_w - 2.0 * PAD,
                    h,
                },
                style,
            });
            y += h + CARD_GAP;
            held += 1;
        }

        let bottom = if held == 0 {
            top + PAD * 3.0
        } else {
            y - CARD_GAP + PAD
        };
        tallest = tallest.max(bottom);

        let name = group
            .label
            .clone()
            .unwrap_or_else(|| group.id.value.clone());
        // The count against the limit, in the header — the number the board is
        // kept for, where a reader is already looking.
        let frame = registry.resolve(
            Style::group_defaults(),
            "group",
            &group.classes,
            &group.props,
        )?;
        let limit = frame.limit;
        annotations.push(Annotation {
            text: match limit {
                Some(n) => format!("{name}  {held}/{}", trim_number(n)),
                None => format!("{name}  {held}"),
            },
            x: x + PAD,
            y: title_h + HEADER - 10.0,
            anchor: Anchor::Start,
            size: 12.0,
            color: ink(&base, &theme),
            weight: "600".into(),
            family: family(&base),
        });

        groups.push(PlacedGroup {
            lines: Vec::new(),
            rect: Rect {
                x,
                y: top,
                w: col_w,
                h: bottom - top,
            },
            style: frame,
        });
    }

    // Every column is drawn to the height of the tallest, or the board reads
    // as ragged shelves rather than as one thing.
    for frame in &mut groups {
        frame.rect.h = tallest - frame.rect.y;
    }

    Ok(Layout {
        guides: Vec::new(),
        polylines: Vec::new(),
        groups,
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Free,
        annotations,
    })
}

#[cfg(feature = "packet")]
pub fn packet(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let unit = Granularity {
        per_row: 32.0,
        step: 8,
    };
    packet_like(diagram, opts, registry, unit)
}

#[cfg(feature = "bytefield")]
pub fn bytefield(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let unit = Granularity {
        per_row: 16.0,
        step: 4,
    };
    packet_like(diagram, opts, registry, unit)
}

#[cfg(any(feature = "packet", feature = "bytefield"))]
fn packet_like(
    diagram: &Diagram,
    opts: GraphOpts,
    registry: &Registry,
    unit: Granularity,
) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);

    let per_row = number(cfg, "width", unit.per_row).max(1.0) as u32;
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
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let def = crate::check::archetype_of(diagram);
    let mut fields = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            Style::node_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        // Only one of the two can be set: the other property does not exist
        // in this genre, so writing it is an unknown-property error naming the
        // genre that does read it.
        let Some((first, last)) = style.bits.or(style.bytes) else {
            continue;
        };
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        fields.push((first, last, label, style));
    }
    fields.sort_by_key(|f| f.0);

    let ruler_h = if ruler { 16.0 } else { 0.0 };
    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
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
        let step = if per_row.is_multiple_of(unit.step) {
            unit.step
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

#[cfg(feature = "treemap")]
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
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;
    let roots = build_tiles(diagram, registry, def, None)?;

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
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
                &node.props_for(def),
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

#[cfg(feature = "quadrant")]
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
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;
    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
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
            &node.props_for(def),
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

#[cfg(feature = "wave")]
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
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    // Resolve every signal first: the name column and the canvas both need
    // to know the widest name and the longest wave before anything is drawn.
    let mut signals = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
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
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
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
                head: None,
                dash: None,
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
                    head: None,
                    dash: None,
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
                        head: None,
                        dash: None,
                    });
                }
                i += 1;
            }
        }
    }

    flush_line(&mut line, polylines);
}

// ---------------------------------------------------------------- sankey

#[cfg(feature = "sankey")]
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
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

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
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
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
            &node.props_for(def),
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
            head: None,
            dash: None,
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

// ------------------------------------------------------- the plot frame

/// A value axis: the span it covers and the round numbers it is marked at.
pub struct Scale {
    pub min: f64,
    pub max: f64,
    pub ticks: Vec<f64>,
}

impl Scale {
    /// Where a value falls along the axis, 0 at `min` and 1 at `max`.
    pub fn fraction(&self, v: f64) -> f64 {
        let span = self.max - self.min;
        if span.abs() < f64::EPSILON {
            0.0
        } else {
            (v - self.min) / span
        }
    }
}

/// Rounds the span outward to human numbers — 1, 2, 2.5 or 5 times a power of
/// ten — so the axis reads in figures a person would have chosen.
pub fn nice_scale(min: f64, max: f64, want: usize) -> Scale {
    let want = want.max(2);
    let (min, max) = if (max - min).abs() < f64::EPSILON {
        (min.min(0.0), max.max(min + 1.0))
    } else {
        (min, max)
    };

    let raw = (max - min) / want as f64;
    let magnitude = 10f64.powf(raw.abs().max(f64::MIN_POSITIVE).log10().floor());
    let step = magnitude
        * match raw / magnitude {
            r if r <= 1.0 => 1.0,
            r if r <= 2.0 => 2.0,
            r if r <= 2.5 => 2.5,
            r if r <= 5.0 => 5.0,
            _ => 10.0,
        };

    let lo = (min / step).floor() * step;
    let hi = (max / step).ceil() * step;

    let mut ticks = Vec::new();
    let mut t = lo;
    // Guard on count, not on floating-point arithmetic reaching `hi` exactly.
    for _ in 0..=(((hi - lo) / step).round() as usize).min(64) {
        ticks.push(t);
        t += step;
    }

    Scale {
        min: lo,
        max: hi,
        ticks,
    }
}

/// Axis lines, gridlines and their labels. Recessive by design: the marks are
/// the subject, and a grid that competes with them is a grid in the way.
#[allow(clippy::too_many_arguments)]
fn draw_frame(
    plot: Rect,
    scale: &Scale,
    // `x_labels` places each label across the plot, 0 at the left edge and 1
    // at the right — slot centres for bars, tick positions for a numeric axis.
    x_labels: &[(f64, String)],
    theme: &Theme,
    base: &Style,
    guides: &mut Vec<Guide>,
    annotations: &mut Vec<Annotation>,
) {
    let rule = if theme.dark { "#1e293b" } else { "#e4e4e7" };
    let axis = if theme.dark { "#334155" } else { "#d4d4d8" };

    for value in &scale.ticks {
        let y = plot.bottom() - scale.fraction(*value) * plot.h;
        guides.push(Guide {
            from: (plot.x, y),
            to: (plot.right(), y),
            color: if *value == 0.0 { axis } else { rule }.into(),
            width: 1.0,
            dash: None,
        });
        annotations.push(Annotation {
            text: trim_number(*value),
            x: plot.x - 8.0,
            y,
            anchor: Anchor::End,
            size: 11.0,
            color: theme.muted.clone(),
            weight: "400".into(),
            family: family(base),
        });
    }

    guides.push(Guide {
        from: (plot.x, plot.y),
        to: (plot.x, plot.bottom()),
        color: axis.into(),
        width: 1.0,
        dash: None,
    });

    for (fraction, name) in x_labels {
        annotations.push(Annotation {
            text: name.clone(),
            x: plot.x + fraction * plot.w,
            y: plot.bottom() + 16.0,
            anchor: Anchor::Middle,
            size: 11.0,
            color: theme.muted.clone(),
            weight: "500".into(),
            family: family(base),
        });
    }
}

/// Swatch, name, one per series — always present past a single series, so
/// identity never rests on colour alone.
#[allow(clippy::too_many_arguments)]
fn draw_legend(
    names: &[String],
    colours: &[String],
    x: f64,
    y: f64,
    theme: &Theme,
    base: &Style,
    nodes: &mut Vec<PlacedNode>,
    annotations: &mut Vec<Annotation>,
) {
    for (i, (name, colour)) in names.iter().zip(colours).enumerate() {
        let row = y + i as f64 * LEGEND_STEP;
        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x,
                y: row,
                w: SWATCH,
                h: SWATCH,
            },
            style: Style {
                fill: Some(colour.clone()),
                stroke: Some("none".into()),
                stroke_width: Some(0.0),
                radius: Some(3.0),
                shape: Some(Shape::Rect),
                ..Style::default()
            },
        });
        annotations.push(Annotation {
            text: name.clone(),
            x: x + SWATCH + 9.0,
            y: row + SWATCH / 2.0,
            anchor: Anchor::Start,
            size: 12.0,
            color: theme.ink.clone(),
            weight: "400".into(),
            family: family(base),
        });
    }
}

// ------------------------------------------------------------------- bar

#[cfg(feature = "bar")]
pub fn bar(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(560.0)
        .max(120.0);
    let height = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(320.0)
        .max(80.0);
    let stacked = toggle(cfg, "stack", false);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let categories: Vec<String> = setting(cfg, "categories")
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_text())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    let mut series = Vec::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let Some(points) = style.points.clone().filter(|p| !p.is_empty()) else {
            continue;
        };
        let colour = style
            .fill
            .clone()
            .unwrap_or_else(|| palette[i.min(palette.len() - 1)].to_string());
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        series.push((name, points, colour, style));
    }

    let slots = categories.len().max(
        series
            .iter()
            .map(|(_, points, _, _)| points.len())
            .max()
            .unwrap_or(0),
    );

    // The axis covers what the data reaches — stacked totals, not the tallest
    // single bar — and always includes zero, or the bars would float.
    let (mut lo, mut hi) = (0.0_f64, 0.0_f64);
    for slot in 0..slots {
        let column: Vec<f64> = series
            .iter()
            .map(|(_, points, _, _)| points.get(slot).copied().unwrap_or(0.0))
            .collect();
        if stacked {
            hi = hi.max(column.iter().filter(|v| **v > 0.0).sum());
            lo = lo.min(column.iter().filter(|v| **v < 0.0).sum());
        } else {
            hi = hi.max(column.iter().copied().fold(f64::MIN, f64::max));
            lo = lo.min(column.iter().copied().fold(f64::MAX, f64::min));
        }
    }
    let scale = match setting(cfg, "y-axis") {
        Some(v) if v.len() == 2 => {
            let (a, b) = (v[0].as_num().unwrap_or(lo), v[1].as_num().unwrap_or(hi));
            nice_scale(a, b, 5)
        }
        _ => nice_scale(lo, hi, 5),
    };

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    let gutter = scale
        .ticks
        .iter()
        .map(|t| text_width(&[trim_number(*t)], 11.0))
        .fold(0.0_f64, f64::max)
        + 14.0;
    let plot = Rect {
        x: gutter,
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

    let slots_for_labels = categories.len().max(1) as f64;
    let x_labels: Vec<(f64, String)> = categories
        .iter()
        .enumerate()
        .map(|(i, name)| ((i as f64 + 0.5) / slots_for_labels, name.clone()))
        .collect();
    draw_frame(
        plot,
        &scale,
        &x_labels,
        &theme,
        &base,
        &mut guides,
        &mut annotations,
    );

    let zero = plot.bottom() - scale.fraction(0.0) * plot.h;
    let slot_w = if slots > 0 {
        plot.w / slots as f64
    } else {
        plot.w
    };
    // A 2px surface gap between fills, and room either side of a group.
    let group_w = slot_w * 0.72;
    let bar_w = if stacked || series.is_empty() {
        group_w
    } else {
        (group_w - 2.0 * (series.len() - 1) as f64) / series.len() as f64
    };

    let mut stack_up = vec![0.0; slots];
    let mut stack_down = vec![0.0; slots];

    for (s, (_, points, colour, style)) in series.iter().enumerate() {
        for slot in 0..slots {
            let value = points.get(slot).copied().unwrap_or(0.0);
            if value == 0.0 {
                continue;
            }

            let left = plot.x + slot_w * slot as f64 + (slot_w - group_w) / 2.0;
            let x = if stacked {
                left
            } else {
                left + s as f64 * (bar_w + 2.0)
            };

            let (base_value, top_value) = if stacked {
                let run = if value > 0.0 {
                    &mut stack_up[slot]
                } else {
                    &mut stack_down[slot]
                };
                let from = *run;
                *run += value;
                (from, *run)
            } else {
                (0.0, value)
            };

            let y0 = plot.bottom() - scale.fraction(base_value) * plot.h;
            let y1 = plot.bottom() - scale.fraction(top_value) * plot.h;

            // Adjacent fills must never touch, stacked segments included.
            let inset = if stacked { 1.0 } else { 0.0 };
            nodes.push(PlacedNode {
                lines: Vec::new(),
                rect: Rect {
                    x,
                    y: y0.min(y1) + inset,
                    w: bar_w.max(1.0),
                    h: ((y1 - y0).abs() - inset * 2.0).max(1.0),
                },
                style: Style {
                    fill: Some(colour.clone()),
                    stroke: Some("none".into()),
                    stroke_width: Some(0.0),
                    // Rounded on the data end only would need two shapes; a
                    // small radius reads as intentional on both.
                    radius: Some(3.0),
                    shape: Some(Shape::Rect),
                    ..style.clone()
                },
            });
        }
    }

    let _ = zero;

    // A legend is not optional past one series: colour alone is not identity.
    let wants_legend = toggle(cfg, "legend", series.len() > 1);
    if wants_legend && !series.is_empty() {
        let names: Vec<String> = series.iter().map(|(n, _, _, _)| n.clone()).collect();
        let colours: Vec<String> = series.iter().map(|(_, _, c, _)| c.clone()).collect();
        draw_legend(
            &names,
            &colours,
            plot.right() + LEGEND_MARGIN,
            plot.y,
            &theme,
            &base,
            &mut nodes,
            &mut annotations,
        );
    }

    Ok(Layout {
        guides,
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Bar,
        annotations,
    })
}

// -------------------------------------------------------------------- xy

#[cfg(feature = "xy")]
pub fn xy(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(560.0)
        .max(120.0);
    let height = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(320.0)
        .max(80.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());
    let categories: Vec<String> = setting(cfg, "categories")
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_text())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    // `series` spreads its numbers across the axis; `scatter` reads them as
    // coordinates (`D-023`). Both end up as `(x, y)` here, with an indexed
    // series taking its position as x — `check` refuses a file mixing the
    // two, so the plot only ever means one of them.
    let mut series = Vec::new();
    let mut scattered = false;
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let points: Vec<(f64, f64)> = match style.pairs.clone().filter(|p| p.len() >= 2) {
            Some(flat) => {
                scattered = true;
                // `chunks_exact` drops a trailing x rather than inventing a y
                // for it; `check` is what tells the author the count is odd.
                let mut pairs: Vec<(f64, f64)> =
                    flat.chunks_exact(2).map(|c| (c[0], c[1])).collect();
                // A line through points in *declaration* order zigzags. Sorting
                // by x costs a scatter nothing and is the only reading of
                // `mark: line` that means anything.
                pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
                pairs
            }
            None => match style.points.clone().filter(|p| !p.is_empty()) {
                Some(values) => values
                    .into_iter()
                    .enumerate()
                    .map(|(n, v)| (n as f64, v))
                    .collect(),
                None => continue,
            },
        };
        let colour = style
            .fill
            .clone()
            .or_else(|| style.stroke.clone())
            .unwrap_or_else(|| palette[i.min(palette.len() - 1)].to_string());
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        series.push((name, points, colour, style));
    }

    // A scatter is points until told otherwise; joining them by default would
    // claim a continuity the data does not have.
    let mark = word(cfg, "mark", if scattered { "dot" } else { "line" });
    let count = series.iter().map(|(_, p, _, _)| p.len()).max().unwrap_or(0);

    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for (_, points, _, _) in &series {
        for (_, v) in points {
            lo = lo.min(*v);
            hi = hi.max(*v);
        }
    }
    if series.is_empty() {
        (lo, hi) = (0.0, 1.0);
    }
    // An area is read against the baseline, so the axis has to include it.
    if mark == "area" {
        lo = lo.min(0.0);
    }
    let scale = match setting(cfg, "y-axis") {
        Some(v) if v.len() == 2 => {
            nice_scale(v[0].as_num().unwrap_or(lo), v[1].as_num().unwrap_or(hi), 5)
        }
        _ => nice_scale(lo, hi, 5),
    };

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    let gutter = scale
        .ticks
        .iter()
        .map(|t| text_width(&[trim_number(*t)], 11.0))
        .fold(0.0_f64, f64::max)
        + 14.0;
    let plot = Rect {
        x: gutter,
        y: title_h,
        w: width,
        h: height,
    };

    // An indexed series is placed by position and needs no x scale; a scatter
    // is placed by the numbers written, so it needs one. A declared `x-axis`
    // supplies it either way.
    let (mut x_lo, mut x_hi) = (f64::MAX, f64::MIN);
    for (_, points, _, _) in &series {
        for (x, _) in points {
            x_lo = x_lo.min(*x);
            x_hi = x_hi.max(*x);
        }
    }
    if series.is_empty() {
        (x_lo, x_hi) = (0.0, count as f64);
    }
    let x_scale = match setting(cfg, "x-axis") {
        Some(v) if v.len() == 2 => Some(nice_scale(
            v[0].as_num().unwrap_or(x_lo),
            v[1].as_num().unwrap_or(x_hi),
            5,
        )),
        _ if scattered => Some(nice_scale(x_lo, x_hi, 5)),
        _ => None,
    };

    // Points sit on the plot's edges, not in slot centres: a line has to
    // reach the axis at both ends or it reads as clipped.
    let at = |x: f64| -> f64 {
        match &x_scale {
            Some(scale) => scale.fraction(x),
            None if count <= 1 => 0.5,
            None => x / (count - 1) as f64,
        }
    };

    let x_labels: Vec<(f64, String)> = match &x_scale {
        Some(scale) => scale
            .ticks
            .iter()
            .map(|t| (scale.fraction(*t), trim_number(*t)))
            .collect(),
        None => categories
            .iter()
            .enumerate()
            .map(|(i, name)| (at(i as f64), name.clone()))
            .collect(),
    };

    let mut guides = Vec::new();
    let mut polylines = Vec::new();
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

    draw_frame(
        plot,
        &scale,
        &x_labels,
        &theme,
        &base,
        &mut guides,
        &mut annotations,
    );

    for (_, points, colour, style) in &series {
        let path: Vec<(f64, f64)> = points
            .iter()
            .map(|(x, v)| {
                (
                    plot.x + at(*x) * plot.w,
                    plot.bottom() - scale.fraction(*v) * plot.h,
                )
            })
            .collect();

        if mark == "area" && path.len() >= 2 {
            let base_y = plot.bottom() - scale.fraction(scale.min.max(0.0)) * plot.h;
            let mut band = path.clone();
            band.push((path[path.len() - 1].0, base_y));
            band.push((path[0].0, base_y));
            let (opaque, _) = crate::render_svg::split_alpha(colour);
            polylines.push(Polyline {
                points: band,
                closed: true,
                fill: Some(format!("{opaque}40")),
                stroke: None,
                width: 0.0,
                head: None,
                dash: None,
            });
        }

        if mark != "dot" && path.len() >= 2 {
            polylines.push(Polyline {
                points: path.clone(),
                closed: false,
                fill: None,
                stroke: Some(colour.clone()),
                width: style.stroke_width.unwrap_or(2.0),
                head: None,
                dash: None,
            });
        }

        // Markers are at least 8px across, or they are not targets.
        if mark == "dot" || path.len() == 1 {
            for (x, y) in path {
                nodes.push(PlacedNode {
                    lines: Vec::new(),
                    rect: Rect {
                        x: x - 4.5,
                        y: y - 4.5,
                        w: 9.0,
                        h: 9.0,
                    },
                    style: Style {
                        fill: Some(colour.clone()),
                        stroke: Some(theme.surface.clone()),
                        stroke_width: Some(1.5),
                        shape: Some(Shape::Ellipse),
                        ..style.clone()
                    },
                });
            }
        }
    }

    let wants_legend = toggle(cfg, "legend", series.len() > 1);
    if wants_legend && !series.is_empty() {
        let names: Vec<String> = series.iter().map(|(n, _, _, _)| n.clone()).collect();
        let colours: Vec<String> = series.iter().map(|(_, _, c, _)| c.clone()).collect();
        draw_legend(
            &names,
            &colours,
            plot.right() + LEGEND_MARGIN,
            plot.y,
            &theme,
            &base,
            &mut nodes,
            &mut annotations,
        );
    }

    Ok(Layout {
        guides,
        polylines,
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Xy,
        annotations,
    })
}

// -------------------------------------------------------------- sequence

#[cfg(feature = "sequence")]
pub fn sequence(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);

    let column = number(cfg, "column", 150.0).max(60.0);
    let row = number(cfg, "row", 46.0).max(20.0);
    let numbered = toggle(cfg, "numbered", false);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(Style::node_defaults(), def.item, &[], &[])?;
    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };

    let index: HashMap<&str, usize> = diagram
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id.as_str(), i))
        .collect();

    let mut guides = Vec::new();
    let mut polylines = Vec::new();
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

    // Participants across the top, each owning a column.
    const HEAD_H: f64 = 42.0;
    let head_y = title_h;
    let lifeline_top = head_y + HEAD_H;
    let first_row = lifeline_top + row * 0.9;
    let rows = diagram.edges.len();
    let bottom = first_row + rows as f64 * row + row * 0.4;

    let centre = |i: usize| column * i as f64 + column / 2.0;

    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            Style::node_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let label = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        let w = (text_width(
            std::slice::from_ref(&label),
            style.font_size.unwrap_or(13.0),
        ) + style.padding.unwrap_or(14.0) * 2.0)
            .min(column - 12.0)
            .max(50.0);

        nodes.push(PlacedNode {
            lines: vec![label],
            rect: Rect {
                x: centre(i) - w / 2.0,
                y: head_y,
                w,
                h: HEAD_H,
            },
            style,
        });

        // The lifeline: dashed, recessive, the whole height of the exchange.
        guides.push(Guide {
            from: (centre(i), lifeline_top),
            to: (centre(i), bottom),
            color: if theme.dark { "#334155" } else { "#d4d4d8" }.into(),
            width: 1.0,
            dash: Some("4 5".into()),
        });
    }

    // An activation bar: the box down a participant's lifeline for as long as
    // a call is outstanding (`D-024`). It is a thin `PlacedNode` pushed after
    // the lifeline rather than a `Guide`, because guides draw *beneath*
    // everything and this has to sit on top of the line it covers.
    for group in diagram.groups.iter().filter(|g| g.activates) {
        let (Some(&first), Some(&last)) = (group.edge_members.first(), group.edge_members.last())
        else {
            continue;
        };
        let Some(&col) = index.get(group.id.as_str()) else {
            continue;
        };
        let style = registry.resolve(
            Style::node_defaults(),
            def.item,
            &group.classes,
            &group.props,
        )?;
        const BAR_W: f64 = 10.0;
        let top = first_row + first as f64 * row - row * 0.5;
        let height = (last - first) as f64 * row + row * 0.9;
        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x: centre(col) - BAR_W / 2.0,
                y: top,
                w: BAR_W,
                h: height,
            },
            style: Style {
                radius: Some(2.0),
                ..style
            },
        });
    }

    // A group frames the rows of the messages it holds.
    for group in diagram.groups.iter().filter(|g| !g.activates) {
        let rows_inside: Vec<usize> = group.edge_members.clone();
        let Some(&first) = rows_inside.first() else {
            continue;
        };
        let last = *rows_inside.last().unwrap_or(&first);

        let touched: Vec<usize> = rows_inside
            .iter()
            .filter_map(|e| {
                let edge = &diagram.edges[*e];
                index
                    .get(edge.from.as_str())
                    .into_iter()
                    .chain(index.get(edge.to.as_str()))
                    .copied()
                    .min()
            })
            .collect();
        let lo = touched.iter().copied().min().unwrap_or(0);
        let hi = rows_inside
            .iter()
            .filter_map(|e| {
                let edge = &diagram.edges[*e];
                index
                    .get(edge.from.as_str())
                    .into_iter()
                    .chain(index.get(edge.to.as_str()))
                    .copied()
                    .max()
            })
            .max()
            .unwrap_or(diagram.nodes.len().saturating_sub(1));

        let style = registry.resolve(
            Style::group_defaults(),
            "group",
            &group.classes,
            &group.props,
        )?;
        // The frame sits between the message before it and the one after,
        // never a whole row above — that would swallow the previous message.
        // Its title needs a band of its own inside that, or it lands on the
        // first message and on the lifeline it crosses.
        let above = row * 0.62 + 14.0;
        let below = row * 0.42;

        // Wide enough to carry its own title clear of the first lifeline it
        // spans. A plate cannot help here: an activation bar is a node and is
        // drawn after the frame, so it would cover the plate as well.
        let title_w = group
            .label
            .as_ref()
            .map(|l| text_width(std::slice::from_ref(l), 12.0) + 26.0)
            .unwrap_or(0.0);
        let left = (centre(lo) - column * 0.36).min(centre(lo) - title_w);

        groups.push(PlacedGroup {
            lines: group
                .label
                .clone()
                .map(|l| vec![l])
                .unwrap_or_else(|| vec![group.id.value.clone()]),
            rect: Rect {
                x: left,
                y: first_row + first as f64 * row - above,
                w: (centre(hi) - left) + column * 0.36,
                h: (last - first) as f64 * row + above + below,
            },
            style,
        });
    }

    // Messages, one row each, in the order they were written.
    for (r, edge) in diagram.edges.iter().enumerate() {
        let (Some(&from), Some(&to)) = (index.get(edge.from.as_str()), index.get(edge.to.as_str()))
        else {
            continue;
        };
        let style = registry.resolve(
            Style::edge_defaults(),
            "edge",
            &edge.classes,
            &edge.effective_props(def.edge_slots),
        )?;
        let stroke = style.stroke.clone().unwrap_or_else(|| "#64748b".into());
        let y = first_row + r as f64 * row;

        let mut label = edge.label.clone().unwrap_or_default();
        if numbered {
            label = if label.is_empty() {
                format!("{}", r + 1)
            } else {
                format!("{}. {label}", r + 1)
            };
        }

        // A message to oneself has no horizontal run: it loops out and back.
        let points = if from == to {
            let x = centre(from);
            let out = column * 0.28;
            vec![
                (x, y - row * 0.22),
                (x + out, y - row * 0.22),
                (x + out, y + row * 0.16),
                (x + 1.0, y + row * 0.16),
            ]
        } else {
            vec![(centre(from), y), (centre(to), y)]
        };

        polylines.push(Polyline {
            points: points.clone(),
            closed: false,
            fill: None,
            stroke: Some(stroke),
            width: style.stroke_width.unwrap_or(1.5),
            head: Some(style.head.unwrap_or(Head::Solid)),
            dash: style.dash_array(),
        });

        if !label.is_empty() {
            let (x, anchor, ly) = if from == to {
                (centre(from) + column * 0.30, Anchor::Start, y - row * 0.05)
            } else {
                ((centre(from) + centre(to)) / 2.0, Anchor::Middle, y - 9.0)
            };
            annotations.push(Annotation {
                text: label,
                x,
                y: ly,
                anchor,
                size: style.font_size.unwrap_or(12.0),
                color: ink(&style, &theme),
                weight: style.font_weight.clone().unwrap_or_else(|| "400".into()),
                family: family(&style),
            });
        }
    }

    Ok(Layout {
        guides,
        polylines,
        groups,
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Sequence,
        annotations,
    })
}

// ----------------------------------------------------------------- gantt

#[cfg(feature = "gantt")]
pub fn gantt(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    use crate::ast::civil_from_days;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);
    let palette = theme.palette();

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(620.0)
        .max(160.0);
    let row_h = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(26.0)
        .max(10.0);
    let today = setting(cfg, "today")
        .and_then(|v| v.first())
        .and_then(|v| v.as_date());
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;

    // Which section, if any, each task belongs to.
    let section_of: HashMap<&str, &str> = diagram
        .groups
        .iter()
        .flat_map(|g| g.members.iter().map(move |m| (m.as_str(), g.id.as_str())))
        .collect();

    let mut tasks = Vec::new();
    for (i, node) in diagram.nodes.iter().enumerate() {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let Some(start) = style.start else { continue };
        let length = style.length.unwrap_or(1.0).max(0.25);
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        let section = section_of.get(node.id.as_str()).copied();
        tasks.push((i, name, start, length, section, style));
    }

    let (first, last) = tasks.iter().fold((i64::MAX, i64::MIN), |(lo, hi), t| {
        (lo.min(t.2), hi.max(t.2 + t.3.ceil() as i64))
    });
    let (first, last) = if tasks.is_empty() {
        (0, 1)
    } else {
        (first, last.max(first + 1))
    };
    let span = (last - first) as f64;

    let name_size = base.font_size.unwrap_or(13.0);
    // Sections get a gutter of their own: a label sharing the task column
    // lands on top of the first task's name.
    let section_w = diagram
        .groups
        .iter()
        .map(|g| {
            let label = g.label.clone().unwrap_or_else(|| g.id.value.clone());
            text_width(std::slice::from_ref(&label), 11.0) + 24.0
        })
        .fold(0.0_f64, f64::max);
    let names_w = section_w
        + tasks
            .iter()
            .map(|t| text_width(std::slice::from_ref(&t.1), name_size))
            .fold(0.0_f64, f64::max)
        + 18.0;

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    const AXIS_H: f64 = 20.0;
    let top = title_h + AXIS_H;
    let day = width / span;
    let x_of = |d: i64| names_w + (d - first) as f64 * day;

    let mut guides = Vec::new();
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

    // Section bands first, so the bars land on them.
    let mut ordered: Vec<(Option<&str>, usize)> = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    for name in tasks.iter().filter_map(|t| t.4) {
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    // Sectioned tasks group together, in the order their sections appear.
    for section in seen.iter().map(|s| Some(*s)).chain(std::iter::once(None)) {
        for (i, task) in tasks.iter().enumerate() {
            if task.4 == section {
                ordered.push((section, i));
            }
        }
    }

    let rule = if theme.dark { "#1e293b" } else { "#e4e4e7" };
    let step = pick_tick(span, (width / 52.0) as usize);
    let mut d = first;
    while d <= last {
        let x = x_of(d);
        guides.push(Guide {
            from: (x, top),
            to: (x, top + ordered.len() as f64 * row_h),
            color: rule.into(),
            width: 1.0,
            dash: None,
        });
        let (y, m, dd) = civil_from_days(d);
        annotations.push(Annotation {
            text: match step {
                s if s >= 28 => format!("{y}-{m:02}"),
                _ => format!("{m:02}-{dd:02}"),
            },
            x: x + 3.0,
            y: title_h + AXIS_H / 2.0,
            anchor: Anchor::Start,
            size: 10.0,
            color: theme.muted.clone(),
            weight: "400".into(),
            family: family(&base),
        });
        d += step;
    }

    for (r, (section, i)) in ordered.iter().enumerate() {
        let (_, name, start, length, _, style) = &tasks[*i];
        let y = top + r as f64 * row_h;
        let colour = style.fill.clone().unwrap_or_else(|| {
            let slot = seen.iter().position(|s| Some(*s) == *section).unwrap_or(0);
            palette[slot.min(palette.len() - 1)].to_string()
        });

        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x: x_of(*start),
                y: y + row_h * 0.18,
                w: (length * day).max(2.0),
                h: row_h * 0.64,
            },
            style: Style {
                fill: Some(colour),
                stroke: Some("none".into()),
                stroke_width: Some(0.0),
                radius: Some(3.0),
                shape: Some(Shape::Rect),
                ..style.clone()
            },
        });

        annotations.push(Annotation {
            text: name.clone(),
            x: names_w - 12.0,
            y: y + row_h / 2.0,
            anchor: Anchor::End,
            size: name_size,
            color: ink(style, &theme),
            weight: style.font_weight.clone().unwrap_or_else(|| "400".into()),
            family: family(style),
        });
    }

    // A section frame spans the rows its tasks occupy.
    for name in &seen {
        let rows: Vec<usize> = ordered
            .iter()
            .enumerate()
            .filter(|(_, (s, _))| *s == Some(*name))
            .map(|(r, _)| r)
            .collect();
        let (Some(&lo), Some(&hi)) = (rows.first(), rows.last()) else {
            continue;
        };
        let group = diagram.groups.iter().find(|g| g.id.as_str() == *name);
        let style = registry.resolve(
            Style::group_defaults(),
            "group",
            group.map(|g| g.classes.as_slice()).unwrap_or(&[]),
            group.map(|g| g.props.as_slice()).unwrap_or(&[]),
        )?;
        let label = group
            .and_then(|g| g.label.clone())
            .unwrap_or_else(|| (*name).to_string());
        let band = Rect {
            x: section_w,
            y: top + lo as f64 * row_h,
            w: names_w - section_w + width,
            h: (hi - lo + 1) as f64 * row_h,
        };

        // The label is an annotation of its own, in the section gutter,
        // centred on the rows it covers.
        annotations.push(Annotation {
            text: label,
            x: 0.0,
            y: band.cy(),
            anchor: Anchor::Start,
            size: 11.0,
            color: style
                .font_color
                .clone()
                .unwrap_or_else(|| theme.muted.clone()),
            weight: "600".into(),
            family: family(&base),
        });

        groups.push(PlacedGroup {
            lines: Vec::new(),
            rect: band,
            style,
        });
    }

    if let Some(today) = today {
        guides.push(Guide {
            from: (x_of(today), top - 4.0),
            to: (x_of(today), top + ordered.len() as f64 * row_h),
            color: "#f43f5e".into(),
            width: 1.5,
            dash: Some("5 4".into()),
        });
    }

    Ok(Layout {
        guides,
        polylines: Vec::new(),
        groups,
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Gantt,
        annotations,
    })
}

/// Marks the calendar often enough to read and rarely enough to breathe.
/// A calendar step coarse enough that the labels fit.
///
/// Span alone is not enough: ten years at a quarterly step is forty labels,
/// and forty labels in 800px is a smear. `most` is how many the axis has room
/// for, so the step is chosen against the page rather than against the
/// calendar alone.
fn pick_tick(span_days: f64, most: usize) -> i64 {
    const STEPS: [i64; 8] = [1, 7, 28, 91, 364, 728, 1820, 3640];
    let most = most.max(2) as f64;
    for step in STEPS {
        if span_days / step as f64 <= most {
            return step;
        }
    }
    STEPS[STEPS.len() - 1]
}

// ---------------------------------------------------------------------------
// timeline — events on a date line
// ---------------------------------------------------------------------------

/// Events on a horizontal date line, labels staggered above and below.
///
/// The stagger is the whole point. Dates cluster — three things happen in one
/// week and nothing for two months — so a timeline that puts every label on
/// one side at one height is unreadable exactly where it matters most.
#[cfg(feature = "timeline")]
pub fn timeline(diagram: &Diagram, opts: GraphOpts, registry: &Registry) -> Result<Layout> {
    use crate::ast::civil_from_days;

    let cfg = &diagram.archetype_props;
    let theme = Theme::of(&opts);
    let def = crate::check::archetype_of(diagram);

    let width = setting(cfg, "size")
        .and_then(|v| v.first())
        .and_then(|v| v.as_num())
        .unwrap_or(720.0)
        .max(160.0);
    let row_h = setting(cfg, "size")
        .and_then(|v| v.get(1))
        .and_then(|v| v.as_num())
        .unwrap_or(34.0)
        .max(14.0);
    // `graph { title }` is the fallback: every genre can be titled there, and
    // a chart that names itself in its own settings still wins (I-053).
    let title = text(cfg, "title").or_else(|| opts.title.clone());

    let base = registry.resolve(chart_defaults(), def.item, &[], &[])?;
    let accent = theme.palette()[0];
    let font = base.font_size.unwrap_or(13.0);

    let mut events = Vec::new();
    for node in &diagram.nodes {
        let style = registry.resolve(
            chart_defaults(),
            def.item,
            &node.classes,
            &node.props_for(def),
        )?;
        let Some(day) = style.start else { continue };
        let name = node.label.clone().unwrap_or_else(|| node.id.value.clone());
        events.push((name, day, style));
    }
    // Date order, and source order for two things on the same day — which is
    // the only tie-break a reader can predict.
    events.sort_by_key(|e| e.1);

    let (first, last) = events.iter().fold((i64::MAX, i64::MIN), |(lo, hi), e| {
        (lo.min(e.1), hi.max(e.1))
    });
    let (first, last) = if events.is_empty() {
        (0, 1)
    } else {
        (first, last.max(first + 1))
    };
    let span = (last - first) as f64;

    // Room at each end so the first and last labels are not half off the page.
    let margin = width * 0.08;
    let plot = width - margin * 2.0;
    let x_of = |d: i64| margin + (d - first) as f64 * plot / span;

    // Pack labels into rows, per side, so two never overlap: an event takes
    // the shallowest row on its side whose last label ended before this one
    // starts.
    let label_w = |name: &str| text_width(std::slice::from_ref(&name.to_string()), font) + 14.0;
    let mut ends: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
    let mut placed: Vec<(usize, usize)> = Vec::new(); // (side, row)
    for (i, (name, day, _)) in events.iter().enumerate() {
        let side = i % 2;
        let half = label_w(name) / 2.0;
        let (left, right) = (x_of(*day) - half, x_of(*day) + half);
        let row = match ends[side].iter().position(|end| *end + 10.0 < left) {
            Some(row) => row,
            None => {
                ends[side].push(0.0);
                ends[side].len() - 1
            }
        };
        ends[side][row] = right;
        placed.push((side, row));
    }

    let title_h = if title.is_some() {
        // Two and a half lines, not two: at two the heading sat on the top
        // gridline and read as part of the chart rather than above it.
        TITLE_SIZE * 2.5
    } else {
        0.0
    };
    const STEM: f64 = 14.0;
    const AXIS_H: f64 = 22.0;
    // A label is two lines — the name, then the date under it — so the space
    // it needs is not one row. Forgetting the second line is what ran the stem
    // through the date and put the axis strip on top of the deepest row.
    let cap = font * 0.8;
    let tail = font * 1.6;
    let above = ends[0].len() as f64 * row_h + cap;
    let axis_y = title_h + above + STEM;
    let below = ends[1].len() as f64 * row_h + tail + cap;
    let height = axis_y + STEM + below + AXIS_H;

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

    // Gridlines carry the axis labels down to the strip at the foot, so a date
    // is read off the same vertical the event sits on.
    let rule = if theme.dark { "#1e293b" } else { "#e4e4e7" };
    let step = match text(cfg, "ticks").as_deref() {
        Some("day") => 1,
        Some("week") => 7,
        Some("month") => 28,
        Some("year") => 364,
        // A centred `2015-05` is about 52px wide; leave it a gap.
        _ => pick_tick(span, (plot / 66.0) as usize),
    };
    let mut d = first;
    while d <= last {
        let x = x_of(d);
        guides.push(Guide {
            from: (x, title_h),
            to: (x, height - AXIS_H),
            color: rule.into(),
            width: 1.0,
            dash: None,
        });
        let (y, m, dd) = civil_from_days(d);
        annotations.push(Annotation {
            text: match step {
                s if s >= 364 => format!("{y}"),
                s if s >= 28 => format!("{y}-{m:02}"),
                _ => format!("{m:02}-{dd:02}"),
            },
            x,
            y: height - AXIS_H / 2.0,
            anchor: Anchor::Middle,
            size: 10.0,
            color: theme.muted.clone(),
            weight: "400".into(),
            family: family(&base),
        });
        d += step;
    }

    // The date line itself, over the gridlines.
    guides.push(Guide {
        from: (0.0, axis_y),
        to: (width, axis_y),
        color: ink(&base, &theme),
        width: 1.4,
        dash: None,
    });

    for ((name, day, style), (side, row)) in events.iter().zip(&placed) {
        let x = x_of(*day);
        let up = *side == 0;
        let depth = STEM + (*row as f64 + 1.0) * row_h;
        let tip = if up { axis_y - depth } else { axis_y + depth };

        // Stops at the edge of the label block, not inside it.
        guides.push(Guide {
            from: (x, axis_y),
            to: (x, if up { tip + tail } else { tip - cap }),
            color: style.stroke.clone().unwrap_or_else(|| accent.into()),
            width: 1.2,
            dash: None,
        });

        // A mark, not a box: it carries no label of its own.
        let r = 5.0;
        nodes.push(PlacedNode {
            lines: Vec::new(),
            rect: Rect {
                x: x - r,
                y: axis_y - r,
                w: r * 2.0,
                h: r * 2.0,
            },
            style: Style {
                fill: Some(style.fill.clone().unwrap_or_else(|| accent.into())),
                stroke: Some(theme.surface.clone()),
                stroke_width: Some(2.0),
                shape: Some(Shape::Ellipse),
                ..Style::default()
            },
        });

        annotations.push(Annotation {
            text: name.clone(),
            x,
            y: tip,
            anchor: Anchor::Middle,
            size: font,
            color: ink(style, &theme),
            weight: "600".into(),
            family: family(&base),
        });
        let (y, m, dd) = civil_from_days(*day);
        annotations.push(Annotation {
            text: format!("{y}-{m:02}-{dd:02}"),
            x,
            y: tip + font * 1.15,
            anchor: Anchor::Middle,
            size: font * 0.8,
            color: theme.muted.clone(),
            weight: "400".into(),
            family: family(&base),
        });
    }

    Ok(Layout {
        guides,
        polylines: Vec::new(),
        groups: Vec::new(),
        nodes,
        edges: Vec::new(),
        opts,
        chart: Chart::Timeline,
        annotations,
    })
}
