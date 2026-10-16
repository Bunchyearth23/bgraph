use crate::{check, examples, layout, parser, props, style};

#[test]
fn geometry_contract_classifies_all_thirty_genres_without_dispatch_fallback() {
    let mut declared = Vec::new();
    let mut managed = 0;
    assert_eq!(props::ARCHETYPES.len(), 30);
    for def in props::ARCHETYPES {
        match def.layout {
            props::LayoutStrategy::Declared => {
                declared.push(def.name);
                assert!(def.layout.solver().is_none());
                assert_eq!(def.layout.statements(), props::LAYOUT_STATEMENTS);
            }
            props::LayoutStrategy::ArchetypeManaged(_) => {
                managed += 1;
                assert!(def.layout.solver().is_some());
                assert!(def.layout.statements().is_empty());
            }
            props::LayoutStrategy::Hybrid { .. } => {
                panic!("{} has no implemented named guide contract", def.name);
            }
        }
        assert!(def.layout.guides().is_empty(), "{}", def.name);
    }
    declared.sort_unstable();
    assert_eq!(
        declared,
        [
            "block",
            "bpmn",
            "class",
            "er",
            "free",
            "harness",
            "network",
            "requirements",
            "state"
        ]
    );
    assert_eq!(managed, 21);
}

#[test]
fn managed_genres_reject_every_layout_form_and_notes_in_shared_analysis() {
    for def in props::ARCHETYPES
        .iter()
        .filter(|def| !def.layout.accepts_layout_block())
    {
        for statement in [
            "",
            "place a at (12, 34)",
            "place a right-of b gap 17",
            "align top a b",
            "equal-width a b",
            "equal-height a b",
            "row a b gap 17",
            "column a b gap 17",
        ] {
            for local in [false, true] {
                let body = format!("node a\nnode b\nlayout {{ {statement} }}");
                let source = if local {
                    format!("type {}\ngroup g {{\n{body}\n}}", def.name)
                } else {
                    format!("type {}\n{body}", def.name)
                };
                let diagram = parser::parse(&source).unwrap();
                let analysis = crate::analyze_source(&source);
                assert!(analysis.layout.is_none(), "{}: {statement}", def.name);
                assert!(
                    analysis.diagnostics.iter().any(|diag| {
                        diag.severity == check::Severity::Error
                            && diag.span == diagram.layouts[0].span
                            && diag.message.contains("cannot use a `layout` block")
                    }),
                    "{} local={local}: {statement}: {:?}",
                    def.name,
                    analysis.diagnostics
                );
                assert!(
                    crate::render_source(&source)
                        .unwrap_err()
                        .to_string()
                        .contains("cannot use a `layout` block")
                );
            }
        }
        for placement in ["", " at (12, 34)", " below a gap 17"] {
            let source = format!("type {}\nnode a\nnote \"annotation\"{placement}", def.name);
            let diagram = parser::parse(&source).unwrap();
            let analysis = crate::analyze_source(&source);
            assert!(analysis.layout.is_none());
            assert!(
                analysis.diagnostics.iter().any(|diag| {
                    diag.severity == check::Severity::Error
                        && diag.span == diagram.notes[0].span
                        && diag.message.contains("cannot render a `note` statement")
                }),
                "{}: {:?}",
                def.name,
                analysis.diagnostics
            );
        }
    }
}

#[test]
fn declared_genres_honour_coordinates_relations_sizes_and_notes() {
    for def in props::ARCHETYPES
        .iter()
        .filter(|def| def.layout.accepts_layout_block())
    {
        let source = format!(
            "type {}\nnode a \"Longer label\"\nnode b \"B\"\nlayout {{\n place a at (73, 41)\n place b right-of a gap 17\n equal-width a b\n align top a b\n}}\nnote \"annotation\" below b gap 13",
            def.name,
        );
        let analysis = crate::analyze_source(&source);
        assert!(
            !analysis.has_errors(),
            "{}: {:?}",
            def.name,
            analysis.diagnostics
        );
        let drawing = analysis.layout.unwrap();
        let a = drawing
            .nodes
            .iter()
            .find(|node| node.id == "a")
            .unwrap()
            .rect;
        let b = drawing
            .nodes
            .iter()
            .find(|node| node.id == "b")
            .unwrap()
            .rect;
        assert!((a.x - 73.0).abs() < 1e-8, "{}: {a:?}", def.name);
        assert!((a.y - 41.0).abs() < 1e-8, "{}: {a:?}", def.name);
        assert!((b.x - a.right() - 17.0).abs() < 1e-8, "{}", def.name);
        assert!((a.y - b.y).abs() < 1e-8, "{}", def.name);
        assert!((a.w - b.w).abs() < 1e-8, "{}", def.name);
        assert!(
            drawing
                .annotations
                .iter()
                .any(|note| note.text == "annotation"),
            "{}",
            def.name
        );
    }
}

