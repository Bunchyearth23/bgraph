#!/usr/bin/env bash
# Build the module Typst's plugin protocol expects.
#
# Separate from `playground/build.sh` on purpose. Typst's protocol works
# through *imports* the host supplies, so a module built with them cannot be
# instantiated by anything else — the playground and the Markdown preview both
# pass an empty import object and would fail to load it.

set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here/.."

target=wasm32-unknown-unknown
target_dir="${CARGO_TARGET_DIR:-target}"
# Drain the producer: grep -q can trigger SIGPIPE, which pipefail treats as failure.
rustup target list --installed | grep -x "$target" >/dev/null \
  || { echo "build.sh: rustup target add $target" >&2; exit 1; }

cargo build --lib --release --target "$target" --target-dir "$target_dir" \
  --no-default-features --features all-archetypes,json,typst

cp "$target_dir/$target/release/bgraph.wasm" typst/bgraph.wasm
printf 'typst/bgraph.wasm — %s\n' "$(du -h typst/bgraph.wasm | cut -f1)"
