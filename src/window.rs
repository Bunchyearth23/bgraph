//! A native window, for `bgraph watch`.
//!
//! The same viewer as the browser page and the same controls — scroll to zoom,
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
pub fn show(input: PathBuf, data: Option<PathBuf>) -> Result<()> {
    let mut viewer = Viewer::new(input, data);

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

        if let Some((_, dy)) = window.get_scroll_wheel() {
            viewer.zoom(dy as f64 * 0.12);
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
            if viewer.stale() {
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
        self.art = Some(match resvg::usvg::Tree::from_str(&svg, &options) {
            Ok(tree) => Art::Tree(Box::new(tree)),
            Err(err) => Art::Failed(format!("the SVG did not parse: {err}")),
        });
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
    fn zoom(&mut self, step: f64) {
        let (cx, cy) = (self.size.0 as f64 / 2.0, self.size.1 as f64 / 2.0);
        let before = self.view.scale;
        let after = (before * (1.0 + step)).clamp(0.05, 40.0);
        self.view.dx = cx - (cx - self.view.dx) * (after / before);
        self.view.dy = cy - (cy - self.view.dy) * (after / before);
        self.view.scale = after;
        self.placed = true;
    }

    /// The whole picture on screen, with a margin.
    fn fit(&mut self) {
        let (w, h) = self.art_size();
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let (vw, vh) = (self.size.0 as f64, self.size.1 as f64);
        let scale = ((vw - 48.0) / w).min((vh - 48.0) / h).clamp(0.02, 4.0);
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
