//! PDF output, written by hand.
//!
//! **A PDF is a vector format, so this needs no rasteriser** — which is the
//! whole reason it ships and PNG does not (`D-027`). The content stream is a
//! few operators: `re` for a rectangle, `m`/`l`/`c` for a path, `Tj` for text.
//! That is the same size of job as the SVG writer and the watch server, and it
//! keeps the dependency list at five.
//!
//! One thing falls out of it that is better than SVG can manage: a PDF viewer
//! is required by the specification to have Helvetica, and the advance table
//! `W-040` added *is* Helvetica's. Text width here is not an estimate — it is
//! exact, and `X-005` does not apply to this output at all.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::layout::{Anchor, Chart, Layout, Rect};
use crate::style::{Head, Shape};

/// A PDF page of the whole diagram, as bytes ready to write to a file.
/// A page size in points, and how much neighbouring sheets share.
///
/// Points because that is what a PDF measures in and what the layout already
/// produces — asking for millimetres would mean a conversion this crate does
/// not otherwise need.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    pub width: f64,
    pub height: f64,
    /// Repeated along each seam, so a box cut in half is readable on both
    /// sheets and a reader can see how two pages join.
    pub overlap: f64,
}

impl Page {
    /// A4 at 72 points to the inch, which is what a PDF's user space is.
    pub const A4: Page = Page {
        width: 595.28,
        height: 841.89,
        overlap: 24.0,
    };

    /// A named size, or `WIDTHxHEIGHT` in points.
    pub fn parse(text: &str) -> Option<Page> {
        match text.to_ascii_lowercase().as_str() {
            "a4" => Some(Page::A4),
            "a3" => Some(Page {
                width: 841.89,
                height: 1190.55,
                ..Page::A4
            }),
            "letter" => Some(Page {
                width: 612.0,
                height: 792.0,
                ..Page::A4
            }),
            other => {
                let (w, h) = other.split_once('x')?;
                Some(Page {
                    width: w.trim().parse().ok()?,
                    height: h.trim().parse().ok()?,
                    ..Page::A4
                })
            }
        }
    }
}

pub fn render(layout: &Layout) -> Vec<u8> {
    render_pages(layout, None)
}

/// The same diagram across as many sheets as it takes.
///
/// One content stream, several pages. A PDF page's `MediaBox` is a rectangle
/// in user space, so a *window* onto the drawing is all a sheet is — there is
/// nothing to re-render, nothing to clip, and no second code path that could
/// disagree with the single-page one.
///
/// Only the PDF gets this. An SVG has no pages, and pretending otherwise would
/// be inventing a concept the format does not have.
pub fn render_paged(layout: &Layout, page: Page) -> Vec<u8> {
    render_pages(layout, Some(page))
}

