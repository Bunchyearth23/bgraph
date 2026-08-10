//! A native window, for `bgraph watch`.
//!
//! The same viewer as the browser page and the same controls — scroll to zoom
//! by 5% a notch, or 10% holding shift,
//! drag to pan, `f` to fit, `0` for actual size, `r` to reload, and the file
//! reloaded when it changes on disk — with no server, no port and no browser.
//!
//! **This is the one place the project takes a dependency tree**, which is why
//! it is behind an off-by-default feature. Everything else here is hand-rolled
//! to keep the crate at five dependencies.
//!
//! It is `minifb` rather than `winit` + `softbuffer` because the *windowing*
//! is the expensive half — 78 crates in the build graph against 219 — and
//! `minifb` is a window
//! and a pixel buffer in one. It is not a webview because a webview would mean
//! WebKitGTK installed on whoever runs the binary, to show a page this already
//! knows how to draw.
//!
//! Without the feature, `watch` serves the HTTP preview instead —
//! which is not a lesser thing, it is what a remote machine and a second
//! screen both want.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use minifb::{Key, KeyRepeat, MouseButton, MouseMode, Window, WindowOptions};

/// The ground the diagram is drawn on, when it does not paint its own.
///
/// The page paints a surface behind the SVG for the same reason (`D-009`): a
/// transparent export over an undefined background is the one thing the
/// preview promised not to do.
const GROUND: (u8, u8, u8) = (0x1c, 0x1f, 0x26);

/// Open the file in a window and block until it is closed.
///
/// **Meant as the last thing a process does.** It deliberately does not tear
/// the window down — see the note where it returns — so a caller that carries
/// on afterwards would hold the window's resources until it exits.
pub fn show(input: PathBuf, data: Option<PathBuf>) -> Result<()> {
    run(Viewer::new(input, data))
}

/// A window on whichever `.bd` in `dir` was edited last.
///
/// Opt-in from the command line, and it has to be: a window that changes
/// subject on its own is one you stop trusting. Asked for outright — while
/// editing six diagrams in turn — it is exactly right, and the alternative is
/// six windows.
pub fn follow(dir: PathBuf) -> Result<()> {
    let Some(first) = crate::watch::newest(&dir) else {
        anyhow::bail!("no `.bd` file in {}", dir.display());
    };
    let mut viewer = Viewer::new(first, None);
    viewer.follow = Some(dir);
    run(viewer)
}

fn run(mut viewer: Viewer) -> Result<()> {
    let mut window = Window::new(
        "bgraph",
        960,
        720,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )
    .context("cannot open a window on this display")?;
    window.set_target_fps(60);

    viewer.reload();
    viewer.size = window.get_size();
    viewer.fit();

    let mut buffer: Vec<u32> = Vec::new();
    let mut dragging: Option<(f32, f32)> = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        let size = window.get_size();
        if size != viewer.size {
            viewer.size = size;
            // A window resized before the picture was ever placed refits; one
            // resized after keeps the view the reader chose.
            if !viewer.placed {
                viewer.fit();
            }
        }

        if let Some((_, dy)) = window.get_scroll_wheel()
            && dy.abs() > f32::EPSILON
        {
            // Only the *sign* is read. The backends disagree about the
            // magnitude of one notch — X11 sends ±1, Wayland sends the axis in
            // fractional pixels, about ±15 — and treating that as a factor
            // made one notch a 2.8× zoom in one direction and a negative
            // scale in the other, which the clamp then showed as 5%.
            //
            // Negative is up, and up zooms in.
            let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
            let step = if shift { 1.10 } else { 1.05 };
            viewer.zoom_by(if dy < 0.0 { step } else { 1.0 / step });
        }

        match (
            window.get_mouse_down(MouseButton::Left),
            window.get_mouse_pos(MouseMode::Pass),
        ) {
            (true, Some(at)) => {
                if let Some(last) = dragging {
                    viewer.view.dx += (at.0 - last.0) as f64;
                    viewer.view.dy += (at.1 - last.1) as f64;
                }
                dragging = Some(at);
            }
            _ => dragging = None,
        }

        if window.is_key_pressed(Key::F, KeyRepeat::No) {
            viewer.fit();
        }
        if window.is_key_pressed(Key::Key0, KeyRepeat::No) {
            viewer.actual_size();
        }
        if window.is_key_pressed(Key::R, KeyRepeat::No) {
            viewer.reload();
        }

        // Poll the timestamp rather than watch the filesystem: a stat four
        // times a second costs nothing and needs no platform-specific code.
        if viewer.checked.elapsed() >= Duration::from_millis(250) {
            viewer.checked = Instant::now();
            // Following a directory, the subject may have changed as well as
            // the file. Repointing counts as opening a picture rather than
            // reloading one, so the view is refitted — keeping a zoom from the
            // previous diagram would drop the reader somewhere arbitrary in
            // the new one.
            let moved = viewer.retarget();
            if moved {
                viewer.reload();
                viewer.placed = false;
                viewer.fit();
            } else if viewer.stale() {
                // The view survives the reload, or zooming while editing would
                // be pointless — the same rule the web page follows.
                viewer.reload();
            }
        }

        viewer.paint(&mut buffer);
        window.set_title(&viewer.title());
        window
            .update_with_buffer(&buffer, viewer.size.0, viewer.size.1)
            .context("cannot present the window")?;
    }

    // Dropping the window tears down `minifb`'s Wayland queue while its own
    // proxies — the registry, the seat, the surfaces — are still attached, and
    // libwayland says so on stderr in twenty lines. It is a teardown order
    // inside the dependency, not something this code can sequence.
    //
    // So the window is not dropped. The process is exiting; the compositor
    // reclaims everything the moment the socket closes, which is the same
    // cleanup a drop would have asked for and without the complaint. This is
    // why `show` is documented as the last thing a process does.
    std::mem::forget(window);
    Ok(())
}

