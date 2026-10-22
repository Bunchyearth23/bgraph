use bgraph::{Bindings, Template};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "bgraph-data-composition-{}-{}",
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

#[test]
#[cfg(all(feature = "json", feature = "pie"))]
fn a_missing_row_field_never_becomes_a_smaller_chart_or_literal_label() {
    let source = "type pie\nslice {{r.id}} \"{{r.label}}\" {{r.value}}\n";
    for missing in ["id", "label", "value"] {
        let mut second = serde_json::json!({"id":"south", "label":"South", "value":2});
        second.as_object_mut().unwrap().remove(missing);
        let json = serde_json::json!({"r":[{"id":"north", "label":"North", "value":1}, second]});
        let bindings = Bindings::from_json(&json.to_string()).unwrap();
        let template = Template::parse(source).unwrap();
        for error in [
            template.render(&bindings).unwrap_err(),
            template.fill_checked(&bindings).unwrap_err(),
        ] {
            let error = error.to_string();
            assert!(error.contains(&format!("r.{missing}")), "{error}");
            assert!(error.contains("row 2"), "{error}");
        }
        assert!(
            template
                .fill(&bindings)
                .unwrap()
                .contains(&format!("{{{{r.{missing}}}}}"))
        );
    }
    let empty = Bindings::new().rows("r", vec![]);
    assert!(Template::parse(source).unwrap().render(&empty).is_ok());
}

#[test]
fn holes_follow_the_complete_statement_context() {
    let values = Bindings::new()
        .set("id", "b")
        .set("anchor", "a")
        .set("label", "Two words")
        .set("x", 200.0)
        .set("y", 120.0);
    for source in [
        "node a\nnode {{id}} {{label}}\nlayout { place {{id}} at ({{x}}, {{y}}) }",
        "node a\ngroup g { node {{id}} {{label}} }\nlayout { place g right-of {{anchor}} }",
        "node a\nnode {{id}} {{label}}\nlayout { place {{id}} right-of {{anchor}} }",
        "node a\nnode {{id}} {{label}}\nlayout { row {{anchor}} {{id}} gap 40 }",
        "node a\nnode {{id}} {{label}}\nlayout { align top {{anchor}} {{id}} }",
        "node a\nnode {{id}} {{label}}\nnote \"caption\" below {{id}}",
    ] {
        let template = Template::parse(source).unwrap_or_else(|e| panic!("{source}: {e:#}"));
        let svg = template
            .render(&values)
            .unwrap_or_else(|e| panic!("{source}: {e:#}"));
        assert!(svg.contains("Two words"), "{source}\n{svg}");
    }
    for arrow in ["<-", "->", "-->", "..>", "<->", "--", ".."] {
        let source = format!("node a\nnode b\n{{{{anchor}}}}{arrow}{{{{id}}}} {{{{label}}}}");
        let svg = Template::parse(&source).unwrap().render(&values).unwrap();
        assert!(svg.contains("Two words"), "{arrow}: {svg}");
    }
}

#[test]
#[cfg(feature = "pie")]
fn nested_rows_cannot_hide_behind_a_present_required_field() {
    let template =
        Template::parse("type pie\nslice {{r.id}} \"{{r.label}}\" {{r.value}}\n").unwrap();
    for invalid in [
        bgraph::Value::Rows(vec![]),
        bgraph::Value::List(vec![bgraph::Value::Rows(vec![])]),
    ] {
        let row = Bindings::new()
            .set("id", "a")
            .set("label", "A")
            .set("value", invalid);
        let bindings = Bindings::new().rows("r", vec![row]);
        for error in [
            template.fill_checked(&bindings).unwrap_err(),
            template.render(&bindings).unwrap_err(),
        ] {
            let message = error.to_string();
            assert!(
                message.contains("r.value")
                    && message.contains("row 1")
                    && message.contains("nested rows"),
                "{message}"
            );
        }
    }
}

#[test]
fn qualified_bound_names_render_and_hostile_names_still_fail() {
    let template = Template::parse("node {{id}} {{label}}").unwrap();
    let values = Bindings::new()
        .set("id", "part.item")
        .set("label", "Qualified");
    assert!(template.render(&values).unwrap().contains("Qualified"));
    let values = Bindings::new()
        .set("id", "a; node injected")
        .set("label", "Ignored");
    assert!(
        template
            .render(&values)
            .unwrap_err()
            .to_string()
            .contains("cannot be a name")
    );
}

