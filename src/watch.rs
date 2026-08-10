//! Live preview: a tiny blocking HTTP server that re-renders on every request
//! and a page that polls the source file's mtime to know when to reload.
//!
//! Deliberately dependency-free — the whole point is a short feedback loop,
//! not a web stack.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result};

/// Serve a diagram, reloading on every save.
///
/// `data` is a JSON file of bindings, when the diagram is a template. It is
/// re-read on each request alongside the source, so editing the *data* reloads
/// the preview too — writing a template against its real numbers is the loop
/// this genre of file is written in, and it was the only one without a live
/// view.
pub fn serve(input: PathBuf, data: Option<PathBuf>, port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("cannot bind 127.0.0.1:{port}"))?;

    println!("bgraph watching {}", input.display());
    if let Some(path) = &data {
        println!("  with data from {}", path.display());
    }
    println!("  → http://127.0.0.1:{port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle(stream, &input, data.as_deref()) {
                    eprintln!("request failed: {err}");
                }
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }

    Ok(())
}

fn handle(mut stream: TcpStream, input: &Path, data: Option<&Path>) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    let path = request.split_whitespace().nth(1).unwrap_or("/");

    let (content_type, body) = match path {
        "/version" => ("text/plain", version(input)),
        "/svg" => match render_source(input, data) {
            Ok(svg) => ("image/svg+xml", svg),
            Err(err) => ("text/plain", format!("{err:#}")),
        },
        _ => ("text/html", page(input, data)),
    };

    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )?;
    stream.write_all(body.as_bytes())?;
    stream.flush()?;

    Ok(())
}

pub(crate) fn render_source(input: &Path, data: Option<&Path>) -> Result<String> {
    let source =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;

    // A template with data is filled first; the error from a bad binding then
    // reaches the page the same way a syntax error does, instead of ending the
    // server.
    //
    // *Without* data a template renders as written, holes and all — which is
    // the opposite of what a one-shot render does, and deliberately. A hole
    // parses, so the shape is drawable, and the shape is what you are editing
    // when you have not wired the data up yet. Refusing here would leave the
    // preview blank for the whole first half of writing a template.
    #[cfg(feature = "json")]
    if let Some(path) = data {
        let raw =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        let values = crate::template::Bindings::from_json(&raw)
            .with_context(|| format!("in {}", path.display()))?;
        return crate::template::render(&source, &values);
    }
    let _ = data;

    let diagram = crate::parser::parse_diagram(&source)?;
    let layout = crate::layout::solve(&diagram)?;
    Ok(crate::render_svg::render(&layout))
}

fn version(input: &Path) -> String {
    fs::metadata(input)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis().to_string())
        .unwrap_or_else(|| "0".into())
}

