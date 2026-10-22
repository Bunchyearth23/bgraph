#![cfg(feature = "cli")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "bgraph-export-audit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, source).unwrap();
        path
    }
    fn render(&self, source: &Path, output: &Path, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_bgraph"))
            .arg(source)
            .arg("-o")
            .arg(output)
            .args(args)
            .output()
            .unwrap()
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn exporting_cannot_overwrite_an_input_or_include() {
    let files = Files::new();
    let source = files.write("main.bd", "include \"styles.bds\"\nnode a \"Keep me\"\n");
    let style = files.write("styles.bds", "style node { fill: #ff0000 }\n");
    for output in [&source, &style] {
        let before = fs::read(output).unwrap();
        let result = files.render(&source, output, &[]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("refusing to overwrite input"));
        assert_eq!(fs::read(output).unwrap(), before);
    }
    #[cfg(unix)]
    {
        let alias = files.0.join("alias.svg");
        std::os::unix::fs::symlink(&source, &alias).unwrap();
        assert!(!files.render(&source, &alias, &[]).status.success());
        fs::remove_file(&alias).unwrap();
        fs::hard_link(&source, &alias).unwrap();
        assert!(!files.render(&source, &alias, &[]).status.success());
        assert!(fs::read_to_string(&source).unwrap().starts_with("include"));
    }
}

#[test]
fn filled_templates_use_the_requested_writer_and_options() {
    let files = Files::new();
    let source = files.write("template.bd", "node a \"{{label}}\"\n");
    let formats: Vec<(&str, &[u8])> = vec![
        ("svg", b"<svg"),
        ("json", b"{"),
        #[cfg(feature = "pdf")]
        ("pdf", b"%PDF-"),
        #[cfg(feature = "png")]
        ("png", b"\x89PNG\r\n\x1a\n"),
    ];
    for (extension, prefix) in formats {
        let output = files.0.join(format!("output.{extension}"));
        let result = files.render(&source, &output, &["--set", "label=Bound"]);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            fs::read(&output).unwrap().starts_with(prefix),
            "{extension}"
        );
    }
    for args in [
        vec!["--set", "label=Bound", "--theme", "missing"],
        vec!["--set", "label=Bound", "--embed-font", "missing.ttf"],
        vec!["--set", "label=Bound", "--page", "a4"],
    ] {
        let output = files.0.join("rejected.svg");
        assert!(!files.render(&source, &output, &args).status.success());
        assert!(!output.exists());
    }
    let plain = files.write("plain.bd", "node a \"Bound\"\n");
    let template_out = files.0.join("template-dark.svg");
    let plain_out = files.0.join("plain-dark.svg");
    assert!(
        files
            .render(
                &source,
                &template_out,
                &["--set", "label=Bound", "--theme", "dark"]
            )
            .status
            .success()
    );
    assert!(
        files
            .render(&plain, &plain_out, &["--theme", "dark"])
            .status
            .success()
    );
    assert_eq!(
        fs::read(template_out).unwrap(),
        fs::read(plain_out).unwrap()
    );
}

#[cfg(all(feature = "csv", feature = "pie"))]
#[test]
fn exporting_cannot_overwrite_external_chart_data() {
    let files = Files::new();
    let source = files.write("main.bd", "type pie { from: \"data.csv\" }\n");
    let data = files.write("data.csv", "name,value\nA,5\n");
    assert!(!files.render(&source, &data, &[]).status.success());
    assert_eq!(fs::read_to_string(data).unwrap(), "name,value\nA,5\n");
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_rejects_missing_characters_and_invalid_page_geometry() {
    use bgraph::render_pdf::{Page, render_paged};
    let error = bgraph::render_source_pdf("node a \"東京 → Москва\"\n")
        .unwrap_err()
        .to_string();
    assert!(error.contains("cannot represent"), "{error}");
    assert!(error.contains("embed-font"), "{error}");
    for size in ["nanx100", "infx100", "0x100", "-1x100", "24x100"] {
        assert!(Page::parse(size).is_none(), "{size}");
    }
    let layout = bgraph::analyze_source("node a\n").into_layout().unwrap();
    for page in [
        Page {
            width: f64::NAN,
            ..Page::A4
        },
        Page {
            overlap: -1.0,
            ..Page::A4
        },
        Page {
            width: 25.0,
            height: 25.0,
            overlap: 24.999999,
        },
    ] {
        assert!(render_paged(&layout, page).is_err());
    }
    let (_, _, width, height) = bgraph::render_svg::canvas(&layout);
    let bytes = render_paged(
        &layout,
        Page {
            width,
            height,
            overlap: 24.0,
        },
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("/Count 1"));
}

#[cfg(unix)]
#[test]
fn directory_exports_preserve_sources_through_links() {
    let files = Files::new();
    let source = files.write("source.bd", "node a \"Keep me\"\n");
    let output = files.0.join("source.svg");
    for symbolic in [true, false] {
        if symbolic {
            std::os::unix::fs::symlink(&source, &output).unwrap();
        } else {
            fs::hard_link(&source, &output).unwrap();
        }
        assert!(bgraph::watch::build_dir(&files.0, None, true).is_err());
        assert_eq!(fs::read_to_string(&source).unwrap(), "node a \"Keep me\"\n");
        fs::remove_file(&output).unwrap();
    }
}

#[test]
fn replacement_preserves_other_hard_links_and_failed_writes_preserve_the_target() {
    let files = Files::new();
    let input = files.write("original.bd", "node keep\n");
    let output = files.0.join("output.svg");
    fs::hard_link(&input, &output).unwrap();
    bgraph::output::write(&output, b"new export").unwrap();
    assert_eq!(fs::read_to_string(input).unwrap(), "node keep\n");
    assert_eq!(fs::read_to_string(output).unwrap(), "new export");
    let directory = files.0.join("directory.svg");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("keep"), b"keep").unwrap();
    assert!(bgraph::output::write(&directory, b"export").is_err());
    assert_eq!(fs::read(directory.join("keep")).unwrap(), b"keep");
    assert!(!fs::read_dir(&files.0).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".bgraph-export-")
    }));
}

#[cfg(feature = "pdf")]
#[test]
fn pdf_rejection_leaves_an_existing_export_unchanged() {
    let files = Files::new();
    let source = files.write("unicode.bd", "node a \"東京\"\n");
    let output = files.write("output.pdf", "previous export");
    assert!(!files.render(&source, &output, &[]).status.success());
    assert_eq!(fs::read_to_string(output).unwrap(), "previous export");
    let mut layout = bgraph::analyze_source("node a\nnode b\na -> b \"label\"\n")
        .into_layout()
        .unwrap();
    layout.edges[0].label = Some("first\r\nsecond".into());
    assert!(
        bgraph::render_pdf::render(&layout).is_err(),
        "every emitted character is validated"
    );
}