#[test]
fn class_and_include_prefix_holes_remain_identifier_positions() {
    let files = Files::new();
    files.write("child.bd", "node child \"Included\"\n");
    let source =
        "style accent { fill: #123456 }\nnode a .{{class}}\ninclude \"child.bd\" as {{prefix}}\n";
    let template = Template::parse(source).unwrap();
    for class in [
        bgraph::Value::Text("accent".into()),
        bgraph::Value::Raw("accent".into()),
    ] {
        let values = Bindings::new().set("class", class).set("prefix", "part");
        let filled = template.fill_checked(&values).unwrap();
        let path = files.write("main.bd", &filled);
        let svg = bgraph::render_file(&path).unwrap();
        assert!(svg.contains("#123456") && svg.contains("Included"), "{svg}");
        assert!(
            bgraph::compose::load(&path)
                .unwrap()
                .diagram
                .nodes
                .iter()
                .any(|node| node.id.as_str() == "part.child")
        );
    }
}

const ANNOTATED: &str = "style highlighted { fill: #abcdef legend: \"Important\" }\nnode a .highlighted\nnode b\nnode middle\nlayout { place a at (0, 0); place b at (300, 0); place middle at (150, 100) }\na -> b via (below middle gap 40)\nnote \"Caption\" .highlighted below a gap 100 { font-size: 14 }\nlegend .highlighted below b gap 100 { font-size: 14 }\n";

fn assert_annotations_render(path: &Path, prefix: &str) {
    let composed = bgraph::compose::load(path).unwrap();
    let diagram = &composed.diagram;
    assert_eq!(diagram.notes.len(), 1);
    assert_eq!(diagram.legends.len(), 1);
    for (classes, placement, target) in [
        (&diagram.notes[0].classes, &diagram.notes[0].placement, "a"),
        (
            &diagram.legends[0].classes,
            &diagram.legends[0].placement,
            "b",
        ),
    ] {
        assert_eq!(classes[0].as_str(), format!("{prefix}.highlighted"));
        let Some(bgraph::ast::Placement::Rel { anchor, .. }) = placement else {
            panic!()
        };
        assert_eq!(anchor.as_str(), format!("{prefix}.{target}"));
    }
    let bgraph::ast::Placement::Rel { anchor, .. } = &diagram.edges[0].via[0] else {
        panic!()
    };
    assert_eq!(anchor.as_str(), format!("{prefix}.middle"));
    let svg = bgraph::render_file(path).unwrap();
    assert!(
        svg.contains("Caption") && svg.contains("Important") && svg.contains("#abcdef"),
        "{svg}"
    );
}

#[test]
fn component_notes_legends_and_waypoints_survive_expansion() {
    let files = Files::new();
    let source = format!("component card {{\n{ANNOTATED}}}\nuse card as one\n");
    let path = files.write("component.bd", &source);
    assert_annotations_render(&path, "one");
    let composed = bgraph::compose::load(&path).unwrap();
    let use_offset = source.find("use card").unwrap();
    let diagram = &composed.diagram;
    for (span, classes, props, placement) in diagram
        .notes
        .iter()
        .map(|n| (n.span, &n.classes, &n.props, &n.placement))
        .chain(
            diagram
                .legends
                .iter()
                .map(|l| (l.span, &l.classes, &l.props, &l.placement)),
        )
    {
        assert_eq!(span.start, use_offset);
        assert_eq!(classes[0].span.start, use_offset);
        for prop in props {
            assert_eq!(prop.span.start, use_offset);
            assert_eq!(prop.name_span.start, use_offset);
            assert_eq!(prop.values_span.start, use_offset);
        }
        let Some(bgraph::ast::Placement::Rel { anchor, .. }) = placement else {
            panic!()
        };
        assert_eq!(anchor.span.start, use_offset);
    }
    let bgraph::ast::Placement::Rel { anchor, .. } = &diagram.edges[0].via[0] else {
        panic!()
    };
    assert_eq!(anchor.span.start, use_offset);
}

