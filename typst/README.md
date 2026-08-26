# bgraph for Typst

Two ways, and the first needs nothing from this directory.

## Already possible: render, then embed

```sh
bgraph arch.bd -o arch.svg
```

```typst
#image("arch.svg")
```

It is the right answer when the picture must exist as a file in its own right
— a README, a wiki, anything that embeds an image it does not build.

It is **not** what this project's own manual does any more. Nine `.svg` files
sat beside their sources in `docs/diagrams/`, and on the day the renderer
changed, eight of the nine were quietly out of date: the manual illustrated a
language it no longer described, and nothing said so (`W-138`).

## The plugin: source inside the document

A Typst plugin is wasm — a package cannot shell out — and the renderer already
compiles to wasm, so `bgraph.typ` is that module wrapped for `plugin()`.

```sh
./typst/build.sh               # → typst/bgraph.wasm
```

This is what the manual does: `docs/manual.typ` imports this file and reads
its `.bd` sources, so the document is illustrated by the compiler it documents
and a diagram that stops parsing stops the build.

```typst
#import "bgraph.typ": bg-diagram, bg-diagram-file

#bg-diagram("
  node ui \"Browser\"
  node api \"API\"
  layout {
    place ui at (0, 0)
    place api right-of ui
  }
  ui -> api
")

#bg-diagram-file(path("arch.bd"), width: 80%)
```

`bg-diagram-file` takes a Typst `path`, not a string. The `path("…")` must be
created in the calling document; this preserves that document's directory
when the package reads the `.bd` file.

**A diagram that does not parse fails the build**, with the compiler's own
message and the line of the document that asked for it. That matches what
`check` does on the command line, and a document that ships with a hole where
a diagram should be is worse than one that refused to build.

## What the plugin cannot do

`include` and `from:` both need a path to resolve against, and a wasm module
has no filesystem. They are refused with a message saying so. A document that
needs them should render the diagram to an SVG first and embed that — the form
above.

## Not published

This is not on Typst Universe and nothing here publishes it. `Q-005` — what
licence this code carries — is unanswered, and publishing is a decision this
repository leaves to a person.
