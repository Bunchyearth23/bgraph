//! Native determinism evidence: source, rendered bytes and unrounded geometry.
#![cfg(feature = "all-archetypes")]

use std::path::{Path, PathBuf};
use std::process::Command;

use bgraph::layout::{Chart, Layout, Rect};
use bgraph::style::Style;
use serde_json::{Value, json};

const LOCAL_RUNS: usize = 3;
const FRESH_PROCESSES: usize = 3;
const FIXTURES: &[(&str, &str, bool)] = &[
    (
        "bpmn-dependent",
        include_str!("fixtures/placement/bpmn-dependent.bd"),
        true,
    ),
    (
        "bpmn-attach-conflict",
        include_str!("fixtures/placement/bpmn-attach-conflict.bd"),
        false,
    ),
    (
        "bpmn-cycle",
        include_str!("fixtures/placement/bpmn-cycle.bd"),
        true,
    ),
    (
        "bpmn-cycle-conflict",
        include_str!("fixtures/placement/bpmn-cycle-conflict.bd"),
        false,
    ),
    ("declared", include_str!("runtime/declared.bd"), true),
    ("group", include_str!("runtime/group.bd"), true),
    ("pie", include_str!("runtime/pie.bd"), true),
    ("components", include_str!("runtime/components.bd"), true),
    ("frame-grid", include_str!("runtime/frame-grid.bd"), true),
    (
        "underconstrained",
        include_str!("runtime/underconstrained.bd"),
        true,
    ),
    ("fractional", include_str!("runtime/fractional.bd"), true),
    (
        "frame-nested",
        include_str!("runtime/frame-nested.bd"),
        true,
    ),
    (
        "frame-external",
        include_str!("runtime/frame-external.bd"),
        true,
    ),
    ("overlap", include_str!("runtime/overlap.bd"), true),
    (
        "missing-anchor",
        include_str!("runtime/missing-anchor.bd"),
        false,
    ),
    ("conflict", include_str!("runtime/conflict.bd"), false),
    (
        "minimum-conflict",
        include_str!("runtime/minimum-conflict.bd"),
        false,
    ),
    (
        "managed-layout",
        include_str!("runtime/managed-layout.bd"),
        false,
    ),
    (
        "unknown-component",
        include_str!("runtime/unknown-component.bd"),
        false,
    ),
    (
        "unicode-error",
        include_str!("runtime/unicode-error.bd"),
        false,
    ),
    (
        "size-chain-conflict",
        include_str!("runtime/size-chain-conflict.bd"),
        false,
    ),
    (
        "frame-inflation-conflict",
        include_str!("runtime/frame-inflation-conflict.bd"),
        false,
    ),
];

fn bits(value: f64) -> String {
    assert!(value.is_finite(), "non-finite geometry: {value}");
    format!("{:016x}", value.to_bits())
}

fn point((x, y): (f64, f64)) -> Value {
    json!([bits(x), bits(y)])
}

fn rect(rect: Rect) -> Value {
    json!([bits(rect.x), bits(rect.y), bits(rect.w), bits(rect.h)])
}

fn style_geometry(style: &Style) -> Value {
    json!({
        "strokeWidth": style.stroke_width.map(bits),
        "radius": style.radius.map(bits),
        "width": style.width.map(bits),
        "height": style.height.map(bits),
        "padding": style.padding.map(bits),
        "fontSize": style.font_size.map(bits),
        "labelOffset": style.label_offset.map(point),
        "bend": style.bend.map(bits),
        "value": style.value.map(bits),
        "x": style.x.map(bits), "y": style.y.map(bits),
        "points": style.points.as_ref().map(|values| values.iter().copied().map(bits).collect::<Vec<_>>()),
        "pairs": style.pairs.as_ref().map(|values| values.iter().copied().map(bits).collect::<Vec<_>>()),
        "setSize": style.set_size.map(bits), "shared": style.shared.map(bits),
        "fromPort": style.from_port.map(point), "toPort": style.to_port.map(point),
        "lane": style.lane.map(bits),
        "scores": style.scores.as_ref().map(|values| values.iter().copied().map(bits).collect::<Vec<_>>()),
        "limit": style.limit.map(bits), "length": style.length.map(bits),
    })
}

