#!/usr/bin/env bash
#
# Prepares the Zed extension for installation.
#
# Zed fetches a tree-sitter grammar from a git repository at a pinned revision,
# even when that repository is this one on the local disk. So this script:
#
#   1. installs `bgraph` and `bgraph-lsp` (preview and diagnostics use PATH),
#   2. regenerates the tree-sitter parser,
#   3. commits, if the generated parser is not committed yet,
#   4. rewrites `repository` and `rev` in extension.toml to point at HEAD.
#
# Re-run it after any change to tree-sitter-bgraph/grammar.js.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
manifest="$here/extension.toml"

cd "$repo"

# ---------------------------------------------------------------- 1. server

echo "==> installing bgraph and bgraph-lsp"
cargo install --path . --bins --force --quiet
server="$(command -v bgraph-lsp || true)"
if [ -z "$server" ]; then
  echo "!! bgraph-lsp is not on PATH — add ~/.cargo/bin to it, or set the" >&2
  echo "   binary path in Zed's settings.json (see editors/zed/README.md)." >&2
else
  echo "    $server"
fi

# ---------------------------------------------------------------- 2. grammar

echo "==> generating the tree-sitter parser"
if command -v tree-sitter >/dev/null 2>&1; then
  (cd tree-sitter-bgraph && tree-sitter generate)
else
  echo "!! tree-sitter CLI not found — install it with:" >&2
  echo "     cargo install tree-sitter-cli" >&2
  echo "   Skipping regeneration; the committed parser will be used." >&2
fi

# ---------------------------------------------------------------- 3. commit

if [ -z "$(git rev-parse --verify HEAD 2>/dev/null || true)" ]; then
  echo "!! this repository has no commits yet." >&2
  echo "   Zed pins a grammar to a revision, so commit first:" >&2
  echo "     git add -A && git commit -m 'initial commit'" >&2
  exit 1
fi

if ! git diff --quiet -- tree-sitter-bgraph; then
  echo "==> committing the regenerated parser"
  git add tree-sitter-bgraph
  git commit -m "tree-sitter-bgraph: regenerate parser" --quiet
fi

rev="$(git rev-parse HEAD)"

# ---------------------------------------------------------------- 4. manifest

echo "==> pinning the grammar to $rev"
tmp="$(mktemp)"
sed -e "s|^repository = \"file://.*\"|repository = \"file://$repo\"|" \
    -e "s|^rev = \".*\"|rev = \"$rev\"|" \
    "$manifest" > "$tmp"
mv "$tmp" "$manifest"

cat <<EOF

Ready.

  In Zed:  cmd/ctrl-shift-p  →  "zed: install dev extension"
           choose  $here

If you change grammar.js later, re-run this script — Zed caches the grammar
per revision, so the pin has to move for it to notice.
EOF
