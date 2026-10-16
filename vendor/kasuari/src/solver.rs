use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use hashbrown::HashMap;

use crate::constraint::Constraint;
use crate::row::{near_zero, Row, Symbol, SymbolKind};
use crate::strength::Strength;
use crate::{
    AddConstraintError, AddEditVariableError, Expression, RelationalOperator,
    RemoveConstraintError, RemoveEditVariableError, SuggestValueError, Term, Variable,
};

#[derive(Debug, Copy, Clone, thiserror::Error)]
#[error("The solver entered an invalid state. If this occurs please report the issue.")]
pub enum InternalSolverError {
    #[error("The objective is unbounded.")]
    ObjectiveUnbounded,
    #[error("Dual optimize failed.")]
    DualOptimizeFailed,
    #[error("Failed to find leaving row.")]
    FailedToFindLeavingRow,
    #[error("Edit constraint not in system")]
    EditConstraintNotInSystem,
}

#[derive(Copy, Clone)]
struct Tag {
    marker: Symbol,
    other: Symbol,
}

#[derive(Copy, Clone)]
enum OptimizeTarget {
    Objective,
    Artificial,
}

/// An add-only solver for a complete model that will never remove or edit constraints.
///
/// Required equality markers are fixed to zero and can be forgotten once inserted.
/// No removal/edit API or access to the underlying incremental solver is exposed.
pub struct StaticSolver {
    solver: Solver,
}

impl Default for StaticSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl StaticSolver {
    /// Construct an empty add-only solver.
    pub fn new() -> Self {
        let mut solver = Solver::new();
        solver.columns = Some(BTreeMap::new());
        Self { solver }
    }

    /// Add a required or weighted relation, retaining handle identity and errors.
    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<(), AddConstraintError> {
        self.solver.add_constraint(constraint.clone())?;
        let marker = self.solver.constraints[&constraint].marker;
        self.solver.forget_dummy(marker);
        Ok(())
    }

    /// Get the current value, or zero for an unknown variable.
    pub fn get_value(&self, variable: Variable) -> f64 {
        self.solver.get_value(variable)
    }
}

#[cfg(test)]
mod static_tests {
    use super::*;
    use crate::WeightedRelation::*;
    use alloc::vec;

    fn indexed(solver: &Solver) {
        let mut expected: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
        for (basic, row) in &solver.rows {
            for symbol in row.cells.keys() {
                expected.entry(*symbol).or_default().insert(*basic);
            }
        }
        assert_eq!(solver.columns.as_ref().unwrap(), &expected);
    }

    #[test]
    fn static_matches_incremental_after_every_relation() {
        for offset in [0.0, 0.125, -17.5] {
            let x = Variable::new();
            let y = Variable::new();
            let width = Variable::new();
            let relations: Vec<Constraint> = vec![
                x | GE(Strength::REQUIRED) | offset,
                y | GE(Strength::REQUIRED) | 0.0,
                (x + y) | EQ(Strength::REQUIRED) | (100.0 + offset),
                x | EQ(Strength::WEAK) | (20.0 + offset),
                y | EQ(Strength::WEAK) | 50.0,
                width | GE(Strength::REQUIRED) | (x + 10.0),
                width | EQ(Strength::STRONG) | 150.0,
                y | LE(Strength::REQUIRED) | 60.0,
            ];
            let mut standard = Solver::new();
            let mut permanent = StaticSolver::new();
            for relation in relations {
                standard.add_constraint(relation.clone()).unwrap();
                permanent.add_constraint(relation).unwrap();
                indexed(&permanent.solver);
                for variable in [x, y, width] {
                    assert_eq!(
                        standard.get_value(variable).to_bits(),
                        permanent.get_value(variable).to_bits()
                    );
                }
            }
        }
    }

    #[test]
    fn tolerated_required_residual_and_duplicate_handles_match() {
        let x = Variable::new();
        let y = Variable::new();
        let mut standard = Solver::new();
        let mut permanent = StaticSolver::new();
        let first: Constraint = x | EQ(Strength::REQUIRED) | (0.1 + 0.2);
        for relation in [
            first.clone(),
            x | EQ(Strength::REQUIRED) | 0.3,
            y | EQ(Strength::REQUIRED) | (x + 0.125),
        ] {
            standard.add_constraint(relation.clone()).unwrap();
            permanent.add_constraint(relation).unwrap();
            indexed(&permanent.solver);
            assert_eq!(
                standard.get_value(x).to_bits(),
                permanent.get_value(x).to_bits()
            );
            assert_eq!(
                standard.get_value(y).to_bits(),
                permanent.get_value(y).to_bits()
            );
        }
        assert!(matches!(
            permanent.add_constraint(first),
            Err(AddConstraintError::DuplicateConstraint)
        ));
        assert!(matches!(
            permanent.add_constraint(x | EQ(Strength::REQUIRED) | 10.0),
            Err(AddConstraintError::UnsatisfiableConstraint)
        ));
    }

