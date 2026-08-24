#![cfg(feature = "kasuari-spike")]

use kasuari::WeightedRelation::*;
use kasuari::{Solver, Strength, Variable};

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

#[test]
fn the_spike_records_kasuaris_underconstrained_boundary() {
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

    let values =
        [window, left_1, right_1, left_2, right_2].map(|variable| solver.get_value(variable));
    let allowed = [[75.0, 0.0, 0.0, 0.0, 75.0], [75.0, 0.0, 50.0, 50.0, 75.0]];
    assert!(
        allowed.contains(&values),
        "Kasuari produced an undocumented third solution: {values:?}"
    );
}