fn geometry(layout: &Layout) -> Value {
    let chart = match &layout.chart {
        Chart::Free => json!({"kind": "free"}),
        Chart::Packet => json!({"kind": "packet"}),
        Chart::Treemap => json!({"kind": "treemap"}),
        Chart::Quadrant => json!({"kind": "quadrant"}),
        Chart::Wave => json!({"kind": "wave"}),
        Chart::Sankey => json!({"kind": "sankey"}),
        Chart::Bar => json!({"kind": "bar"}),
        Chart::Xy => json!({"kind": "xy"}),
        Chart::Sequence => json!({"kind": "sequence"}),
        Chart::Gantt => json!({"kind": "gantt"}),
        Chart::Timeline => json!({"kind": "timeline"}),
        Chart::Pie(pie) => json!({
            "kind": "pie", "center": point((pie.cx, pie.cy)),
            "radius": bits(pie.radius), "inner": bits(pie.inner), "gapColor": pie.gap_color,
            "slices": pie.slices.iter().map(|slice| json!({
                "start": bits(slice.start), "end": bits(slice.end), "fill": slice.fill,
            })).collect::<Vec<_>>(),
        }),
    };
    let routes = bgraph::render_svg::route_all(layout);
    let (dx, dy, width, height) = bgraph::render_svg::canvas(layout);
    json!({
        "nodes": layout.nodes.iter().map(|node| json!({
            "id": node.id, "lines": node.lines, "rect": rect(node.rect),
            "styleGeometry": style_geometry(&node.style),
        })).collect::<Vec<_>>(),
        "groups": layout.groups.iter().map(|group| json!({
            "id": group.id, "lines": group.lines, "rect": rect(group.rect),
            "styleGeometry": style_geometry(&group.style),
        })).collect::<Vec<_>>(),
        "edges": layout.edges.iter().map(|edge| json!({
            "from": edge.from, "to": edge.to, "label": edge.label,
            "arrow": format!("{:?}", edge.arrow),
            "via": edge.via.iter().copied().map(point).collect::<Vec<_>>(),
            "span": [edge.span.start, edge.span.end], "styleGeometry": style_geometry(&edge.style),
        })).collect::<Vec<_>>(),
        "guides": layout.guides.iter().map(|guide| json!({
            "from": point(guide.from), "to": point(guide.to), "width": bits(guide.width),
            "color": guide.color, "dash": guide.dash,
        })).collect::<Vec<_>>(),
        "polylines": layout.polylines.iter().map(|line| json!({
            "points": line.points.iter().copied().map(point).collect::<Vec<_>>(),
            "width": bits(line.width), "closed": line.closed, "fill": line.fill,
            "stroke": line.stroke, "dash": line.dash, "head": format!("{:?}", line.head), "over": line.over,
        })).collect::<Vec<_>>(),
        "annotations": layout.annotations.iter().map(|text| json!({
            "text": text.text, "position": point((text.x, text.y)), "size": bits(text.size),
            "anchor": format!("{:?}", text.anchor), "title": text.title,
            "color": text.color, "weight": text.weight, "family": text.family,
        })).collect::<Vec<_>>(),
        "routes": routes.iter().map(|route| json!({
            "points": route.points.iter().copied().map(point).collect::<Vec<_>>(),
            "label": route.label.as_ref().map(|(text, bounds)| json!({"text": text, "rect": rect(*bounds)})),
        })).collect::<Vec<_>>(),
        "canvas": [bits(dx), bits(dy), bits(width), bits(height)],
        "optionsGeometry": {
            "padding": bits(layout.opts.padding), "gap": bits(layout.opts.gap),
            "grid": layout.opts.grid.map(point),
            "gridSpec": layout.opts.grid_spec.map(|grid| json!({
                "columns": grid.columns, "rows": grid.rows,
                "gapColumn": bits(grid.gap_col), "gapRow": bits(grid.gap_row),
            })),
        },
        "chart": chart,
    })
}

fn case(name: &str, source: &str, valid: bool) -> Value {
    // Checkout settings must not make the Windows input differ from Unix.
    let source = source.replace("\r\n", "\n");
    let analysis = bgraph::analyze_source(&source);
    assert_eq!(
        !analysis.has_errors(),
        valid,
        "{name}: {:?}",
        analysis.diagnostics
    );
    let render = bgraph::render_source(&source);
    assert_eq!(render.is_ok(), valid, "{name}: {render:?}");
    let (svg, render_error) = match render {
        Ok(svg) => (Some(svg), None),
        Err(error) => (None, Some(error.to_string())),
    };
    json!({
        "id": name, "source": source, "valid": valid,
        "svg": svg, "renderError": render_error,
        "diagnostics": analysis.diagnostics.iter().map(|diag| json!({
            "severity": match diag.severity { bgraph::check::Severity::Error => "error", bgraph::check::Severity::Warning => "warning" },
            "span": [diag.span.start, diag.span.end], "message": diag.message,
        })).collect::<Vec<_>>(),
        "geometry": analysis.layout.as_ref().map(geometry),
    })
}