    #[test]
    fn sparse_index_tracks_pivots_edits_removal_artificial_and_reset() {
        let x = Variable::new();
        let y = Variable::new();
        let mut solver = Solver::new();
        solver.columns = Some(BTreeMap::new());
        let sum: Constraint = (x + y) | EQ(Strength::REQUIRED) | 100.0;
        for relation in [
            x | GE(Strength::REQUIRED) | 0.0,
            y | GE(Strength::REQUIRED) | 0.0,
            sum.clone(),
        ] {
            solver.add_constraint(relation).unwrap();
            indexed(&solver);
        }
        solver.add_edit_variable(x, Strength::STRONG).unwrap();
        indexed(&solver);
        for value in [30.0, 120.0, 5.0, -10.0] {
            solver.suggest_value(x, value).unwrap();
            indexed(&solver);
            assert_eq!(solver.get_value(x) + solver.get_value(y), 100.0);
            assert!(solver.get_value(x) >= 0.0 && solver.get_value(y) >= 0.0);
        }
        solver.remove_edit_variable(x).unwrap();
        indexed(&solver);
        solver.remove_constraint(&sum).unwrap();
        indexed(&solver);
        solver.reset();
        indexed(&solver);
        assert!(solver.rows.is_empty());
        let mut artificial = Row::new(0.0);
        artificial.insert_symbol(Symbol::new(10_000, SymbolKind::Slack), 1.0);
        assert!(solver.add_with_artificial_variable(&artificial).unwrap());
        indexed(&solver);
    }

    #[test]
    fn static_required_chain_keeps_tableau_linear() {
        let variables: Vec<_> = (0..2_000).map(|_| Variable::new()).collect();
        let mut solver = StaticSolver::new();
        solver
            .add_constraint(variables[0] | EQ(Strength::REQUIRED) | 0.0)
            .unwrap();
        for pair in variables.windows(2) {
            solver
                .add_constraint(pair[1] | EQ(Strength::REQUIRED) | (pair[0] + 1.0))
                .unwrap();
        }
        indexed(&solver.solver);
        assert_eq!(solver.get_value(*variables.last().unwrap()), 1_999.0);
        assert!(
            solver
                .solver
                .rows
                .values()
                .map(|row| row.cells.len())
                .sum::<usize>()
                <= variables.len()
        );
    }

    #[test]
    fn static_matches_incremental_on_generated_required_and_weighted_models() {
        for seed in 1..=64 {
            let variables: Vec<_> = (0..4).map(|_| Variable::new()).collect();
            let mut random = seed;
            let mut next = || {
                random = (random * 1_664_525 + 1_013_904_223) % (1u64 << 32);
                random
            };
            let mut standard = Solver::new();
            let mut permanent = StaticSolver::new();
            for _ in 0..32 {
                let coefficients: Vec<_> = (0..4).map(|_| (next() % 5) as f64 - 2.0).collect();
                let expression = Expression::new(
                    variables
                        .iter()
                        .zip(&coefficients)
                        .map(|(variable, coefficient)| Term::new(*variable, *coefficient))
                        .collect(),
                    (next() % 33) as f64 / 8.0 - 2.0,
                );
                let operator = match next() % 3 {
                    0 => RelationalOperator::Equal,
                    1 => RelationalOperator::LessOrEqual,
                    _ => RelationalOperator::GreaterOrEqual,
                };
                let strength = match next() % 4 {
                    0 => Strength::REQUIRED,
                    1 => Strength::STRONG,
                    2 => Strength::MEDIUM,
                    _ => Strength::WEAK,
                };
                let relation = Constraint::new(expression, operator, strength);
                let left = standard.add_constraint(relation.clone());
                let right = permanent.add_constraint(relation);
                assert_eq!(left.is_ok(), right.is_ok(), "seed {seed}");
                if left.is_err() {
                    assert!(matches!(
                        left,
                        Err(AddConstraintError::UnsatisfiableConstraint)
                    ));
                    assert!(matches!(
                        right,
                        Err(AddConstraintError::UnsatisfiableConstraint)
                    ));
                    break;
                }
                indexed(&permanent.solver);
                for variable in &variables {
                    assert_eq!(
                        standard.get_value(*variable).to_bits(),
                        permanent.get_value(*variable).to_bits(),
                        "seed {seed}"
                    );
                }
            }
        }
    }
}

#[derive(Clone)]
struct EditInfo {
    tag: Tag,
    constraint: Constraint,
    constant: f64,
}

/// A constraint solver using the Cassowary algorithm. For proper usage please see the top level
/// crate documentation.
pub struct Solver {
    constraints: HashMap<Constraint, Tag>,
    var_data: BTreeMap<Variable, (f64, Symbol, usize)>,
    var_for_symbol: BTreeMap<Symbol, Variable>,
    public_changes: Vec<(Variable, f64)>,
    changed: BTreeSet<Variable>,
    should_clear_changes: bool,
    rows: BTreeMap<Symbol, Box<Row>>,
    // Only add-only solvers need this incidence index; incremental API semantics stay unchanged.
    columns: Option<BTreeMap<Symbol, BTreeSet<Symbol>>>,
    edits: BTreeMap<Variable, EditInfo>,
    infeasible_rows: Vec<Symbol>, // never contains external symbols
    objective: Row,
    artificial: Option<Row>,
    id_tick: usize,
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}

