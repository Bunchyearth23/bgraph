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

## Required external proof

The five-platform native CI matrix remains unexecuted. No real macOS, Windows ARM,
or hosted native ARM/Windows run is claimed from QEMU/Wine. GitHub has no
authenticated host and no macOS host was provided. These required executions
keep W-160 and the full goal unfinished until actual evidence exists.
No workflow dispatch, push or publication was performed.