fn page(input: &Path, data: Option<&Path>) -> String {
    let (status, content) = match render_source(input, data) {
        Ok(svg) => ("", svg),
        Err(err) => (
            "error",
            format!(
                "<pre class=\"err\">{}</pre>",
                html_escape(&format!("{err:#}"))
            ),
        ),
    };

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>bgraph — {name}</title>
<style>
  :root {{ color-scheme: light dark; --surround: #d4d4d8; --ink: #52525b; }}
  @media (prefers-color-scheme: dark) {{
    :root {{ --surround: #3f3f46; --ink: #a1a1aa; }}
  }}
  html, body {{ height: 100%; }}
  body {{ margin: 0; overflow: hidden; background: var(--surround);
         font: 13px ui-sans-serif, system-ui, sans-serif; color: var(--ink); }}

  /* The viewport clips; the canvas is what moves. Transforming one element
     keeps the SVG itself untouched, so what you see is what you export. */
  #viewport {{ position: fixed; inset: 0; cursor: grab; touch-action: none; }}
  #viewport.dragging {{ cursor: grabbing; }}
  #canvas {{ position: absolute; top: 0; left: 0; transform-origin: 0 0;
            will-change: transform; }}

  /* A checkerboard, not a solid color: the diagram paints its own surface, so
     whatever shows through here is genuinely transparent in the exported file. */
  #canvas svg {{ display: block;
                background-color: #ffffff;
                background-image:
                  linear-gradient(45deg, #cbd5e1 25%, transparent 25%),
                  linear-gradient(-45deg, #cbd5e1 25%, transparent 25%),
                  linear-gradient(45deg, transparent 75%, #cbd5e1 75%),
                  linear-gradient(-45deg, transparent 75%, #cbd5e1 75%);
                background-size: 16px 16px;
                background-position: 0 0, 0 8px, 8px -8px, -8px 0; }}

  .err {{ color: #dc2626; font: 12px ui-monospace, monospace; white-space: pre-wrap;
         max-width: 80ch; padding: 16px; margin: 24px;
         border: 1px solid #dc2626; border-radius: 8px;
         background: #fff; }}

  #bar {{ position: fixed; bottom: 10px; left: 12px; right: 12px;
         display: flex; gap: 14px; align-items: baseline;
         pointer-events: none; }}
  #zoom {{ font-variant-numeric: tabular-nums; }}
  #keys {{ opacity: .65; }}
  #name {{ margin-left: auto; }}
</style>
</head>
<body>
<div id="viewport"><div id="canvas">{content}</div></div>
<div id="bar">
  <span id="zoom">100%</span>
  <span id="keys">scroll to zoom · drag to pan · <b>0</b> reset · <b>f</b> fit</span>
  <span id="name">{name} · live{status_label}</span>
</div>
<script>
const viewport = document.getElementById('viewport');
const canvas = document.getElementById('canvas');
const readout = document.getElementById('zoom');

// Keyed by file, so two watched diagrams do not fight over one view.
const KEY = 'bgraph:view:{name}';
let view = {{ x: 0, y: 0, k: 1 }};

function apply() {{
  canvas.style.transform =
    'translate(' + view.x + 'px,' + view.y + 'px) scale(' + view.k + ')';
  readout.textContent = Math.round(view.k * 100) + '%';
  // The page reloads on every save; without this, each save would throw the
  // view away and re-centre — which makes zooming useless while editing.
  try {{ sessionStorage.setItem(KEY, JSON.stringify(view)); }} catch (e) {{}}
}}

function bounds() {{
  const svg = canvas.querySelector('svg');
  if (!svg) return null;
  const w = svg.width.baseVal.value || svg.clientWidth;
  const h = svg.height.baseVal.value || svg.clientHeight;
  return w && h ? {{ w: w, h: h }} : null;
}}

function fit() {{
  const b = bounds();
  if (!b) return;
  const pad = 32;
  const k = Math.min(
    (innerWidth - pad * 2) / b.w,
    (innerHeight - pad * 2) / b.h,
    1,
  );
  view = {{ k: k, x: (innerWidth - b.w * k) / 2, y: (innerHeight - b.h * k) / 2 }};
  apply();
}}

function zoomAt(px, py, factor) {{
  const k = Math.min(16, Math.max(0.05, view.k * factor));
  // Hold the point under the cursor still: it is the only anchor that makes
  // wheel-zoom feel like moving the paper rather than resizing it.
  view.x = px - (px - view.x) * (k / view.k);
  view.y = py - (py - view.y) * (k / view.k);
  view.k = k;
  apply();
}}

viewport.addEventListener('wheel', (e) => {{
  e.preventDefault();
  if (e.shiftKey) {{
    view.x -= e.deltaY;
    apply();
    return;
  }}
  zoomAt(e.clientX, e.clientY, Math.exp(-e.deltaY * 0.0016));
}}, {{ passive: false }});

let dragging = null;
viewport.addEventListener('pointerdown', (e) => {{
  dragging = {{ x: e.clientX - view.x, y: e.clientY - view.y }};
  viewport.setPointerCapture(e.pointerId);
  viewport.classList.add('dragging');
}});
viewport.addEventListener('pointermove', (e) => {{
  if (!dragging) return;
  view.x = e.clientX - dragging.x;
  view.y = e.clientY - dragging.y;
  apply();
}});
for (const done of ['pointerup', 'pointercancel']) {{
  viewport.addEventListener(done, () => {{
    dragging = null;
    viewport.classList.remove('dragging');
  }});
}}

addEventListener('keydown', (e) => {{
  if (e.key === '0') {{ view.k = 1; apply(); }}
  else if (e.key === 'f') fit();
  else if (e.key === '+' || e.key === '=') zoomAt(innerWidth / 2, innerHeight / 2, 1.2);
  else if (e.key === '-') zoomAt(innerWidth / 2, innerHeight / 2, 1 / 1.2);
}});

viewport.addEventListener('dblclick', fit);

// Restore the view a previous render left behind, or frame the diagram.
try {{
  const saved = JSON.parse(sessionStorage.getItem(KEY) || 'null');
  if (saved && isFinite(saved.k)) {{ view = saved; apply(); }} else fit();
}} catch (e) {{ fit(); }}

let current = null;
async function tick() {{
  try {{
    const v = await (await fetch('/version', {{cache: 'no-store'}})).text();
    if (current === null) current = v;
    else if (v !== current) location.reload();
  }} catch (e) {{ /* server restarting */ }}
}}
setInterval(tick, 300);
tick();
</script>
</body>
</html>"#,
        name = html_escape(&input.display().to_string()),
        status_label = if status.is_empty() { "" } else { " · error" },
    )
}

/// The page is a `format!` template, so every CSS and JS brace is doubled in
/// the source. One missed pair produces a page that serves fine and silently
/// does nothing — hence the tests below.
pub fn preview_page(input: &Path) -> String {
    page(input, None)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered() -> String {
        page(Path::new("diagram.bd"), None)
    }

    /// Everything between `<style>`/`<script>` and their closing tag.
    fn between(page: &str, tag: &str) -> String {
        let open = format!("<{tag}>");
        let close = format!("</{tag}>");
        let start = page.find(&open).expect("tag is present") + open.len();
        let end = page[start..].find(&close).expect("tag is closed") + start;
        page[start..end].to_string()
    }

    #[test]
    fn the_template_leaves_no_doubled_braces_behind() {
        // `format!` needs `{{` for a literal brace. A missed pair reaches the
        // browser as `{{`, which is a syntax error in both CSS and JS — and
        // the page still serves, so nothing else would notice.
        let page = rendered();
        for tag in ["style", "script"] {
            let body = between(&page, tag);
            assert!(!body.contains("{{"), "doubled `{{{{` survived into <{tag}>");
            assert!(!body.contains("}}"), "doubled `}}}}` survived into <{tag}>");
        }
    }

    #[test]
    fn braces_balance_in_the_emitted_script() {
        let script = between(&rendered(), "script");
        let opens = script.matches('{').count();
        let closes = script.matches('}').count();
        assert_eq!(opens, closes, "unbalanced braces in the emitted script");
    }

    #[test]
    fn the_view_survives_a_reload() {
        // The page reloads on every save. Without persistence the view would
        // reset each time, which makes zoom useless while editing.
        let script = between(&rendered(), "script");
        assert!(script.contains("sessionStorage.setItem"));
        assert!(script.contains("sessionStorage.getItem"));
        assert!(
            script.contains("location.reload()"),
            "live reload still works"
        );
    }

    #[test]
    fn the_diagram_is_moved_by_transforming_a_wrapper() {
        // Transforming the wrapper leaves the SVG untouched, so what is on
        // screen is what the file exports.
        let page = rendered();
        assert!(page.contains(r#"<div id="canvas">"#));
        let script = between(&page, "script");
        assert!(script.contains("canvas.style.transform"));
        assert!(script.contains("scale("));
    }

    #[test]
    fn zoom_holds_the_point_under_the_cursor() {
        let script = between(&rendered(), "script");
        assert!(script.contains("function zoomAt"));
        assert!(script.contains("view.x = px - (px - view.x)"));
        // A wheel handler that does not cancel the event scrolls the page too.
        assert!(script.contains("passive: false"));
        assert!(script.contains("e.preventDefault()"));
    }

    #[test]
    fn the_page_says_how_to_drive_it() {
        let page = rendered();
        assert!(page.contains("scroll to zoom"));
        assert!(page.contains("drag to pan"));
    }
}