/// Where the picture sits in the window: a scale and an offset in pixels.
#[derive(Clone, Copy)]
struct View {
    scale: f64,
    dx: f64,
    dy: f64,
}

/// A parsed picture, or what it said when it would not parse.
enum Art {
    Tree(Box<resvg::usvg::Tree>),
    Failed(String),
}

struct Viewer {
    input: PathBuf,
    /// The directory being followed, when the window has no fixed subject.
    follow: Option<PathBuf>,
    data: Option<PathBuf>,
    art: Option<Art>,
    /// The file's modified time when it was last read.
    stamp: Option<std::time::SystemTime>,
    checked: Instant,
    view: View,
    /// Whether the reader has placed the picture themselves. Until they have,
    /// a resize refits.
    placed: bool,
    size: (usize, usize),
}

impl Viewer {
    fn new(input: PathBuf, data: Option<PathBuf>) -> Viewer {
        Viewer {
            input,
            follow: None,
            data,
            art: None,
            stamp: None,
            checked: Instant::now(),
            view: View {
                scale: 1.0,
                dx: 0.0,
                dy: 0.0,
            },
            placed: false,
            size: (960, 720),
        }
    }

    /// Read the file and parse it. A failure is kept rather than raised: a
    /// broken file mid-edit should say so, not close the window.
    fn reload(&mut self) {
        self.stamp = std::fs::metadata(&self.input)
            .and_then(|m| m.modified())
            .ok();

        let svg = match crate::watch::render_source(&self.input, self.data.as_deref()) {
            Ok(svg) => svg,
            Err(err) => {
                self.art = Some(Art::Failed(format!("{err:#}")));
                return;
            }
        };

        let mut options = resvg::usvg::Options::default();
        options.fontdb_mut().load_system_fonts();
        resolve_generic_families(&mut options);
        self.art = Some(match resvg::usvg::Tree::from_str(&svg, &options) {
            Ok(tree) => Art::Tree(Box::new(tree)),
            Err(err) => Art::Failed(format!("the SVG did not parse: {err}")),
        });
    }

    /// Point at the newest file in the followed directory, if that is a
    /// different one. Returns whether the subject moved.
    fn retarget(&mut self) -> bool {
        let Some(dir) = self.follow.as_ref() else {
            return false;
        };
        match crate::watch::newest(dir) {
            Some(newest) if newest != self.input => {
                self.input = newest;
                true
            }
            // A directory that has just lost its last file keeps showing what
            // it had. Blanking the window would say the diagram was deleted,
            // which is a stronger claim than "nothing to point at".
            _ => false,
        }
    }

    fn stale(&self) -> bool {
        std::fs::metadata(&self.input)
            .and_then(|m| m.modified())
            .ok()
            != self.stamp
    }

    fn art_size(&self) -> (f64, f64) {
        match &self.art {
            Some(Art::Tree(tree)) => {
                let s = tree.size();
                (s.width() as f64, s.height() as f64)
            }
            _ => (0.0, 0.0),
        }
    }

