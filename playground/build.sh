#!/usr/bin/env bash
# Build the wasm module the playground loads.
#
#   ./build.sh          # → playground/public/bgraph.wasm
#
# Vite serves `public/` at the site root, so the page fetches `/bgraph.wasm`
# in development and the file is copied verbatim into `dist/` on a build.
#
# There used to be a second form that folded the module into the page as a
# data URI, so it could be opened from the filesystem by double-clicking. It
# went when the playground became an application: `pnpm dev` is one command,
# it gives a real editor and hot reload, and a 1.5 MB single file was a worse
# answer to a question nobody was asking any more (`W-135`).

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here/.."

target=wasm32-unknown-unknown
# No `cli`, no `csv`, no `pdf`: a page has no argument parser, no disk to read
# a table from and nowhere to put a PDF. `wasm` is the entry points.
features=all-archetypes,json,wasm,kasuari-spike

rustup target list --installed | grep -qx "$target" \
  || { echo "build.sh: rustup target add $target" >&2; exit 1; }

cargo build --lib --release --target "$target" --no-default-features --features "$features"
mkdir -p playground/public
cp "target/$target/release/bgraph.wasm" playground/public/bgraph.wasm
printf 'playground/public/bgraph.wasm — %s\n' "$(du -h playground/public/bgraph.wasm | cut -f1)"
