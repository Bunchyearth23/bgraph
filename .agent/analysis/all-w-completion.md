# All non-blocked W: completion audit

Objective, explicitly requested: **all non blocked W done and proven**.
Initial remaining scope: W-153, W-156, W-157, W-158, W-159, W-160 and W-164
(styles). W-154 and W-155 were already closed after 21 targeted tests and the
runtime gate. The preexisting W-164 ID collision with the landed export remains
unchanged. No item is removed from this objective to make completion easier.

The preceding turn made progress: W-154/W-155 transitioned to landed with
fresh evidence. This goal pass adds implementation and evidence; the goal
remains active until every requirement below is proved.

## Requirements and authoritative evidence

| Work | Completion requirement | Current evidence / remaining proof |
|---|---|---|
| W-153 | One simultaneous declared solver for nodes, frames, attachments and large diagrams; no production fallback or silent displacement; source validation, nested local origins and cycles; viable performance. | **Landed.** Unique solver at every size, 107 vendor tests and seven completion regressions; final full suite and runtime gate green. Final guard: 20k nodes 1.116s per complete call; no budget increased. `w153-completion.md`. |
| W-156 | Same source/version gives identical raw coordinates and output between repeated runs and every supported native/wasm runtime. | **Landed.** Five real native runner jobs and aggregate comparison succeeded at `90163b7`; all 52 cases strictly identical, repeated in memory and fresh processes. Independent download/digest audit: `native-ci-proof.json`. Global font-test failures remain in W-160. |
| W-157 | Minimum-cardinality source conflict, provenance and values, across the whole model; deterministic diagnostics on every surface; internal errors remain typed. | **Landed.** Eight tests including 24 exhaustive seven-equation model oracles; independent proof review; later smaller component selected; exact diagnostic across every runtime. `w157-completion.md`. |
| W-158 | Visible preview inspector for bounds, anchors, parents, X/Y relations and conflicts; source/geometry selection; LSP declaration↔layout navigation, provenance hover and outline. | **Landed.** 75 LSP / 14 watch / 4 explain tests, actual Chromium six scenarios with reviewed screenshots and zero JS errors; real VS Code navigation/references/hover/outline passes. `w158-completion.md`. |
| W-159 | Exhaustive Declared/ArchetypeManaged/Hybrid contract in vocabulary; validation, completion, docs/schema and solver dispatch derived once; nothing accepted then ignored. | **Landed.** 9 Declared / 21 Managed / no actual Hybrid. Exhaustive matrix and all 30 final feature configurations pass; complete gallery/runtime proof. `w159-completion.md`. |
| W-160 | Reproducible no-default/CLI/LSP/browser/WASI/preview/Typst/VSIX gate and actual editor loading; native CI really executed, not merely configured. | **Landed.** Final hosted run `38898281988` succeeds at `8630085`: all 16 jobs, 714 tests, 22/30/4 runtime gate, five real native targets, strict comparison and performance guard. Licensed fixed-font and embedded-PNG corrections pass without skipped tests. `w160-completion.md`, `w160-font-portability.md`. |
| W-164 styles | Canonical archetype target for every alternative word, correct cascade and override provenance, editor completion/hover and generated reference. | **Landed.** Six public integrations and four correctly shaped BPMN words verified, 40 paint/override combinations, real LSP theme/provenance, generated reference current. `w164-completion.md`. |

## Final integrated evidence

Every command below completed with **exit 0** on the final portable-math code:

- `CARGO_TARGET_DIR=target/goal-root cargo test --locked --all-features`:
  **689 library tests**, one intentionally ignored historical benchmark;
  one binary test, nine solver integrations, one LSP integration, one native
  integration, **six public style integrations**, three doctests — **710 passed**.
  `/tmp/bgraph-goal-allfeatures-complete.log`.
- `CARGO_TARGET_DIR=target/goal-clippy cargo clippy --locked --all-targets --all-features -- -D warnings`:
  `/tmp/bgraph-goal-clippy-complete.log`.
- `cargo fmt --all --check`: `/tmp/bgraph-goal-fmt-complete.log`.
- `./tests/runtime/gate.sh --extensions`: **22 fixtures, 30 archetypes and four
  file contexts**, including minimum-cardinality selection and four BPMN
  attachment/cycle fixtures. `/tmp/bgraph-goal-runtime-final.log`.
- `bash tests/runtime/vscode-host.sh`: **seven fixtures plus two inspection
  scenarios**, `/tmp/bgraph-goal-vscode-host-portable.log`.
