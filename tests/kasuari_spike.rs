use kasuari::WeightedRelation::*;
use kasuari::{Solver, Strength, Variable};
use std::collections::BTreeSet;
use std::process::Command;

fn required_chain() -> [f64; 4] {
    let left = Variable::new();
    let width = Variable::new();
    let gap = Variable::new();
    let right = Variable::new();

    let mut solver = Solver::new();
    solver
        .add_constraints([
            left | EQ(Strength::REQUIRED) | 20.0,
            width | EQ(Strength::REQUIRED) | 80.0,
            gap | EQ(Strength::REQUIRED) | 32.0,
            right | EQ(Strength::REQUIRED) | (left + width + gap),
        ])
        .unwrap();

    [left, width, gap, right].map(|variable| solver.get_value(variable))
}

#[test]
fn a_fully_declared_layout_is_stable_through_kasuari() {
    let expected = [20.0, 80.0, 32.0, 132.0];
    for _ in 0..512 {
        assert_eq!(required_chain(), expected);
    }
}

fn underconstrained_values() -> [f64; 5] {
    let window = Variable::new();
    let left_1 = Variable::new();
    let right_1 = Variable::new();
    let left_2 = Variable::new();
    let right_2 = Variable::new();

    let mut solver = Solver::new();
    solver
        .add_constraints([
            window | GE(Strength::REQUIRED) | 0.0,
            left_1 | EQ(Strength::REQUIRED) | 0.0,
            right_2 | EQ(Strength::REQUIRED) | window,
            left_2 | GE(Strength::REQUIRED) | right_1,
            left_1 | LE(Strength::REQUIRED) | right_1,
            left_2 | LE(Strength::REQUIRED) | right_2,
            (right_1 - left_1) | EQ(Strength::WEAK) | 50.0,
            (right_2 - left_2) | EQ(Strength::WEAK) | 100.0,
        ])
        .unwrap();
    solver.add_edit_variable(window, Strength::STRONG).unwrap();
    solver.suggest_value(window, 75.0).unwrap();

    [window, left_1, right_1, left_2, right_2].map(|variable| solver.get_value(variable))
}

#[test]
fn the_spike_records_kasuaris_underconstrained_boundary() {
    let values = underconstrained_values();
    let allowed = [[75.0, 0.0, 0.0, 0.0, 75.0], [75.0, 0.0, 50.0, 50.0, 75.0]];
    assert!(
        allowed.contains(&values),
        "Kasuari produced an undocumented third solution: {values:?}"
    );
}

#[test]
fn the_cassowary_shadow_is_stable_on_layout_permutations() {
    let statements = [
        "place a at (10, 20)",
        "place b right-of a gap 30 align top",
        "place c below b",
        "equal-width a b",
    ];
    let source = |order: &[&str]| format!(
        "node a {{ size: 80 40 }}\nnode b {{ size: 60 20 }}\nnode c {{ size: 30 30 }}\nlayout {{\n {}\n{}\n{}\n{}\n}}",
        order[0], order[1], order[2], order[3]
    );
    let permutations = permute(statements);
    let expected = bgraph::render_source(&source(&statements))
        .unwrap_or_else(|error| panic!("baseline is rejected: {error:#}"));

    for order in permutations {
        let source = source(&order);
        assert_eq!(
            bgraph::render_source(&source).unwrap_or_else(|error| {
                panic!("permuted source is rejected: {source}\n{error:#}")
            }),
            expected,
            "Cassowary shadow depends on statement order: {source}"
        );
    }
}

#[test]
fn the_cassowary_shadow_is_stable_through_local_repetitions() {
    let source = "node a { size: 80 40 }\nnode b { size: 60 20 }\nnode c { size: 30 30 }\nlayout {\n place a at (10, 20)\n place b right-of a gap 30 align top\n place c below b\n equal-width a b\n}\n";
    let baseline = bgraph::render_source(source).unwrap();

    for _ in 0..512 {
        assert_eq!(
            bgraph::render_source(source).unwrap(),
            baseline,
            "render drifted on a repeated call"
        );
    }
}

#[test]
fn deterministic_fork_is_stable_across_processes() {
    const CHILD: &str = "BGRAPH_KASUARI_DETERMINISM_CHILD";
    const MARKER: &str = "BGRAPH_KASUARI_VALUES=";

    if std::env::var_os(CHILD).is_some() {
        println!("{MARKER}{:?}", underconstrained_values());
        return;
    }

    let executable = std::env::current_exe().expect("the integration-test executable");
    let mut seen = BTreeSet::new();
    for _ in 0..32 {
        let output = Command::new(&executable)
            .args([
                "--exact",
                "deterministic_fork_is_stable_across_processes",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .expect("the child determinism probe runs");
        assert!(output.status.success(), "child probe failed: {output:?}");
        let stdout = String::from_utf8(output.stdout).expect("test output is UTF-8");
        let value = stdout
            .lines()
            .find_map(|line| line.split_once(MARKER).map(|(_, value)| value))
            .unwrap_or_else(|| panic!("child probe emitted no value: {stdout}"));
        seen.insert(value.to_string());
    }

    assert_eq!(
        seen.len(),
        1,
        "the same system produced different solutions across processes: {seen:?}"
    );
}

#[test]
fn canonical_relations_match_the_cassowary_shadow() {
    for source in [
        "node a { size: 80 40 }\nnode b { size: 60 20 }\nlayout { place a at (10, 20)\n place b right-of a gap 30 align top }",
        "node a { size: 30 30 }\nnode b { size: 40 50 }\nlayout { place a right-of b gap 12\n place b at (5, 7) }",
        "graph { flow: down, gap: 18 }\nnode a { size: 80 40 }\nnode b { size: 30 20 }",
        "graph { grid: 100 60 }\nnode a\nnode b\nlayout { place a at (col 2, row 3)\n place b above a align right }",
    ] {
        bgraph::render_source(source)
            .unwrap_or_else(|error| panic!("shadow rejected:\n{source}\n{error:#}"));
    }
}

#[test]
fn the_cassowary_shadow_reports_a_minimal_conflict_set() {
    let source = "node a { size: 80 40 }\nnode b { size: 60 20 }\nlayout {\n place a right-of b\n place b right-of a\n}";

    let error = bgraph::render_source(source).expect_err("cyclic explicit placement must fail");
    let message = format!("{error}");
    if message.contains("Kasuari node conflict") {
        assert!(message.contains("bytes") && message.contains("anchor"));
        return;
    }
    assert!(
        message.contains("anchor cycle between:"),
        "conflict was neither a Kasuari conflict payload nor an anchor cycle: {message}"
    );
}

fn permute<T: Clone>(items: [T; 4]) -> Vec<[T; 4]> {
    let mut out = Vec::with_capacity(24);
    let values = items.to_vec();
    let mut used = vec![false; values.len()];
    let mut current = Vec::with_capacity(values.len());

    fn backtrack<T: Clone>(
        values: &[T],
        used: &mut Vec<bool>,
        current: &mut Vec<T>,
        out: &mut Vec<[T; 4]>,
    ) {
        if current.len() == values.len() {
            let permutation = current.clone().try_into().unwrap_or_else(|_| {
                unreachable!("all permutations have length 4 and this path is checked above")
            });
            out.push(permutation);
            return;
        }
        for i in 0..values.len() {
            if used[i] {
                continue;
            }
            used[i] = true;
            current.push(values[i].clone());
            backtrack(values, used, current, out);
            current.pop();
            used[i] = false;
        }
    }

    backtrack(&values, &mut used, &mut current, &mut out);
    out
}
