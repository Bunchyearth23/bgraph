use super::*;
use std::collections::HashMap;

fn labels(source: &str, line: &str) -> Vec<String> {
    Server {
        docs: HashMap::new(),
    }
    .items_for(source, line, source.len())
    .into_iter()
    .filter_map(|item| item["label"].as_str().map(str::to_string))
    .collect()
}

#[test]
fn geometry_contract_controls_top_level_group_and_layout_completions() {
    for def in props::ARCHETYPES {
        for container in ["", "group g {\n"] {
            let source = format!("type {}\n{container}  ", def.name);
            let offered = labels(&source, "  ");
            for word in ["layout", "note"] {
                assert_eq!(
                    offered.iter().any(|label| label == word),
                    def.layout.accepts_layout_block(),
                    "{} in {container:?}: {word}: {offered:?}",
                    def.name
                );
            }
            for statement in props::LAYOUT_STATEMENTS {
                assert!(
                    !offered.iter().any(|label| label == statement),
                    "{} offers {statement} outside layout",
                    def.name
                );
            }
            let source = format!("type {}\n{container}layout {{\n  ", def.name);
            let mut offered = labels(&source, "  ");
            offered.sort_unstable();
            let mut expected = def
                .layout
                .statements()
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>();
            expected.sort_unstable();
            assert_eq!(offered, expected, "{} in {container:?}", def.name);
        }
    }
}

#[test]
fn declared_layout_completion_inserts_a_block_and_stays_quiet_in_prose() {
    let source = "type free\n";
    let items = Server {
        docs: HashMap::new(),
    }
    .items_for(source, "", source.len());
    let layout = items.iter().find(|item| item["label"] == "layout").unwrap();
    assert_eq!(layout["insertText"], "layout {\n  $0\n}");
    for source in ["type free\n// layout ", "type free\nnode a \"layout "] {
        assert!(labels(source, source.lines().last().unwrap()).is_empty());
    }
}

#[test]
fn note_anchor_completion_respects_geometry_and_removed_inline_placement_stays_quiet() {
    for def in props::ARCHETYPES {
        let source = format!("type {}\nnode a\nnote \"annotation\" below ", def.name);
        assert_eq!(
            labels(&source, "note \"annotation\" below ")
                .iter()
                .any(|label| label == "a"),
            def.layout.accepts_layout_block(),
            "{}",
            def.name,
        );
        for line in ["node b right-of ", "node b right-of a align "] {
            let source = format!("type {}\nnode a\n{line}", def.name);
            assert!(labels(&source, line).is_empty(), "{}: {line}", def.name);
        }
    }
}
