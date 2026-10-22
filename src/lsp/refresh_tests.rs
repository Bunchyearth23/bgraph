use super::*;
use std::path::PathBuf;

struct Files(PathBuf);

impl Files {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "bgraph-lsp-refresh-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, name: &str, text: &str) -> String {
        let path = self.0.join(name);
        std::fs::write(&path, text).unwrap();
        file_uri::uri_of(&path)
    }
}

impl Drop for Files {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn notify(server: &mut Server, method: &str, params: Value, parent: &str) -> Vec<Value> {
    assert!(matches!(server.handle(method, &params), Response::Nothing));
    server
        .drain_notifications(method, &params)
        .into_iter()
        .find(|note| note["params"]["uri"] == parent)
        .unwrap_or_else(|| panic!("{method} did not refresh {parent}"))["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .clone()
}

#[test]
fn parent_diagnostics_follow_unsaved_children_saves_and_close() {
    let files = Files::new();
    let source = "include \"child.bd\"\nnode root\n";
    let parent = files.write("root.bd", source);
    let child = files.write("child.bd", "node a { bogus: 1 }\n");
    let mut server = Server::default();
    assert!(
        !notify(
            &mut server,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":parent,"text":source}}),
            &parent
        )
        .is_empty()
    );
    assert!(
        notify(
            &mut server,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":child,"text":"node a\n"}}),
            &parent
        )
        .is_empty()
    );
    assert!(
        !notify(
            &mut server,
            "textDocument/didChange",
            json!({"textDocument":{"uri":child},"contentChanges":[{"text":"node a @\n"}]}),
            &parent
        )
        .is_empty()
    );
    files.write("child.bd", "node a\n");
    // A disk save must not hide a different, still-open buffer.
    assert!(
        !notify(
            &mut server,
            "textDocument/didSave",
            json!({"textDocument":{"uri":child}}),
            &parent
        )
        .is_empty()
    );
    assert!(
        notify(
            &mut server,
            "textDocument/didClose",
            json!({"textDocument":{"uri":child}}),
            &parent
        )
        .is_empty()
    );
    files.write("child.bd", "node a @\n");
    assert!(
        !notify(
            &mut server,
            "textDocument/didSave",
            json!({"textDocument":{"uri":child}}),
            &parent
        )
        .is_empty()
    );
    files.write("child.bd", "node a\n");
    assert!(
        notify(
            &mut server,
            "textDocument/didSave",
            json!({"textDocument":{"uri":child}}),
            &parent
        )
        .is_empty()
    );
}

#[test]
fn watched_creation_and_style_edits_refresh_parent_diagnostics() {
    let files = Files::new();
    let source = "include \"palette.bds\"\nnode root .card\n";
    let parent = files.write("root.bd", source);
    let mut server = Server::default();
    assert!(
        !notify(
            &mut server,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":parent,"text":source}}),
            &parent
        )
        .is_empty()
    );
    let style = files.write("palette.bds", "style card { fill: #ffffff }\n");
    let changed = json!({"changes":[{"uri":style,"type":1}]});
    let diagnostics = notify(
        &mut server,
        "workspace/didChangeWatchedFiles",
        changed.clone(),
        &parent,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    files.write("palette.bds", "style card { bogus: 1 }\n");
    assert!(
        !notify(
            &mut server,
            "workspace/didChangeWatchedFiles",
            changed.clone(),
            &parent
        )
        .is_empty()
    );
    files.write("palette.bds", "style card { fill: #ffffff }\n");
    assert!(
        notify(
            &mut server,
            "workspace/didChangeWatchedFiles",
            changed,
            &parent
        )
        .is_empty()
    );
}

#[test]
fn composition_records_missing_include_dependencies() {
    let files = Files::new();
    let path = files.0.join("root.bd");
    let source = "include \"missing.bd\"\nnode root\n";
    std::fs::write(&path, source).unwrap();
    let composed = crate::compose::compose(&path, Some(source)).unwrap();
    assert!(composed.map.dependencies().contains(&path));
    assert!(
        composed
            .map
            .dependencies()
            .contains(&files.0.join("missing.bd"))
    );
}

#[cfg(feature = "csv")]
#[test]
fn watched_data_changes_refresh_parent_and_keep_missing_data_dependency() {
    let files = Files::new();
    let source = "type pie { from: \"rows.csv\" }\n";
    let parent = files.write("root.bd", source);
    let mut server = Server::default();
    assert!(
        !notify(
            &mut server,
            "textDocument/didOpen",
            json!({"textDocument":{"uri":parent,"text":source}}),
            &parent
        )
        .is_empty()
    );
    let composed = crate::compose::compose(&files.0.join("root.bd"), Some(source)).unwrap();
    assert!(
        composed
            .map
            .dependencies()
            .contains(&files.0.join("rows.csv"))
    );
    let data = files.write("rows.csv", "name,value\nFirst,10\nSecond,20\nThird,30\n");
    let changed = json!({"changes":[{"uri":data,"type":1}]});
    let diagnostics = notify(
        &mut server,
        "workspace/didChangeWatchedFiles",
        changed.clone(),
        &parent,
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    files.write("rows.csv", "name,value\nFirst,oops\n");
    assert!(
        !notify(
            &mut server,
            "workspace/didChangeWatchedFiles",
            changed,
            &parent
        )
        .is_empty()
    );
}

fn request(server: &mut Server, method: &str, uri: &str, text: &str, needle: &str) -> Value {
    let (line, character) = LineIndex::new(text).line_col(text.find(needle).unwrap());
    let params = json!({
        "textDocument": { "uri": uri },
        "position": { "line": line, "character": character },
        "context": { "includeDeclaration": true }
    });
    let Response::Result(answer) = server.handle(method, &params) else {
        panic!("{method} did not answer");
    };
    answer
}

#[test]
fn navigation_and_hover_follow_unsaved_includes_and_restore_disk_on_close() {
    let files = Files::new();
    let source = "include \"é.bd\" as p\nnode root\nlayout { place root below p.a }\n";
    let parent = files.write("root.bd", source);
    let disk = "node a \"saved\"\nlayout { place a at (12, 34) }\n";
    let child = files.write("é.bd", disk);
    let dirty = "// unsaved\ngroup frame \"🦀\" { node a \"draft\" { width: 180 } }\nlayout { place a at (120, 340) }\n";
    let mut server = Server::default();
    for (uri, text) in [(&parent, source), (&child, dirty)] {
        server.handle(
            "textDocument/didOpen",
            &json!({"textDocument":{"uri":uri,"text":text}}),
        );
    }

    let definition = request(
        &mut server,
        "textDocument/definition",
        &parent,
        source,
        "p.a",
    );
    assert_eq!(definition["uri"], child);
    assert_eq!(
        definition["range"]["start"],
        json!({"line":1,"character":24})
    );
    let refs = request(
        &mut server,
        "textDocument/references",
        &parent,
        source,
        "p.a",
    );
    let child_refs: Vec<_> = refs
        .as_array()
        .unwrap()
        .iter()
        .filter(|location| location["uri"] == child)
        .collect();
    assert_eq!(child_refs.len(), 2, "{refs}");
    assert_eq!(
        child_refs[0]["range"]["start"],
        json!({"line":1,"character":24})
    );
    assert_eq!(
        child_refs[1]["range"]["start"],
        json!({"line":2,"character":15})
    );
    let hover = request(&mut server, "textDocument/hover", &parent, source, "p.a");
    let content = hover["contents"]["value"].as_str().unwrap();
    assert!(content.contains("at 120, 340"), "{content}");
    assert!(content.contains("180.00 ×"), "{content}");
    assert!(content.contains("%C3%A9.bd#L2,25"), "{content}");

    let changed = format!("// moved again\n{dirty}");
    server.handle(
        "textDocument/didChange",
        &json!({"textDocument":{"uri":child},"contentChanges":[{"text":changed}]}),
    );
    let moved = request(
        &mut server,
        "textDocument/definition",
        &parent,
        source,
        "p.a",
    );
    assert_eq!(moved["range"]["start"], json!({"line":2,"character":24}));

    server.handle(
        "textDocument/didClose",
        &json!({"textDocument":{"uri":child}}),
    );
    let restored = request(
        &mut server,
        "textDocument/definition",
        &parent,
        source,
        "p.a",
    );
    assert_eq!(restored["range"]["start"], json!({"line":0,"character":5}));
    let hover = request(&mut server, "textDocument/hover", &parent, source, "p.a");
    let content = hover["contents"]["value"].as_str().unwrap();
    assert!(content.contains("at 12, 34"), "{content}");
    assert!(!content.contains("at 120, 340"), "{content}");
}

#[test]
fn inspection_can_resolve_an_included_buffer_not_saved_yet() {
    let files = Files::new();
    let source = "include \"new.bd\" as p\nnode root\nroot -> p.a\n";
    let parent = files.write("root.bd", source);
    let child = file_uri::uri_of(&files.0.join("new.bd"));
    let mut server = Server::default();
    for (uri, text) in [
        (&parent, source),
        (&child, "node a\nlayout { place a at (50, 60) }\n"),
    ] {
        server.handle(
            "textDocument/didOpen",
            &json!({"textDocument":{"uri":uri,"text":text}}),
        );
    }
    let definition = request(
        &mut server,
        "textDocument/definition",
        &parent,
        source,
        "p.a",
    );
    assert_eq!(definition["uri"], child);
    assert_eq!(
        definition["range"]["start"],
        json!({"line":0,"character":5})
    );
    let hover = request(&mut server, "textDocument/hover", &parent, source, "p.a");
    assert!(
        hover["contents"]["value"]
            .as_str()
            .unwrap()
            .contains("at 50, 60"),
        "{hover}"
    );
    server.handle(
        "textDocument/didClose",
        &json!({"textDocument":{"uri":child}}),
    );
    assert_eq!(
        request(
            &mut server,
            "textDocument/definition",
            &parent,
            source,
            "p.a"
        ),
        Value::Null
    );
    assert_eq!(
        request(
            &mut server,
            "textDocument/references",
            &parent,
            source,
            "p.a"
        ),
        json!([])
    );
}
