# Troubleshooting

Start with the symptom below. If a diagram is invalid, fix its first error
and check again before changing other settings.

## The terminal cannot find bgraph

If the terminal says “command not found” or “not recognized”:

1. Check that you extracted or installed the executable for your system.
2. Try it from its own folder: `./bgraph --version` on Linux/macOS or
   `.\bgraph.exe --version` in PowerShell.
3. Add that executable folder to your user `PATH` and open a new terminal.
4. Restart your editor too if it was open during installation.

For a local source build, try `./target/release/bgraph --version` or
`.\target\release\bgraph.exe --version` from the repository folder.
See [Installation](installation.md) for the two installation routes.

## The input file cannot be found

Run the command from the folder containing the source, or supply its full
path. Quote paths containing spaces:

```sh
bgraph "my diagrams/architecture.bd" -o architecture.svg
```

Check that the filename really ends in `.bd` rather than `.bd.txt`.
The path after `-o` is the output image, not the source.

## A file does not render

```sh
bgraph check architecture.bd
```

Check the reported file, line and column. Frequent causes are:

- Missing double quotes around a visible label.
- A `#` comment; comments start with `//`.
- A node identifier or `.style` name that was never declared.
- A property in the wrong block: graph, element, edge or type.
- A missing brace or statement separator.

A class member such as `- id: Uuid` needs its own line; put the closing brace
on a later line.

## The browser still shows the old image

A static SVG export does not update itself. Render it again:

```sh
bgraph architecture.bd -o architecture.svg
```

For live editing, keep the watcher running:

```sh
bgraph watch architecture.bd --open --port 0
```

Save the `.bd` source in your editor. The watcher reads saved content;
the VS Code preview pane can read unsaved content. Make sure the browser
tab points to the URL printed by the current watcher.

## The browser preview does not open

Copy the printed URL into the browser manually. Keep the watch command
running in its terminal. `--port 0` avoids a conflict with another process
using the default port.

If `--open` is reported as an unknown option, update the CLI. A new editor
extension does not replace an old `bgraph` executable.

For a template, pass its data too:

```sh
bgraph watch traffic.bd --data traffic.json --open --port 0
```

## VS Code does not offer Open Preview

Check that the extension is installed and that the document's language mode
is **bgraph**. Focus the source editor and run **bgraph: Open Preview**
from the command palette. Update the VSIX if the command is absent.

A missing preview renderer means the package is incomplete. Reinstall a
complete VSIX, or rebuild it with `editors/vscode/package.sh` from the
repository.

If the pane reports that `include` or `from:` needs file access, use
`bgraph watch` for that diagram.

## Zed does not preview with Ctrl+Shift+V

1. Open the containing **folder as a project**, then its saved `.bd` file.
2. Try **task: spawn → bgraph: Preview**.
3. If the task is absent, update or reinstall the bgraph extension.
4. If the task works but the shortcut does not, merge the
   [keymap entry](editeurs.md#3-add-the-preview-shortcut).
5. Check that an updated `bgraph` is on Zed's `PATH`.

Zed uses a browser preview. The supplied task uses Linux/macOS shell
syntax; the extension README describes the PowerShell override.
See the [Zed setup](editeurs.md#zed).

## Colors work, but editor suggestions do not

Syntax highlighting and preview can work without the language server.
[Install `bgraph-lsp`](installation.md), restart the editor, or set its
absolute path in the [editor settings](editeurs.md).
Restart the language server after replacing its binary.

## layout is rejected in my chart

Types such as `pie`, `packet`, `bar` and `gantt` calculate positions from
data. Remove `layout` and use their type settings.
See [the geometry table](graphiques.md#two-ways-to-control-positions).

## Boxes overlap or placement requirements conflict

bgraph diagnoses collisions and incompatible requirements. Increase a gap,
change an anchor or remove a contradictory relationship.

Use `bgraph explain architecture.bd api` and the browser preview inspector
to examine the element's dimensions and relationships. An explicit
`label-offset` can also cause a reported label collision if it leaves
insufficient space.

## Old inline placement syntax is rejected

Use declarations followed by a separate `layout` block:

```bgraph
node a "A"
node b "B"
layout {
  place a at (0, 0)
  place b right-of a gap 40
}
```

Placements appended to a `node` declaration are no longer current syntax.
`fmt --migrate` does not convert them.

## A theme does not change some colors

Settings declared in the source take precedence over `--theme`. Remove
the values you want the theme to choose. See
[the style cascade](styles.md#understand-the-cascade).

## PNG is unavailable or the font looks different

PNG needs a [build with the png feature](installation.md#optional-enable-png-output).
Use an installed font, or `--embed-font /path/to/Font.ttf` with a real
TrueType font file. Embedding in an SVG also carries the font to recipients
who do not have it.

Text widths are estimated before rendering. Embedding a font does not
recalculate those estimates, so a bold group title or a different typeface
can still exceed its background plate. This remains a known layout limitation.

## include works in the CLI but not in an embedded renderer

The browser playground, VS Code pane and Typst plugin render self-contained
text and cannot resolve files on disk. Render the composed source with
the CLI, then insert its SVG into your page or document.

## A prefixed include reports an unknown shared style

If an included diagram loads a style file and you include that diagram
under a prefix, include its shared style file at the top level too:

```text
include "house.bds"
include "application.bd" as backend
```

The current resolver otherwise can lose the original class name, such as
`.service`. The [composition example](composition.md#include-a-diagram-under-a-prefix)
shows this working arrangement.

## Information to keep when asking for help

Include your `bgraph --version` output, operating system, exact command,
full diagnostic and the smallest `.bd` source that reproduces it.
For an editor problem, also record the editor and extension versions.
