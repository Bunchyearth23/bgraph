# W-160 — integrated runtime and editor gates

## Reproducible gate

`tests/runtime/gate.sh --extensions` builds fresh no-default library, native
CLI/LSP, browser wasm, WASI, Typst plugin and Zed shim; it packages the VS Code
extension with the wasm module built by that invocation. The JavaScript gate
compares diagnostics and SVG through all runtimes, actual Typst compilation,
watch HTTP output and the packaged Markdown renderer. Repeated native processes
and fresh wasm instances must remain identical. The corpus now includes
**22 fixtures**, the compiler's **30 archetypes**, and **four file contexts**.
Five added fixtures cover minimum-cardinality conflict selection and BPMN
attachment dependents, explicit conflicts and consistent/inconsistent cycles.

The native platform harness and strict five-platform comparator are described
in `w156-native-proof.md`. A configured matrix is not remote execution proof.

## Actual editor evidence in this goal pass

- `bash tests/runtime/vscode-host.sh` — **exit 0**, output
  `VS Code host: 7 fixtures passed; 2 inspection scenarios passed`.
  `/tmp/bgraph-goal-vscode-host-final.log`.
  This launches the real VS Code extension host with a disposable profile,
  Xvfb and a fresh CLI/server copied into that profile. It compares published
  diagnostics, invokes actual definition/reference/hover/outline providers,
  checks declared and nested-frame navigation, and requires layout outline
  symbols. Failed runs retain their isolated profile for diagnosis.
- `WASI_SDK_PATH=/usr tree-sitter build --wasm -o /tmp/bgraph-goal-grammar.wasm tree-sitter-bgraph`
  — **exit 0**.
- `python3 tests/runtime/zed-host.py --grammar /tmp/bgraph-goal-grammar.wasm`
  — **exit 0**, actual Zed loads the extension component and pinned grammar,
  launches the real LSP and publishes matching diagnostics for six fixtures.
  `/tmp/bgraph-goal-zed-host-final.log`; evidence and input hashes:
  `/tmp/bgraph-zed-host-4hs_9526/`.

The VS Code gate found a genuine protocol violation that unit calls had missed:
`DocumentSymbol.detail: null` made the language client reject the entire group
outline. Optional details are now strings and the durable host assertion proves
that groups and layouts reach the user. A separate public-LSP integration found
a theme hover priority bug (`style node`); its final rerun belongs to W-164.

## Final portable-source reruns

- `./tests/runtime/gate.sh --extensions` — **exit 0**, 22 fixtures, 30 archetypes,
  four file contexts; `/tmp/bgraph-goal-runtime-final.log`.
- VS Code host — **exit 0**, seven fixtures plus two inspections;
  `/tmp/bgraph-goal-vscode-host-portable.log`.
- Zed host with frozen final-math CLI/LSP — **exit 0**, six fixtures;
  `/tmp/bgraph-goal-zed-host-portable.log`, evidence `/tmp/bgraph-zed-host-1zregcuu/`.
- Final feature configurations — **30/30**, `/tmp/bgraph-goal-features-final.log`.
- Chromium inspector — **six scenarios**, zero page errors, all captures reviewed;
  `/tmp/bgraph-inspector-ctfmjn/result.json`.
- Complete all-features suite, Clippy and format check — **exit 0**;
  `.agent/analysis/all-w-completion.md` lists commands and exact counts.

The final Windows ARM64 MSVC CLI/LSP cross-build also completed **exit 0**;
`/tmp/bgraph-goal-windows-arm64-final.log`. No execution is inferred from it.

## Hosted CI completed

The user authorized a private validation branch and completed GitHub CLI
browser authentication. The first run `38895893757` proved all five native
targets, but its global test step found two environment-dependent font tests.
Those failures were fixed with licensed fixed-font inputs and real embedded-font
loading in PNG, preserving other font aliases; six focused regressions and an
independent review pass. See `w160-font-portability.md`.

Final run [38898281988](https://github.com/Bunchyearth23/bgraph/actions/runs/38898281988)
at `863008506ed4f2546de7c2b95f9983a4ce26ddbe` is **completed / success**.
Every one of its **16 jobs** succeeded. The global check's Format, Clippy, Test,
Performance regression guard, grammar-skip guard and no-archetype build all
succeeded, with no required step skipped. The Test step proves **714 passed**:
693 library, one binary, nine solver integrations, one LSP, one native, six
style integrations and three doctests; one historical measurement remains
intentionally ignored and has a separately executed performance guard.

The hosted runtime gate proves **22 fixtures**, **30 archetypes** and **four file
contexts**. All five native runner jobs and the strict aggregate comparison
pass at this same revision. The actual artifacts were downloaded and the
comparator rerun locally with the exact `GITHUB_SHA`; it passed all **52 cases**.
Archive digests, job IDs, repeated-process evidence and native metadata are
recorded in `native-ci-proof-final.json`.

Hosted guard: 20k connected nodes average **385.76 ms** per complete invocation;
public `render_source` on 5k nodes **94.73 ms**, with unchanged budgets. Complete
run data: `/tmp/bgraph-ci-second-progress.json`, `/tmp/bgraph-ci-second-all.log`;
strict comparison: `/tmp/bgraph-ci-native-comparison-8630085.log`.

No required W-160 validation remains. Code is on the authorized private branch
`codex/placement-proof-20261016`; no release tag, release or registry publication
was created. The original local main worktree remains available for review.
