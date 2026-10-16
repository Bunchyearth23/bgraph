use std::collections::BTreeSet;

use super::*;

fn feasible(all: &[SourcedConstraint], indices: &[usize]) -> Result<bool> {
    let mut solver = Solver::new();
    for &index in indices {
        if !add_to_solver(&mut solver, &all[index])? {
            return Ok(false);
        }
    }
    Ok(true)
}

// Remove blocks first: large diagrams usually contain few relevant source relations.
fn irreducible(all: &[SourcedConstraint], mut active: Vec<usize>) -> Result<Vec<usize>> {
    let mut partitions = 2;
    while active.len() > 1 {
        let chunk = active.len().div_ceil(partitions);
        let mut reduced = None;
        for start in (0..active.len()).step_by(chunk) {
            let end = (start + chunk).min(active.len());
            let candidate = active[..start]
                .iter()
                .chain(&active[end..])
                .copied()
                .collect::<Vec<_>>();
            if !feasible(all, &candidate)? {
                reduced = Some(candidate);
                break;
            }
        }
        if let Some(candidate) = reduced {
            active = candidate;
            partitions = partitions.saturating_sub(1).max(2);
        } else if partitions >= active.len() {
            break;
        } else {
            partitions = (partitions * 2).min(active.len());
        }
    }
    Ok(active)
}

