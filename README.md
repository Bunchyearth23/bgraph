# bgraph

**Write a text file. Turn it into a diagram.**

bgraph reads a plain text file ending in `.bd` and generates an SVG image
or a PDF document. You describe the elements, their connections and their
appearance. For architecture and process diagrams, you also choose their
relative positions. For charts, you supply the data.

Use a text editor to change the source, keep a browser or editor preview
open, and export the finished drawing. You do not need to write a program
to use the command-line tools.

![A worked application diagram](examples/free.svg)

## Start here

The **[user guide](docs/guide/index.md)** is the starting point for a first-time
user:

1. **[Install bgraph](docs/guide/installation.md)** from a supplied package or
   from a source checkout. Verify that `bgraph --version` works.
2. **[Create your first diagram](docs/guide/demarrage.md)** with a complete
   example, line-by-line explanations and expected results.
3. **[Set up VS Code or Zed](docs/guide/editeurs.md)** for suggestions,
   diagnostics and a preview shortcut.

The guide, site navigation and worked teaching examples are in English.
The [gallery](examples/) contains a source and verified SVG for each of the
30 diagram types.

### Already installed? Start from an example

In a terminal, open a folder where you want to keep your diagram and run:

```sh
bgraph init free -o architecture.bd
bgraph check architecture.bd
bgraph architecture.bd -o architecture.svg
bgraph watch architecture.bd --open --port 0
```

This creates a source file, checks it, exports an image and opens a live
preview on a free local port. Keep the last command running. Edit
`architecture.bd` in your text editor and save to update the browser.
Stop the preview with `Ctrl+C` in its terminal.

The `.bd` file is what you edit. The `.svg` file is an output image;
render again to refresh that export. Closing the browser does not stop
the preview server.

## A small, complete diagram

Save this as `architecture.bd`:

```bgraph
node browser "Browser"
node api "API"
node db "Database"

layout {
  place browser at (0, 0)
  place api right-of browser gap 60
  place db below api gap 48
}

browser -> api "HTTPS"
api -> db "SQL" { router: ortho }
```

`node` declares a box. `api` is its identifier; `"API"` is its visible
label. `layout` describes positions. `right-of` and `below` keep the
drawing related to its neighbors, and `gap` reserves space between them.
An arrow such as `browser -> api` connects the named boxes.

See [the first tutorial](docs/guide/demarrage.md) to add colors and styles,
read an error and export a PDF.

## Preview in an editor

- **VS Code:** install the local VSIX, open a `.bd` file and press
  `Ctrl+Shift+V` (`Cmd+Shift+V` on macOS). The pane updates while you type,
  including unsaved changes. Its renderer is bundled.
- **Zed:** install the tools and extension, open the containing folder as a
  project, and run **task: spawn → bgraph: Preview**. Merge the supplied
  keymap entry to use `Ctrl+Shift+V`. The preview opens in a browser and
  updates on save.

The [editor guide](docs/guide/editeurs.md) covers installation, language-server
paths and troubleshooting. VS Code's direct pane and Markdown renderer
need self-contained source; use `bgraph watch` for `include` or external
`from:` data files.

## What can I draw?

The full build includes **29 specialised genres, plus `free`**.
A genre is a diagram type. List the types present in your build with
`bgraph init`, then create a complete example with `bgraph init <type>`.

| Need | Examples |
|---|---|
| Architecture and networks | `free`, `block`, `network` |
| Software and data models | `class`, `er`, `state`, `requirements` |
| Processes and interactions | `bpmn`, `sequence`, `journey`, `kanban` |
| Charts and comparisons | `pie`, `bar`, `xy`, `radar`, `quadrant`, `treemap` |
| Flows and intersections | `sankey`, `venn` |
| Dates and history | `gantt`, `timeline`, `gitgraph` |
| Hierarchies | `orgchart`, `mindmap`, `treeview` |
| Electronics and infrastructure | `packet`, `bytefield`, `wave`, `harness`, `rack` |

For example, `type pie` interprets declarations as chart data:

```bgraph
type pie { title: "Traffic sources" donut: 0.55 }

slice organic "Organic search" 4820
slice direct "Direct" 2410
slice referral "Referrals" 1180
```

See [Choose a diagram type](docs/guide/graphiques.md) and
the [worked sources and images](examples/).

## Useful next steps

- [Placement](docs/guide/placement.md): relative anchors, rows, columns,
  alignments and groups.
- [Styles](docs/guide/styles.md): reusable classes, shapes, colors, margins
  and `--theme light|dark|print`.
- [Commands](docs/guide/commandes.md): SVG, PDF, optional PNG, checks,
  formatting and comparison.
- [Composition](docs/guide/composition.md): shared style files, reusable
  components, templates and CSV input.
- [Integrations](docs/guide/integrations.md): Rust, the browser playground
  and Typst.
- [Troubleshooting](docs/guide/depannage.md): installation, diagnostics,
  previews and older syntax.

For complete syntax and property lookup, use the
[language manual](docs/language.md),
[generated reference](docs/reference.md) and
[Rust API contract](docs/API.md).

## Read the documentation as a local website

From a source checkout, with Bash and Python 3:

```sh
./docs.sh
```

Open [http://127.0.0.1:8000](http://127.0.0.1:8000). The site adds navigation,
search and an illustrated gallery. Leave the terminal open and stop it with
`Ctrl+C`. `./docs.sh build` produces a static site in `site/`.

See [Maintain this documentation](docs/guide/documentation.md) for manual
setup, including PowerShell. Building the site does not publish it.

## Build and test from source

Install both commands from the repository root:

```sh
cargo install --locked --path . --bins
```

For a local build and the test suite:

```sh
cargo build --locked --release
cargo test --locked
```

See [Installation](docs/guide/installation.md) for Rust requirements and
optional PNG support. The browser playground is in
[playground/](playground/README.md); the Typst package is in
[typst/](typst/README.md).

The runtime gate compares the CLI, LSP, browser wasm, WASI, live preview,
Typst and packaged VS Code renderer:

```sh
./tests/runtime/gate.sh --extensions
```

It covers 22 fixtures, all 30 diagram types and four file contexts.
It requires Node 24, Typst, `zip`/`unzip` and Rust's
`wasm32-unknown-unknown` and `wasm32-wasip1` targets.
Separate [VS Code](tests/runtime/vscode-host.sh) and
[Zed](editors/zed/README.md#isolated-host-verification-linux) checks exercise
real editor hosts with disposable profiles.

## Prepare an internal binary export

From the repository, `./export.sh` prepares an `export/` folder with Linux
x86_64, Windows x86_64 and Windows ARM64 binaries, a VSIX, the Zed extension
and the Typst module. It does not include the project source.

Windows cross-compilation on Linux uses `cargo-xwin`, installed by the
script when needed. To supply existing Windows binaries instead, use
`--windows-bin-dir /path/to/windows/binaries`. Read `./export.sh --help`
before building an export.

## License

[bgraph Noncommercial License 1.0](LICENSE) is written for this project.

**Noncommercial use is permitted:** personal work, study, research, hobby
projects, and use inside a charity, school, public research body or government.

**Commercial use requires a separate written agreement.** Use by or for a
company in the course of its work is commercial, whether or not anything is sold.

**You may pass on an unchanged build** you received, free of charge and with
the license, to someone whose use is also noncommercial.

**You may not modify it, and the source is confidential.** Repository access
is personal and grants no right to copy, publish or pass on the source.
Read [LICENSE](LICENSE) for the full terms.