- Real Zed, final frozen CLI/LSP: **six fixtures**, grammar/component actually
  loaded; `/tmp/bgraph-goal-zed-host-portable.log`, input hashes and evidence
  `/tmp/bgraph-zed-host-1zregcuu/`.
- Real Chromium inspector: **six scenarios**, zero page errors, six screenshots
  visually reviewed by the UI owner. `/tmp/bgraph-inspector-ctfmjn/result.json`.
- Final no-default plus each of 29 archetype features: **30/30**,
  `/tmp/bgraph-goal-features-final.log`.
- Final benchmark `--locked -- --check`: passed with unchanged budgets,
  `/tmp/bgraph-goal-benchmark-final.log`.
- Final native cross-target snapshot comparison: **52 complete cases identical**
  on Linux x64, ARM/QEMU and Windows MSVC/Wine, including raw f64 bits and SVG
  bytes. `/tmp/bgraph-native-local/parity-final.json`. This is explicitly a
  partial platform set, not hosted/native-five-platform success.
- Final Windows ARM64 MSVC CLI/LSP cross-build: **exit 0**,
  `/tmp/bgraph-goal-windows-arm64-final.log`; compilation only, not native execution.
- Native comparator self-test: complete synthetic proof accepted and **18**
  changed or incomplete proofs rejected; real incomplete evidence rejected.

The integration gates found real defects beyond isolated unit checks: invalid
`DocumentSymbol.detail: null` rejected by the VS Code client; public `style node`
hover priority; missing BPMN word defaults; and Windows sine bit variation.
Each was fixed centrally and the affected public gate rerun. An earlier full
suite had one style-hover wording assertion failure; it is superseded by the
fully green final run above, not counted as a success.

Index transitions follow the user's explicit completion objective. W-156/W-160
are not dropped, reclassified blocked or counted done. Their required external
proof remains in scope. No commit, push, workflow dispatch or publication occurred.

## Baseline

`CARGO_TARGET_DIR=target/pipeline-work cargo test --locked --all-features --quiet`
completed successfully at the start of this goal pass: 645 library tests,
one intentionally ignored measurement, one binary test, nine Kasuari
integration tests, one LSP integration test and three doctests. Log:
`/tmp/bgraph-goal-baseline-tests.log`. This baseline is not proof of the new code.

## External execution access

Observed `gh auth status`: no authenticated GitHub host. No macOS host is
provided. A user question requesting a usable host/runner/authenticated session
is pending while implementation and local proof continue. No workflow has
been claimed run remotely, no code has been pushed, and no external artifact
has been published.

