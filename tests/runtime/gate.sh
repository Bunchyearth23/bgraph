#!/usr/bin/env bash
# Execute the same fixtures through native, browser, WASI and Typst runtimes.
# --extensions also builds Zed and exercises the packaged VS Code renderer.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
case "${1:-}" in
  ""|--extensions) ;;
  *) echo "usage: $0 [--extensions]" >&2; exit 2 ;;
esac
for tool in cargo rustup node typst; do
  command -v "$tool" >/dev/null || { echo "gate: missing $tool" >&2; exit 1; }
done
for target in wasm32-unknown-unknown wasm32-wasip1; do
  rustup target list --installed | grep -qx "$target" \
    || { echo "gate: rustup target add $target" >&2; exit 1; }
done
gate_dir="$(mktemp -d)"
trap 'rm -rf "$gate_dir"' EXIT
# Every path below names this directory, even if the caller configured Cargo.
export CARGO_TARGET_DIR="$PWD/target/runtime-gate"
cargo build --locked --no-default-features
cargo build --locked --bins
cargo build --locked --lib --target wasm32-unknown-unknown \
  --no-default-features --features all-archetypes,json,wasm
cp "$CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/bgraph.wasm" "$gate_dir/browser.wasm"
cargo build --locked --lib --target wasm32-wasip1 \
  --no-default-features --features all-archetypes,json,wasm
cp "$CARGO_TARGET_DIR/wasm32-wasip1/debug/bgraph.wasm" "$gate_dir/wasi.wasm"
cargo build --locked --lib --target wasm32-unknown-unknown \
  --no-default-features --features all-archetypes,json,typst
cp "$CARGO_TARGET_DIR/wasm32-unknown-unknown/debug/bgraph.wasm" "$gate_dir/typst.wasm"
if [[ "${1:-}" == --extensions ]]; then
  CARGO_TARGET_DIR="$PWD/editors/zed/target" cargo build --locked \
    --manifest-path editors/zed/Cargo.toml --target wasm32-wasip1
  # Package the module built by this invocation, never a stale local preview.
  mkdir -p "$gate_dir/editors"
  cp -R editors/vscode "$gate_dir/editors/vscode"
  cp LICENSE "$gate_dir/LICENSE"
  cp "$gate_dir/browser.wasm" "$gate_dir/editors/vscode/bgraph.wasm"
  "$gate_dir/editors/vscode/package.sh"
  npm --prefix "$gate_dir/editors/vscode" test
fi
node tests/runtime/gate.mjs "$gate_dir" "${1:-}"
