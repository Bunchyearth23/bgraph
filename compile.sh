#!/usr/bin/env bash
# Compile a Typst document to out/.
#
# The repository carries no font files. But the system only has Archivo Regular
# and SemiBold Italic — the theme asks for weights 500 to 800, and Typst was
# synthesising every one of them from Regular. Fake bold in a printed manual is
# the kind of thing you see once and cannot unsee.
#
# So the faces are read from where they already live on this machine, outside
# the tree. If that directory is absent the build still works; it just falls
# back to whatever fontconfig can offer, and the headings go soft.
set -euo pipefail

faces="${BTYP_FONTS:-$HOME/Desktop/typ-base/btyp}"
font_args=()
[[ -d "$faces" ]] && font_args=(--font-path "$faces")

# The documents render their diagrams through bgraph's own Typst plugin
# (`W-138`), which is a wasm module and not a checked-in file. Built here when
# it is missing, so `./compile.sh docs/manual.typ` is still one command on a
# fresh clone.
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
[[ -f "$here/typst/bgraph.wasm" ]] || "$here/typst/build.sh"

[[ $# -eq 1 ]] || { echo "usage: ./compile.sh docs/<name>.typ" >&2; exit 1; }
src="$1"
name="$(basename "$src" .typ)"

mkdir -p out
typst compile --root . "${font_args[@]}" "$src" "out/$name.pdf"
echo "out/$name.pdf"
