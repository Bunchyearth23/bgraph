//! Dependency-free pipeline benchmark. Run `cargo bench --bench pipeline`.

use std::time::{Duration, Instant};

fn large_graph() -> String {
    let mut source = String::from("graph { flow: right }\nnode n0 \"0\"\n");
    for index in 1..250 {
        source.push_str(&format!("node n{index} \"{index}\"\n"));
        source.push_str(&format!("n{} -> n{index} \"flow\"\n", index - 1));
    }
    source.push_str("layout {\n  place n0 at (0, 0)\n");
    for index in 1..250 {
        source.push_str(&format!("  place n{index} right-of n{}\n", index - 1));
    }
    source.push_str("}\n");
    source
}

fn placement_chain(count: usize) -> String {
    let mut source = (0..count)
        .map(|index| format!("node n{index} {{ size: 10 10 }}\n"))
        .collect::<String>();
    source.push_str("layout {\n place n0 at (0, 0)\n row ");
    for index in 0..count {
        source.push_str(&format!("n{index} "));
    }
    source.push_str("gap 2\n}\n");
    source
}

fn ortho() -> String {
    let mut source = String::new();
    for row in 0..8 {
        for column in 0..8 {
            source.push_str(&format!("node n{row}_{column} \"\"\n"));
        }
    }
    source.push_str("layout {\n");
    for row in 0..8 {
        for column in 0..8 {
            source.push_str(&format!(
                "  place n{row}_{column} at ({}, {})\n",
                column * 130,
                row * 100
            ));
        }
    }
    source.push_str("}\n");
    for row in 0..8 {
        for column in 0..7 {
            source.push_str(&format!(
                "n{row}_{column} -> n{row}_{} {{ router: ortho }}\n",
                column + 1
            ));
        }
    }
    source
}

fn template() -> String {
    let mut source = String::from("type pie\n");
    for index in 0..200 {
        source.push_str(&format!("slice s{index} \"Slice {index}\" {index}\n"));
    }
    source
}

fn chart() -> &'static str {
    "type bar { title: \"Monthly revenue\" }\nseries north \"North\" 24 39 31 47 51 64 58 72 81 69 77 90\nseries south \"South\" 18 26 32 40 44 49 55 61 57 66 72 84\n"
}

fn frames(count: usize, nested: bool) -> String {
    if nested {
        (0..count / 50)
            .map(|chain| {
                let mut source = (0..50)
                    .map(|depth| format!("group g{chain}_{depth} {{\n padding: 1\n"))
                    .collect::<String>();
                source.push_str(&format!("node a{chain} {{ size: 20 20 }}\n"));
                source.push_str(&"}\n".repeat(50));
                source
            })
            .collect()
    } else {
        (0..count)
            .map(|index| {
                format!("group g{index} {{\n padding: 1\n node n{index} {{ size: 20 20 }}\n}}\n")
            })
            .collect()
    }
}

fn measure(name: &str, source: &str, runs: u32) -> Duration {
    let parse_start = Instant::now();
    let mut diagrams = Vec::new();
    for _ in 0..runs {
        diagrams.push(bgraph::parser::parse_diagram(source).expect("benchmark parses"));
    }
    let parse = parse_start.elapsed();
    let layout_start = Instant::now();
    let layouts: Vec<_> = diagrams
        .iter()
        .map(|diagram| bgraph::layout::solve(diagram).expect("benchmark lays out"))
        .collect();
    let layout = layout_start.elapsed();
    let validation_start = Instant::now();
    for (diagram, solved) in diagrams.iter().zip(&layouts) {
        std::hint::black_box(bgraph::check::after_layout(diagram, solved));
    }
    let validation = validation_start.elapsed();
    let render_start = Instant::now();
    for solved in &layouts {
        let _ = bgraph::render_svg::render(solved);
    }
    let render = render_start.elapsed();
    let total = parse + layout + validation + render;
    println!(
        "{name:12} runs={runs} parse={parse:?} layout={layout:?} layout_mean={:?} validation={validation:?} render={render:?} total={total:?} mean={:?}",
        layout / runs,
        total / runs
    );
    total
}

