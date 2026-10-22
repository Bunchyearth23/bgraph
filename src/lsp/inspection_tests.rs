use super::*;
use crate::ast;

#[test]
fn declaration_layout_navigation_prefers_controlling_relation_and_groups() {
    let text = "group frame {\n node a\n node b\n layout {\n place b right-of a\n place a at (20, 30)\n }\n}\nlayout { place frame at (100, 100) }\n";
    let index = LineIndex::new(text);
    let declaration = text.find("a\n").unwrap();
    let result =
        navigation::definition(text, &index, declaration, "untitled:test", &HashMap::new());
    assert_eq!(
        result["range"],
        index.range(ast::Span {
            start: text.find("a at").unwrap(),
            end: text.find("a at").unwrap() + 1
        })
    );
    let result = navigation::definition(
        text,
        &index,
        text.find("frame {").unwrap(),
        "untitled:test",
        &HashMap::new(),
    );
    assert_eq!(result["range"]["start"]["line"], 8);
    let result = navigation::definition(
        text,
        &index,
        text.find("frame at").unwrap(),
        "untitled:test",
        &HashMap::new(),
    );
    assert_eq!(result["range"]["start"]["line"], 0);
    let refs = navigation::references(text, declaration, "untitled:test", true, &HashMap::new());
    assert_eq!(refs.as_array().unwrap().len(), 3);
}

#[test]
fn references_are_dispatched_and_keep_utf16_columns() {
    let text = "node a\nnode b\na -> b\nlayout { place a at (0, 0) }\n";
    let uri = "untitled:inspection";
    let mut server = Server::default();
    server.docs.insert(uri.into(), text.into());
    let params = json!({"textDocument":{"uri":uri},"position":{"line":0,"character":5},"context":{"includeDeclaration":true}});
    let refs = server.references(&params);
    assert_eq!(refs.as_array().unwrap().len(), 3);
    let unicode = "group g \"🦀\" { node a }\nlayout { place a at (0, 0) }\n";
    let result = navigation::definition(
        unicode,
        &LineIndex::new(unicode),
        unicode.find("a at").unwrap(),
        "untitled:unicode",
        &HashMap::new(),
    );
    assert_eq!(result["range"]["start"], json!({"line":0,"character":20}));
    assert_eq!(
        protocol::capabilities()["capabilities"]["referencesProvider"],
        true
    );
    assert!(matches!(
        server.handle("textDocument/references", &params),
        Response::Result(_)
    ));
}

#[test]
fn included_definitions_references_and_hover_use_child_locations() {
    let dir = std::env::temp_dir().join(format!("bgraph-lsp-inspect-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("root.bd");
    let child = dir.join("é.bd");
    let text = "include \"é.bd\" as p\nnode root\nlayout { place root below p.a }\n";
    std::fs::write(&entry, text).unwrap();
    std::fs::write(&child, "node a \"🦀\"\nlayout { place a at (12, 34) }\n").unwrap();
    let uri = file_uri::uri_of(&entry);
    let offset = text.find("p.a").unwrap() + 2;
    let result = navigation::definition(text, &LineIndex::new(text), offset, &uri, &HashMap::new());
    assert!(result["uri"].as_str().unwrap().ends_with("%C3%A9.bd"));
    assert_eq!(result["range"]["start"], json!({"line":0,"character":5}));
    let refs = navigation::references(text, offset, &uri, true, &HashMap::new());
    assert_eq!(refs.as_array().unwrap().len(), 3);
    assert_eq!(refs[1]["range"]["start"], json!({"line":1,"character":15}));
    let hover = hover::file_hover(text, &LineIndex::new(text), offset, &uri, &HashMap::new());
    let content = hover["contents"]["value"].as_str().unwrap();
    assert!(content.contains("`p.a`"), "{content}");
    assert!(content.contains("%C3%A9.bd#L1,6"), "{content}");
    assert!(content.contains("at 12, 34"), "{content}");
    assert_eq!(
        diagnostic_engine::path_of(&file_uri::uri_of(&child)).unwrap(),
        child
    );
    assert!(diagnostic_engine::path_of("file:///tmp/%FF.bd").is_none());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn hover_exposes_group_bounds_parent_and_unsolved_conflicts() {
    let text = include_str!("../../tests/runtime/frame-nested.bd");
    let result = hover::file_hover(
        text,
        &LineIndex::new(text),
        text.find("inner {").unwrap(),
        "untitled:frame",
        &HashMap::new(),
    );
    let content = result["contents"]["value"].as_str().unwrap();
    assert!(content.contains("**group** `inner`"));
    assert!(content.contains("Parent frame: `outer`"));
    assert!(content.contains("Bounds:"));
    let text = include_str!("../../tests/runtime/size-chain-conflict.bd");
    let result = hover::file_hover(
        text,
        &LineIndex::new(text),
        text.find("b c").unwrap(),
        "untitled:conflict",
        &HashMap::new(),
    );
    let content = result["contents"]["value"].as_str().unwrap();
    assert!(content.contains("**diagnostic:**"), "{content}");
    assert!(content.contains("width relation"), "{content}");
}

#[test]
fn outline_optional_details_are_strings_not_invalid_nulls() {
    for text in [
        include_str!("../../tests/runtime/frame-nested.bd"),
        "style plain { fill: #123456 }\nnode a\n",
    ] {
        let symbols = navigation::symbols(text);
        for symbol in symbols.as_array().unwrap() {
            assert!(
                symbol.get("detail").is_none_or(|detail| detail.is_string()),
                "DocumentSymbol detail must be a string or absent: {symbol}"
            );
            assert!(symbol["range"]["start"] != Value::Null);
        }
    }
}

#[test]
fn hover_on_node_theme_uses_style_context_before_keyword_documentation() {
    for archetype in ["bpmn", "xy"] {
        if crate::props::archetype(archetype).is_none() {
            continue;
        }
        let text = format!("type {archetype}\nstyle node {{ fill: #404040 }}\n");
        let result = hover::file_hover(
            &text,
            &LineIndex::new(&text),
            text.find("node").unwrap(),
            "untitled:theme",
            &HashMap::new(),
        );
        let content = result["contents"]["value"].as_str().unwrap();
        assert!(content.contains("**style** `node`"), "{content}");
        assert!(content.contains("**theme**"), "{content}");
    }
}