impl Solver {
    /// Construct a new solver.
    pub fn new() -> Solver {
        Solver {
            constraints: HashMap::new(),
            var_data: BTreeMap::new(),
            var_for_symbol: BTreeMap::new(),
            public_changes: Vec::new(),
            changed: BTreeSet::new(),
            should_clear_changes: false,
            rows: BTreeMap::new(),
            columns: None,
            edits: BTreeMap::new(),
            infeasible_rows: Vec::new(),
            objective: Row::new(0.0),
            artificial: None,
            id_tick: 1,
        }
    }

    pub fn add_constraints<I: IntoIterator<Item = Constraint>>(
        &mut self,
        constraints: I,
    ) -> Result<(), AddConstraintError> {
        for constraint in constraints {
            self.add_constraint(constraint)?;
        }
        Ok(())
    }

    /// Add a constraint to the solver.
    pub fn add_constraint(&mut self, constraint: Constraint) -> Result<(), AddConstraintError> {
        if self.constraints.contains_key(&constraint) {
            // TODO detrmine if we could just ignore duplicate constraints
            return Err(AddConstraintError::DuplicateConstraint);
        }

        // Creating a row causes symbols to reserved for the variables in the constraint. If this
        // method exits with an exception, then its possible those variables will linger in the var
        // map. Since its likely that those variables will be used in other constraints and since
        // exceptional conditions are uncommon, i'm not too worried about aggressive cleanup of the
        // var map.
        let (mut row, tag) = self.create_row(&constraint);
        let mut subject = Solver::choose_subject(&row, &tag);

        // If choose_subject could find a valid entering symbol, one last option is available if the
        // entire row is composed of dummy variables. If the constant of the row is zero, then this
        // represents redundant constraints and the new dummy marker can enter the basis. If the
        // constant is non-zero, then it represents an unsatisfiable constraint.
        if subject.kind() == SymbolKind::Invalid && Solver::all_dummies(&row) {
            if !near_zero(row.constant) {
                return Err(AddConstraintError::UnsatisfiableConstraint);
            } else {
                subject = tag.marker;
            }
        }

        // If an entering symbol still isn't found, then the row must be added using an artificial
        // variable. If that fails, then the row represents an unsatisfiable constraint.
        if subject.kind() == SymbolKind::Invalid {
            let satisfiable = self.add_with_artificial_variable(&row)?;
            if !satisfiable {
                return Err(AddConstraintError::UnsatisfiableConstraint);
            }
        } else {
            row.solve_for_symbol(subject);
            self.substitute(subject, &row);
            if subject.kind() == SymbolKind::External && row.constant != 0.0 {
                let v = self.var_for_symbol[&subject];
                self.var_changed(v);
            }
            self.insert_row(subject, row);
        }

        self.constraints.insert(constraint, tag);

        // Optimizing after each constraint is added performs less aggregate work due to a smaller
        // average system size. It also ensures the solver remains in a consistent state.
        self.optimize(OptimizeTarget::Objective)?;
        Ok(())
    }

    /// Remove a constraint from the solver.
    pub fn remove_constraint(
        &mut self,
        constraint: &Constraint,
    ) -> Result<(), RemoveConstraintError> {
        let tag = self
            .constraints
            .remove(constraint)
            .ok_or(RemoveConstraintError::UnknownConstraint)?;

        // Remove the error effects from the objective function
        // *before* pivoting, or substitutions into the objective
        // will lead to incorrect solver results.
        self.remove_constraint_effects(constraint, &tag);

        // If the marker is basic, simply drop the row. Otherwise,
        // pivot the marker into the basis and then drop the row.
        if self.remove_row(tag.marker).is_none() {
            let (leaving, mut row) = self.get_marker_leaving_row(tag.marker).ok_or(
                RemoveConstraintError::InternalSolverError(
                    InternalSolverError::FailedToFindLeavingRow,
                ),
            )?;
            row.solve_for_symbols(leaving, tag.marker);
            self.substitute(tag.marker, &row);
        }

        // Optimizing after each constraint is removed ensures that the
        // solver remains consistent. It makes the solver api easier to
        // use at a small tradeoff for speed.
        self.optimize(OptimizeTarget::Objective)?;

        // Check for and decrease the reference count for variables referenced by the constraint
        // If the reference count is zero remove the variable from the variable map
        for term in &constraint.expr().terms {
            if !near_zero(term.coefficient) {
                let mut should_remove = false;
                if let Some(&mut (_, _, ref mut count)) = self.var_data.get_mut(&term.variable) {
                    *count -= 1;
                    should_remove = *count == 0;
                }
                if should_remove {
                    self.var_for_symbol.remove(&self.var_data[&term.variable].1);
                    self.var_data.remove(&term.variable);
                }
            }
        }
        Ok(())
    }

