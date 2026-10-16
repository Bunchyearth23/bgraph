use crate::{analyze_source, check::Severity, render_source};

#[test]
fn rendering_preserves_every_fatal_analysis_diagnostic() {
    let source = "node a { shape: unknown }\nnode b\na -> missing\n";
    let analysis = analyze_source(source);
    let errors: Vec<_> = analysis
        .diagnostics
        .iter()
        .filter(|diag| diag.severity == Severity::Error)
        .collect();
    assert!(errors.len() >= 2);
    let rendered = render_source(source)
        .expect_err("invalid source")
        .to_string();
    for diagnostic in errors {
        assert!(rendered.contains(&diagnostic.message), "{rendered}");
    }
}

#[test]
fn rendering_preserves_recovered_syntax_and_semantic_errors() {
    let source = "node a @\nnode b { shape: unknown }\n";
    let analysis = analyze_source(source);
    assert!(analysis.has_errors());
    let error = render_source(source)
        .expect_err("invalid source")
        .to_string();
    for diagnostic in analysis.diagnostics {
        if diagnostic.severity == Severity::Error {
            assert!(error.contains(&diagnostic.message), "{error}");
        }
    }
}

#[test]
fn source_analysis_expands_components_without_file_access() {
    let source = "component service {\nnode app \"{{label}}\"\nnode db \"Database\"\nlayout { place db below app gap 30 }\n}\nuse service as web { label: \"API\" }\n";
    let analysis = analyze_source(source);
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    let layout = analysis.layout.as_ref().expect("component geometry");
    assert!(layout.nodes.iter().any(|node| node.id == "web.app"));
    assert!(layout.nodes.iter().any(|node| node.id == "web.db"));
    assert!(layout.groups.iter().any(|group| group.id == "web"));
    assert_eq!(
        render_source(source).unwrap(),
        crate::render_svg::render(layout)
    );
}

#[test]
fn unknown_components_produce_fatal_analysis_diagnostics() {
    let source = "use unknown as service\n";
    let analysis = analyze_source(source);
    assert!(analysis.has_errors());
    assert!(analysis.layout.is_none());
    assert!(
        analysis.diagnostics[0]
            .message
            .contains("unknown component")
    );
    assert!(render_source(source).is_err());
}

#[test]
fn component_expansion_errors_point_to_the_failing_use() {
    for failing in [
        "use unknown as broken",
        "use box as broken { id: \"not a name!\" }",
    ] {
        let source = format!(
            "component box {{\nnode {{{{id}}}}\n}}\nuse box as valid {{ id: first }}\n{failing}\n"
        );
        let analysis = analyze_source(&source);
        let errors: Vec<_> = analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == Severity::Error)
            .collect();
        assert_eq!(errors.len(), 1, "{failing}: {:?}", analysis.diagnostics);
        let diagnostic = errors[0];
        assert_eq!(
            &source[diagnostic.span.start..diagnostic.span.end],
            failing,
            "{:?}",
            diagnostic
        );
        assert_eq!(
            render_source(&source).unwrap_err().to_string(),
            diagnostic.message
        );
        let indexed = crate::lsp::diagnostics_for(
            &source,
            Some(std::path::Path::new("/project/components.bd")),
        );
        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0]["message"], diagnostic.message);
        assert_eq!(indexed[0]["range"]["start"]["line"], 4);
        let mut diagram = crate::parser::parse(&source).unwrap();
        let errors = crate::compose::expand(&mut diagram);
        let error = &errors[0];
        assert_eq!(error.span, diagnostic.span);
        assert_eq!(error.message, diagnostic.message);
    }
}

#[test]
fn text_surfaces_reject_includes_with_the_same_diagnostic() {
    let source = "include \"unavailable.bd\"\nnode a\n";
    let analysis = analyze_source(source);
    assert!(analysis.has_errors());
    assert!(analysis.layout.is_none());
    let message = &analysis.diagnostics[0].message;
    assert!(message.contains("render_file"));
    assert_eq!(render_source(source).unwrap_err().to_string(), *message);
    #[cfg(feature = "pdf")]
    assert_eq!(
        crate::render_source_pdf(source).unwrap_err().to_string(),
        *message
    );
}

#[test]
fn geometry_warnings_reach_analysis_and_allow_rendering() {
    let source = "node a { size: 80 40 }\nnode b { size: 80 40 }\nlayout {\nplace a at (0, 0)\nplace b at (20, 10)\n}\n";
    let analysis = analyze_source(source);
    assert!(!analysis.has_errors(), "{:?}", analysis.diagnostics);
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diag| { diag.severity == Severity::Warning && diag.message.contains("overlap") })
    );
    assert!(render_source(source).is_ok());
}
