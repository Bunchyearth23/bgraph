use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::check::Severity;

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "bgraph-multifile-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, source).unwrap();
        path
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn errors(analysis: &crate::FileAnalysis) -> Vec<(&Path, usize, &crate::check::Diag)> {
    analysis
        .analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .map(|diagnostic| {
            let (path, line, _) = analysis.map.locate(diagnostic.span.start);
            (path, line + 1, diagnostic)
        })
        .collect()
}

#[test]
fn syntax_and_semantic_errors_keep_every_readable_file_location() {
    let files = Files::new();
    let first = files.write("first.bd", "node broken @\nnode a { shape: unknown }\n");
    let second = files.write("second.bd", "node b { mystery: 1 }\n");
    let entry = files.write(
        "main.bd",
        "include \"first.bd\" as first\ninclude \"second.bd\" as second\nnode root { another: 2 }\n",
    );
    let analysis = crate::analyze_file(&entry).unwrap();
    let reported = errors(&analysis);
    assert!(reported.iter().any(|(path, line, diagnostic)| {
        *path == first && *line == 1 && diagnostic.message.contains("syntax error")
    }));
    assert!(reported.iter().any(|(path, line, diagnostic)| {
        *path == first && *line == 2 && diagnostic.message.contains("unknown")
    }));
    assert!(reported.iter().any(|(path, line, diagnostic)| {
        *path == second && *line == 1 && diagnostic.message.contains("mystery")
    }));
    assert!(reported.iter().any(|(path, line, diagnostic)| {
        *path == entry && *line == 3 && diagnostic.message.contains("another")
    }));
    let render = crate::render_file(&entry).unwrap_err().to_string();
    let preview = crate::watch::preview_page(&entry);
    for (path, line, diagnostic) in reported {
        let location = format!("{}:{line}:", path.display());
        assert!(render.contains(&location), "{render}");
        assert!(render.contains(&diagnostic.message), "{render}");
        assert!(preview.contains(&location), "{preview}");
    }
}

#[test]
fn missing_nested_include_is_reported_on_its_own_include_statement() {
    let files = Files::new();
    let child = files.write(
        "child.bd",
        "node a\ninclude \"absent.bd\"\nnode broken @\nnode b { strange: 1 }\n",
    );
    let entry = files.write("main.bd", "node root\ninclude \"child.bd\" as child\n");
    let analysis = crate::analyze_file(&entry).unwrap();
    let reported = errors(&analysis);
    for expected in [2, 3, 4] {
        assert!(
            reported
                .iter()
                .any(|(path, line, _)| *path == child && *line == expected)
        );
    }
    let relays =
        crate::lsp::diagnostics_for(&std::fs::read_to_string(&entry).unwrap(), Some(&entry));
    assert_eq!(relays.len(), reported.len());
    for relay in relays {
        assert_eq!(relay["range"]["start"]["line"], 1);
        assert!(
            relay["relatedInformation"][0]["location"]["uri"]
                .as_str()
                .unwrap()
                .ends_with("/child.bd")
        );
    }
}

#[test]
fn component_failures_do_not_hide_the_next_invalid_instance() {
    let files = Files::new();
    let child = files.write(
        "child.bd",
        "component box {\nnode n\n}\nuse box as valid\nuse unknown as first\nuse unknown as second\n",
    );
    let entry = files.write("main.bd", "node root\ninclude \"child.bd\" as child\n");
    let analysis = crate::analyze_file(&entry).unwrap();
    let reported = errors(&analysis);
    assert_eq!(reported.len(), 2);
    assert_eq!((reported[0].0, reported[0].1), (child.as_path(), 5));
    assert_eq!((reported[1].0, reported[1].1), (child.as_path(), 6));
    assert!(
        analysis
            .analysis
            .diagram
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|node| { node.id.as_str() == "child.valid.n" })
    );
}