fn render_pages(layout: &Layout, page: Option<Page>) -> Vec<u8> {
    let (dx, dy, width, height) = crate::render_svg::canvas(layout);

    let mut c = Content::new(height);

    if let Some(background) = &layout.opts.background
        && background != "none"
    {
        c.fill_colour(background);
        let _ = writeln!(c.body, "0 0 {} {} re f", num(width), num(height));
    }

    for guide in &layout.guides {
        c.stroke_colour(guide.color.as_str());
        c.width(guide.width);
        c.dash(guide.dash.as_deref(), guide.width);
        c.line(shift(guide.from, dx, dy), shift(guide.to, dx, dy));
    }

    for shape in layout.polylines.iter().filter(|p| !p.over) {
        let points: Vec<(f64, f64)> = shape.points.iter().map(|p| shift(*p, dx, dy)).collect();
        c.path(&points, shape.closed);
        c.paint(shape.fill.as_deref(), shape.stroke.as_deref(), shape.width);
    }

    for group in &layout.groups {
        c.box_of(&offset(group.rect, dx, dy), Shape::Rect, &group.style);
        c.text_lines(&group.lines, &offset(group.rect, dx, dy), &group.style);
    }

    // Slices need arcs, which are the one thing a rectangle cannot fake.
    if let Chart::Pie(pie) = &layout.chart {
        for slice in &pie.slices {
            c.fill_colour(&slice.fill);
            c.slice(
                shift((pie.cx, pie.cy), dx, dy),
                pie.radius,
                pie.inner,
                slice.start,
                slice.end,
            );
        }
    }

    // The *routed* edges, not two centres joined: routing, endpoint clipping
    // and label placement are what make a line an edge, and the SVG writer
    // already computes all three.
    let routed = crate::render_svg::route_all(layout);
    for (edge, route) in layout.edges.iter().zip(&routed) {
        if route.points.len() < 2 {
            continue;
        }
        let points: Vec<(f64, f64)> = route.points.iter().map(|p| shift(*p, dx, dy)).collect();
        let stroke = edge
            .style
            .stroke
            .clone()
            .unwrap_or_else(|| "#64748b".into());
        let width = edge.style.stroke_width.unwrap_or(1.5);

        c.stroke_colour(&stroke);
        c.width(width);
        c.dash(edge.style.dash.as_deref(), width);
        c.path(&points, false);
        let _ = writeln!(c.body, "S");
        c.dash(None, width);

        // Each end is decided on its own, exactly as in the SVG: a declared
        // cardinality replaces the arrowhead there.
        let head = edge.style.head.unwrap_or(Head::Solid);
        let end = edge
            .style
            .to_head
            .or_else(|| edge.arrow.head_end.then_some(head));
        let start = edge
            .style
            .from_head
            .or_else(|| edge.arrow.head_start.then_some(head));

        if let Some(h) = end.filter(|h| *h != Head::None) {
            let n = points.len();
            c.head(h, points[n - 2], points[n - 1], &stroke, width);
        }
        if let Some(h) = start.filter(|h| *h != Head::None) {
            c.head(h, points[1], points[0], &stroke, width);
        }
    }

    // Labels after the lines, so no edge is drawn over a plate.
    for (edge, route) in layout.edges.iter().zip(&routed) {
        let Some((text, rect)) = &route.label else {
            continue;
        };
        let plate = offset(*rect, dx, dy);
        // The plate is the background showing through, which is what keeps a
        // label readable where it crosses a line.
        if let Some(background) = &layout.opts.background
            && background != "none"
        {
            c.fill_colour(background);
            let _ = writeln!(
                c.body,
                "{} {} {} {} re f",
                num(plate.x),
                num(c.y(plate.bottom())),
                num(plate.w),
                num(plate.h)
            );
        }
        let size = edge.style.font_size.unwrap_or(11.0);
        c.fill_colour(edge.style.font_color.as_deref().unwrap_or("#475569"));
        for (i, line) in text.split('\n').enumerate() {
            c.show(
                line,
                (plate.cx(), plate.y + size * (0.95 + i as f64 * 1.25)),
                size,
                Anchor::Middle,
                "400",
            );
        }
    }

    for node in &layout.nodes {
        let rect = offset(node.rect, dx, dy);
        c.box_of(&rect, node.style.shape.unwrap_or(Shape::Rect), &node.style);
        c.text_lines(&node.lines, &rect, &node.style);
    }

    // The marks that live inside a shape, after it — the same second pass the
    // SVG writer makes, in the same place, so the two cannot disagree.
    for shape in layout.polylines.iter().filter(|p| p.over) {
        let points: Vec<(f64, f64)> = shape.points.iter().map(|p| shift(*p, dx, dy)).collect();
        c.path(&points, shape.closed);
        c.paint(shape.fill.as_deref(), shape.stroke.as_deref(), shape.width);
    }

    for a in &layout.annotations {
        c.stroke_colour(&a.color);
        c.fill_colour(&a.color);
        c.show(
            &a.text,
            shift((a.x, a.y), dx, dy),
            a.size,
            a.anchor,
            &a.weight,
        );
    }

    assemble(&c.body, &c.alphas, width, height, page)
}

fn shift((x, y): (f64, f64), dx: f64, dy: f64) -> (f64, f64) {
    (x + dx, y + dy)
}

fn offset(r: Rect, dx: f64, dy: f64) -> Rect {
    Rect {
        x: r.x + dx,
        y: r.y + dy,
        ..r
    }
}

/// PDF numbers: no exponent form, and a short fixed precision keeps the file
/// readable when something goes wrong.
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// The content stream, plus the alpha states it needed along the way.
struct Content {
    body: String,
    /// PDF puts transparency in a named graphics state rather than on the
    /// operator, so each distinct alpha is collected and named once.
    alphas: BTreeMap<String, f64>,
    height: f64,
}

impl Content {
    fn new(height: f64) -> Content {
        Content {
            body: String::new(),
            alphas: BTreeMap::new(),
            height,
        }
    }

    /// PDF's y axis grows upward and the layout's grows downward.
    fn y(&self, y: f64) -> f64 {
        self.height - y
    }

