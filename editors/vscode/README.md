# bgraph for VS Code

Syntax highlighting, live diagram preview and the `bgraph` language server for `.bd` files.

## Start using the extension

If you received a `.vsix`, open VS Code's Extensions view, choose **Install
from VSIX…** from its `…` menu, and select that file. Open a `.bd` file and
press **Ctrl+Shift+V** (**Cmd+Shift+V** on macOS) to preview it.

The preview renderer is bundled. For completion and diagnostics, install
`bgraph-lsp` separately and restart VS Code. New to the language? Follow
[Your first diagram](../../docs/guide/demarrage.md) and the
[step-by-step editor setup](../../docs/guide/editeurs.md#vs-code).

## What is in here

| Piece | What it does |
|---|---|
| `syntaxes/bgraph.tmLanguage.json` | Highlighting. A TextMate grammar, not tree-sitter — VS Code reads these natively, so nothing has to be compiled or pinned. |
| `src/extension.js` | Launches `bgraph-lsp`, opens diagram previews and renders Markdown fences. |
| `language-configuration.json` | Comments, brackets, indentation. |
| `bgraph.wasm` | The renderer for both previews. Built by `package.sh`; not committed. |

The server is the same binary the Zed extension talks to, and it reuses the
front half of the compiler — so the editor and `bgraph check` can never
disagree about what a file means.

## Diagram preview

Open a `.bd` file and press **Ctrl+Shift+V** (**Cmd+Shift+V** on macOS), or
choose **bgraph: Open Preview** from the command palette or the editor's title
menu. A preview opens beside the source and updates while you type, including
unsaved changes. An untitled buffer works too once its language is set to
`bgraph`. Opening the command again reuses that document's preview; different
documents have their own panels. Syntax and rendering errors appear in the panel.

The renderer is bundled, so preview needs neither `bgraph` nor `bgraph-lsp` on
`PATH`. It renders the current buffer only: `include` and `from:` require file
access and are rejected by this WASM renderer. Use `bgraph watch file.bd` for
diagrams composed from several files. The LSP still resolves those files for
diagnostics and navigation.

The shortcut is limited to `bgraph` editors, preserving the Markdown preview's
shortcut in Markdown files.

## Diagrams in the Markdown preview

A ` ```bgraph ` fence is drawn in VS Code's Markdown preview — the one thing
Mermaid gets used for that this could not do. It renders through `bgraph.wasm`,
bundled beside the extension, **not** through a `bgraph` on `PATH`: a preview
that only worked for people who had already installed the CLI would miss
exactly the readers it is for.

`package.sh` builds that module inside the repository (it needs `rustup target
add wasm32-unknown-unknown`). An exported extension directory can instead
provide an already-built `bgraph.wasm`; packaging fails if no renderer is
available. If an installed module is missing or cannot load, a fence falls
back to a code block. An error *inside* a fence is reported in place.

Markdown fences use the same current-text renderer, so `include` and `from:`
have the same limitations as the diagram preview.

## Building a package

```sh
./package.sh              # fetches the language client if needed, bundles it
./package.sh --no-server  # highlighting and preview; no npm, no network
```

Both write `bgraph-<version>.vsix` beside this file. Install it with:

```sh
code --install-extension bgraph-1.0.0.vsix --force
```

A VSIX is a zip with an OPC content-types part and a manifest, so `package.sh`
builds one with `zip` rather than fetching `@vscode/vsce` — the same call this
project makes for its SVG writer, its HTTP server and its PDF writer. Every
fact the manifest repeats — name, version, publisher, engine range — is read
back out of `package.json`, and a test fails if any of them is written twice.

The `--no-server` package still highlights and previews: `src/extension.js`
requires the language client *inside* `activate`, after registering preview,
so a build without `node_modules` reports unavailable diagnostics and completion.

## The server

```sh
cargo install --locked --path ../.. --bins --force
```

If it is not on `PATH`, point `bgraph.server.path` at it. Without it you get
highlighting and preview, with no diagnostics or completion.

For development, open this directory in VS Code and press F5.

Run `npm test` for preview lifecycle, error handling, Markdown compatibility
and bundled-renderer checks. Build the package first to provide `bgraph.wasm`.

## Keeping the grammar honest

The keyword list in the TextMate grammar is derived from `props.rs`. A
tree-sitter query that names a node type the grammar has dropped fails loudly —
Zed stops recognising the language at all. A TextMate grammar that misses a
keyword just stops colouring it, which nobody notices.

`the_textmate_grammar_knows_every_word_the_compiler_does` re-derives the list
on every `cargo test` for that reason. **Adding an archetype means its item
word must reach this file**, and the suite is what tells you.
