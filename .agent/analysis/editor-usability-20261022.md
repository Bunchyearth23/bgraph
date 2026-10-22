# Editor usability follow-up — 2026-10-22

Requested: improve the LSP and editor plugins, especially the oddly sized
VS Code “Open Preview”. Work was split between preview implementation,
LSP consistency and actual editor-host checks. Existing audit fixes and user
changes were preserved. No commit, push, user-profile installation or release.

## Changes and causes

- The former VS Code preview scaled SVG to pane width only. A tall drawing
  could exceed the visible height; rerendering replaced the entire webview.
  The viewer now fits both available dimensions, centers the drawing, leaves
  small drawings at natural size, and reacts to panel resizing.
- Fit, 100%, zoom buttons, cursor-centered wheel zoom, pointer drag and
  keyboard controls are available. A manual view survives edits, rendering
  errors and webview recreation. The host retains the latest render until
  the webview is ready. The command uses a title-bar icon instead of a long
  title-bar command label.
- The webview has a nonce-restricted script, no network or local-resource
  access, and displays renderer errors as text. Source labels remain escaped
  by the common SVG writer. No browser or UI dependency was added.
- Diagnostics already used unsaved included documents, but definitions,
  references and hover recomposed from disk. All four now use the server's
  shared open-document snapshot. Hover analyzes that composition once.
  Closing an included document restores disk contents. A never-saved file
  with a file URI can be included while open.
- Both plugins recognize `.bds` shared stylesheets. VS Code preview is
  offered for diagrams, with guidance if called directly on a stylesheet.
  VS Code's LSP selector includes untitled bgraph documents.

## Evidence

| Check | Result |
|---|---|
| `cargo test --locked` | Exit 0; 798 passed, 1 manual benchmark ignored |
| `cargo clippy --locked --all-targets -- -D warnings` | Exit 0; no warnings |
| `cargo build --locked --no-default-features` | Exit 0 |
| `cargo build --locked --release --bins` | Exit 0; updated `target/release/bgraph` and `bgraph-lsp` |
| `cargo fmt --all -- --check`, `git diff --check` | Exit 0 |
| VS Code extension `npm test` | Exit 0; 12 tests |
| `./docs.sh build` | Exit 0; strict MkDocs |
| `./editors/vscode/package.sh` | Exit 0; rebuilt WASM and full 1.0.1 VSIX |
| Actual VS Code 1.143.0 / Linux | 8 fixtures, 2 navigation scenarios, untitled diagnostics, preview lifecycle and measured geometry pass |
| Actual Zed 1.25.2 / Linux | 7 fixtures, grammar, WASI component, LSP, preview task and keybinding pass |

The LSP regression was observed failing before the fix: navigation landed at
saved child line 0, while the live declaration had moved to line 1 after an
emoji. The added checks pin UTF-16 columns, references, hover dimensions and
placement, close-to-disk recovery and never-saved included files.

Client tests cover both-dimensional fitting, natural size, resize, manual
camera restoration, platform-independent wheel increments, text-only errors,
recovery, message readiness, lifecycle, Markdown and package contents.

The VS Code host test measures actual SVG and viewport rectangles for tall
and wide diagrams, changes the editor split, and exercises zoom, 100% and Fit.
Its DOM probe is injected only by the isolated test harness, using the
webview's existing CSP nonce; production has no test API. Zed's preview test
also verifies a hostile filename, save-before-preview, dynamic URL, reload
and a single watcher after repeat. No user editor profile is touched.

Temporary evidence: `/tmp/bgraph-lsp-plugin-{cargo-test,clippy,minimal}.log`,
`/tmp/bgraph-vscode-host-after.log`, `/tmp/bgraph-vscode-after.png`,
`/tmp/bgraph-vscode-after-geometry.png`, `/tmp/bgraph-zed-host-sa9v6rda`,
`/tmp/bgraph-zed-host-5upz70ry`. These paths are session-local; the tests above
are retained in the repository for reproduction.

`trail check` ran and exited 1 with 20 phrase-overlap warnings across the
entire uncommitted audit/editor worktree. Reviewed as false positives, not
waived runtime failures: font/canvas documentation describes known limits;
the index reports measured tests rather than treating a manual build as
proof; the real-host probe measures geometry; VS Code selectors and package
tests do not interpolate filenames into Zed shell arguments; the preview
test explicitly asserts that webview HTML remains unchanged. The Zed task
itself was unchanged here and passed the hostile-filename host test. No
learning was edited or removed to silence these warnings.

## Remaining limits

The bundled VS Code renderer still operates on the current buffer only:
`include` and external `from:` data need `bgraph watch`. This restriction is
documented in the guide and extension README; the LSP remains file-aware.
The manual benchmark, Windows/macOS hosts and screen readers were not run.
Artifact preparation does not replace an already installed extension or LSP.
Existing parent work states and unrelated audit findings remain unchanged.