    fn rgb(colour: &str) -> Option<(f64, f64, f64, Option<f64>)> {
        let (opaque, alpha) = crate::render_svg::split_alpha(colour);
        let hex = opaque.strip_prefix('#')?;
        if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let part = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok().map(f64::from);
        Some((part(0)? / 255.0, part(2)? / 255.0, part(4)? / 255.0, alpha))
    }

    fn alpha(&mut self, value: Option<f64>, stroking: bool) {
        let a = value.unwrap_or(1.0);
        let name = format!(
            "GS{}{}",
            if stroking { "S" } else { "F" },
            (a * 1000.0) as u32
        );
        self.alphas.insert(name.clone(), a);
        let _ = writeln!(self.body, "/{name} gs");
    }

    fn fill_colour(&mut self, colour: &str) {
        if let Some((r, g, b, a)) = Content::rgb(colour) {
            self.alpha(a, false);
            let _ = writeln!(self.body, "{} {} {} rg", num(r), num(g), num(b));
        }
    }

    fn stroke_colour(&mut self, colour: &str) {
        if let Some((r, g, b, a)) = Content::rgb(colour) {
            self.alpha(a, true);
            let _ = writeln!(self.body, "{} {} {} RG", num(r), num(g), num(b));
        }
    }

    fn width(&mut self, w: f64) {
        let _ = writeln!(self.body, "{} w", num(w.max(0.1)));
    }

    fn dash(&mut self, pattern: Option<&str>, width: f64) {
        let w = width.max(0.5);
        let d = match pattern {
            Some("dashed") => format!("[{} {}]", num(w * 4.0), num(w * 3.0)),
            Some("dotted") => format!("[{} {}]", num(w), num(w * 2.5)),
            Some(explicit) if explicit.contains(' ') => format!("[{explicit}]"),
            _ => "[]".to_string(),
        };
        let _ = writeln!(self.body, "{d} 0 d");
    }

    fn line(&mut self, from: (f64, f64), to: (f64, f64)) {
        let _ = writeln!(
            self.body,
            "{} {} m {} {} l S",
            num(from.0),
            num(self.y(from.1)),
            num(to.0),
            num(self.y(to.1))
        );
    }

    fn path(&mut self, points: &[(f64, f64)], closed: bool) {
        let Some(first) = points.first() else { return };
        let _ = writeln!(self.body, "{} {} m", num(first.0), num(self.y(first.1)));
        for p in &points[1..] {
            let _ = writeln!(self.body, "{} {} l", num(p.0), num(self.y(p.1)));
        }
        if closed {
            let _ = writeln!(self.body, "h");
        }
    }

    /// `f`, `S` or `B`, depending on what was asked for — the operator carries
    /// the decision, so a shape with neither fill nor stroke draws nothing
    /// rather than a default black.
    fn paint(&mut self, fill: Option<&str>, stroke: Option<&str>, width: f64) {
        let has_fill = fill.is_some_and(|f| f != "none" && Content::rgb(f).is_some());
        let has_stroke = stroke.is_some_and(|s| s != "none" && Content::rgb(s).is_some());

        if has_fill {
            self.fill_colour(fill.unwrap_or_default());
        }
        if has_stroke {
            self.stroke_colour(stroke.unwrap_or_default());
            self.width(width);
        }
        let op = match (has_fill, has_stroke) {
            (true, true) => "B",
            (true, false) => "f",
            (false, true) => "S",
            (false, false) => "n",
        };
        let _ = writeln!(self.body, "{op}");
    }

    /// Four beziers make a circle to within a thousandth of a radius, which is
    /// well past what a page can show.
    fn ellipse(&mut self, r: &Rect) {
        const K: f64 = 0.552_284_75;
        let (cx, cy) = (r.cx(), self.y(r.cy()));
        let (a, b) = (r.w / 2.0, r.h / 2.0);
        let (kx, ky) = (a * K, b * K);
        let _ = writeln!(self.body, "{} {} m", num(cx + a), num(cy));
        for (c1, c2, end) in [
            ((cx + a, cy + ky), (cx + kx, cy + b), (cx, cy + b)),
            ((cx - kx, cy + b), (cx - a, cy + ky), (cx - a, cy)),
            ((cx - a, cy - ky), (cx - kx, cy - b), (cx, cy - b)),
            ((cx + kx, cy - b), (cx + a, cy - ky), (cx + a, cy)),
        ] {
            let _ = writeln!(
                self.body,
                "{} {} {} {} {} {} c",
                num(c1.0),
                num(c1.1),
                num(c2.0),
                num(c2.1),
                num(end.0),
                num(end.1)
            );
        }
        let _ = writeln!(self.body, "h");
    }

