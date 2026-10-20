# Rust, browser and Typst integrations

For a first diagram, use [the command-line tutorial](demarrage.md).
This page is for embedding bgraph in another program or document workflow.

## Call bgraph from Rust

The project is not published on crates.io. With repository access, add a
path dependency to your application's `Cargo.toml`:

```toml
[dependencies]
bgraph = { path = "/path/to/bgraph", default-features = false }
```

Replace the path with your local checkout. Without additional features,
the library includes `free`. Enable specific types with
`features = ["pie", "packet"]`, or use `all-archetypes` for every type.
The `pdf` feature enables PDF functions.

A complete SVG-rendering program:

```rust
fn main() {
    let source = r#"
        node a "Start"
        node b "Finish"
        layout { place b right-of a gap 48 }
        a -> b
    "#;
    let svg = bgraph::render_source(source).expect("valid diagram");
    std::fs::write("diagram.svg", svg).expect("write SVG");
}
```

`render_source` takes self-contained text and does not read files.
In a function returning a compatible `Result`, resolve includes with:

```rust
let svg = bgraph::render_file(std::path::Path::new("architecture.bd"))?;
```

The [API contract](API.md) describes supported functions, diagnostic units
and inspection surfaces that are outside the compatibility promise.

## Run the browser playground locally

From the repository, with Rust, Node.js and pnpm installed:

```sh
rustup target add wasm32-unknown-unknown
./playground/build.sh
cd playground
pnpm install
pnpm dev
```

The build script requires Bash. Open the address printed by Vite.
The playground has a source editor, SVG output, diagnostics and a menu of
worked examples supplied by the compiler.

`include` and external `from:` files are unavailable: the page has no
source directory on disk. Use the CLI to generate their SVG instead.

## Insert an image into Typst

The simplest integration is an ordinary SVG export:

```sh
bgraph architecture.bd -o architecture.svg
```

In a Typst document beside the image:

```typst
#image("architecture.svg", width: 80%)
```

Regenerate the image whenever its source changes.

## Render while Typst compiles

To prepare the local plugin from the repository:

```sh
rustup target add wasm32-unknown-unknown
./typst/build.sh
```

In a document at the repository root:

```typst
#import "typst/bgraph.typ": bg-diagram-file

#bg-diagram-file(path("architecture.bd"), width: 80%)
```

Adjust the import if your document lives elsewhere. Create
`path("architecture.bd")` in the calling document so its directory is
preserved. An invalid diagram fails the Typst compilation with bgraph's
diagnostic.

The plugin takes self-contained source. Generate an SVG beforehand for
includes or external CSV data. The
[Typst package README](https://github.com/Bunchyearth23/bgraph/blob/main/typst/README.md)
also describes installing the local package.