    /// Test whether a constraint has been added to the solver.
    pub fn has_constraint(&self, constraint: &Constraint) -> bool {
        self.constraints.contains_key(constraint)
    }

    /// Add an edit variable to the solver.
    ///
    /// This method should be called before the `suggest_value` method is
    /// used to supply a suggested value for the given edit variable.
    pub fn add_edit_variable(
        &mut self,
        v: Variable,
        strength: Strength,
    ) -> Result<(), AddEditVariableError> {
        if self.edits.contains_key(&v) {
            return Err(AddEditVariableError::DuplicateEditVariable);
        }
        if strength == Strength::REQUIRED {
            return Err(AddEditVariableError::BadRequiredStrength);
        }
        let cn = Constraint::new(
            Expression::from_term(Term::new(v, 1.0)),
            RelationalOperator::Equal,
            strength,
        );
        self.add_constraint(cn.clone()).unwrap();
        self.edits.insert(
            v,
            EditInfo {
                tag: self.constraints[&cn],
                constraint: cn,
                constant: 0.0,
            },
        );
        Ok(())
    }

    /// Remove an edit variable from the solver.
    pub fn remove_edit_variable(&mut self, v: Variable) -> Result<(), RemoveEditVariableError> {
        if let Some(constraint) = self.edits.remove(&v).map(|e| e.constraint) {
            self.remove_constraint(&constraint).map_err(|e| match e {
                RemoveConstraintError::UnknownConstraint => {
                    RemoveEditVariableError::InternalSolverError(
                        InternalSolverError::EditConstraintNotInSystem,
                    )
                }
                RemoveConstraintError::InternalSolverError(s) => {
                    RemoveEditVariableError::InternalSolverError(s)
                }
            })?;
            Ok(())
        } else {
            Err(RemoveEditVariableError::UnknownEditVariable)
        }
    }

    /// Test whether an edit variable has been added to the solver.
    pub fn has_edit_variable(&self, v: &Variable) -> bool {
        self.edits.contains_key(v)
    }

    /// Suggest a value for the given edit variable.
    ///
    /// This method should be used after an edit variable has been added to
    /// the solver in order to suggest the value for that variable.
    pub fn suggest_value(
        &mut self,
        variable: Variable,
        value: f64,
    ) -> Result<(), SuggestValueError> {
        let (info_tag_marker, info_tag_other, delta) = {
            let info = self
                .edits
                .get_mut(&variable)
                .ok_or(SuggestValueError::UnknownEditVariable)?;
            let delta = value - info.constant;
            info.constant = value;
            (info.tag.marker, info.tag.other, delta)
        };
        // tag.marker and tag.other are never external symbols

        // The nice version of the following code runs into non-lexical borrow issues.
        // Ideally the `if row...` code would be in the body of the if. Pretend that it is.
        {
            let infeasible_rows = &mut self.infeasible_rows;
            if self
                .rows
                .get_mut(&info_tag_marker)
                .map(|row| {
                    if row.add(-delta) < 0.0 {
                        infeasible_rows.push(info_tag_marker);
                    }
                })
                .is_some()
                || self
                    .rows
                    .get_mut(&info_tag_other)
                    .map(|row| {
                        if row.add(delta) < 0.0 {
                            infeasible_rows.push(info_tag_other);
                        }
                    })
                    .is_some()
            {
            } else {
                for (symbol, row) in &mut self.rows {
                    let coeff = row.coefficient_for(info_tag_marker);
                    let diff = delta * coeff;
                    if diff != 0.0 && symbol.kind() == SymbolKind::External {
                        let v = self.var_for_symbol[symbol];
                        // inline var_changed - borrow checker workaround
                        if self.should_clear_changes {
                            self.changed.clear();
                            self.should_clear_changes = false;
                        }
                        self.changed.insert(v);
                    }
                    if coeff != 0.0 && row.add(diff) < 0.0 && symbol.kind() != SymbolKind::External
                    {
                        infeasible_rows.push(*symbol);
                    }
                }
            }
        }
        self.dual_optimize()?;
        Ok(())
    }

    fn var_changed(&mut self, v: Variable) {
        if self.should_clear_changes {
            self.changed.clear();
            self.should_clear_changes = false;
        }
        self.changed.insert(v);
    }

    /// Fetches all changes to the values of variables since the last call to this function.
    ///
    /// The list of changes returned is not in a specific order. Each change comprises the variable
    /// changed and the new value of that variable.
    pub fn fetch_changes(&mut self) -> &[(Variable, f64)] {
        if self.should_clear_changes {
            self.changed.clear();
            self.should_clear_changes = false;
        } else {
            self.should_clear_changes = true;
        }
        self.public_changes.clear();
        for &v in &self.changed {
            if let Some(var_data) = self.var_data.get_mut(&v) {
                let new_value = self
                    .rows
                    .get(&var_data.1)
                    .map(|r| r.constant)
                    .unwrap_or(0.0);
                let old_value = var_data.0;
                if old_value != new_value {
                    self.public_changes.push((v, new_value));
                    var_data.0 = new_value;
                }
            }
        }
        &self.public_changes
    }