    /// Zoom about the centre, so what you are looking at stays where it is.
    ///
    /// Multiplicative rather than additive: one notch in and one notch out
    /// returns to exactly where you were, and a step means the same thing at
    /// 20% as at 800%.
    fn zoom_by(&mut self, factor: f64) {
        let (cx, cy) = (self.size.0 as f64 / 2.0, self.size.1 as f64 / 2.0);
        let before = self.view.scale;
        let after = (before * factor).clamp(0.05, 40.0);
        self.view.dx = cx - (cx - self.view.dx) * (after / before);
        self.view.dy = cy - (cy - self.view.dy) * (after / before);
        self.view.scale = after;
        self.placed = true;
    }

    /// The whole picture on screen, with a margin.
    fn fit(&mut self) {
        let (w, h) = self.art_size();
        self.fit_to(w, h);
    }

    /// The arithmetic of [`fit`], apart from the picture — so the rule about
    /// what a starting zoom may be is testable without a font stack.
    fn fit_to(&mut self, w: f64, h: f64) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (vw, vh) = (self.size.0 as f64, self.size.1 as f64);
        // Never larger than actual size — a small diagram opens at 100% rather
        // than blown up to fill the window — and snapped down to a multiple of
        // 5%, so the number in the title bar is one you can reason about and
        // step from. 87.3% is a true answer to the wrong question.
        let room = ((vw - 48.0) / w).min((vh - 48.0) / h).min(1.0);
        let scale = ((room * 20.0).floor() / 20.0).max(0.05);
        self.view = View {
            scale,
            dx: (vw - w * scale) / 2.0,
            dy: (vh - h * scale) / 2.0,
        };
    }

    fn actual_size(&mut self) {
        let (w, h) = self.art_size();
        let (vw, vh) = (self.size.0 as f64, self.size.1 as f64);
        self.view = View {
            scale: 1.0,
            dx: (vw - w) / 2.0,
            dy: (vh - h) / 2.0,
        };
        self.placed = true;
    }

    /// Rasterise into the window's buffer.
    fn paint(&self, buffer: &mut Vec<u32>) {
        let (w, h) = (self.size.0 as u32, self.size.1 as u32);
        buffer.resize(self.size.0 * self.size.1, 0);

        let Some(mut pixmap) = tiny_skia::Pixmap::new(w.max(1), h.max(1)) else {
            return;
        };
        let ground = match &self.art {
            // A file that will not parse says so in colour as well as in the
            // title: a window that looks the same broken and working is one
            // you stop trusting.
            Some(Art::Failed(_)) => (0x3a, 0x14, 0x14),
            _ => GROUND,
        };
        pixmap.fill(tiny_skia::Color::from_rgba8(
            ground.0, ground.1, ground.2, 0xff,
        ));

        if let Some(Art::Tree(tree)) = &self.art {
            let t = tiny_skia::Transform::from_translate(self.view.dx as f32, self.view.dy as f32)
                .pre_scale(self.view.scale as f32, self.view.scale as f32);
            resvg::render(tree, t, &mut pixmap.as_mut());
        }

        // `tiny-skia` gives premultiplied RGBA; `minifb` wants 0RGB.
        for (out, px) in buffer.iter_mut().zip(pixmap.pixels()) {
            *out = ((px.red() as u32) << 16) | ((px.green() as u32) << 8) | px.blue() as u32;
        }
    }

    fn title(&self) -> String {
        let name = self
            .input
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "bgraph".into());
        match &self.art {
            Some(Art::Failed(err)) => {
                format!("{name} — {}", err.lines().next().unwrap_or("error"))
            }
            _ => format!("{name} — {}%", (self.view.scale * 100.0).round()),
        }
    }
}

