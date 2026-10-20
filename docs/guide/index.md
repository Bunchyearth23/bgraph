# Turn text into a diagram

**bgraph turns a plain text file into a diagram you can view, print or share.**
You write the boxes, labels, connections and colors in a file ending in
`.bd`. bgraph reads that file and produces an image.

For an architecture or process diagram, you choose where the elements go.
For a chart, such as a pie chart, you provide the data and bgraph calculates
the positions. You can work in an ordinary text editor; you do not need to
write a program or use a drawing application.

![An application diagram with services and groups](examples/free.svg)

## New to bgraph? Follow these three steps

1. **[Install bgraph](installation.md).** Choose a supplied package or build
   from source, then check that the command works.
2. **[Create your first diagram](demarrage.md).** Copy a complete example,
   make an image and change it while the preview is open.
3. **[Set up your editor](editeurs.md).** Optional extensions add suggestions,
   error highlighting and a preview shortcut in VS Code or Zed.

The first tutorial assumes no knowledge of the bgraph language. Read it
before the full language manual or property reference.

## What are these files and tools?

| Name | Meaning |
|---|---|
| `architecture.bd` | Your editable diagram source: an ordinary text file |
| `architecture.svg` | A generated image, viewable in a browser and scalable without blurring |
| `architecture.pdf` | A generated document for printing or sharing |
| `bgraph` | The command that checks a source file, renders it or starts a browser preview |
| `bgraph-lsp` | An optional helper that gives your editor suggestions and diagnostics |
| `.bds` | A reusable file containing styles; you can ignore it for your first diagram |

Changing an SVG does not update the `.bd` file. Keep editing the source and
generate the image again when you want an updated export.

## Already installed? Try a complete example

Open a terminal in a folder where you can create files:

```sh
bgraph init free -o architecture.bd
bgraph architecture.bd -o architecture.svg
bgraph watch architecture.bd --open --port 0
```

The first command creates a worked example. The second creates an image.
The third opens a browser preview on a free local port; leave its terminal
running and save changes in your editor to update the drawing. Press
`Ctrl+C` in that terminal to stop the preview.

To start with a shorter example and understand every line, follow
[Your first diagram](demarrage.md).

## Find the next guide

| I want to… | Read |
|---|---|
| Put boxes next to each other, align them or make groups | [Placement](placement.md) |
| Change colors, shapes, fonts and spacing | [Styles and colors](styles.md) |
| Pick a chart or a specialized notation | [Diagram types](graphiques.md) |
| See a complete source and image for every type | [Gallery of all 30 types](galerie.md) |
| Export images or automate rendering | [Commands and exports](commandes.md) |
| Use VS Code or Zed | [Editors](editeurs.md) |
| Share styles, combine files or load data | [Files, components and data](composition.md) |
| Call bgraph from Rust, a browser or Typst | [Integrations](integrations.md) |
| Fix an error or a preview problem | [Troubleshooting](depannage.md) |
| Look up an exact property | [Generated reference](reference.md) |

## Other things you can draw

### A chart from data

![Traffic sources as a donut chart](examples/pie.svg){ .overview-image }

Create the complete example with `bgraph init pie -o traffic.bd`.

### A business process

![A BPMN process diagram](examples/bpmn.svg){ .overview-image }

Create the complete example with `bgraph init bpmn -o process.bd`.

The full build includes **29 specialized diagram types plus `free`**.
Use `bgraph init` to list the types available in your particular build.
