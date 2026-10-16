# W-153 — one simultaneous placement solver

## Implemented contract

The declared-layout production path is now `layout::solve` →
`constraint_layout::solve_declared` → `frames::solve`, for every graph size,
with or without frames. The 512-entity threshold, node-only alternate solver
and fixed-geometry `validate_frames` fallback are removed. The previous
`place_all`/alignment/subtree translation implementation remains only behind
`cfg(test)`, as an oracle and an explicitly ignored comparison benchmark.

Every supported node placement, frame containment and local translation,
alignment, dimension equality and BPMN boundary attachment is lowered into
the same model. Members of placed nested frames retain local `at` origins;
external anchors refer to final world coordinates. Attachment equations and
their dependents are simultaneous, including consistent event/host cycles at
any graph size. Contradictory cycles fail with sourced constraints. Ordinary
non-attachment dependency cycles retain the existing source validator policy.

No collision correction or additional hard origin is introduced. Existing
container origins are inserted before their dependent relations; this avoids
repeatedly changing the free basis of a chain that is already required to
start at that origin.

Frame containment includes padding, title and node-caption overhang. Tight
left/top preferences and weak width/height minimisation select the content
bounds. The shared validator recomputes the exact content bounds and rejects
a slack or inflated frame; it never moves source geometry to repair it.
Min/max bounds are piecewise linear, so this postcondition remains necessary.
There is no other geometry producer hidden behind that validation.

## Performance architecture and invariants

The vendored Kasuari `StaticSolver` exposes only addition and value lookup.
It has no `Deref`, constraint removal, edit variable or incremental mutation
API. It preserves constraint handles, duplicate classification and typed
internal errors. It can forget only an internal required Dummy marker fixed
to exactly zero; a tolerated nonzero residual is retained. Slack and Error
symbols, source requirements and weighted preferences are never discarded.

An ordered column incidence index limits substitution and leaving-row scans
to affected rows. Substitution updates only changed cell memberships, without
reindexing an entire destination row. The standard incremental `Solver` keeps
its original full-row iteration and API semantics. Shared pivot, artificial
variable, removal and reset paths maintain the index when enabled.

Models are partitioned by variable connectivity, including a component for
constant-only constraints. Canonical source order remains unchanged for
conflict selection. Insertion alone follows a deterministic dependency
schedule (anchor → entity, member → frame, ties and scheduling-cycle breaks
by entity name). No equation is added to break a scheduling cycle. Every
unsatisfiable component passes its **complete** model to W-157's exact
minimum-cardinality conflict extraction; components are ranked by core size,
then sorted canonical source indices. Internal solver errors are propagated.

The post-layout overlap validator uses an interval sweep on the axis with
fewer projected candidate intersections. Its final strict rectangle predicate
is unchanged. Actual pairs are restored to declaration-pair order before
producing the original messages/spans; BPMN host/event exemptions remain.
No pair limit or warning limit hides real collisions. Dense overlapping
inputs can still require quadratic output, because they have that many actual
diagnostics. Edge routing and label checks retain their existing algorithms.

## Functional evidence on these sources

All commands below exited 0; older, interrupted runs are not counted.

| Command / log | Result |
|---|---|
| `CARGO_TARGET_DIR=target/solver-work cargo test --lib --all-features` — `/tmp/bgraph-w153-allfeatures-final2.log` | **685 passed, 0 failed, 1 ignored**, 17.02 s. Includes every gallery SVG/PDF and existing origin/alignment/size provenance regressions. |
| `CARGO_TARGET_DIR=target/solver-work cargo test --lib tests::hostile_input_never_panics -- --exact --nocapture` — `/tmp/bgraph-w153-hostile-final.log` | 1 passed, 8.60 s, including 5,000 implicit nodes and 2,000 explicit anchors. |
| `CARGO_TARGET_DIR=target/solver-vendor cargo test --manifest-path vendor/kasuari/Cargo.toml --lib` — `/tmp/bgraph-w153-vendor-final2.log` | **107 passed**. Existing incremental API suite plus StaticSolver bitwise oracles after every addition, tolerated residual/duplicate/conflict cases, 64 generated required/weighted models, indexed pivot/edit/remove/artificial/reset invariants and a 2,000-variable linear-tableau chain. |
| `CARGO_TARGET_DIR=target/solver-work cargo clippy --all-targets --all-features -- -D warnings` — `/tmp/bgraph-w153-clippy-final.log` | Passed; also compiles integration and benchmark targets. |
| `CARGO_TARGET_DIR=target/solver-work cargo check --no-default-features --lib` — `/tmp/bgraph-w153-nodefault-final.log` | Passed. |
| `git diff --check` | Passed. |

