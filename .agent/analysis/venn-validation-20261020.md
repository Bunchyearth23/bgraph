# Venn readability — 2026-10-20

Work: W-171. Validation preceded the documentation and Venn commit. User-installed binaries and extensions have not been replaced; Git records the subsequently requested commit/push delivery.

## Result

Circle areas still encode set totals. Three-circle placement now uses the lens-derived pair distances when they form a triangle, with explicit handling for coincident centers. Infeasible distances keep the schematic fallback and an indicative-area warning. Two-set geometry keeps the existing exact lens solution.

A/B/C connect the circles to colored set keys. Names wrap outside the picture; totals use a separate aligned column. Each row reserves its resolved font height and the columns allow for the largest font. Circle letters prefer exclusive space, then avoid outlines and other letters; a circle too small to hold a letter keeps its key in the legend.

Shared counts are named by pair in a separate key. They include members of the third set; the renderer does not invent exclusive pair counts or a triple count. Omitted links mean zero. This removes the old bare numbers, whose center-to-center positions could be outside a small lens or inside the triple region.

No grammar, dependency, property or export API was added. The English beginner guide, generated reference, init example, checked-in Venn source/SVG and only the Venn golden were updated. The other gallery goldens pass unchanged.

## Evidence

- 751 Rust tests passed, zero failed, one existing manual performance test ignored. Six new regressions cover feasible three-pair areas, named totals, wrapped/mixed-font text clearance, circle-key clearance, finite/identical/contained/empty cases and missing/zero intersections. Repeated rendering is byte-identical.
- Clippy for all targets/features with warnings denied, formatting, the Venn-only no-default-feature build and whitespace checks passed.
- Runtime gate with extensions passed: 22 fixtures across CLI, LSP, browser WASM, WASI, watch, Typst and the packaged VS Code renderer; 30 gallery types and four file contexts also passed. Six JS extension tests passed. This executes the renderer but does not rerun a native GUI host session.
- Ten fixtures produced 40 successful SVG/PNG/PDF/JSON exports: normal three-set example, two partial sets, unequal circles, containment, identical circles, empty sets, long names with mixed fonts, disjoint sets, infeasible distances and a dark surface. SVGs parse as XML, PNGs decode, JSON coordinates are finite and pdfinfo accepts all ten one-page PDFs.
- The PNG contact sheet and final normal/long-name pictures were reviewed. A PDF with an explicitly embedded installed DejaVu Sans was also reviewed. Default PDFs retain the existing viewer font-substitution behavior; their parser/text checks passed.
- Strict MkDocs build passed. The guide's complete Venn example is the same data exercised by the regression and export checks.
- Release CLI and local VSIX were rebuilt. The release CLI and the actual packaged release WASM produce identical SVG on all ten Venn fixtures.

Golden: `venn be043a9c49825e07`. Hashes and structured results are in `venn-validation-20261020.json`. Temporary export evidence is in `/tmp/bgraph-venn-qa-7gmet979/`; the durable before/after images are beside this report.

## Learning check

`trail check` exits 1 with 20 matches in the already edited documentation and INDEX. These are lexical matches, not conflicting Venn implementations. The checker treats ordinary terms such as “file”, “build”, “preview”, and dates in report paths as restoring prohibited behavior. Each match was read: the docs still quote filenames, preserve the preview installation boundary, execute complete examples and describe the existing packaging and routing behavior. The new Venn geometry keeps named pair totals and the indicative-area warning. No filename interpolation, frame-origin constraint or router implementation was changed.

The existing team learning `2026-10-20-learning-checker-vocabulary-matches-need-semantic-arnaud.md` documents this checker limitation. Carry this rationale into a commit or PR description if one is requested; this result is not counted as a passing check. The Venn learning accompanies the change.

## Remaining boundary

The user accepted the candidate for commit/push. Installation and user runtime validation remain. Three-set regions are indicative, and infeasible pair distances remain schematic. Validation did not perform source publication, commit, push, a new remote CI run or a user-profile installation; the user subsequently requested Git delivery. Existing English documentation and unrelated tool-configuration edits were preserved.
