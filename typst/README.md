# bgraph for Typst

Two ways, and the first needs nothing from this directory.

## Already possible: render, then embed

```sh
bgraph arch.bd -o arch.svg
```

```typst
#image("arch.svg")
```

This is what this project's own manual does. It is the right answer when the
diagram is a file beside the document: the `.bd` is versioned next to the
`.svg`, both are reviewable, and the document build stays a document build.

## The plugin: source inside the document

A Typst plugin is wasm — a package cannot shell out — and the renderer already
compiles to wasm, so `bgraph.typ` is that module wrapped for `plugin()`.

```sh
./typst/build.sh               # → typst/bgraph.wasm
```

```typst
#import "bgraph.typ": diagram, diagram-file

#diagram("
  node ui \"Browser\"
  node api \"API\" right-of ui
  ui -> api
")

#diagram-file("arch.bd", width: 80%)
```

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
