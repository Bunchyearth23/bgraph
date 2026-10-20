# W-140 to W-150 — task list

Historical implementation order: secure configurable surfaces, complete
contracts and tools, measure and harden, then improve usability and rendering.

## W-140 — feature matrix

- [x] Add the build matrix to CI.
- [x] Check minimal, `pie`, `packet`, JSON/CSV/PDF, WASM, PNG and CLI/completions builds locally.
- [x] Add a test preventing accidental reductions to the CI matrix.

## W-141 — LSP modularization

- [x] Extract positions, protocol, diagnostics and lexical context.
- [x] Extract completion and snippet methods.
- [x] Extract navigation, hover, symbols and references.
- [x] Move unit tests closer to their modules.
- [x] Check internal interfaces and measure the reduction in `src/lsp.rs`.

## W-142 — global layout

- [x] Define a global cost function for labels and routes.
- [x] Add residual-conflict diagnostics to layout JSON.
- [x] Add label-contention and orthogonal-corridor cases.
- [x] Check determinism and golden-output regressions.

## W-143 — fuzzing

- [x] Create a versioned hostile-input corpus.
- [x] Fuzz parsing, validation, layout and SVG/PDF/JSON rendering.
- [x] Run the corpus in CI.
- [x] Turn discovered regressions into deterministic tests.

## W-144 — benchmarks

- [x] Define large-graph, composition, orthogonal-routing, template and chart fixtures.
- [x] Measure parsing, layout and rendering separately.
- [x] Keep results or regression thresholds in CI.

## W-145 — public API

- [x] List public modules and types that consumers can actually use.
- [x] Reduce internal visibility or document the lack of stability.
- [x] Verify Rust integration examples against that boundary.

## W-146 — JSON diagnostics

- [x] Add `bgraph check --json` for simple and composed files, including composition errors.
- [x] Add CLI tests for the schema and exit status.
- [x] Document the stable schema in the reference.

## W-147 — onboarding

- [x] Rewrite the README around a first diagram and preview.
- [x] Add a progression from styles to data to the Rust library.
- [x] Verify every command and snippet in the suite.

## W-148 — documentation consistency

- [x] Choose the canonical count: “29 specialised genres, plus free”.
- [x] Align README, manual, release notes, playground and generated text.
- [x] Add a guard against inconsistent counts.

## W-149 — visual snapshots

- [x] Select representative shape, label and chart fixtures.
- [x] Generate reproducible PNG snapshots.
- [x] Compare images with an explicit CI threshold.

## W-150 — vocabulary schema

- [x] Define the versioned metadata format.
- [x] Generate the schema from `props.rs`.
- [x] Expose `bgraph schema`.
- [x] Test that the schema stays synchronized with compiler tables.
