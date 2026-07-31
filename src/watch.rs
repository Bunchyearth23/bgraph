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

pub fn serve(input: PathBuf, port: u16) -> Result<()> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .with_context(|| format!("cannot bind 127.0.0.1:{port}"))?;

    println!("bgraph watching {}", input.display());
    println!("  → http://127.0.0.1:{port}");

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle(stream, &input) {
                    eprintln!("request failed: {err}");
                }
            }
            Err(err) => eprintln!("connection failed: {err}"),
        }
    }

    Ok(())
}

fn handle(mut stream: TcpStream, input: &Path) -> Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request = String::new();
    reader.read_line(&mut request)?;

    let path = request.split_whitespace().nth(1).unwrap_or("/");

    let (content_type, body) = match path {
        "/version" => ("text/plain", version(input)),
        "/svg" => match render_source(input) {
            Ok(svg) => ("image/svg+xml", svg),
            Err(err) => ("text/plain", format!("{err:#}")),
        },
        _ => ("text/html", page(input)),
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

fn render_source(input: &Path) -> Result<String> {
    let source =
        fs::read_to_string(input).with_context(|| format!("cannot read {}", input.display()))?;
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

fn page(input: &Path) -> String {
    let (status, content) = match render_source(input) {
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
  :root {{ color-scheme: light dark; --surround: #d4d4d8; }}
  @media (prefers-color-scheme: dark) {{ :root {{ --surround: #3f3f46; }} }}
  body {{ margin: 0; min-height: 100vh; display: grid; place-items: center;
         background: var(--surround); font: 13px ui-sans-serif, system-ui, sans-serif; }}
  #stage {{ padding: 24px; }}
  /* A checkerboard, not a solid color: the diagram paints its own surface, so
     whatever shows through here is genuinely transparent in the exported file. */
  #stage svg {{ max-width: 96vw; height: auto; display: block;
                background-color: #ffffff;
                background-image:
                  linear-gradient(45deg, #cbd5e1 25%, transparent 25%),
                  linear-gradient(-45deg, #cbd5e1 25%, transparent 25%),
                  linear-gradient(45deg, transparent 75%, #cbd5e1 75%),
                  linear-gradient(-45deg, transparent 75%, #cbd5e1 75%);
                background-size: 16px 16px;
                background-position: 0 0, 0 8px, 8px -8px, -8px 0; }}
  .err {{ color: #dc2626; font: 12px ui-monospace, monospace; white-space: pre-wrap;
         max-width: 80ch; padding: 16px; border: 1px solid #dc2626; border-radius: 8px; }}
  #tag {{ position: fixed; bottom: 10px; right: 12px; color: #71717a; }}
</style>
</head>
<body>
<div id="stage">{content}</div>
<div id="tag">{name} · live{status_label}</div>
<script>
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
