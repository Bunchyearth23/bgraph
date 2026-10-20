# Editors

`bgraph-lsp` is a language server speaking LSP over stdin and stdout. It works
with any editor that speaks the protocol; two are packaged here, and the rest
need a few lines of configuration.

What the server gives you, in every editor below:

| | |
|---|---|
| Diagnostics | the same ones `bgraph check` prints, including the ones that need the boxes placed |
| Completion | properties, keywords and archetype settings, filtered by where the cursor is |
| Hover | the resolved style of a box, and what a property means |
| Signature help | the positional shape of the statement being written |
| Go to definition | from an edge endpoint, an anchor or an `attach` to the item it names, under whichever word the genre declares it with |
| Document symbols | the outline |
| Colours | a swatch beside every colour literal, and a picker |
| Highlight | every other mention of the identifier under the cursor |

Syntax highlighting comes from one of two grammars in this repository:

- **`tree-sitter-bgraph/`** — the grammar, used by Zed, Neovim, Helix and Emacs.
  Its highlight, bracket, indent and outline queries live beside the Zed
  extension, in **`editors/zed/languages/bgraph/`**, because Zed was the first
  editor to need them. Every editor below points at that one directory rather
  than keeping a copy — a copy is a copy that goes stale.
- **`editors/vscode/syntaxes/bgraph.tmLanguage.json`** — TextMate, used by VS Code.

Both are kept in step with `src/syntax.pest` by a test, so neither can quietly
fall behind the language.

---

## First, put the server on `PATH`

Every editor below assumes it can run `bgraph-lsp`.

```sh
cargo build --release
cp target/release/bgraph-lsp ~/.local/bin/      # or anywhere on PATH
```

---

## VS Code

Packaged. Build the extension and install it:

```sh
cd editors/vscode
./package.sh                    # or ./package.sh --no-server for highlighting and preview
code --install-extension bgraph-1.0.0.vsix --force
```

### Preview a .bd file

Press **Ctrl+Shift+V** (**Cmd+Shift+V** on macOS) in a bgraph editor, or run
**bgraph: Open Preview**. The side pane renders the current buffer as you type,
including unsaved and untitled documents. The extension reuses one pane per
document and displays rendering errors in place. Its bundled wasm renderer
works independently of the language server and CLI.

Sources must be self-contained: the wasm preview does not resolve `include`
or external `from:` files. Use `bgraph watch file.bd --open` for those.

### Diagrams in the Markdown preview

A ` ```bgraph ` fence is drawn in VS Code's Markdown preview. It renders
through the wasm module bundled in the extension, not through a `bgraph` on
`PATH` — a preview that only worked for people who had already installed the
CLI would miss exactly the readers it is for. `package.sh` builds and bundles
that module; without it the fence quietly falls back to a code block rather
than taking the document's rendering down.

## Zed

Packaged in `editors/zed`. The language supplies **bgraph: Preview**, a task
which saves the current file and runs `bgraph watch` on a free port, opening
the browser after the server starts. Open the containing folder as a project
before the `.bd` file so Zed exposes the language task. Saving updates the
drawing. The `editors/zed/keymap.json` snippet binds **Ctrl+Shift+V** in bgraph editors;
merge it into your Zed keymap. See the extension's README for setup.
The task uses Linux/macOS shell syntax; its README describes the PowerShell
argument override separately.

This preview uses the browser: Zed's extension API does not provide a custom
preview pane or a hook for rendering bgraph fences in Markdown.

---

## Neovim

Neovim has both halves built in: `vim.lsp` for the server and `nvim-treesitter`
for the grammar. Nothing needs to be packaged.

```lua
-- 1. Teach Neovim the file type.
vim.filetype.add({ extension = { bd = "bgraph" } })

-- 2. The language server.
vim.lsp.config.bgraph = {
  cmd = { "bgraph-lsp" },
  filetypes = { "bgraph" },
  -- A diagram is a single file: there is no project to find, so the
  -- directory the file lives in is the right root.
  root_markers = { ".git" },
}
vim.lsp.enable("bgraph")

-- 3. Highlighting, from the grammar in this repository.
require("nvim-treesitter.parsers").get_parser_configs().bgraph = {
  install_info = {
    url = "/path/to/bgraph/tree-sitter-bgraph",
    files = { "src/parser.c" },
  },
  filetype = "bgraph",
}
```

Then `:TSInstall bgraph`. Neovim reads its queries from `queries/<lang>/` on
the runtime path, so link the ones in this repository rather than copying
them:

```sh
mkdir -p ~/.config/nvim/queries/bgraph
ln -s /path/to/bgraph/editors/zed/languages/bgraph/highlights.scm \
      ~/.config/nvim/queries/bgraph/highlights.scm
ln -s /path/to/bgraph/editors/zed/languages/bgraph/indents.scm \
      ~/.config/nvim/queries/bgraph/indents.scm
```

---

## Helix

Two entries in `~/.config/helix/languages.toml`:

```toml
[language-server.bgraph]
command = "bgraph-lsp"

[[language]]
name = "bgraph"
scope = "source.bgraph"
file-types = ["bd"]
comment-token = "//"
indent = { tab-width = 2, unit = "  " }
language-servers = ["bgraph"]

[[grammar]]
name = "bgraph"
source = { path = "/path/to/bgraph/tree-sitter-bgraph" }
```

Then:

```sh
hx --grammar fetch
hx --grammar build
```

Helix looks for its queries in `~/.config/helix/runtime/queries/bgraph/`:

```sh
mkdir -p ~/.config/helix/runtime/queries/bgraph
for q in highlights indents; do
  ln -s /path/to/bgraph/editors/zed/languages/bgraph/$q.scm \
        ~/.config/helix/runtime/queries/bgraph/$q.scm
done
```

---

## Emacs

`eglot` is built in from Emacs 29. A derived mode is needed because `eglot`
keys off the major mode:

```elisp
(define-derived-mode bgraph-mode prog-mode "bgraph"
  "Major mode for bgraph diagrams."
  (setq-local comment-start "// ")
  (setq-local comment-end ""))

(add-to-list 'auto-mode-alist '("\\.bd\\'" . bgraph-mode))

(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs '(bgraph-mode . ("bgraph-lsp"))))

(add-hook 'bgraph-mode-hook #'eglot-ensure)
```

For highlighting, `treesit` (Emacs 29+) can load the grammar:

```elisp
(add-to-list 'treesit-language-source-alist
             '(bgraph "/path/to/bgraph/tree-sitter-bgraph"))
;; M-x treesit-install-language-grammar RET bgraph RET
```

---

## Anything else

The server needs no arguments and no configuration file: run `bgraph-lsp` and
speak LSP at it. `.bd` is the extension. `//` starts a comment, and `#` never
does — it begins a hex colour, which is the one thing worth telling an editor
by hand if it is guessing.
