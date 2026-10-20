# W-169 — VS Code diagram preview — 2026-10-20

The extension now offers `bgraph.showPreview` from the command palette and
editor title menu, with Ctrl+Shift+V in bgraph editors (Cmd+Shift+V on macOS).
The existing bundled WASM renderer serves the diagram panel and Markdown.

## Behavior

- One `bgraph.preview` WebviewPanel per document URI; opening it again reveals
  and refreshes that panel while leaving focus in the source editor.
- Updates after 150 ms without another edit, using `TextDocument.getText()`;
  dirty files and untitled bgraph buffers do not need to be saved.
- Each document's edits affect its own panel. Closing a source document closes
  its preview; closing a panel cancels pending rendering, and reopening works.
- Syntax/rendering failures replace the picture with escaped text. A missing
  renderer reports how to rebuild the extension.
- Preview registration occurs before loading the optional language client, so
  `--no-server` builds can still preview. Missing LSP does not disable preview.
- Webview scripts are disabled, local resource roots are empty, and CSP allows
  inline styling and data images with all other resource types denied.
- WASM allocations are released in `finally`, including rendering failures.

## Files changed

- `editors/vscode/src/extension.js`: command, panel lifecycle and shared renderer.
- `editors/vscode/package.json`: command, shortcut, menu and `npm test` script.
- `editors/vscode/package.sh`: run the incremental WASM build inside the full
  repository when its target is available; the old timestamp check only
  considered `src/lib.rs`. Exported directories package their supplied module
  and fail clearly if it is missing, preserving an existing VSIX on failure.
- `editors/vscode/README.md`: usage, fallback and filesystem limitations.
- `editors/vscode/test/preview.test.cjs`: behavior tests with the real bundled
  WASM and an injected VS Code API, deliberately without vscode-languageclient.
- `editors/vscode/test/package.test.cjs`: actual packaging from an exported
  directory with a supplied WASM, plus the missing-module failure path.

No dependency was added. No preexisting Rust, Zed or index edits were reverted.

## Validation performed by the VS Code agent

| Command/check | Result |
|---|---|
| `node --check src/extension.js` | Passed |
| `bash -n package.sh` | Passed |
| `npm test` | 6 tests passed against rebuilt WASM |
| `./package.sh` | Full VSIX built successfully |
| Full VSIX contents compared to current package.json, extension.js and WASM | Matched; language client present |
| `./package.sh --no-server` | VSIX built successfully, WASM present, node_modules absent |
| Exported-directory packaging without repository sources/build script | Supplied WASM packaged byte-for-byte; missing WASM rejected |
| Final `./package.sh` | Full VSIX restored; incremental renderer build passed |
| `git diff --check -- editors/vscode` | Passed |

The tests exercise dirty-buffer rendering, URI-based panel reuse, untitled
documents, isolated/debounced updates, close/reopen/deactivation cleanup,
hostile labels, escaped exceptions, syntax errors and recovery, missing WASM,
Markdown success/fallback, and refused `include` declarations. Tests do not
need an installed VS Code binary.

The parent agent owns the separate real VS Code extension-host check in
`tests/runtime/vscode-host.cjs`; its evidence is recorded in the combined
session validation rather than claimed here.

## Limits and remaining runtime evidence

The preview renders the current text only. WASM does not resolve `include` or
`from:`; these require `bgraph watch file.bd`. LSP multifile diagnostics remain
available separately. The existing synchronous WASM renderer can occupy the
extension host while a large diagram is rendering; debounce limits repeated
work but does not provide background rendering. No pan/zoom, export controls,
new webview framework or renderer worker was added.

The package was built locally without installation, publication or profile
changes. Interactive visual inspection in the user's installed editor remains
distinct from the automated API/lifecycle checks.

Trail retrieval estimate for this agent: 235,862 tokens saved across six calls.
