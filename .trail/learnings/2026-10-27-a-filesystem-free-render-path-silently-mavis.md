---
title: A filesystem-free render path silently ignores disk features
author: mavis
date: 2026-10-27
branch: main
id: l-383bd145
scope: repo
status: active
touches:
  - editors/vscode/src/extension.js
  - src/compose.rs
---

## Decided

`type pie { from: "sales.csv" }` in the VS Code preview drew a pie **without
the data and without any diagnostic**. `read_data` only runs on the file path
(`src/compose.rs`); the in-memory path the WASM pane uses ignores `from:`
altogether. The extension now names `bgraph watch` for an `include` and shows a
notice beside a picture that ignored a table — the drawing is kept, because it
is right about everything the buffer does declare.

## Tried and ruled out

- Hiding the picture instead: the rest of the diagram is correct, and hiding it
  would be a worse lie than a caveat.
- Failing the render: the file is not an error, it is a capability the surface
  does not have.

## Watch out

- The same surface gap exists in the browser playground, which also uses the
  in-memory path. It is not fixed yet.
- A message naming `bgraph::render_file` is a Rust function call in a place
  where nobody can call it. The wording is pinned by a test so a change in
  `src/lib.rs` fails loudly instead of restoring the unusable message.