#[test]
fn prefixed_includes_keep_annotation_references_and_source_locations() {
    let files = Files::new();
    files.write("first.bd", "node unrelated\n");
    let child = files.write("child.bd", ANNOTATED);
    let entry = files.write(
        "main.bd",
        "include \"first.bd\"\ninclude \"child.bd\" as module\n",
    );
    assert_annotations_render(&entry, "module");
    let composed = bgraph::compose::load(&entry).unwrap();
    let d = &composed.diagram;
    for (span, classes, props, placement, declaration) in [
        (
            d.notes[0].span,
            &d.notes[0].classes,
            &d.notes[0].props,
            &d.notes[0].placement,
            "note \"Caption\"",
        ),
        (
            d.legends[0].span,
            &d.legends[0].classes,
            &d.legends[0].props,
            &d.legends[0].placement,
            "legend .",
        ),
    ] {
        let offset = ANNOTATED.find(declaration).unwrap();
        assert_eq!(
            composed.map.locate(span.start),
            (
                child.as_path(),
                ANNOTATED[..offset].matches('\n').count(),
                0
            )
        );
        assert_eq!(composed.map.locate(classes[0].span.start).0, child);
        let span = classes[0].span;
        assert_eq!(&composed.map.text[span.start..span.end], "highlighted");
        for prop in props {
            assert_eq!(
                &composed.map.text[prop.name_span.start..prop.name_span.end],
                "font-size"
            );
            assert_eq!(
                &composed.map.text[prop.values_span.start..prop.values_span.end],
                "14"
            );
        }
        let Some(bgraph::ast::Placement::Rel { anchor, .. }) = placement else {
            panic!()
        };
        assert_eq!(composed.map.locate(anchor.span.start).0, child);
        assert!(matches!(
            &composed.map.text[anchor.span.start..anchor.span.end],
            "a" | "b"
        ));
    }
    let bgraph::ast::Placement::Rel { anchor, .. } = &d.edges[0].via[0] else {
        panic!()
    };
    assert_eq!(composed.map.locate(anchor.span.start).0, child);
    assert_eq!(
        &composed.map.text[anchor.span.start..anchor.span.end],
        "middle"
    );
}

#[test]
fn unsupported_component_declarations_fail_at_the_use_instead_of_disappearing() {
    for body in [
        "component inner { node a }\nuse inner as nested",
        "use another as nested",
        "include \"file.bd\"",
        "type pie",
        "graph { padding: 10 }",
    ] {
        let source = format!("component outer {{\n{body}\n}}\nuse outer as one\n");
        let analysis = bgraph::analyze_source(&source);
        let errors: Vec<_> = analysis
            .diagnostics
            .iter()
            .filter(|d| d.severity == bgraph::check::Severity::Error)
            .collect();
        assert!(!errors.is_empty(), "{source}");
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("not supported inside a component")),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .all(|e| e.span.start == source.find("use outer").unwrap()),
            "{errors:?}"
        );
        assert!(bgraph::render_source(&source).is_err());
    }
}

#[test]
fn component_annotations_need_relative_anchors_instead_of_disappearing_or_overlapping() {
    for annotation in [
        "note \"Caption\"",
        "legend",
        "note \"Caption\" at (10, 20)",
        "legend at (10, 20)",
        "note \"Caption\" at (col 1, row 2)",
    ] {
        let source = format!(
            "component card {{ node a; {annotation} }}\nuse card as one\nuse card as two\nlayout {{ place one at (100, 100); place two at (500, 100) }}\n"
        );
        let error = bgraph::render_source(&source).unwrap_err().to_string();
        assert!(
            error.contains("need a relative placement") && error.contains("anchor it"),
            "{error}"
        );
    }
}

#[test]
#[cfg(all(feature = "csv", feature = "pie"))]
fn both_csv_entry_points_render_the_same_quoted_fields() {
    let csv =
        "id,name,value\r\np,\"Paid, all \"\"networks\"\"\",410\r\no,\"Organic\nsearch\",4820\r\n";
    let files = Files::new();
    files.write("data.csv", csv);
    let entry = files.write("main.bd", "type pie { from: \"data.csv\" }\n");
    let direct = bgraph::render_file(&entry).unwrap();
    let bound = bgraph::template::rows_from_csv("r", csv).unwrap();
    let template =
        Template::parse("type pie\nslice {{r.id}} \"{{r.name}}\" {{r.value}}\n").unwrap();
    let svg = template.render(&bound).unwrap();
    for label in ["Paid, all &quot;networks&quot;", "Organic", "search"] {
        assert!(svg.contains(label), "{svg}");
        assert!(direct.contains(label), "{direct}");
    }
}

#[test]
#[cfg(all(feature = "csv", feature = "pie"))]
fn both_csv_entry_points_reject_duplicate_columns_and_malformed_rows() {
    let files = Files::new();
    let entry = files.write("main.bd", "type pie { from: \"data.csv\" }\n");
    for csv in [
        "name,value,value\nA,1,2\n",
        "name,value\nA\n",
        "name,value\nA,1,2\n",
        "name,value\n\"A,1\n",
        "name,value\n\"A\"junk,1\n",
        "name,value\nA\"oops,1\n",
    ] {
        files.write("data.csv", csv);
        assert!(bgraph::template::rows_from_csv("r", csv).is_err(), "{csv}");
        let error = bgraph::render_file(&entry).unwrap_err().to_string();
        assert!(error.contains("data.csv"), "{csv}: {error}");
    }
}
