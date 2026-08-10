# bgraph for VS Code

Syntax highlighting and the `bgraph` language server for `.bd` files.

## What is in here

| Piece | What it does |
|---|---|
| `syntaxes/bgraph.tmLanguage.json` | Highlighting. A TextMate grammar, not tree-sitter — VS Code reads these natively, so nothing has to be compiled or pinned. |
| `src/extension.js` | Launches `bgraph-lsp` and speaks LSP to it. Everything semantic comes from there. |
| `language-configuration.json` | Comments, brackets, indentation. |

The server is the same binary the Zed extension talks to, and it reuses the
front half of the compiler — so the editor and `bgraph check` can never
disagree about what a file means.

## Building a package

```sh
./package.sh              # fetches the language client if needed, bundles it
./package.sh --no-server  # highlighting only; no npm, no network
```

Both write `bgraph-<version>.vsix` beside this file. Install it with:

```sh
code --install-extension bgraph-0.1.0.vsix
```

A VSIX is a zip with an OPC content-types part and a manifest, so `package.sh`
builds one with `zip` rather than fetching `@vscode/vsce` — the same call this
project makes for its SVG writer, its HTTP server and its PDF writer. Every
fact the manifest repeats — name, version, publisher, engine range — is read
back out of `package.json`, and a test fails if any of them is written twice.

The `--no-server` package still highlights: `src/extension.js` requires the
language client *inside* `activate`, so a build without `node_modules` says so
once instead of failing to activate and leaving `.bd` files looking like plain
text.

## The server

```sh
cargo install --path ../..   # puts `bgraph-lsp` on PATH
```

If it is not on `PATH`, point `bgraph.server.path` at it. Without it you get
highlighting and nothing else — no diagnostics, no completion.

For development, open this directory in VS Code and press F5.

## Keeping the grammar honest

The keyword list in the TextMate grammar is derived from `props.rs`. A
tree-sitter query that names a node type the grammar has dropped fails loudly —
Zed stops recognising the language at all. A TextMate grammar that misses a
keyword just stops colouring it, which nobody notices.

`the_textmate_grammar_knows_every_word_the_compiler_does` re-derives the list
on every `cargo test` for that reason. **Adding an archetype means its item
word must reach this file**, and the suite is what tells you.
