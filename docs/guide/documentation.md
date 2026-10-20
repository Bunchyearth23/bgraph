# Maintain this documentation

The documentation site uses MkDocs with its bundled Read the Docs theme.
Its Python environment is separate from the Rust compiler.

All user documentation, navigation, generated gallery text and teaching
examples are maintained in **English**. Keep beginner instructions complete:
say where a command runs, what file it uses and what success looks like.

## Open the local site

From the repository root, on Linux/macOS with Bash and Python 3:

```sh
./docs.sh
```

The script prepares the documentation environment and serves the site at
**<http://127.0.0.1:8000>**. Leave the terminal running and open that address
in a browser. Use `Ctrl+C` to stop it.

To change the port or read launcher help:

```sh
./docs.sh serve --dev-addr 127.0.0.1:8001
./docs.sh --help
```

This script requires a network connection when dependencies must first be
installed. It does not install the bgraph CLI or start the diagram preview.

## Build a static site

```sh
./docs.sh build
```

The output is `site/`. The build uses strict validation and publishes
nothing. Run it in another terminal if the development server is still open.

To view the built site over HTTP:

```sh
python3 -m http.server 8000 --directory site
```

Use HTTP rather than opening the generated HTML directly: search loads a
local index.

## Manual setup, including PowerShell

From the repository root with Python 3:

```sh
python3 -m venv .tools/docs-venv
.tools/docs-venv/bin/python -m pip install -r requirements-docs.txt
.tools/docs-venv/bin/python -m mkdocs serve
```

In Windows PowerShell:

```powershell
py -m venv .tools/docs-venv
.tools/docs-venv/Scripts/python.exe -m pip install -r requirements-docs.txt
.tools/docs-venv/Scripts/python.exe -m mkdocs serve
```

The site is at `http://127.0.0.1:8000`. To build it, replace `serve` with
`build --strict`. No environment activation is needed because these
commands name the environment's Python directly.

## Where content belongs

| Content | Source |
|---|---|
| Beginner and task guides | `docs/guide/*.md` |
| Navigation, language and search settings | `mkdocs.yml` |
| Site appearance | `docs/guide/stylesheets/extra.css` |
| Full language manual | `docs/language.md` |
| Rust API contract | `docs/API.md` |
| Exhaustive property/type reference | `src/docgen.rs` and the tables in `src/props.rs` |
| Worked examples | `src/examples.rs`, then the generated `examples/` gallery |
| Canonical manual links and gallery-page text | `docs/site_hooks.py` |

The existing guide filenames are retained so previously shared page URLs
continue to work. Their titles, navigation and content are English.

**Do not edit `docs/reference.md` by hand.** After changing the generator,
regenerate it with:

```sh
cargo run --locked --bin bgraph -- docs -o docs/reference.md
```

This rebuilds the generator before executing it.

The site hook exposes the three canonical manuals without maintaining
duplicate copies. It also assembles the gallery from the existing
`.bd`/`.svg` pairs. Project tests check those pairs against the compiler;
MkDocs itself does not run the Rust renderer.

## Verify a documentation change

1. Follow the beginner route from a new working folder.
2. Render every complete diagram example you changed.
3. Check links and anchors with a strict site build.
4. Open the home page, installation page, tutorial, editor guide and gallery
   in a browser. Check navigation and search.
5. Run the repository's documentation-related Rust tests when changing
   README examples, canonical manuals or editor paths.

Use a `bgraph` code fence only for a complete diagram that can be rendered
as written. Use `text` for a partial block, a template needing bindings or
one file from a multi-file example, and supply its required context.

The site includes the guides, three manuals and gallery. The operational
index, historical work lists and session journals are repository records.
