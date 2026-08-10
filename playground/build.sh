#!/usr/bin/env bash
# Build the wasm module and, optionally, fold it into a single file.
#
#   ./build.sh            # bgraph.wasm beside index.html — serve the directory
#   ./build.sh --inline   # standalone.html, openable from the filesystem
#
# Two forms because browsers refuse `fetch` from a `file://` page, and a
# playground you cannot open by double-clicking is a playground nobody tries.

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here/.."

target=wasm32-unknown-unknown
# No `cli`, no `csv`, no `pdf`: a page has no argument parser, no disk to read
# a table from and nowhere to put a PDF. `wasm` is the four entry points.
features=all-archetypes,json,wasm

rustup target list --installed | grep -qx "$target" \
  || { echo "build.sh: rustup target add $target" >&2; exit 1; }

cargo build --lib --release --target "$target" --no-default-features --features "$features"
cp "target/$target/release/bgraph.wasm" playground/bgraph.wasm
printf 'playground/bgraph.wasm — %s\n' "$(du -h playground/bgraph.wasm | cut -f1)"

[[ "${1:-}" == "--inline" ]] || exit 0

# The module as a data URI inside the page. Through a file rather than an
# argument: a megabyte of base64 is past what `execve` will carry, and the
# first attempt failed with "Argument list too long".
node -e '
  const fs = require("fs");
  const page = fs.readFileSync("playground/index.html", "utf8");
  const wasm = fs.readFileSync("playground/bgraph.wasm").toString("base64");
  fs.writeFileSync(
    "playground/standalone.html",
    page.replace("./bgraph.wasm", "data:application/wasm;base64," + wasm),
  );
'

printf 'playground/standalone.html — %s\n' "$(du -h playground/standalone.html | cut -f1)"