#[test]
fn each_managed_callback_is_the_dispatch_used_by_its_worked_example() {
    for def in props::ARCHETYPES {
        let Some(solve) = def.layout.solver() else {
            continue;
        };
        let source = examples::EXAMPLES
            .iter()
            .find(|(name, _)| *name == def.name)
            .unwrap()
            .1;
        let diagram = parser::parse(source).unwrap();
        let diagnostics = check::check(&diagram);
        assert!(
            !diagnostics
                .iter()
                .any(|diag| diag.severity == check::Severity::Error),
            "{}: {diagnostics:?}",
            def.name
        );
        let opts = style::GraphOpts::from_props(&diagram.graph).unwrap();
        let registry = style::Registry::build(&diagram.styles).unwrap();
        let direct = solve(&diagram, opts, &registry).unwrap();
        let dispatched = layout::solve(&diagram).unwrap();
        let boxes = |drawing: &layout::Layout| {
            drawing
                .nodes
                .iter()
                .map(|node| {
                    (
                        node.id.clone(),
                        node.rect.x,
                        node.rect.y,
                        node.rect.w,
                        node.rect.h,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(boxes(&direct), boxes(&dispatched), "{}", def.name);
        assert_eq!(direct.groups.len(), dispatched.groups.len(), "{}", def.name);
        assert!(crate::render_source(source).is_ok(), "{}", def.name);
    }
}

#[test]
fn reference_and_machine_schema_expose_the_same_geometry_contract() {
    let reference = crate::docgen::reference();
    let schema: serde_json::Value = serde_json::from_str(&crate::docgen::schema()).unwrap();
    assert!(reference.contains("no current genre is Hybrid"));
    for def in props::ARCHETYPES {
        let schema_def = schema["archetypes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["name"] == def.name)
            .unwrap();
        assert_eq!(
            schema_def["layoutCapability"],
            def.layout_capability().as_str()
        );
        assert_eq!(
            schema_def["layoutStatements"],
            serde_json::json!(def.layout.statements())
        );
        assert_eq!(
            schema_def["layoutGuides"],
            serde_json::json!(def.layout.guides())
        );
        assert!(reference.contains(&format!(
            "| `{}` | `{}` |",
            def.name,
            def.layout_capability().as_str()
        )));
    }
}

#[test]
fn hybrid_requires_an_explicit_guide_contract_and_accepts_no_entity_statements() {
    let strategy = props::LayoutStrategy::Hybrid {
        solver: |_, _, _| unreachable!("no current archetype exposes this reserved contract"),
        guides: &["named-lanes"],
    };
    assert_eq!(strategy.capability(), props::LayoutCapability::Hybrid);
    assert_eq!(strategy.guides(), &["named-lanes"]);
    assert!(!strategy.accepts_layout_block());
    assert!(strategy.statements().is_empty());
}

#[test]
fn declared_genres_honour_series_and_both_size_axes_in_global_and_local_layouts() {
    for def in props::ARCHETYPES
        .iter()
        .filter(|def| def.layout.accepts_layout_block())
    {
        for horizontal in [true, false] {
            for local in [false, true] {
                let relation = if horizontal {
                    "row a b gap 19\n equal-height a b\n align top a b"
                } else {
                    "column a b gap 19\n equal-width a b\n align left a b"
                };
                let body = format!(
                    "node a \"A\" {{ size: 70 90 }}\nnode b \"B\" {{ size: 120 30 }}\nlayout {{\n place a at (0, 0)\n {relation}\n}}"
                );
                let source = if local {
                    format!("type {}\ngroup g {{\n{body}\n}}", def.name)
                } else {
                    format!("type {}\n{body}", def.name)
                };
                let analysis = crate::analyze_source(&source);
                assert!(
                    !analysis.has_errors(),
                    "{} horizontal={horizontal} local={local}: {:?}",
                    def.name,
                    analysis.diagnostics
                );
                let drawing = analysis.layout.unwrap();
                let a = drawing
                    .nodes
                    .iter()
                    .find(|node| node.id == "a")
                    .unwrap()
                    .rect;
                let b = drawing
                    .nodes
                    .iter()
                    .find(|node| node.id == "b")
                    .unwrap()
                    .rect;
                if horizontal {
                    assert!((b.x - a.right() - 19.0).abs() < 1e-8, "{}", def.name);
                    assert!((a.h - b.h).abs() < 1e-8, "{}", def.name);
                    assert!((a.y - b.y).abs() < 1e-8, "{}", def.name);
                } else {
                    assert!((b.y - a.bottom() - 19.0).abs() < 1e-8, "{}", def.name);
                    assert!((a.w - b.w).abs() < 1e-8, "{}", def.name);
                    assert!((a.x - b.x).abs() < 1e-8, "{}", def.name);
                }
            }
        }
    }
}