    fn box_of(&mut self, r: &Rect, shape: Shape, style: &crate::style::Style) {
        match shape {
            Shape::Ellipse => self.ellipse(r),
            Shape::Diamond => {
                let pts = [
                    (r.cx(), r.y),
                    (r.right(), r.cy()),
                    (r.cx(), r.bottom()),
                    (r.x, r.cy()),
                ];
                self.path(&pts, true);
            }
            Shape::Hexagon => {
                let inset = (r.w * 0.18).min(r.h / 2.0);
                let pts = [
                    (r.x + inset, r.y),
                    (r.right() - inset, r.y),
                    (r.right(), r.cy()),
                    (r.right() - inset, r.bottom()),
                    (r.x + inset, r.bottom()),
                    (r.x, r.cy()),
                ];
                self.path(&pts, true);
            }
            _ => {
                let _ = writeln!(
                    self.body,
                    "{} {} {} {} re",
                    num(r.x),
                    num(self.y(r.bottom())),
                    num(r.w),
                    num(r.h)
                );
            }
        }
        self.paint(
            style.fill.as_deref(),
            style.stroke.as_deref(),
            style.stroke_width.unwrap_or(1.5),
        );
    }

    /// A pie slice: two radii and an arc, the arc in bezier segments no longer
    /// than a quarter turn so the approximation stays inside a hairline.
    fn slice(&mut self, centre: (f64, f64), radius: f64, inner: f64, start: f64, end: f64) {
        let (cx, cy) = (centre.0, self.y(centre.1));
        // Degrees clockwise from twelve o'clock, into PDF's counter-clockwise
        // radians from three o'clock.
        let at = |deg: f64, r: f64| {
            let a = (90.0 - deg).to_radians();
            (cx + r * crate::math::cos(a), cy + r * crate::math::sin(a))
        };
        let steps = (((end - start).abs() / 90.0).ceil() as usize).max(1);
        let step = (end - start) / steps as f64;

        let first = at(start, radius);
        let _ = writeln!(self.body, "{} {} m", num(first.0), num(first.1));
        for i in 0..steps {
            self.arc(
                cx,
                cy,
                radius,
                start + step * i as f64,
                start + step * (i + 1) as f64,
            );
        }
        if inner > 0.0 {
            let back = at(end, inner);
            let _ = writeln!(self.body, "{} {} l", num(back.0), num(back.1));
            for i in (0..steps).rev() {
                self.arc(
                    cx,
                    cy,
                    inner,
                    start + step * (i + 1) as f64,
                    start + step * i as f64,
                );
            }
        } else {
            let _ = writeln!(self.body, "{} {} l", num(cx), num(cy));
        }
        let _ = writeln!(self.body, "h f");
    }

    fn arc(&mut self, cx: f64, cy: f64, r: f64, from: f64, to: f64) {
        let (a0, a1) = ((90.0 - from).to_radians(), (90.0 - to).to_radians());
        let k = 4.0 / 3.0 * crate::math::tan((a1 - a0) / 4.0);
        let (p0, p1) = (
            (cx + r * crate::math::cos(a0), cy + r * crate::math::sin(a0)),
            (cx + r * crate::math::cos(a1), cy + r * crate::math::sin(a1)),
        );
        let c1 = (
            p0.0 - k * r * crate::math::sin(a0),
            p0.1 + k * r * crate::math::cos(a0),
        );
        let c2 = (
            p1.0 + k * r * crate::math::sin(a1),
            p1.1 - k * r * crate::math::cos(a1),
        );
        let _ = writeln!(
            self.body,
            "{} {} {} {} {} {} c",
            num(c1.0),
            num(c1.1),
            num(c2.0),
            num(c2.1),
            num(p1.0),
            num(p1.1)
        );
    }