pub(super) fn minimum(all: &[SourcedConstraint], failing: usize) -> Result<Vec<SourcedConstraint>> {
    let required = all
        .iter()
        .enumerate()
        .filter_map(|(index, source)| {
            (source.constraint.strength() == Strength::REQUIRED).then_some(index)
        })
        .collect::<Vec<_>>();
    for &index in &required {
        if !feasible(all, &[index])? {
            return Ok(vec![all[index].clone()]);
        }
    }
    let mut seed = required
        .iter()
        .copied()
        .filter(|&index| index <= failing)
        .collect::<Vec<_>>();
    if feasible(all, &seed)? {
        if feasible(all, &required)? {
            return Ok(Vec::new());
        }
        seed.clone_from(&required);
    }
    let mut best = irreducible(all, seed)?;
    let mut pending = best
        .iter()
        .rev()
        .map(|&index| vec![index])
        .collect::<Vec<_>>();
    let mut visited = BTreeSet::new();
    let mut satisfiable_exclusions: Vec<Vec<usize>> = Vec::new();
    while best.len() > 2 {
        let Some(excluded) = pending.pop() else {
            break;
        };
        if !visited.insert(excluded.clone())
            || satisfiable_exclusions.iter().any(|known| {
                known
                    .iter()
                    .all(|index| excluded.binary_search(index).is_ok())
            })
        {
            continue;
        }
        let active = required
            .iter()
            .copied()
            .filter(|index| excluded.binary_search(index).is_err())
            .collect::<Vec<_>>();
        if active.len() < 2 {
            continue;
        }
        if feasible(all, &active)? {
            satisfiable_exclusions.push(excluded);
            continue;
        }
        let core = irreducible(all, active)?;
        if core.len() < best.len() || (core.len() == best.len() && core < best) {
            best.clone_from(&core);
        }
        // Any smaller conflict must omit a member of this irreducible core.
        for index in core.into_iter().rev() {
            if let Err(position) = excluded.binary_search(&index) {
                let mut next = excluded.clone();
                next.insert(position, index);
                pending.push(next);
            }
        }
    }
    let mut conflict = best
        .into_iter()
        .map(|index| all[index].clone())
        .collect::<Vec<_>>();
    conflict.sort_by_key(|source| {
        (
            source.provenance.span.start,
            source.provenance.span.end,
            source.provenance.origin,
            source.provenance.axis,
            source.provenance.node,
            source.provenance.anchor.unwrap_or(usize::MAX),
        )
    });
    Ok(conflict)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sourced(constraints: impl IntoIterator<Item = Constraint>) -> Vec<SourcedConstraint> {
        constraints
            .into_iter()
            .enumerate()
            .map(|(index, constraint)| SourcedConstraint {
                constraint,
                provenance: provenance(
                    ConstraintOrigin::AbsolutePlacement,
                    Axis::X,
                    index,
                    Span {
                        start: index,
                        end: index + 1,
                    },
                ),
            })
            .collect()
    }

    #[test]
    fn minimum_is_smaller_than_the_first_greedy_irreducible_core() {
        let x = Variable::new();
        let y = Variable::new();
        let z = Variable::new();
        let all = sourced([
            x | EQ(Strength::REQUIRED) | 0.0,
            y | EQ(Strength::REQUIRED) | 0.0,
            z | EQ(Strength::REQUIRED) | y,
            x | EQ(Strength::REQUIRED) | z,
            x | EQ(Strength::REQUIRED) | 1.0,
        ]);
        let conflict = minimum(&all, 4).unwrap();
        assert_eq!(conflict.len(), 2);
        assert_eq!(
            conflict
                .iter()
                .map(|c| c.provenance.span.start)
                .collect::<Vec<_>>(),
            [0, 4]
        );
    }

    #[test]
    fn later_constraints_can_supply_a_smaller_conflict_than_the_first_failure() {
        let x = Variable::new();
        let y = Variable::new();
        let z = Variable::new();
        let all = sourced([
            x | GE(Strength::REQUIRED) | 0.0,
            y | GE(Strength::REQUIRED) | 0.0,
            (x + y) | LE(Strength::REQUIRED) | -1.0,
            z | EQ(Strength::REQUIRED) | 0.0,
            z | EQ(Strength::REQUIRED) | 1.0,
        ]);
        let conflict = minimum(&all, 2).unwrap();
        assert_eq!(conflict.len(), 2);
        assert_eq!(
            conflict
                .iter()
                .map(|c| c.provenance.span.start)
                .collect::<Vec<_>>(),
            [3, 4]
        );
    }

    #[test]
    fn a_single_constant_contradiction_is_found_after_the_failing_prefix() {
        let x = Variable::new();
        let all = sourced([
            x | EQ(Strength::REQUIRED) | 0.0,
            x | EQ(Strength::REQUIRED) | 1.0,
            kasuari::Expression::from_constant(1.0) | EQ(Strength::REQUIRED) | 0.0,
        ]);
        let conflict = minimum(&all, 1).unwrap();
        assert_eq!(conflict.len(), 1);
        assert_eq!(conflict[0].provenance.span.start, 2);
    }

    #[test]
    fn three_relations_are_proven_minimum_when_the_initial_core_has_four() {
        let u = Variable::new();
        let v = Variable::new();
        let w = Variable::new();
        let x = Variable::new();
        let y = Variable::new();
        let all = sourced([
            u | EQ(Strength::REQUIRED) | 0.0,
            u | EQ(Strength::REQUIRED) | v,
            v | EQ(Strength::REQUIRED) | w,
            w | EQ(Strength::REQUIRED) | 1.0,
            x | GE(Strength::REQUIRED) | 0.0,
            y | GE(Strength::REQUIRED) | 0.0,
            (x + y) | LE(Strength::REQUIRED) | -1.0,
        ]);
        let conflict = minimum(&all, 3).unwrap();
        assert_eq!(conflict.len(), 3);
        assert_eq!(
            conflict
                .iter()
                .map(|c| c.provenance.span.start)
                .collect::<Vec<_>>(),
            [4, 5, 6]
        );
        for left in 0..all.len() {
            for right in left + 1..all.len() {
                assert!(feasible(&all, &[left, right]).unwrap());
            }
        }
    }

    #[test]
    fn diagram_conflicts_choose_cardinality_before_component_insertion_order() {
        let source = "graph { flow: down }\nnode a { size: 40 20 }\nnode b { size: 80 20 }\nnode c\nnode d\nlayout {\n align left a b\n align right a b\n place c at (0, 0)\n place d at (0, 80)\n align top c d\n}\n";
        let diagram = crate::parser::parse(source).unwrap();
        let error = crate::layout::solve(&diagram).err().expect("conflicts");
        let conflict = error
            .downcast_ref::<ConstraintConflict>()
            .expect("typed conflict");
        assert_eq!(conflict.constraints.len(), 3);
        for equation in &conflict.constraints {
            assert!(
                equation.source.span.start >= source.find(" place c").unwrap(),
                "{error}"
            );
        }
    }

    #[test]
    fn soft_preferences_are_not_source_contradictions() {
        let x = Variable::new();
        let all = sourced([
            x | EQ(Strength::REQUIRED) | 0.0,
            x | EQ(Strength::STRONG) | 1.0,
        ]);
        assert!(minimum(&all, 1).unwrap().is_empty());
        assert!(minimum(&[], 0).unwrap().is_empty());
    }

    #[test]
    fn exact_cardinality_matches_exhaustive_small_linear_models() {
        for seed in 0..24_u64 {
            let x = Variable::new();
            let y = Variable::new();
            let mut state = seed + 1;
            let mut constraints = Vec::new();
            for _ in 0..7 {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                let a = ((state >> 8) % 5) as f64 - 2.0;
                let b = ((state >> 16) % 5) as f64 - 2.0;
                let value = ((state >> 24) % 7) as f64 - 3.0;
                let expression = x * a + y * b;
                constraints.push(match state % 3 {
                    0 => expression | EQ(Strength::REQUIRED) | value,
                    1 => expression | LE(Strength::REQUIRED) | value,
                    _ => expression | GE(Strength::REQUIRED) | value,
                });
            }
            let all = sourced(constraints);
            let mut expected = usize::MAX;
            for mask in 1_u32..(1 << all.len()) {
                if mask.count_ones() as usize >= expected {
                    continue;
                }
                let indices = (0..all.len())
                    .filter(|index| mask & (1 << index) != 0)
                    .collect::<Vec<_>>();
                if !feasible(&all, &indices).unwrap() {
                    expected = indices.len();
                }
            }
            let actual = minimum(&all, 2).unwrap();
            assert_eq!(
                actual.len(),
                if expected == usize::MAX { 0 } else { expected },
                "seed {seed}"
            );
            if !actual.is_empty() {
                assert!(!satisfiable(&actual).unwrap());
            }
        }
    }

    #[test]
    fn duplicate_handles_are_idempotent_and_engine_errors_remain_typed() {
        assert!(classify_add_result(Err(AddConstraintError::DuplicateConstraint)).unwrap());
        assert!(!classify_add_result(Err(AddConstraintError::UnsatisfiableConstraint)).unwrap());
        let error = classify_add_result(Err(AddConstraintError::InternalSolverError(
            kasuari::InternalSolverError::ObjectiveUnbounded,
        )))
        .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<AddConstraintError>(),
            Some(AddConstraintError::InternalSolverError(_))
        ));
        let x = Variable::new();
        let source = sourced([x | EQ(Strength::REQUIRED) | 0.0]).remove(0);
        let mut solver = Solver::new();
        assert!(add_to_solver(&mut solver, &source).unwrap());
        assert!(add_to_solver(&mut solver, &source).unwrap());
    }
}
