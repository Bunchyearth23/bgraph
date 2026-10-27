# Use VS Code or Zed

An editor extension adds syntax colors, suggestions and error highlighting.
You can still follow the [first tutorial](demarrage.md) with any plain text
editor and `bgraph watch`.

The two extensions share **`bgraph-lsp`**, the language-server command.
It supplies diagnostics, completion, hover information, navigation and
document formatting. The extensions and the command-line tools are
installed separately.

## Choose your preview

| | VS Code | Zed |
|---|---|---|
| Where the preview appears | A pane beside the source | Your default browser |
| When it updates | While you type, including unsaved text | When files are saved |
| Shortcut | `Ctrl+Shift+V`; `Cmd+Shift+V` on macOS | `Ctrl+Shift+V` after adding the keymap entry |
| CLI required for preview | No; the renderer is bundled | Yes; an updated `bgraph` on `PATH` |
| `include` and external `from:` files | The pane says so; use the browser watcher instead | Supported through the watcher, using saved files |

## VS Code

### 1. Install the extension

If you received a `.vsix` file:

1. Open VS Code's **Extensions** view.
2. Open its `…` menu and choose **Install from VSIX…**.
3. Select the supplied `bgraph-<version>.vsix` file.
4. Reload VS Code if it asks.

If the `code` command is installed, the terminal equivalent is:

```sh
code --install-extension /path/to/bgraph-1.0.1.vsix --force
```

Replace the example path with your actual package path. The extension is
installed locally from that file.

To build a package from the repository, use a Bash terminal at the repository
root with Node/npm, `zip` and the Rust WebAssembly target available:

```sh
rustup target add wasm32-unknown-unknown
./editors/vscode/package.sh
code --install-extension editors/vscode/bgraph-1.0.1.vsix --force
```

Use the filename produced by the script if its version differs.

### 2. Open a source file and preview it

Open `architecture.bd` from the tutorial. Its language mode should be
**bgraph**. If it is plain text, use the language selector in VS Code's
status bar to choose bgraph.

Keep the source editor focused and press **`Ctrl+Shift+V`**
(**`Cmd+Shift+V`** on macOS). You can also open the command palette and run
**bgraph: Open Preview**.

A pane opens beside the source. Change a label: the image updates without
saving. An untitled document works once its language is set to bgraph.
Each document has its own pane; repeating the command reuses that pane.
Syntax or rendering errors appear in it.

The picture fits the pane's width **and** height. **Fit** restores that view;
**100%** shows natural size. Zoom with **+ / −** or the wheel, and drag to
move. Your manual zoom and position stay in place while you edit. With the
preview focused, use arrow keys to move, `F` or `Home` to fit, and `0` for
natural size. The editor title bar also has an **Open Preview** icon.

The pane needs neither `bgraph` nor `bgraph-lsp` installed. It renders the
current text only, so it cannot read `include` files or external `from:`
data. When such a diagram is open, the pane says so: an `include` is refused
with a message naming `bgraph watch`, and a pie that reads its slices from a
table keeps its picture with a note beside it saying that the table was not
read. For those, use:

```sh
bgraph watch architecture.bd --open --port 0
```

### 3. Enable suggestions and diagnostics

[Install the command-line tools](installation.md), including `bgraph-lsp`,
then restart VS Code so it sees the updated `PATH`.

If the server is elsewhere, set its absolute path in VS Code's settings:

```json
{
  "bgraph.server.path": "/absolute/path/to/bgraph-lsp"
}
```

On Windows, point to `bgraph-lsp.exe` and escape backslashes in JSON, for
example `"C:\\Tools\\bgraph\\bin\\bgraph-lsp.exe"`.

The server also works in untitled buffers set to bgraph. Both editors
recognize `.bds` shared stylesheets. For included files that are open in the
editor, diagnostics, navigation and hover use unsaved changes; closing the
file returns to its saved version. To preview a stylesheet's effect, open
the `.bd` diagram that uses it.

