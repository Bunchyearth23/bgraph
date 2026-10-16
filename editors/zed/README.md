# bgraph for Zed

Syntax highlighting (tree-sitter) and a language server for `.bd` files.

## What it gives you

**Highlighting** — keywords, node definitions vs. references, style names,
property keys, colors, arrows and labels are all distinguished. The grammar
lives in [`tree-sitter-bgraph/`](../../tree-sitter-bgraph) and mirrors the pest
grammar the compiler uses, so the editor and the renderer agree on what a file
means.

**Diagnostics** — exactly the errors `bgraph check` prints, underlined in
place: unknown nodes and styles, anchor cycles, misspelled properties (with a
suggestion), invalid enum values, self-referencing anchors. Warnings for
properties that have no effect where they are written.

A file that uses `include` is **composed before it is checked**, against the
buffer rather than the version on disk. Errors in included files are relayed
at the relevant `include`, with related information pointing to the original
file and range.

**No Markdown preview.** Zed's extension API has no hook for extending its
Markdown rendering, so this is a limit of the editor rather than something
left undone. `bgraph watch` beside the editor is the answer meanwhile.

**Completion**, context-aware:

| Where you are | What you get |
|---|---|
| start of a line | `type`, `graph`, `style`, `node`, `include`, `component`, `use`, `note`, `legend`, the three constraints, plus node ids |
| after `type` | archetype names |
| after `.` | declared style names |
| after `extends` | declared style names |
| after `right-of` / `above` / … | every item the file declares, under **any** of the genre's words |
| inside `node { }` | node properties only |
| inside an edge `{ }` | edge properties only |
| inside `graph { }` | graph settings only |
| inside `type pie { }` | that archetype's settings only |
| after `shape:`, `router:`, `arrow:` … | that property's accepted values |

It is also deliberately **quiet**. It never fires inside a label or a
comment; `.` only opens the style list where a class reference can actually
go, so `0.55` and `..>` stay silent; aliases (`border`, `text-color`, …) are
valid and documented but kept out of the list; and `value` / `bits` appear
only under the archetype that reads them. Space is not a trigger character —
a popup on every word boundary is the fastest way to make completion feel
like an obstacle.

**Hover** — a property's syntax and meaning; a node's label, styles and
**resolved position and size**; a style's flattened property list.

**Go to definition** — from any item reference or `.class` to its declaration,
under whichever word the genre uses: a `slice`, a `field`, a `gateway`. It only
looked for `node` until 2026-08-11, so in every genre with a word of its own it
quietly did nothing.

**Outline** — styles and nodes in the symbol list.

Completion and navigation fall back to a lexical scan when the document does
not parse, which is exactly when you need them most.

## Install

```sh
./editors/zed/install.sh
```

The script installs `bgraph-lsp`, regenerates the tree-sitter parser, commits
it if needed, and pins `extension.toml` to the current revision — Zed fetches
grammars from a git revision even for a local path, so that pin has to be
real.

Then, in Zed: `cmd/ctrl-shift-p` → **zed: install dev extension** → pick
`editors/zed`.

Re-run the script after editing `grammar.js`; Zed caches a grammar per
revision and will not notice otherwise.

## If the server does not start

The extension looks for `bgraph-lsp` on `PATH`. Point Zed at it directly
instead:

```json
{
  "lsp": {
    "bgraph-lsp": {
      "binary": { "path": "/home/you/.cargo/bin/bgraph-lsp" }
    }
  }
}
```

`zed --foreground` prints the extension's log, including the error message the
extension returns when it cannot find the binary.

## Isolated host verification (Linux)

The optional host check runs the real Zed application with Xvfb, a private
D-Bus session and a temporary profile. It loads a freshly built extension
component and grammar. The Rust extension resolves an executable `bgraph-lsp`
wrapper on its isolated `PATH`; no LSP binary setting bypasses that lookup.
The check compares the language server's actual diagnostics with CLI output
for valid, overlapping and conflicting placements, nested frames, a conflict
through two equal-width constraints, and an error after an emoji.
Diagnostic columns are compared as Unicode scalar counts, matching the CLI.
The proxy
records and forwards LSP messages unchanged, including every diagnostic URI
and range. Screenshots, logs, artifact hashes and `result.json` stay in the
temporary evidence directory printed by the command.

It needs Zed, Python 3.11+, `xvfb-run`, `dbus-run-session`, ImageMagick `import`,
the installed Rust `wasm32-wasip2` target and a Tree-sitter WASI compiler.
With the packaged Clang/WASI sysroot on this Linux installation:

```sh
cargo build --locked --bins
CARGO_TARGET_DIR=target/zed-work cargo build --locked --release \
  --manifest-path editors/zed/Cargo.toml --target wasm32-wasip2
WASI_SDK_PATH=/usr tree-sitter build --wasm -o /tmp/bgraph-grammar.wasm tree-sitter-bgraph
python3 tests/runtime/zed-host.py --grammar /tmp/bgraph-grammar.wasm
```

Set `WASI_SDK_PATH` to your SDK root when using another compiler installation.
The ordinary runtime gate's `wasm32-wasip1` build checks compilation; this
separate check loads the component in Zed itself. The helper enables trust
only inside its disposable profile and never installs into the user's profile.

## Layout

```
editors/zed/
  extension.toml              manifest: grammar pin + language server
  Cargo.toml                  compiled to a wasm component by Zed
  src/lib.rs                  locates and launches bgraph-lsp
  install.sh                  build, generate, pin
  languages/bgraph/
    config.toml               file suffixes, comments, brackets
    highlights.scm            syntax highlighting
    brackets.scm              bracket matching
    indents.scm               auto-indent inside blocks
    outline.scm               symbol outline
```

These `.scm` files are the canonical queries. Other tree-sitter editors
(Neovim, Helix) can point at them directly.
