# bgraph for Zed

Syntax highlighting, language-server support and a live browser preview for `.bd` files.

New to bgraph? Start with [Your first diagram](../../docs/guide/demarrage.md)
and the [step-by-step Zed setup](../../docs/guide/editeurs.md#zed). You need
both the updated CLI and this extension. Open the containing folder as a
project before opening its `.bd` file, then run **task: spawn → bgraph: Preview**.

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

**Live preview** — the bundled `bgraph: Preview` task saves the current `.bd`
file and opens `bgraph watch` in your default browser. Each later save reloads
the drawing. See [Preview](#preview) to bind it to `Ctrl+Shift+V`.

Zed's extension API does not provide a custom preview pane or a Markdown
renderer hook, so the preview uses the browser.

**Completion**, context-aware:

| Where you are | What you get |
|---|---|
| start of a line outside layout | `type`, `graph`, `style`, `node`, `include`, `component`, `use`, `note`, `legend`, plus the genre's item words and node ids |
| inside `layout { }` | `place`, `align`, `equal-width`, `equal-height`, `row`, `column`; no node properties |
| after `place` | declared elements in the current scope |
| after `place api` | absolute or relative placement snippets with numeric defaults |
| after a relative anchor | `gap` and cross-axis `align` |
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

**Layout snippets** use the declared identifiers as choices and numeric
coordinate defaults (`0, 0`). Tab moves through the fields. Row and column
snippets insert ordinary `gap 32`, without literal square brackets.
The completion edit replaces the complete identifier, including hyphens,
and works in inline blocks and incomplete statements.

**Signature help** describes placements, coordinates, gaps, alignments and
ordered members as you type, including after `(` and `,` in coordinates.
It stays silent inside strings and comments.

**Formatting** uses the same formatter as `bgraph fmt`, through Zed's normal
document-formatting command. A file that cannot parse is left unchanged.

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

The script installs `bgraph` and `bgraph-lsp`, regenerates the tree-sitter parser, commits
it if needed, and pins `extension.toml` to the current revision — Zed fetches
grammars from a git revision even for a local path, so that pin has to be
real.

Then, in Zed: `cmd/ctrl-shift-p` → **zed: install dev extension** → pick
`editors/zed`.

Re-run the script after editing `grammar.js`; Zed caches a grammar per
revision and will not notice otherwise.

## Preview

With the updated extension installed and the updated `bgraph` on `PATH`, open
the containing folder in Zed, then a saved `.bd` file, and run
**task: spawn → bgraph: Preview**. On the verified host, language tasks are
unavailable when opening a file alone without its folder. The task runs
`bgraph watch <current-file> --open --port 0`: the OS chooses a free local
port, and the task terminal displays the preview URL. An older CLI needs
updating before it understands `--open`.

### Add the `Ctrl+Shift+V` shortcut (optional)

The shortcut is optional: **task: spawn → bgraph: Preview** does the same
thing without it. Zed extensions cannot install keybindings, so the entry is
supplied for you to merge into your own keymap.

1. Open the keymap **file**: command palette → **zed: open keymap file**.
   (The action named *zed: open keymap* opens the keymap *editor*; the one
   you want opens the JSON file.) The file is
   `~/.config/zed/keymap.json` on macOS and Linux, and
   `~\AppData\Roaming\Zed\keymap.json` on Windows.
2. The file holds one JSON **array**. If it does not exist yet, create it with
   the contents of step 3. If it exists, keep everything already in it.
3. Add this object as one element of that array — between the `[` and the `]`,
   after the last existing object, separated from it by a comma:

```json
{
  "context": "Editor && extension == bd",
  "bindings": {
    "ctrl-shift-v": ["task::Spawn", { "task_name": "bgraph: Preview" }]
  }
}
```

   A keymap with nothing else in it is exactly this:

```json
[
  {
    "context": "Editor && extension == bd",
    "bindings": {
      "ctrl-shift-v": ["task::Spawn", { "task_name": "bgraph: Preview" }]
    }
  }
]
```

   The entry is also shipped as [`keymap.json`](keymap.json), so you can copy
   it from there instead of typing it.
4. Save the file. Zed reads the keymap immediately — no restart is needed.

To undo the merge, delete the object again and save.

### What the shortcut does

With a `.bd` file focused, `Ctrl+Shift+V` spawns the same **bgraph: Preview**
task: the buffer is saved first, then `bgraph watch <file> --open --port 0`
runs in a task terminal, a free loopback port is chosen, and your default
browser opens on `http://127.0.0.1:<port>`. Keep the browser open while
editing and save to reload the drawing. The task reuses its terminal and
prevents concurrent runs; on the verified Zed host, pressing the shortcut
again replaces the previous watcher and opens the new preview. `Ctrl+C` in
the task terminal stops it. If the browser cannot open, the terminal keeps the
URL available to open manually. Included files use their last saved version.

The binding is scoped to `.bd` editors, so `Ctrl+Shift+V` keeps its usual
meaning everywhere else — in a Markdown file it still opens Zed's own
Markdown preview. On macOS the key is *control*+shift+V; to use another key,
change the `"ctrl-shift-v"` string. If Zed reports a keymap error, the file is
not valid JSON — the usual cause is a missing or extra comma between two
objects.

The bundled task uses the shell syntax shared by Linux/macOS shells (also
Fish). It passes the filename through an environment variable before expanding
it inside quotes: Zed versions that concatenate task arguments must preserve
spaces without interpreting `$()` or backticks in a filename.
For PowerShell, override the task in **zed: open project tasks** and replace
the filename argument with `"\"$env:BGRAPH_PREVIEW_FILE\""`. That variant has
not been verified on a Windows host.

See Zed's [task documentation](https://zed.dev/docs/tasks) for project task
overrides and custom keybindings.

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
python3 tests/runtime/zed-host.py --grammar /tmp/bgraph-grammar.wasm --preview
```

Set `WASI_SDK_PATH` to your SDK root when using another compiler installation.
The ordinary runtime gate's `wasm32-wasip1` build checks compilation; this
separate check loads the component in Zed itself. The helper enables trust
only inside its disposable profile and never installs into the user's profile.

`--preview` additionally needs `xdotool`. It loads the bundled task and the
keymap snippet into that isolated profile, presses the actual shortcut, and
checks saving, filename quoting, live reload and the watcher left after a
repeat. A recording browser opener captures the requested URL without opening
a browser outside the test session.

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
    tasks.json                save current file and open live browser preview
  keymap.json                 optional Ctrl+Shift+V entry to merge into user keymap
```

These `.scm` files are the canonical queries. Other tree-sitter editors
(Neovim, Helix) can point at them directly.
