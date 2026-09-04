// bgraph for Typst.
//
// A Typst plugin is wasm — a package cannot shell out — and the renderer
// already compiles to wasm, so this is that module wrapped for `plugin()`.
//
// Build the module first:
//
//   ./typst/build.sh               # → typst/bgraph.wasm
//
// Its own module, not the playground's: Typst's protocol works through imports
// the host supplies, so a module built for it cannot be started by anything
// that does not supply them.
//
// then, from a document:
//
//   #import "bgraph.typ": bg-diagram
//   #bg-diagram(read("arch.bd"))
//
// The two-command form works today with no plugin at all and is the right
// answer when the diagram is already a file beside the document:
//
//   bgraph arch.bd -o arch.svg
//   #image("arch.svg")
//
// This is for when the source lives *in* the document, or is assembled there.

#let _plugin = plugin("bgraph.wasm")

/// Render bgraph source and return it as an image.
///
/// `width` is passed straight to `image`, so a diagram scales like any other
/// figure.
///
/// **A diagram that does not parse fails the build**, with the compiler's own
/// message and the line of the document that asked for it. That is Typst's
/// doing — a plugin returning non-zero is an error — and it is the right
/// answer: it matches what `check` does on the command line, and a document
/// that ships with a hole where a diagram should be is worse than one that
/// refused to build.
#let bg-diagram(source, width: auto, alt: none) = image(
  _plugin.bgraph_typst_render(bytes(source)),
  format: "svg",
  width: width,
  alt: alt,
)

/// The same, reading the source from a file beside the document.
///
/// Pass a Typst `path`, not a string: `path("arch.bd")`. Typst resolves a
/// path where it is created, so the package then reads relative to the calling
/// document rather than relative to this package.
#let bg-diagram-file(source, width: auto, alt: none) = bg-diagram(
  read(source),
  width: width,
  alt: alt,
)