    /// A mark at the end of a line. The arrow forms are drawn; a crow's-foot
    /// cardinality is drawn as the three strokes it is, because a filled
    /// triangle there would read as direction rather than as *many*.
    fn head(&mut self, head: Head, from: (f64, f64), to: (f64, f64), colour: &str, width: f64) {
        let (dxa, dya) = (to.0 - from.0, to.1 - from.1);
        let len = (dxa * dxa + dya * dya).sqrt();
        if len < 0.001 {
            return;
        }
        let (ux, uy) = (dxa / len, dya / len);
        let (px, py) = (-uy, ux);
        let size = (width * 4.0).max(7.0);
        // Back along the line from the tip, and out to each side.
        let at =
            |back: f64, side: f64| (to.0 - ux * back + px * side, to.1 - uy * back + py * side);

        self.stroke_colour(colour);
        self.fill_colour(colour);
        self.width(width);

        match head {
            Head::Solid => {
                self.path(&[to, at(size, size * 0.42), at(size, -size * 0.42)], true);
                let _ = writeln!(self.body, "f");
            }
            Head::Open => {
                self.path(&[at(size, size * 0.42), to, at(size, -size * 0.42)], false);
                let _ = writeln!(self.body, "S");
            }
            Head::Dot => {
                let r = size * 0.32;
                let centre = at(r, 0.0);
                self.ellipse(&Rect {
                    x: centre.0 - r,
                    y: centre.1 - r,
                    w: r * 2.0,
                    h: r * 2.0,
                });
                let _ = writeln!(self.body, "f");
            }
            // UML, the same three shapes the SVG draws. A hollow one is
            // stroked and not filled here rather than painted with the
            // surface: a PDF has no transparency to see through in the first
            // place, and `B` would paint over the line it sits on anyway.
            Head::Hollow => {
                self.path(&[to, at(size, size * 0.5), at(size, -size * 0.5)], true);
                let _ = writeln!(self.body, "S");
            }
            Head::Diamond | Head::Composed => {
                let long = size * 1.4;
                self.path(
                    &[
                        to,
                        at(long * 0.5, size * 0.42),
                        at(long, 0.0),
                        at(long * 0.5, -size * 0.42),
                    ],
                    true,
                );
                let _ = writeln!(
                    self.body,
                    "{}",
                    if head == Head::Composed { "f" } else { "S" }
                );
            }
            // Cardinality: a bar, a foot, or a circle, in the combinations the
            // crow's-foot notation gives them.
            Head::One | Head::ZeroOne => {
                self.path(&[at(size, size * 0.5), at(size, -size * 0.5)], false);
                let _ = writeln!(self.body, "S");
                if head == Head::ZeroOne {
                    let r = size * 0.28;
                    let centre = at(size * 1.9, 0.0);
                    self.ellipse(&Rect {
                        x: centre.0 - r,
                        y: centre.1 - r,
                        w: r * 2.0,
                        h: r * 2.0,
                    });
                    let _ = writeln!(self.body, "S");
                }
            }
            Head::Many | Head::ZeroMany | Head::OneMany => {
                for side in [size * 0.5, 0.0, -size * 0.5] {
                    self.path(&[to, at(size, side)], false);
                    let _ = writeln!(self.body, "S");
                }
                if head == Head::ZeroMany {
                    let r = size * 0.28;
                    let centre = at(size * 1.9, 0.0);
                    self.ellipse(&Rect {
                        x: centre.0 - r,
                        y: centre.1 - r,
                        w: r * 2.0,
                        h: r * 2.0,
                    });
                    let _ = writeln!(self.body, "S");
                } else if head == Head::OneMany {
                    self.path(
                        &[at(size * 1.7, size * 0.5), at(size * 1.7, -size * 0.5)],
                        false,
                    );
                    let _ = writeln!(self.body, "S");
                }
            }
            Head::None => {}
        }
    }

    fn text_lines(&mut self, lines: &[String], r: &Rect, style: &crate::style::Style) {
        if lines.is_empty() {
            return;
        }
        let size = style.font_size.unwrap_or(14.0);
        let weight = style.font_weight.clone().unwrap_or_else(|| "500".into());
        let colour = style.font_color.clone().unwrap_or_else(|| "#111827".into());
        self.fill_colour(&colour);

        let step = size * 1.35;
        // The same sink the SVG writer applies, from the same function: a
        // cylinder's cap eats only from the top, so centring on the box puts
        // the text over the ellipse.
        let sink = if style.shape == Some(Shape::Cylinder) {
            crate::render_svg::cylinder_cap(r) / 2.0
        } else {
            0.0
        };
        let top = r.cy() + sink - (lines.len() as f64 - 1.0) * step / 2.0 + size * 0.34;
        let (dx, dy) = style.label_offset.unwrap_or((0.0, 0.0));
        for (i, line) in lines.iter().enumerate() {
            self.show(
                line,
                (r.cx() + dx, top + i as f64 * step + dy),
                size,
                Anchor::Middle,
                &weight,
            );
        }
    }

