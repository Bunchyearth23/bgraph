use crate::{layout, parser};
use std::collections::BTreeMap;

fn geometry(source: &str) -> BTreeMap<String, [u64; 4]> {
    let diagram = parser::parse(source).unwrap();
    let solved = layout::solve(&diagram).unwrap();
    let mut result = BTreeMap::new();
    for node in &solved.nodes {
        result.insert(
            node.id.clone(),
            [
                node.rect.x.to_bits(),
                node.rect.y.to_bits(),
                node.rect.w.to_bits(),
                node.rect.h.to_bits(),
            ],
        );
    }
    for group in &solved.groups {
        result.insert(
            group.id.clone(),
            [
                group.rect.x.to_bits(),
                group.rect.y.to_bits(),
                group.rect.w.to_bits(),
                group.rect.h.to_bits(),
            ],
        );
    }
    result
}

#[test]
fn scheduled_frames_preserve_geometry_under_declaration_and_layout_permutations() {
    let first = "group outer {\n padding: 8\n group inner {\n padding: 4\n node a { size: 30 20 }\n node b { size: 30 20 }\n}\n}\nnode c { size: 30 20 }\nlayout {\n place outer at (300, 200)\n place a at (10, 20)\n place b right-of a gap 2\n place c below outer gap 15\n}\n";
    let second = "node c { size: 30 20 }\ngroup outer {\n padding: 8\n group inner {\n padding: 4\n node b { size: 30 20 }\n node a { size: 30 20 }\n}\n}\nlayout {\n place c below outer gap 15\n place b right-of a gap 2\n place a at (10, 20)\n place outer at (300, 200)\n}\n";
    let expected = geometry(first);
    for source in [first, second, first, second] {
        assert_eq!(geometry(source), expected);
    }
}

#[test]
#[cfg(feature = "bpmn")]
fn boundary_cycles_and_dependents_have_stable_raw_geometry() {
    let cycle = include_str!("../../tests/fixtures/placement/bpmn-cycle.bd");
    let permuted = "type bpmn\ntask work { size: 200 80 }\nevent alarm boundary timer { attach: work }\nlayout {\n place work above alarm gap -20\n}\n";
    assert_eq!(geometry(cycle), geometry(permuted));
    let dependent = include_str!("../../tests/fixtures/placement/bpmn-dependent.bd");
    let diagram = parser::parse(dependent).unwrap();
    let solved = layout::solve(&diagram).unwrap();
    assert_eq!(
        (solved.nodes[1].rect.x, solved.nodes[1].rect.y),
        (80.0, 60.0)
    );
    assert_eq!(
        (solved.nodes[2].rect.x, solved.nodes[2].rect.y),
        (80.0, 120.0)
    );
    assert_eq!(geometry(dependent), geometry(dependent));
    for source in [
        include_str!("../../tests/fixtures/placement/bpmn-attach-conflict.bd"),
        include_str!("../../tests/fixtures/placement/bpmn-cycle-conflict.bd"),
    ] {
        let diagram = parser::parse(source).unwrap();
        assert!(
            layout::solve(&diagram)
                .err()
                .unwrap()
                .downcast_ref::<super::ConstraintConflict>()
                .is_some()
        );
    }
}

#[test]
fn thousands_of_required_chain_relations_are_solved_simultaneously() {
    let count = 2_000;
    let mut source = String::new();
    for index in 0..count {
        source.push_str(&format!("node n{index} {{ size: 10 10 }}\n"));
    }
    source.push_str("layout {\n place n0 at (0, 0)\n row ");
    for index in 0..count {
        source.push_str(&format!("n{index} "));
    }
    source.push_str("gap 2\n}\n");
    let diagram = parser::parse(&source).unwrap();
    let solved = layout::solve(&diagram).unwrap();
    assert_eq!(solved.nodes.len(), count);
    for (index, node) in solved.nodes.iter().enumerate() {
        assert_eq!(
            (node.rect.x, node.rect.y, node.rect.w, node.rect.h),
            ((index * 12) as f64, 0.0, 10.0, 10.0)
        );
    }
}