#[test]
fn a_cross_file_conflict_keeps_both_constraint_locations() {
    let files = Files::new();
    let child = files.write("child.bd", "node a\nlayout { place a at (0, 0) }\n");
    let entry = files.write(
        "main.bd",
        "include \"child.bd\" as child\nnode b\nlayout {\nplace b at (0, 80)\nalign top child.a b\n}\n",
    );
    let analysis = crate::analyze_file(&entry).unwrap();
    let reported = errors(&analysis);
    assert!(
        reported
            .iter()
            .any(|(path, line, _)| *path == child && *line == 2)
    );
    assert!(
        reported
            .iter()
            .any(|(path, line, _)| *path == entry && *line == 4)
    );
    assert!(
        reported
            .iter()
            .any(|(path, line, _)| *path == entry && *line == 5)
    );
    let render = crate::render_file(&entry).unwrap_err().to_string();
    for (_, _, diagnostic) in reported {
        assert!(render.contains(&diagnostic.message), "{render}");
    }
}

#[test]
fn lsp_relays_each_child_at_the_right_include_and_clears_after_correction() {
    let files = Files::new();
    files.write("first.bd", "node a { strange: 1 }\n");
    files.write("second.bd", "node b { other: 1 }\n");
    let text = "include \"first.bd\" as first\nnode root\ninclude \"second.bd\" as second\n";
    let entry = files.write("main.bd", text);
    let reported = crate::lsp::diagnostics_for(text, Some(&entry));
    assert_eq!(reported.len(), 2);
    for (diagnostic, line, suffix) in [
        (&reported[0], 0, "/first.bd"),
        (&reported[1], 2, "/second.bd"),
    ] {
        assert_eq!(diagnostic["range"]["start"]["line"], line);
        assert_eq!(diagnostic["range"]["start"]["character"], 0);
        let child = &diagnostic["relatedInformation"][0];
        assert!(child["location"]["uri"].as_str().unwrap().ends_with(suffix));
        assert_eq!(child["location"]["range"]["start"]["line"], 0);
        assert!(
            child["location"]["range"]["start"]["character"]
                .as_u64()
                .unwrap()
                > 0
        );
    }
    files.write("first.bd", "node a\n");
    files.write("second.bd", "node b\n");
    assert!(crate::lsp::diagnostics_for(text, Some(&entry)).is_empty());
}

#[test]
fn repeated_includes_relay_to_the_instance_that_produced_each_error() {
    let files = Files::new();
    files.write("child.bd", "node a { strange: 1 }\n");
    let text = "include \"child.bd\" as first\nnode root\ninclude \"child.bd\" as second\n";
    let entry = files.write("main.bd", text);
    let reported = crate::lsp::diagnostics_for(text, Some(&entry));
    assert_eq!(reported.len(), 2);
    assert_eq!(reported[0]["range"]["start"]["line"], 0);
    assert_eq!(reported[1]["range"]["start"]["line"], 2);
}

#[test]
fn included_unicode_columns_use_scalars_for_cli_and_utf16_for_lsp() {
    let files = Files::new();
    files.write("child.bd", "node a \"🦀\" { mystery: 1 }\n");
    let text = "include \"child.bd\"\n";
    let entry = files.write("main.bd", text);
    let analysis = crate::analyze_file(&entry).unwrap();
    let diagnostic = &analysis.analysis.diagnostics[0];
    let (_, line, scalar_column) = analysis.map.locate(diagnostic.span.start);
    assert_eq!((line, scalar_column), (0, 13));
    let relay = crate::lsp::diagnostics_for(text, Some(&entry));
    let child = &relay[0]["relatedInformation"][0];
    assert_eq!(child["location"]["range"]["start"]["line"], 0);
    assert_eq!(child["location"]["range"]["start"]["character"], 14);
}

#[test]
fn invalid_style_file_statements_keep_their_actual_line() {
    let files = Files::new();
    let entry = files.write("main.bd", "include \"styles.bds\"\nnode root\n");
    for statement in [
        "a -> b",
        "group g { node a }",
        "type pie",
        "graph { padding: 10 }",
    ] {
        let child = files.write("styles.bds", &format!("// header\n{statement}\n"));
        let analysis = crate::analyze_file(&entry).unwrap();
        let reported = errors(&analysis);
        let invalid = reported
            .iter()
            .find(|(_, _, diagnostic)| {
                diagnostic
                    .message
                    .contains("holds `style` declarations only")
            })
            .unwrap();
        assert_eq!((invalid.0, invalid.1), (child.as_path(), 2));
    }
}
