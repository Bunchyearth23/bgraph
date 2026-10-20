# Your first diagram

By the end of this tutorial, you will have a source file, an SVG image and
a live browser preview. You will also know how to change the labels,
placement and appearance.

**Before you start:** [install bgraph](installation.md) and check that
`bgraph --version` works. Use any plain text editor, such as VS Code, Zed
or a basic text editor.

Run commands in a **terminal**: Terminal on Linux/macOS, or PowerShell on
Windows. Type one command at a time and press Enter. Diagram code goes in
your **text editor**, saved as a `.bd` file.

## 1. Make a working folder

Open a terminal and create a folder for this tutorial:

```sh
mkdir bgraph-tutorial
cd bgraph-tutorial
```

These commands work in a typical Linux/macOS shell and in Windows
PowerShell. Keep using this folder for the commands below. A filename such
as `architecture.bd` refers to a file in the terminal's current folder.

## 2. Save a complete diagram

Create a plain text file named **`architecture.bd`** in that folder. Copy
all of the following into it and save:

```bgraph
node browser "Browser"
node api "API"

layout {
  place browser at (0, 0)
  place api right-of browser gap 60
}

browser -> api "HTTPS"
```

Make sure the editor has not saved it as `architecture.bd.txt`.

Here is what each part means:

| Text | Meaning |
|---|---|
| `node` | Declare a box |
| `browser` or `api` | An identifier used to refer to that box elsewhere in the file |
| `"Browser"` or `"API"` | The label the reader sees inside the box |
| `layout { … }` | A block describing where the boxes go |
| `place browser at (0, 0)` | Anchor the Browser box at the drawing's origin |
| `place api right-of browser gap 60` | Put API to its right, with 60 drawing units between their borders |
| `browser -> api "HTTPS"` | Draw an arrow from Browser to API and label it HTTPS |

Identifiers and visible labels serve different purposes: changing `"API"`
to `"Web service"` changes the drawing; renaming `api` means updating every
reference to that identifier.

A file without a `type` declaration uses `free`, the general-purpose
boxes-and-arrows diagram type.

## 3. Check the file and make an image

In the terminal, run:

```sh
bgraph check architecture.bd
bgraph architecture.bd -o architecture.svg
```

A successful check has no error diagnostics. The render command creates
**`architecture.svg`** in the same folder. Open it in a browser, for example
by dragging it into a browser window.

You should see **Browser on the left, API on the right, and an arrow
pointing from Browser to API**. The two rectangles have a gap between them.

The `.bd` file is the source you edit. The `.svg` file is the generated
image you share. Rendering again replaces the output image.

## 4. Keep a live preview open

In the terminal:

```sh
bgraph watch architecture.bd --open --port 0
```

The browser opens and the terminal prints an address such as
`http://127.0.0.1:54321`. Your port number will differ: `--port 0` asks for
a free port. If the browser does not open, copy the printed address into it.

Leave this command running. In your editor, change `"Browser"` to
`"Web browser"` and save the file. The browser preview updates. Change
`gap 60` to `gap 100` and save again: the boxes move farther apart.

The preview reads **saved files**. It does not rewrite the source or update
your exported SVG. To refresh the export, run the render command again in
another terminal, or stop the watcher first.

Press **`Ctrl+C` in the watcher's terminal** when you want to stop it.
Closing the browser tab alone does not stop the watcher.

## 5. Add a database and some color

Replace the source with this complete version, then save:

```bgraph
graph {
  title: "A small application"
  padding: 32
}

style service { fill: #dbeafe stroke: #2563eb 1.5 radius: 8 }
style database extends service { shape: cylinder fill: #d1fae5 stroke: #059669 1.5 }

node browser "Web browser"
node api "API" .service
node db "Database" .database

layout {
  place browser at (0, 0)
  place api right-of browser gap 60
  place db below api gap 48
}

browser -> api "HTTPS"
api -> db "SQL" { router: ortho }
```

You should now see a blue API box and a green cylinder below it. The new
arrow connects API to Database.

`style service` names reusable appearance settings. `.service` applies
that style to a node. `extends service` reuses it as the base for another
style. `router: ortho` requests right-angle routing for that arrow.

For a PDF, run:

```sh
bgraph architecture.bd -o architecture.pdf
```

## 6. Recognize and fix a mistake

Temporarily change the last arrow's destination from `db` to `database`,
without changing the node declaration. Save and run:

```sh
bgraph check architecture.bd
```

The diagnostic reports an unknown endpoint and shows where it was used.
A style name does not declare a node: `database` is a style here, while
`db` is the node identifier. Restore `db`, save and check again.

Most language mistakes are diagnosed rather than silently ignored. Fix the
first error first, then rerun the check.

## Remember these rules

- Put visible labels in double quotes.
- Use `//` for comments. `#` begins a hexadecimal color.
- Use braces `{ … }` for blocks and newlines to separate statements.
- Declare boxes before referring to them in your own first examples.
- Keep editing the `.bd` file; regenerate the export when it is ready.

## Where to go next

- [Editor setup](editeurs.md): use `Ctrl+Shift+V` in VS Code or Zed.
  VS Code's pane can preview unsaved text; Zed opens the saved-file browser
  preview.
- [Placement](placement.md): align boxes, make rows and group related elements.
- [Styles](styles.md): colors, shapes, fonts and spacing.
- [Diagram types](graphiques.md): charts, processes and specialized notations.
- [Gallery](galerie.md): start from a worked example with
  `bgraph init <type> -o example.bd`.
