# playground

The bgraph renderer compiled to wasm, in a page: source on the left, SVG on
the right, every diagnostic underneath.

```sh
./build.sh          # the wasm module → public/bgraph.wasm
pnpm install
pnpm dev            # http://localhost:5173
pnpm build          # → dist/, a folder you can serve or unzip anywhere
```

`build.sh` is a separate step because it needs a Rust toolchain and the wasm
target, and the page does not: once the module is in `public/`, working on the
interface is a JavaScript loop with nothing Rust in it.

## What is here

| Path | What it is |
|---|---|
| `src/lib/bgraph.js` | The wasm boundary. The only file that knows a pointer exists |
| `src/lib/language.js` | Syntax highlighting, as a CodeMirror stream tokenizer |
| `src/lib/Editor.svelte` | The editor, with a diagnostic dot per line in the gutter |
| `src/lib/Viewer.svelte` | The picture, with pan and zoom |
| `src/App.svelte` | The two panes, the genre menu, share and download |

## Two things worth knowing

**The genre menu is the module's own table.** `bgraph_examples()` hands back
the `EXAMPLES` the crate carries, so the page cannot offer a genre the module
cannot render, or miss one it can — the same argument that made the reference
manual generated rather than written.

**Rendering and checking are separate calls.** A file can render *and* have
four warnings. `bgraph_render` answers with the SVG or a string beginning
`error:`, one channel so there is no second call to forget; `bgraph_check`
answers with every diagnostic, positions and all.

**A diagram that reads another file is drawn from what the page has.** The
module renders in memory, so `include` is refused — with a message naming
`bgraph watch` rather than a Rust function — and a chart that reads its data
from a table keeps its picture with a note beside it saying the table was not
read. Hiding the picture would be a worse lie than the caveat.

There used to be a `--inline` build that folded the module into a single
1.5 MB HTML file, openable by double-clicking. It went when this became an
application (`W-135`): `pnpm dev` is one command and gives a real editor.