/// Tell the renderer what the generic font families mean.
///
/// A browser asks fontconfig; `resvg` does not, so `sans-serif` in an SVG
/// resolves to usvg's built-in guess — and the default stack this project
/// emits, `Inter, ui-sans-serif, system-ui, sans-serif`, is a *web* stack:
/// `ui-sans-serif` and `system-ui` are CSS generics that exist in no font
/// database, and `Inter` is on very few machines. Everything therefore lands
/// on the generic, and the generic was whatever usvg guessed — which is how a
/// diagram came out set in a face nothing else on the machine uses.
///
/// The lists are ordered by preference and filtered by what is actually
/// installed, so this states a wish rather than an assumption.
fn resolve_generic_families(options: &mut resvg::usvg::Options) {
    const SANS: &[&str] = &[
        "Inter",
        "Archivo",
        "Noto Sans",
        "DejaVu Sans",
        "Liberation Sans",
        "Arial",
        "Helvetica",
    ];
    const SERIF: &[&str] = &["Noto Serif", "DejaVu Serif", "Liberation Serif", "Georgia"];
    const MONO: &[&str] = &[
        "JetBrainsMono NFM",
        "JetBrains Mono",
        "Noto Sans Mono",
        "DejaVu Sans Mono",
        "Liberation Mono",
    ];

    // Read the installed families once, then hand the database back: the
    // lookup borrows it and the setters need it mutably.
    let families: std::collections::HashSet<String> = options
        .fontdb
        .faces()
        .flat_map(|f| f.families.iter().map(|(name, _)| name.clone()))
        .collect();
    let installed = |names: &[&str]| -> Option<String> {
        names
            .iter()
            .find(|n| families.contains(**n))
            .map(|n| n.to_string())
    };

    let (sans, serif, mono) = (installed(SANS), installed(SERIF), installed(MONO));

    if let Some(sans) = sans {
        options.fontdb_mut().set_sans_serif_family(sans.clone());
        // The last resort when a stack names nothing that exists — which is
        // exactly the case for the stack this project emits.
        options.font_family = sans;
    }
    if let Some(serif) = serif {
        options.fontdb_mut().set_serif_family(serif);
    }
    if let Some(mono) = mono {
        options.fontdb_mut().set_monospace_family(mono);
    }
}

/// Whether a window can be opened at all. A machine with no display — a
/// container, an ssh session — has to be told, not left waiting for a window
/// that never appears.
pub fn available() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() || std::env::var_os("DISPLAY").is_some()
}

/// Where the file is, for the message when there is no display.
pub fn describe(input: &Path) -> String {
    input.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer() -> Viewer {
        let mut v = Viewer::new(PathBuf::from("x.svg"), None);
        v.size = (800, 600);
        v
    }

    #[test]
    fn the_opening_zoom_is_a_number_you_can_reason_about() {
        // 87.3% is a true answer to the wrong question. A picture that fits
        // opens at 100% rather than blown up, and one that does not opens at a
        // multiple of 5% — which is also the step, so the numbers line up.
        let mut v = viewer();
        v.size = (800, 600);

        for (w, h) in [(200.0, 150.0), (2000.0, 400.0), (900.0, 3000.0)] {
            v.art = None;
            v.fit_to(w, h);
            let percent = v.view.scale * 100.0;
            assert!(
                (percent / 5.0).fract().abs() < 1e-9,
                "{percent}% is not a multiple of five"
            );
            assert!(v.view.scale <= 1.0, "never larger than actual size");
            assert!(v.view.scale >= 0.05);
        }
    }

    #[test]
    fn a_notch_is_five_percent_and_shift_is_ten() {
        let mut v = viewer();
        v.zoom_by(1.05);
        assert!((v.view.scale - 1.05).abs() < 1e-9, "{}", v.view.scale);

        let mut v = viewer();
        v.zoom_by(1.10);
        assert!((v.view.scale - 1.10).abs() < 1e-9, "{}", v.view.scale);
    }

    #[test]
    fn one_notch_each_way_returns_to_where_it_was() {
        // Multiplicative rather than additive, so a step means the same thing
        // at 20% as at 800% and the pair cancels exactly.
        let mut v = viewer();
        v.zoom_by(0.4);
        let before = v.view;
        v.zoom_by(1.05);
        v.zoom_by(1.0 / 1.05);
        assert!((v.view.scale - before.scale).abs() < 1e-9);
        assert!((v.view.dx - before.dx).abs() < 1e-6, "and it stays put");
        assert!((v.view.dy - before.dy).abs() < 1e-6);
    }

    #[test]
    fn zooming_never_leaves_the_scale_at_or_below_zero() {
        // The bug this replaced: a factor derived from Wayland's axis value
        // went negative, and the clamp showed that as 5% — a wheel *up* that
        // shrank the picture to nothing.
        let mut v = viewer();
        for _ in 0..400 {
            v.zoom_by(1.0 / 1.05);
        }
        assert!(v.view.scale >= 0.05, "{}", v.view.scale);

        for _ in 0..800 {
            v.zoom_by(1.05);
        }
        assert!(v.view.scale <= 40.0, "{}", v.view.scale);
    }
}
