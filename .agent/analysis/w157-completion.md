# W-157 — minimum-cardinality source conflicts

## Contract and implementation

`src/constraint_layout/conflict.rs` searches the entire required-constraint
model. The initial failing insertion is only a hint for a first upper bound;
it does not truncate the universe. Every singleton is checked first. Block
removal obtains an irreducible upper bound, then an iterative exclusion search
explores alternate cores. Any smaller conflict must omit at least one member
of the current irreducible core. Branches therefore cover every potentially
smaller conflict. Satisfiable supersets and visited exclusion sets prune only
branches that cannot contain one. Cardinality two is optimal after all
singletons have been proved feasible. No time, size or cardinality cutoff can
silently replace the exact result. Worst-case search is exponential, inherent
to this exact requirement.

Each failed independent frame/placement component is searched, and the global
result is selected by cardinality before canonical original constraint indices.
The published relations retain original source spans, axis, origin, operands,
values and transitive size/attachment provenance. Output ordering is canonical.
Nonlinear frame-bound consistency remains a separate source validation error.

Duplicate insertion of the same handle is idempotent. Only
`UnsatisfiableConstraint` becomes a source conflict. Other solver errors
propagate as typed errors; no internal failure is presented as an author mistake.

## Executed proofs

Eight focused tests passed in the current goal pass, including:

- A greedy irreducible core of four versus the actual minimum of two.
- Smaller conflicts after the first failing prefix, including a later singleton.
- A minimum of three with all pairs proved satisfiable.
- Twenty-four generated seven-constraint linear models compared against an
  exhaustive enumeration of all nonempty subsets.
- A real DSL diagram whose earlier component has a four-equation conflict and
  later component has three; the later minimum is selected.
- Weak constraints excluded from source contradictions, empty models,
  duplicate-handle insertion and downcast-preserved internal engine errors.

Final focused rerun exited **0**, eight tests passed (0.04 s); output:
`/tmp/bgraph-goal-conflict-final.log`. Native CLI check also exited 1 as
required for this invalid source, with exactly three diagnostics on lines
10, 11 and 12 (`/tmp/bgraph-goal-minimum-source.json`). The final runtime gate adds `minimum-conflict.bd` and
requires exactly the three later source lines, then compares the diagnostic
through CLI, LSP, browser wasm, WASI, preview, Typst and packaged VS Code.
That end-to-end gate completed **exit 0** on portable-math sources: 22 fixtures,
30 archetypes and four file contexts. `/tmp/bgraph-goal-runtime-final.log`
contains the explicit minimum-conflict PASS and every surface comparison.

## Independent logic review

A second agent reviewed the exclusion-search invariant and the full-component
partition/ranking separately from implementation and tests; no cardinality
defect was found. Singleton lower bound and satisfiable-exclusion pruning were
checked explicitly. Selection is deterministic; it does not promise the
lexicographically first tied minimum inside a component.
