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

## Installing

```sh
cargo install --path ../..          # puts `bgraph-lsp` on PATH
cd editors/vscode && npm install    # vscode-languageclient
```

Then run the extension from VS Code (F5), or copy this directory into
`~/.vscode/extensions/bgraph`.

If `bgraph-lsp` is not on `PATH`, point `bgraph.server.path` at it.

## Keeping the grammar honest

The keyword list in the TextMate grammar is derived from `props.rs`. A
tree-sitter query that names a node type the grammar has dropped fails loudly —
Zed stops recognising the language at all. A TextMate grammar that misses a
keyword just stops colouring it, which nobody notices.

`the_textmate_grammar_knows_every_word_the_compiler_does` re-derives the list
on every `cargo test` for that reason. **Adding an archetype means its item
word must reach this file**, and the suite is what tells you.