The dedicated `constraint_layout/completion_tests.rs` adds seven regressions:
2,000 connected required placements; 600 explicitly placed frames; 1,000
frames in twenty depth-50 chains; coherent and contradictory BPMN cycles
above the old threshold; raw-bit geometry under source/layout permutations;
and attached-event downstream placement with stable raw geometry. The overlap
module compares the sweep against the original all-pairs predicate across
64 generated models (including zero/negative sizes and nonfinite public Rects)
and checks both axes of a nonoverlapping 20,000-box chain.

Stable source fixtures, available to the separate native/runtime gates:

- `tests/fixtures/placement/bpmn-dependent.bd`
- `tests/fixtures/placement/bpmn-attach-conflict.bd`
- `tests/fixtures/placement/bpmn-cycle.bd`
- `tests/fixtures/placement/bpmn-cycle-conflict.bd`

The standalone upstream tests require `rstest = "0.26"`, restored as a
**dev-only** dependency from the upstream
[Kasuari 0.4.12 manifest](https://raw.githubusercontent.com/ratatui/kasuari/v0.4.12/Cargo.toml).
No production dependency was added.

## Performance guard

`CARGO_TARGET_DIR=target/solver-bench cargo bench --bench pipeline -- --check`
exited **0**. Log: `/tmp/bgraph-w153-bench-guard.log`. The benchmark includes
parse, layout (and its source validation), post-layout validation and SVG
generation. `public-5000` directly calls `render_source`, including the public
analysis/validator funnel. Times are observed on the shared machine under
concurrent load; no before/after speed ratio is claimed.

| Case | Repetitions | Mean layout | Mean complete invocation | Allowed complete invocation |
|---|---:|---:|---:|---:|
| 2,000 connected nodes | 2 | 301.68 ms | 396.80 ms | 500 ms |
| 20,000 connected nodes | 2 | 1.547 s | 2.067 s | 3 s |
| 600 sibling frames | 2 | 59.10 ms | 84.11 ms | 500 ms |
| 1,000 nested frames, depth 50 | 2 | 748.25 ms | 767.37 ms | 2 s |
| Public `render_source`, 5,000 nodes | 2 | included | 461.23 ms | 2 s |

All original benchmark cases retain ten repetitions and their original
five-second **total** guard, including composition and routing. The large
cases use two repetitions with explicit per-invocation budgets to contain CI
duration; none is skipped by `--check`. Measured post-layout validation of
20,000 nodes is 137.01 ms total for two calls (68.51 ms per call).

## Audit and scope

`graft callers place_all --depth all` finds only the two test-oracle callers.
`graft grep 'MAX_DIRECT_NODES|uses_simultaneous_solver|solve_nodes|validate_frames' --in src/`
finds no remaining references. Production does not compute geometry with the
previous fixpoint, irrespective of graph size or attachments.

This report proves the solver, validator and local performance guard. Native
cross-process/cross-platform and browser/WASI/Typst/editor evidence belongs
to W-156/W-160's separate final-source gates. Simultaneous changes to bare
BPMN item defaults belong to W-164 and must be covered by the coordinator's
final integrated run; this report does not substitute older results for that
run. No commit or publication was performed.

Graft context saved approximately **442,286 tokens across 10 retrieval calls**
in this goal pass (including the initial strategy/call-closure retrievals).

## Final-source guard after portable math integration

`CARGO_TARGET_DIR=target/solver-bench cargo bench --locked --bench pipeline -- --check`
was rerun on the frozen production sources, including the W-156 portable math
implementation. It exited **0**; `/tmp/bgraph-goal-benchmark-final.log`.
No budget, iteration count or case was changed for this rerun.

| Case | Repetitions | Mean layout | Mean complete invocation |
|---|---:|---:|---:|
| 2,000 connected nodes | 2 | 52.16 ms | 71.89 ms |
| 20,000 connected nodes | 2 | 877.00 ms | 1.116 s |
| 600 sibling frames | 2 | 34.98 ms | 48.44 ms |
| 1,000 nested frames | 2 | 424.77 ms | 434.84 ms |
| Public `render_source`, 5,000 nodes | 2 | included | 222.29 ms |

All original cases still use ten repetitions and pass their original
five-second total guard. The 20,000-node post-layout validation phase totals
85.70 ms for two calls (42.85 ms per call). These are final observed durations,
not a speed ratio against the earlier shared-load run.