#[test]
fn hundreds_of_placed_frames_have_no_transition_threshold() {
    let count = 600;
    let mut source = String::new();
    for index in 0..count {
        source.push_str(&format!(
            "group g{index} {{\n padding: 1\n node n{index} {{ size: 20 20 }}\n}}\n"
        ));
    }
    source.push_str("layout {\n");
    for index in 0..count {
        source.push_str(&format!(" place g{index} at ({}, 10)\n", index * 40));
    }
    source.push_str("}\n");
    let diagram = parser::parse(&source).unwrap();
    let solved = layout::solve(&diagram).unwrap();
    assert_eq!(solved.groups.len(), count);
    for (index, node) in solved.nodes.iter().enumerate() {
        assert_eq!((node.rect.x, node.rect.y), ((index * 40 + 1) as f64, 11.0));
    }
    assert!(
        !crate::check::after_layout(&diagram, &solved)
            .iter()
            .any(|diag| diag.severity == crate::check::Severity::Error)
    );
}

#[test]
fn a_thousand_nested_frames_keep_exact_content_bounds_and_local_origins() {
    let mut source = String::new();
    for chain in 0..20 {
        for depth in 0..50 {
            source.push_str(&format!("group g{chain}_{depth} {{\n padding: 1\n"));
        }
        source.push_str(&format!("node n{chain} {{ size: 20 20 }}\n"));
        source.push_str(&"}\n".repeat(50));
    }
    source.push_str("layout {\n");
    for chain in 0..20 {
        source.push_str(&format!(" place g{chain}_0 at ({}, 100)\n", chain * 200));
    }
    source.push_str("}\n");
    let diagram = parser::parse(&source).unwrap();
    let solved = layout::solve(&diagram).unwrap();
    assert_eq!(solved.groups.len(), 1_000);
    for (chain, node) in solved.nodes.iter().enumerate() {
        assert_eq!(
            (node.rect.x, node.rect.y),
            ((chain * 200 + 50) as f64, 150.0)
        );
    }
    assert!(
        !crate::check::after_layout(&diagram, &solved)
            .iter()
            .any(|diag| diag.severity == crate::check::Severity::Error)
    );
}

#[cfg(feature = "bpmn")]
fn boundary_cycle_source(gap: i32) -> String {
    let mut source = String::from(
        "type bpmn\nevent alarm boundary timer { attach: work }\ntask work { size: 200 80 }\n",
    );
    for index in 0..600 {
        source.push_str(&format!("task filler{index} {{ size: 10 10 }}\n"));
    }
    source.push_str(&format!("layout {{\n place work above alarm gap {gap}\n"));
    for index in 0..600 {
        source.push_str(&format!(" place filler{index} at ({}, 300)\n", index * 20));
    }
    source.push_str("}\n");
    source
}

#[test]
#[cfg(feature = "bpmn")]
fn boundary_host_cycles_are_simultaneous_above_the_old_threshold() {
    let diagram = parser::parse(&boundary_cycle_source(-20)).unwrap();
    let solved = layout::solve(&diagram).unwrap();
    let event = solved.nodes[0].rect;
    let host = solved.nodes[1].rect;
    assert_eq!(event.cx(), host.cx());
    assert_eq!(event.cy(), host.bottom());
    assert_eq!(host.bottom() - 20.0, event.y);
    assert!(
        !crate::check::after_layout(&diagram, &solved)
            .iter()
            .any(|diag| diag.severity == crate::check::Severity::Error)
    );
}

#[test]
#[cfg(feature = "bpmn")]
fn inconsistent_boundary_cycles_are_source_errors_above_the_old_threshold() {
    let diagram = parser::parse(&boundary_cycle_source(0)).unwrap();
    let error = layout::solve(&diagram).err().unwrap();
    assert!(
        error.downcast_ref::<super::ConstraintConflict>().is_some(),
        "{error}"
    );
    assert!(error.to_string().contains("alarm.height"), "{error}");
}