The ARM runner labels for private repositories are supported by the current
[GitHub-hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)
and [GitHub's private-repository ARM availability announcement](https://github.blog/changelog/2026-01-29-arm64-standard-runners-are-now-available-in-private-repositories/).
These references prove runner availability, not execution of this repository.

## Completion state

All seven original required work items are implemented and proved. W-156 and
W-160 are now landed from actual native and hosted execution evidence; none
was dropped or removed from the objective. Final production revision is
`863008506ed4f2546de7c2b95f9983a4ce26ddbe`, private validation branch
`codex/placement-proof-20261016`. The local main branch has not been pushed.

[Final hosted CI](https://github.com/Bunchyearth23/bgraph/actions/runs/38898281988)
is **completed / success**, all **16 jobs** green. Global Format, Clippy, Test,
Performance, grammar-presence and no-default checks each succeeded. It proves
714 passing tests, runtime 22 fixtures/30 archetypes/four file contexts, all
five real native runner jobs and the exact aggregate comparison. Downloaded
final artifacts also pass the same strict local comparator, requiring the
correct SHA. Independent digest/metadata audit: `native-ci-proof-final.json`.

The historical access blockers and first CI font failure below were resolved;
they are retained as evidence of the actual path, not outstanding requirements.
The first run's native proof remains independently valid at its own SHA.

## Access revalidation after local completion

The next continuation confirmed `gh auth status` is still unauthenticated.
Read-only `git ls-remote origin HEAD` via existing SSH credentials succeeded
(exit 0), returning `5d0320c47c87b994e950307ab274df5473dbb078`. This proves
repository read access, not GitHub Actions API access or remote test execution.
No GitHub connector or remote execution tool is available in this session.
The CI workflow and local result remain prepared. The user was asked for
permission to push a private validation branch, as required by AGENTS.md,
and for an authenticated GitHub CLI session to inspect jobs and download
artifacts. Neither answer nor API authentication has arrived yet.

The previous goal turn made concrete implementation and validation progress.
This continuation obtains new access evidence and a concrete remote-run
authorization request; the full goal remains incomplete.

A further access recheck again confirms no authenticated GitHub host and no
answer authorizing the private validation push. The same external execution
condition has persisted across three goal turns. All independent local work
and proof are complete; W-156/W-160 remain required and building in INDEX.
Resume after the user supplies the pending authorization and API access, or
provides the five native artifacts plus their actual execution evidence.

## Authorized remote validation

The user explicitly authorized the private validation branch push. A dedicated
branch will carry the complete verified worktree, leaving the local main branch
and its index unchanged. Git author identity uses the repository's configured
identity. Existing GitHub SSH repository access works; the standard Git
credential helper supplies no GitHub HTTP credential. Remote CI can start
on the branch push, but job/artifact API access remains unavailable until
GitHub CLI authentication is provided. No release tag is created.

## Private validation branch pushed

`git push --set-upstream origin codex/placement-proof-20261016` completed
exit 0 on the user's explicit authorization. The branch contains the full
validated implementation, fixtures, CI matrix and reports at commit `90163b7`.
No release tag, release workflow dispatch or registry publication occurred.
The API authentication process is confirmed live; runner progress is not
yet observable without its browser authorization.

## Remote execution observed

GitHub CLI authentication completed as the repository owner. Run `38895893757`
was inspected through the authenticated API: head SHA equals `90163b7`, status
`in_progress`; all five native targets have active jobs. This resolves the
previous external-access obstacle. `gh run watch` now follows this specific
live run; no duplicate workflow or release was dispatched.

## First hosted run: native proof and font failures

Run `38895893757` completed with failure only in the global `check` test step.
The five actual native runner jobs, strict aggregate comparator, eight feature
builds and 22/30/4 runtime gate all succeeded. All five native artifacts were
downloaded and the strict comparator rerun locally with the exact GITHUB_SHA;
52 source/SVG/raw-geometry/diagnostic snapshots match exactly.

Two previously machine-dependent tests fail on the runner's installed fonts:
the generic first-font subset test selects NotoColorEmoji (no glyf/loca outlines),
and PNG goldens use different installed text faces. The fix in progress supplies
a fixed licensed test font. Investigation also found a production issue: PNG's
usvg font database did not load the font already provided by `embed_font`; the
approved fix makes PNG honor that existing option. Assertions and byte-level
PNG goldens remain strict. W-160 remains unfinished until corrected hosted CI
fully succeeds; the failed run is not described as green.

## Corrected hosted run

Commit `863008506ed4f2546de7c2b95f9983a4ce26ddbe` was pushed on the same
authorized private validation branch. It contains the fixed licensed test font,
PNG embedded-font loading, alias-preservation regression, reviewed strict PNG
hashes and the first native proof. Local final validation passed 714 tests,
Clippy and format checks.

The new actual run
[38898281988](https://github.com/Bunchyearth23/bgraph/actions/runs/38898281988)
is confirmed `in_progress` at this exact revision. W-160 remains open until
all required steps succeed; the original failed run is preserved as history.
A trailing blank in the copied third-party license was normalized locally for
the final documentation commit, without changing the license wording or font.

## Final requirement audit

- W-153: single solver source/caller audit; no 512 limit or production fallback;
  large/nested/attachment-cycle tests, 107 vendor tests, exact overlap oracle;
  unchanged performance guard now also succeeds remotely.
- W-156: five actual target runners, 52 raw-bit/SVG/diagnostic/source snapshots,
  repeated runs and fresh children, downloaded/digest-verified native artifacts;
  exact aggregate and independent local comparisons succeed.
- W-157: exact minimum cardinality on full model and all failed components;
  exhaustive small-model oracle, typed internal error tests and cross-interface
  fixture; independent algorithm review.
- W-158: actual Chromium six scenarios and screenshots, real editor hosts,
  navigation/reference/hover/outline, parents/anchors/conflicts and explain.
- W-159: single exhaustive metadata strategy for validation, completion, docs,
  schema and dispatch; 30 feature configurations and complete genre matrix.
- W-160: all hosted jobs/steps passed, including runtime and performance;
  original font failures are corrected, not ignored or thresholded away.
- W-164 styles: default form shapes and paint layers, all override levels,
  public LSP integrations, generated reference and real Chromium review.

All report paths exist; INDEX contains no remaining planned/building/blocked
W in the original scope. Completion is supported by source, tests, actual
editor/browser behavior and hosted artifacts, rather than configuration alone.