    /// Reset the solver to the empty starting condition.
    ///
    /// This method resets the internal solver state to the empty starting
    /// condition, as if no constraints or edit variables have been added.
    /// This can be faster than deleting the solver and creating a new one
    /// when the entire system must change, since it can avoid unnecessary
    /// heap (de)allocations.
    pub fn reset(&mut self) {
        self.rows.clear();
        if let Some(columns) = &mut self.columns {
            columns.clear();
        }
        self.constraints.clear();
        self.var_data.clear();
        self.var_for_symbol.clear();
        self.changed.clear();
        self.should_clear_changes = false;
        self.edits.clear();
        self.infeasible_rows.clear();
        self.objective = Row::new(0.0);
        self.artificial = None;
        self.id_tick = 1;
    }

    fn insert_row(&mut self, basic: Symbol, row: Box<Row>) {
        if let Some(columns) = &mut self.columns {
            for symbol in row.cells.keys() {
                columns.entry(*symbol).or_default().insert(basic);
            }
        }
        self.rows.insert(basic, row);
    }

    fn remove_row(&mut self, basic: Symbol) -> Option<Box<Row>> {
        let row = self.rows.remove(&basic)?;
        if let Some(columns) = &mut self.columns {
            for symbol in row.cells.keys() {
                if let Some(rows) = columns.get_mut(symbol) {
                    rows.remove(&basic);
                    if rows.is_empty() {
                        columns.remove(symbol);
                    }
                }
            }
        }
        Some(row)
    }

    fn remove_column(&mut self, symbol: Symbol) {
        if let Some(columns) = &mut self.columns {
            for basic in columns.remove(&symbol).unwrap_or_default() {
                self.rows.get_mut(&basic).unwrap().remove(symbol);
            }
        } else {
            for row in self.rows.values_mut() {
                row.remove(symbol);
            }
        }
    }

    fn candidate_rows(&self, symbol: Symbol) -> Vec<Symbol> {
        match &self.columns {
            Some(columns) => columns
                .get(&symbol)
                .into_iter()
                .flatten()
                .copied()
                .collect(),
            None => self.rows.keys().copied().collect(),
        }
    }

    fn forget_dummy(&mut self, symbol: Symbol) {
        if symbol.kind() != SymbolKind::Dummy {
            return;
        }
        if let Some(row) = self.rows.get(&symbol) {
            // Preserve the incremental solver's treatment of a tolerated, nonzero residual.
            if row.constant != 0.0 || !Self::all_dummies(row) {
                return;
            }
            self.remove_row(symbol);
        }
        self.remove_column(symbol);
        self.objective.remove(symbol);
    }

    /// Get the symbol for the given variable.
    ///
    /// If a symbol does not exist for the variable, one will be created.
    fn get_var_symbol(&mut self, v: Variable) -> Symbol {
        let id_tick = &mut self.id_tick;
        let var_for_symbol = &mut self.var_for_symbol;
        let value = self.var_data.entry(v).or_insert_with(|| {
            let s = Symbol::new(*id_tick, SymbolKind::External);
            var_for_symbol.insert(s, v);
            *id_tick += 1;
            (f64::NAN, s, 0)
        });
        value.2 += 1;
        value.1
    }