    fn show(&mut self, text: &str, at: (f64, f64), size: f64, anchor: Anchor, weight: &str) {
        if text.is_empty() {
            return;
        }
        // Helvetica is one of the fourteen a viewer must have, so the advance
        // table `W-040` added is exact here rather than an estimate.
        let bold = weight == "bold" || weight.parse::<u32>().is_ok_and(|w| w >= 600);
        let font = if bold { "/F2" } else { "/F1" };
        let width = crate::layout::advance(text, size);
        let x = match anchor {
            Anchor::Start => at.0,
            Anchor::Middle => at.0 - width / 2.0,
            Anchor::End => at.0 - width,
        };
        let _ = writeln!(
            self.body,
            "BT {font} {} Tf {} {} Td ({}) Tj ET",
            num(size),
            num(x),
            num(self.y(at.1)),
            escape(text)
        );
    }
}

/// PDF strings are parenthesised, so the delimiters and the escape character
/// have to be escaped. Anything outside Latin-1 has no place in a base-14 font
/// and is dropped rather than emitted as mojibake.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x100 && !c.is_control() => out.push(c),
            _ => out.push('?'),
        }
    }
    out
}

/// The file around the content stream: the object table, the page tree, the
/// two fonts and the alpha states, then the cross-reference table.
fn assemble(
    body: &str,
    alphas: &BTreeMap<String, f64>,
    width: f64,
    height: f64,
    page: Option<Page>,
) -> Vec<u8> {
    let mut states = String::new();
    for (name, alpha) in alphas {
        let stroking = name.starts_with("GSS");
        let key = if stroking { "CA" } else { "ca" };
        let _ = write!(
            states,
            "/{name} << /Type /ExtGState /{key} {} >> ",
            num(*alpha)
        );
    }

    // Each sheet is a window onto the one drawing: the tiles below are
    // `MediaBox` rectangles, not re-renders. Without a page size there is one
    // sheet and it is the whole canvas, exactly as before.
    let boxes: Vec<(f64, f64, f64, f64)> = match page {
        None => vec![(0.0, 0.0, width, height)],
        Some(p) => {
            let step_x = (p.width - p.overlap).max(1.0);
            let step_y = (p.height - p.overlap).max(1.0);
            let cols = ((width / step_x).ceil() as usize).max(1);
            let rows = ((height / step_y).ceil() as usize).max(1);
            let mut out = Vec::with_capacity(cols * rows);
            // Row by row from the top, because that is the order a reader
            // lays the sheets out — the PDF's own y axis runs the other way,
            // which is why the top edge is `height - …` here.
            for row in 0..rows {
                for col in 0..cols {
                    let x0 = col as f64 * step_x;
                    let top = row as f64 * step_y;
                    out.push((x0, height - top - p.height, x0 + p.width, height - top));
                }
            }
            out
        }
    };

    // Object numbering: 1 catalog, 2 pages, then one object per page, then
    // the shared content stream and the two fonts.
    let content_id = 3 + boxes.len();
    let kids: Vec<String> = (0..boxes.len()).map(|i| format!("{} 0 R", 3 + i)).collect();

    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            boxes.len()
        ),
    ];
    for (x0, y0, x1, y1) in &boxes {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [{} {} {} {}] /Contents {content_id} 0 R \
             /Resources << /Font << /F1 {} 0 R /F2 {} 0 R >> /ExtGState << {states} >> >> >>",
            num(*x0),
            num(*y0),
            num(*x1),
            num(*y1),
            content_id + 1,
            content_id + 2
        ));
    }
    objects.push(format!(
        "<< /Length {} >>\nstream\n{body}endstream",
        body.len()
    ));
    objects.push(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
            .to_string(),
    );
    objects.push(
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
            .to_string(),
    );

    let mut out = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, object) in objects.iter().enumerate() {
        offsets.push(out.len());
        let _ = write!(out, "{} 0 obj\n{object}\nendobj\n", i + 1);
    }

    let xref = out.len();
    let _ = writeln!(out, "xref\n0 {}\n0000000000 65535 f ", objects.len() + 1);
    for offset in &offsets {
        let _ = writeln!(out, "{offset:010} 00000 n ");
    }
    let _ = write!(
        out,
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    );

    out.into_bytes()
}