fn snapshot() -> Value {
    assert_eq!(
        bgraph::props::ARCHETYPES.len(),
        30,
        "native proof requires all archetypes"
    );
    assert_eq!(bgraph::examples::EXAMPLES.len(), 30);
    let mut cases = Vec::new();
    for (name, source) in bgraph::examples::EXAMPLES {
        assert!(
            bgraph::props::archetype(name).is_some(),
            "example without a genre: {name}"
        );
        cases.push(case(&format!("example/{name}"), source, true));
    }
    for (name, source, valid) in FIXTURES {
        cases.push(case(&format!("fixture/{name}"), source, *valid));
    }
    cases.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    assert!(cases.windows(2).all(|pair| pair[0]["id"] != pair[1]["id"]));
    json!({
        "archetypes": bgraph::props::ARCHETYPES.iter().map(|genre| genre.name).collect::<Vec<_>>(),
        "buildProfile": if cfg!(debug_assertions) { "debug" } else { "release" },
        "features": {
            "cli": cfg!(feature = "cli"), "pdf": cfg!(feature = "pdf"),
            "json": cfg!(feature = "json"), "csv": cfg!(feature = "csv"),
            "completions": cfg!(feature = "completions"), "png": cfg!(feature = "png"),
            "wasm": cfg!(feature = "wasm"), "typst": cfg!(feature = "typst"),
        },
        "cases": cases,
    })
}

fn first_difference(left: &Value, right: &Value, path: &str) -> Option<String> {
    if left == right {
        return None;
    }
    match (left, right) {
        (Value::Array(left), Value::Array(right)) if left.len() == right.len() => left
            .iter()
            .zip(right)
            .enumerate()
            .find_map(|(index, (left, right))| {
                first_difference(left, right, &format!("{path}/{index}"))
            }),
        (Value::Object(left), Value::Object(right)) if left.keys().eq(right.keys()) => left
            .iter()
            .find_map(|(key, left)| first_difference(left, &right[key], &format!("{path}/{key}"))),
        _ => Some(path.to_string()),
    }
}

fn assert_identical(expected: &Value, actual: &Value, context: &str) {
    if let Some(path) = first_difference(expected, actual, "") {
        panic!("native snapshot mismatch in {context} at JSON pointer {path}");
    }
}

fn write_snapshot(path: &Path, value: &Value) {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn native_snapshot() {
    let baseline = snapshot();
    for run in 1..LOCAL_RUNS {
        assert_identical(
            &baseline,
            &snapshot(),
            &format!("in-process run {}", run + 1),
        );
    }
    let child = std::env::var_os("BGRAPH_NATIVE_CHILD").is_some();
    let mut child_runs = Vec::new();
    if !child {
        let scratch = Scratch(std::env::temp_dir().join(format!(
            "bgraph-native-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
        )));
        std::fs::create_dir_all(&scratch.0).unwrap();
        let executable = std::env::current_exe().unwrap();
        let runner: Vec<String> = std::env::var("BGRAPH_NATIVE_CHILD_RUNNER")
            .map(|raw| {
                serde_json::from_str(&raw)
                    .expect("BGRAPH_NATIVE_CHILD_RUNNER must be a JSON argv array")
            })
            .unwrap_or_default();
        for run in 0..FRESH_PROCESSES {
            let output_path = scratch.0.join(format!("child-{run}.json"));
            let mut command = if let Some(program) = runner.first() {
                let mut command = Command::new(program);
                command.args(&runner[1..]).arg(&executable);
                command
            } else {
                Command::new(&executable)
            };
            let output = command
                .args(["--exact", "native_snapshot", "--nocapture"])
                .env("BGRAPH_NATIVE_CHILD", "1")
                .env("BGRAPH_NATIVE_SNAPSHOT", &output_path)
                .output()
                .unwrap_or_else(|error| panic!("could not start native child {run}: {error}"));
            assert!(
                output.status.success(),
                "native child {run} failed: {:?}\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            let value: Value =
                serde_json::from_slice(&std::fs::read(&output_path).unwrap()).unwrap();
            assert_identical(
                &baseline,
                &value["snapshot"],
                &format!("fresh process {}", run + 1),
            );
            assert_eq!(value["verification"]["inProcessRuns"], LOCAL_RUNS);
            child_runs.push(LOCAL_RUNS);
        }
    }
    let value = json!({
        "schemaVersion": 1, "compilerVersion": env!("CARGO_PKG_VERSION"),
        "revision": std::env::var("BGRAPH_NATIVE_REVISION").unwrap_or_else(|_| "working-tree".to_string()),
        "platform": {
            "os": std::env::consts::OS, "arch": std::env::consts::ARCH,
            "runner": std::env::var("BGRAPH_NATIVE_RUNNER").unwrap_or_else(|_| "local".to_string()),
        },
        "verification": { "inProcessRuns": LOCAL_RUNS, "freshProcesses": child_runs.len(), "childInProcessRuns": child_runs },
        "snapshot": baseline,
    });
    if let Some(path) = std::env::var_os("BGRAPH_NATIVE_SNAPSHOT") {
        write_snapshot(Path::new(&path), &value);
    }
    println!(
        "native snapshot: {} cases, {LOCAL_RUNS} in-process runs, {} fresh processes ({}/{})",
        value["snapshot"]["cases"].as_array().unwrap().len(),
        if child { 0 } else { FRESH_PROCESSES },
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}