    /// Create a new Row object for the given constraint.
    ///
    /// The terms in the constraint will be converted to cells in the row. Any term in the
    /// constraint with a coefficient of zero is ignored. This method uses the `get_var_symbol`
    /// method to get the symbol for the variables added to the row. If the symbol for a given cell
    /// variable is basic, the cell variable will be substituted with the basic row.
    ///
    /// The necessary slack and error variables will be added to the row. If the constant for the
    /// row is negative, the sign for the row will be inverted so the constant becomes positive.
    ///
    /// The tag will be updated with the marker and error symbols to use for tracking the movement
    /// of the constraint in the tableau.
    fn create_row(&mut self, constraint: &Constraint) -> (Box<Row>, Tag) {
        let expr = constraint.expr();
        let mut row = Row::new(expr.constant);

        // Substitute the current basic variables into the row.
        for term in &expr.terms {
            if !near_zero(term.coefficient) {
                let symbol = self.get_var_symbol(term.variable);
                if let Some(other_row) = self.rows.get(&symbol) {
                    row.insert_row(other_row, term.coefficient);
                } else {
                    row.insert_symbol(symbol, term.coefficient);
                }
            }
        }

        let objective = &mut self.objective;

        // Add the necessary slack, error, and dummy variables.
        let tag = match constraint.op() {
            RelationalOperator::GreaterOrEqual | RelationalOperator::LessOrEqual => {
                let coeff = if constraint.op() == RelationalOperator::LessOrEqual {
                    1.0
                } else {
                    -1.0
                };
                let slack = Symbol::new(self.id_tick, SymbolKind::Slack);
                self.id_tick += 1;
                row.insert_symbol(slack, coeff);
                if constraint.strength() < Strength::REQUIRED {
                    let error = Symbol::new(self.id_tick, SymbolKind::Error);
                    self.id_tick += 1;
                    row.insert_symbol(error, -coeff);
                    objective.insert_symbol(error, constraint.strength().value());
                    Tag {
                        marker: slack,
                        other: error,
                    }
                } else {
                    Tag {
                        marker: slack,
                        other: Symbol::invalid(),
                    }
                }
            }
            RelationalOperator::Equal => {
                if constraint.strength() < Strength::REQUIRED {
                    let errplus = Symbol::new(self.id_tick, SymbolKind::Error);
                    self.id_tick += 1;
                    let errminus = Symbol::new(self.id_tick, SymbolKind::Error);
                    self.id_tick += 1;
                    row.insert_symbol(errplus, -1.0); // v = eplus - eminus
                    row.insert_symbol(errminus, 1.0); // v - eplus + eminus = 0
                    objective.insert_symbol(errplus, constraint.strength().value());
                    objective.insert_symbol(errminus, constraint.strength().value());
                    Tag {
                        marker: errplus,
                        other: errminus,
                    }
                } else {
                    let dummy = Symbol::new(self.id_tick, SymbolKind::Dummy);
                    self.id_tick += 1;
                    row.insert_symbol(dummy, 1.0);
                    Tag {
                        marker: dummy,
                        other: Symbol::invalid(),
                    }
                }
            }
        };

        // Ensure the row has a positive constant.
        if row.constant < 0.0 {
            row.reverse_sign();
        }
        (Box::new(row), tag)
    }

    /// Choose the subject for solving for the row.
    ///
    /// This method will choose the best subject for using as the solve
    /// target for the row. An invalid symbol will be returned if there
    /// is no valid target.
    ///
    /// The symbols are chosen according to the following precedence:
    ///
    /// 1) The first symbol representing an external variable.
    /// 2) A negative slack or error tag variable.
    ///
    /// If a subject cannot be found, an invalid symbol will be returned.
    fn choose_subject(row: &Row, tag: &Tag) -> Symbol {
        for s in row.cells.keys() {
            if s.kind() == SymbolKind::External {
                return *s;
            }
        }
        if (tag.marker.kind() == SymbolKind::Slack || tag.marker.kind() == SymbolKind::Error)
            && row.coefficient_for(tag.marker) < 0.0
        {
            return tag.marker;
        }
        if (tag.other.kind() == SymbolKind::Slack || tag.other.kind() == SymbolKind::Error)
            && row.coefficient_for(tag.other) < 0.0
        {
            return tag.other;
        }
        Symbol::invalid()
    }

    /// Add the row to the tableau using an artificial variable.
    ///
    /// This will return false if the constraint cannot be satisfied.
    fn add_with_artificial_variable(&mut self, row: &Row) -> Result<bool, InternalSolverError> {
        // Create and add the artificial variable to the tableau
        let art = Symbol::new(self.id_tick, SymbolKind::Slack);
        self.id_tick += 1;
        self.insert_row(art, Box::new(row.clone()));
        self.artificial = Some(row.clone());

        // Optimize the artificial objective. This is successful
        // only if the artificial objective is optimized to zero.
        self.optimize(OptimizeTarget::Artificial)?;
        let success = near_zero(self.artificial.as_ref().unwrap().constant);
        self.artificial = None;

        // If the artificial variable is basic, pivot the row so that
        // it becomes basic. If the row is constant, exit early.
        if let Some(mut row) = self.remove_row(art) {
            if row.cells.is_empty() {
                return Ok(success);
            }
            let entering = Solver::any_pivotable_symbol(&row); // never External
            if entering.kind() == SymbolKind::Invalid {
                return Ok(false); // unsatisfiable (will this ever happen?)
            }
            row.solve_for_symbols(art, entering);
            self.substitute(entering, &row);
            self.insert_row(entering, row);
        }

        // Remove the artificial row from the tableau
        self.remove_column(art);
        self.objective.remove(art);
        Ok(success)
    }