fn measure_public_source() -> Duration {
    let source = placement_chain(5_000);
    let start = Instant::now();
    for _ in 0..2 {
        std::hint::black_box(bgraph::render_source(&source).expect("public pipeline renders"));
    }
    let total = start.elapsed();
    println!("public-5000  runs=2 total={total:?} mean={:?}", total / 2);
    total
}

fn measure_composition() -> Duration {
    const RUNS: u32 = 10;
    let root = std::env::temp_dir().join(format!("bgraph-bench-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create composition fixture directory");
    let entry = root.join("entry.bd");
    std::fs::write(
        &entry,
        "include \"nodes.bd\"\napi -> worker \"request\" { router: ortho }\n",
    )
    .expect("write composition entry");
    std::fs::write(
        root.join("nodes.bd"),
        "node api \"API\"\nnode worker \"Worker\"\nlayout {\n  place api at (0, 0)\n  place worker right-of api gap 80\n}\n",
    )
    .expect("write composition member");

    let compose_start = Instant::now();
    let diagrams: Vec<_> = (0..RUNS)
        .map(|_| {
            bgraph::compose::load(&entry)
                .expect("compose benchmark loads")
                .diagram
        })
        .collect();
    let compose = compose_start.elapsed();
    let layout_start = Instant::now();
    let layouts: Vec<_> = diagrams
        .iter()
        .map(|diagram| bgraph::layout::solve(diagram).expect("compose benchmark lays out"))
        .collect();
    let layout = layout_start.elapsed();
    let render_start = Instant::now();
    for solved in &layouts {
        let _ = bgraph::render_svg::render(solved);
    }
    let render = render_start.elapsed();
    let total = compose + layout + render;
    println!("compose      parse={compose:?} layout={layout:?} render={render:?} total={total:?}");
    total
}

fn main() {
    let case_filter =
        std::env::args().find_map(|arg| arg.strip_prefix("--case=").map(str::to_owned));
    let cases = [
        ("large-graph", large_graph()),
        ("ortho", ortho()),
        ("template", template()),
        ("chart", chart().to_string()),
        ("frames-50", frames(50, false)),
        ("frames-100", frames(100, false)),
        ("nested-50", frames(50, true)),
        ("nested-100", frames(100, true)),
        ("chain-2000", placement_chain(2_000)),
        ("chain-20000", placement_chain(20_000)),
        ("frames-600", frames(600, false)),
        ("nested-1000", frames(1_000, true)),
    ];
    let mut totals: Vec<_> = cases
        .iter()
        .filter(|(name, _)| case_filter.as_ref().is_none_or(|filter| name == filter))
        .map(|(name, source)| {
            // The original cases retain their 5s total/10-run guard. Large-document
            // budgets are per invocation, measured twice to keep the CI gate short.
            let (runs, limit) = match *name {
                "chain-2000" | "frames-600" => (2, Duration::from_millis(500) * 2),
                "chain-20000" => (2, Duration::from_secs(3) * 2),
                "nested-1000" => (2, Duration::from_secs(2) * 2),
                _ => (10, Duration::from_secs(5)),
            };
            (*name, measure(name, source, runs), limit)
        })
        .collect();
    if case_filter
        .as_deref()
        .is_none_or(|filter| filter == "compose")
    {
        totals.push(("compose", measure_composition(), Duration::from_secs(5)));
    }
    if case_filter
        .as_deref()
        .is_none_or(|filter| filter == "public-5000")
    {
        totals.push((
            "public-5000",
            measure_public_source(),
            Duration::from_secs(4),
        ));
    }
    if std::env::args().any(|argument| argument == "--check") {
        for (name, total, limit) in totals {
            assert!(
                total <= limit,
                "{name} exceeded its regression budget: {total:?} > {limit:?}"
            );
        }
    }
}