### Diagrams inside Markdown

Create a Markdown file containing:

````markdown
```bgraph
node a "Start"
node b "Finish"
layout { place b right-of a gap 48 }
a -> b
```
````

Open VS Code's usual Markdown preview. The fenced diagram is rendered by
the extension's bundled renderer. The shortcut keeps its normal Markdown
meaning in a `.md` file. Markdown blocks have the same `include` and
external-data limits as the direct diagram pane.

## Zed

### 1. Install the tools and extension

For a supplied export, follow the instructions shipped with its `zed`
folder. The export contains `install.sh` for Linux and `install.ps1` for
Windows; run the appropriate installer, then select that folder through
**zed: install dev extension**.

From the source repository, run this in a Bash terminal at its root:

```sh
./editors/zed/install.sh
```

This installs `bgraph` and `bgraph-lsp`, prepares the grammar and updates
the extension's pinned grammar revision. It can create a commit containing
regenerated grammar files.

In Zed, open the command palette, run **zed: install dev extension**, and
choose `editors/zed` in the repository.

### 2. Open the folder, then the file

Open the folder containing `architecture.bd` as a Zed project, then open
the file. A window containing only an isolated file does not expose the
language task on the verified Zed host.

Open the command palette and run **task: spawn**, then select
**bgraph: Preview**. The task saves the current file, starts the watcher on
a free port and opens the browser. Keep the task running; subsequent saves
update the drawing. Included files use their saved versions on disk.

### 3. Add the preview shortcut

Run **zed: open keymap file** — the action named *zed: open keymap* opens the
keymap editor, not the JSON file (`~/.config/zed/keymap.json` on macOS and
Linux, `~\AppData\Roaming\Zed\keymap.json` on Windows). If the file does not
exist yet, create it with the array below. Add the following object to the
existing top-level array; preserve your other entries and separate objects
with commas:

```json
{
  "context": "Editor && extension == bd",
  "bindings": {
    "ctrl-shift-v": ["task::Spawn", { "task_name": "bgraph: Preview" }]
  }
}
```

The same entry is provided in `editors/zed/keymap.json`. The extension
supplies the task; this keymap entry supplies its shortcut.

With the `.bd` editor focused, press `Ctrl+Shift+V`. Repeating the task
replaces the active watcher on the verified host. Stop it with `Ctrl+C`
in its terminal. If opening the browser fails, use the printed URL.

The supplied task uses Linux/macOS shell syntax, including Fish. For
PowerShell, the [Zed extension README](https://github.com/Bunchyearth23/bgraph/blob/main/editors/zed/README.md#preview)
describes the project-task override; that variant has not been verified
in a Windows host.

Zed uses the browser preview. The extension does not provide a native
diagram pane or render bgraph blocks inside Zed's Markdown preview.

### If Zed cannot find the language server

Add an absolute path in Zed's settings:

```json
{
  "lsp": {
    "bgraph-lsp": {
      "binary": { "path": "/absolute/path/to/bgraph-lsp" }
    }
  }
}
```

Use `bgraph-lsp.exe` on Windows. Restart the language server after changing
its executable or installing a new version.

## Editing features to try

- Inside `layout`, completion offers placement instructions and identifiers
  from the current scope.
- Snippets supply numeric defaults such as `at (0, 0)`. Press `Tab` to move
  through their fields.
- After a relative placement, completion offers anchors, `gap` and alignment.
- Parameter help describes coordinates and placement arguments as you type.
- The editor's document-formatting command uses bgraph's formatter.
  A source file that cannot parse is left unchanged.

Suggestions stay quiet inside strings and comments.

## Other editors

`bgraph-lsp` communicates over standard input and output using LSP.
Configure your editor to launch it for `.bd` files. The repository provides
the grammar and queries for other tree-sitter clients.

See the [repository editor guide](https://github.com/Bunchyearth23/bgraph/blob/main/editors/README.md)
for Neovim, Helix and Emacs configurations.