    /// Substitute the parametric symbol with the given row.
    ///
    /// This method will substitute all instances of the parametric symbol
    /// in the tableau and the objective function with the given row.
    fn substitute(&mut self, symbol: Symbol, row: &Row) {
        if let Some(columns) = &mut self.columns {
            let affected = columns.remove(&symbol).unwrap_or_default();
            for other_symbol in affected {
                let other_row = self.rows.get_mut(&other_symbol).unwrap();
                // Only the replaced symbol and the incoming row's cells can change membership.
                // Reindexing the whole destination row makes dense weighted models quadratic again.
                let previous: Vec<_> = row
                    .cells
                    .keys()
                    .map(|cell| (*cell, other_row.cells.contains_key(cell)))
                    .collect();
                let constant_changed = other_row.substitute(symbol, row);
                for (cell, existed) in previous {
                    let exists = other_row.cells.contains_key(&cell);
                    if exists && !existed {
                        columns.entry(cell).or_default().insert(other_symbol);
                    } else if existed && !exists {
                        let basics = columns.get_mut(&cell).unwrap();
                        basics.remove(&other_symbol);
                        if basics.is_empty() {
                            columns.remove(&cell);
                        }
                    }
                }
                let infeasible =
                    other_symbol.kind() != SymbolKind::External && other_row.constant < 0.0;
                if other_symbol.kind() == SymbolKind::External && constant_changed {
                    // Inline var_changed to keep the two disjoint field borrows local.
                    if self.should_clear_changes {
                        self.changed.clear();
                        self.should_clear_changes = false;
                    }
                    self.changed.insert(self.var_for_symbol[&other_symbol]);
                }
                if infeasible {
                    self.infeasible_rows.push(other_symbol);
                }
            }
        } else {
            for (&other_symbol, other_row) in &mut self.rows {
                let constant_changed = other_row.substitute(symbol, row);
                if other_symbol.kind() == SymbolKind::External && constant_changed {
                    let v = self.var_for_symbol[&other_symbol];
                    // inline var_changed
                    if self.should_clear_changes {
                        self.changed.clear();
                        self.should_clear_changes = false;
                    }
                    self.changed.insert(v);
                }
                if other_symbol.kind() != SymbolKind::External && other_row.constant < 0.0 {
                    self.infeasible_rows.push(other_symbol);
                }
            }
        }
        self.objective.substitute(symbol, row);
        if let Some(ref mut artificial) = self.artificial {
            artificial.substitute(symbol, row);
        }
    }

    /// Optimize the system for the given objective function.
    ///
    /// This method performs iterations of Phase 2 of the simplex method
    /// until the objective function reaches a minimum.
    fn optimize(&mut self, target: OptimizeTarget) -> Result<(), InternalSolverError> {
        loop {
            let entering = {
                let objective = match target {
                    OptimizeTarget::Objective => &self.objective,
                    OptimizeTarget::Artificial => self.artificial.as_ref().unwrap(),
                };
                Solver::get_entering_symbol(objective)
            };
            if entering.kind() == SymbolKind::Invalid {
                return Ok(());
            }
            let (leaving, mut row) = self
                .get_leaving_row(entering)
                .ok_or(InternalSolverError::ObjectiveUnbounded)?;
            // pivot the entering symbol into the basis
            row.solve_for_symbols(leaving, entering);
            self.substitute(entering, &row);
            if entering.kind() == SymbolKind::External && row.constant != 0.0 {
                let v = self.var_for_symbol[&entering];
                self.var_changed(v);
            }
            self.insert_row(entering, row);
        }
    }

    /// Optimize the system using the dual of the simplex method.
    ///
    /// The current state of the system should be such that the objective
    /// function is optimal, but not feasible. This method will perform
    /// an iteration of the dual simplex method to make the solution both
    /// optimal and feasible.
    fn dual_optimize(&mut self) -> Result<(), InternalSolverError> {
        while let Some(leaving) = self.infeasible_rows.pop() {
            let row = if self
                .rows
                .get(&leaving)
                .is_some_and(|row| row.constant < 0.0)
            {
                self.remove_row(leaving)
            } else {
                None
            };
            if let Some(mut row) = row {
                let entering = self.get_dual_entering_symbol(&row);
                if entering.kind() == SymbolKind::Invalid {
                    return Err(InternalSolverError::DualOptimizeFailed);
                }
                // pivot the entering symbol into the basis
                row.solve_for_symbols(leaving, entering);
                self.substitute(entering, &row);
                if entering.kind() == SymbolKind::External && row.constant != 0.0 {
                    let v = self.var_for_symbol[&entering];
                    self.var_changed(v);
                }
                self.insert_row(entering, row);
            }
        }
        Ok(())
    }

    /// Compute the entering variable for a pivot operation.
    ///
    /// This method will return first symbol in the objective function which
    /// is non-dummy and has a coefficient less than zero. If no symbol meets
    /// the criteria, it means the objective function is at a minimum, and an
    /// invalid symbol is returned.
    /// Could return an External symbol
    fn get_entering_symbol(objective: &Row) -> Symbol {
        for (symbol, value) in &objective.cells {
            if symbol.kind() != SymbolKind::Dummy && *value < 0.0 {
                return *symbol;
            }
        }
        Symbol::invalid()
    }

    /// Compute the entering symbol for the dual optimize operation.
    ///
    /// This method will return the symbol in the row which has a positive
    /// coefficient and yields the minimum ratio for its respective symbol
    /// in the objective function. The provided row *must* be infeasible.
    /// If no symbol is found which meats the criteria, an invalid symbol
    /// is returned.
    /// Could return an External symbol
    fn get_dual_entering_symbol(&self, row: &Row) -> Symbol {
        let mut entering = Symbol::invalid();
        let mut ratio = f64::INFINITY;
        for (symbol, value) in &row.cells {
            if *value > 0.0 && symbol.kind() != SymbolKind::Dummy {
                let coeff = self.objective.coefficient_for(*symbol);
                let r = coeff / *value;
                if r < ratio {
                    ratio = r;
                    entering = *symbol;
                }
            }
        }
        entering
    }

    /// Get the first Slack or Error symbol in the row.
    ///
    /// If no such symbol is present, and Invalid symbol will be returned.
    /// Never returns an External symbol
    fn any_pivotable_symbol(row: &Row) -> Symbol {
        for symbol in row.cells.keys() {
            if symbol.kind() == SymbolKind::Slack || symbol.kind() == SymbolKind::Error {
                return *symbol;
            }
        }
        Symbol::invalid()
    }

    /// Compute the row which holds the exit symbol for a pivot.
    ///
    /// This method will return an iterator to the row in the row map
    /// which holds the exit symbol. If no appropriate exit symbol is
    /// found, the end() iterator will be returned. This indicates that
    /// the objective function is unbounded.
    /// Never returns a row for an External symbol
    fn get_leaving_row(&mut self, entering: Symbol) -> Option<(Symbol, Box<Row>)> {
        let mut ratio = f64::INFINITY;
        let mut found = None;
        for symbol in self.candidate_rows(entering) {
            let row = &self.rows[&symbol];
            if symbol.kind() != SymbolKind::External {
                let temp = row.coefficient_for(entering);
                if temp < 0.0 {
                    let temp_ratio = -row.constant / temp;
                    if temp_ratio < ratio {
                        ratio = temp_ratio;
                        found = Some(symbol);
                    }
                }
            }
        }
        found.map(|s| (s, self.remove_row(s).unwrap()))
    }

    /// Compute the leaving row for a marker variable.
    ///
    /// This method will return an iterator to the row in the row map
    /// which holds the given marker variable. The row will be chosen
    /// according to the following precedence:
    ///
    /// 1) The row with a restricted basic varible and a negative coefficient for the marker with
    ///    the smallest ratio of -constant / coefficient.
    ///
    /// 2) The row with a restricted basic variable and the smallest ratio of constant /
    ///    coefficient.
    ///
    /// 3) The last unrestricted row which contains the marker.
    ///
    /// If the marker does not exist in any row, the row map end() iterator
    /// will be returned. This indicates an internal solver error since
    /// the marker *should* exist somewhere in the tableau.
    fn get_marker_leaving_row(&mut self, marker: Symbol) -> Option<(Symbol, Box<Row>)> {
        let mut r1 = f64::INFINITY;
        let mut r2 = r1;
        let mut first = None;
        let mut second = None;
        let mut third = None;
        for symbol in self.candidate_rows(marker) {
            let row = &self.rows[&symbol];
            let c = row.coefficient_for(marker);
            if c == 0.0 {
                continue;
            }
            if symbol.kind() == SymbolKind::External {
                third = Some(symbol);
            } else if c < 0.0 {
                let r = -row.constant / c;
                if r < r1 {
                    r1 = r;
                    first = Some(symbol);
                }
            } else {
                let r = row.constant / c;
                if r < r2 {
                    r2 = r;
                    second = Some(symbol);
                }
            }
        }
        first.or(second).or(third).and_then(|s| {
            if s.kind() == SymbolKind::External && self.rows[&s].constant != 0.0 {
                let v = self.var_for_symbol[&s];
                self.var_changed(v);
            }
            self.remove_row(s).map(|r| (s, r))
        })
    }

    /// Remove the effects of a constraint on the objective function.
    fn remove_constraint_effects(&mut self, constraint: &Constraint, tag: &Tag) {
        if tag.marker.kind() == SymbolKind::Error {
            self.remove_marker_effects(tag.marker, constraint.strength().value());
        }
        if tag.other.kind() == SymbolKind::Error {
            self.remove_marker_effects(tag.other, constraint.strength().value());
        }
    }

    /// Remove the effects of an error marker on the objective function.
    fn remove_marker_effects(&mut self, marker: Symbol, strength: f64) {
        if let Some(row) = self.rows.get(&marker) {
            self.objective.insert_row(row, -strength);
        } else {
            self.objective.insert_symbol(marker, -strength);
        }
    }

    /// Test whether a row is composed of all dummy variables.
    fn all_dummies(row: &Row) -> bool {
        for symbol in row.cells.keys() {
            if symbol.kind() != SymbolKind::Dummy {
                return false;
            }
        }
        true
    }

    /// Get the stored value for a variable.
    ///
    /// Normally values should be retrieved and updated using `fetch_changes`, but this method can
    /// be used for debugging or testing.
    pub fn get_value(&self, v: Variable) -> f64 {
        self.var_data
            .get(&v)
            .and_then(|s| self.rows.get(&s.1).map(|r| r.constant))
            .unwrap_or(0.0)
    }
}
