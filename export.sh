#!/usr/bin/env bash
# Build a source-free local distribution of bgraph.
#
# The output contains only compiled binaries, generated editor artefacts,
# documentation and the licence.  It deliberately does not copy the Rust
# sources, the Pest grammar or grammar.js.
#
#   ./export.sh
#   BGRAPH_WINDOWS_BIN_DIR=/path/to/windows/release ./export.sh
#   ./export.sh --output /tmp/bgraph-export

set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
output="$repo/export"
windows_bin_dir="${BGRAPH_WINDOWS_BIN_DIR:-}"

usage() {
  cat <<'USAGE'
Usage: ./export.sh [--output DIR] [--windows-bin-dir DIR]

Build a source-free distribution containing:
  linux-x86_64/   bgraph and bgraph-lsp
  windows-x86_64/ bgraph.exe and bgraph-lsp.exe
  windows-aarch64/ bgraph.exe and bgraph-lsp.exe
  vscode/         the VS Code .vsix
  zed/            the precompiled Zed extension and generated grammar
  typst/          the installable Typst package

On Linux, cargo-xwin is installed automatically when needed and builds both
MSVC targets with clang/lld plus the Windows SDK. To avoid that
installation, pass a directory containing one subdirectory per target (for
example DIR/x86_64-pc-windows-msvc and DIR/aarch64-pc-windows-msvc) with
--windows-bin-dir (or BGRAPH_WINDOWS_BIN_DIR).

An existing output must be a recognized bgraph export. It is replaced only
after every build succeeds; other directories and symbolic links are refused.
USAGE
}

while (($#)); do
  case "$1" in
    --output)
      [[ $# -ge 2 ]] || { echo "export.sh: --output needs a directory" >&2; exit 2; }
      case "$2" in
        /*) output="$2" ;;
        *) output="$repo/$2" ;;
      esac
      shift 2
      ;;
    --windows-bin-dir)
      [[ $# -ge 2 ]] || { echo "export.sh: --windows-bin-dir needs a directory" >&2; exit 2; }
      windows_bin_dir="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "export.sh: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -n "$windows_bin_dir" ]]; then
  windows_bin_dir="$(realpath -m -- "$windows_bin_dir")"
fi
cd "$repo"

# Resolve aliases before checking ancestors, so "export/.." cannot erase the repo.
[[ ! -L "$(realpath -ms -- "$output")" ]] \
  || { echo "export.sh: output must not be a symbolic link" >&2; exit 2; }
output="$(realpath -m -- "$output")"
case "$repo/" in
  "$output/"*) echo "export.sh: output must not contain the repository" >&2; exit 2 ;;
esac
[[ "$output" != / && "$output" != "$(realpath -m -- "$HOME")" ]] \
  || { echo "export.sh: refusing unsafe output directory: $output" >&2; exit 2; }

recognized_export() {
  [[ -d "$1" && ! -L "$1" ]] || return 1
  if [[ -f "$1/.bgraph-export" && ! -L "$1/.bgraph-export" ]] \
    && [[ "$(cat "$1/.bgraph-export")" == 'bgraph source-free export v1' ]]; then
    return 0
  fi
  # Older exports have no marker: accept only their complete, known layout.
  local entry
  for entry in "$1"/* "$1"/.[!.]* "$1"/..?*; do
    [[ -e "$entry" || -L "$entry" ]] || continue
    [[ ! -L "$entry" ]] || return 1
    case "${entry##*/}" in
      linux-x86_64|windows-x86_64|windows-aarch64|vscode|zed|typst|README.md) ;;
      *) return 1 ;;
    esac
  done
  for entry in linux-x86_64/bin/bgraph linux-x86_64/bin/bgraph-lsp \
    windows-x86_64/bin/bgraph.exe windows-x86_64/bin/bgraph-lsp.exe \
    windows-aarch64/bin/bgraph.exe windows-aarch64/bin/bgraph-lsp.exe \
    zed/extension.wasm zed/extension.toml typst/bgraph.wasm typst/typst.toml; do
    [[ -f "$1/$entry" ]] || return 1
  done
  for entry in "$1/vscode"/bgraph-*.vsix; do
    [[ -f "$entry" && ! -L "$entry" ]] && return 0
  done
  return 1
}

check_output() {
  if [[ -e "$1" || -L "$1" ]] && ! recognized_export "$1"; then
    echo "export.sh: refusing to replace an unrecognized directory: $1" >&2
    exit 2
  fi
}
check_output "$output"

command -v cargo >/dev/null || { echo "export.sh: cargo is required" >&2; exit 1; }
command -v rustup >/dev/null || { echo "export.sh: rustup is required" >&2; exit 1; }

version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo/Cargo.toml" | head -1)"
[[ -n "$version" ]] || { echo "export.sh: cannot read Cargo.toml version" >&2; exit 1; }

destination="$output"
mkdir -p -- "$(dirname "$destination")"
output="$(mktemp -d "$(dirname "$destination")/.bgraph-export.XXXXXXXX")"
backup=""
cleanup() {
  local status=$?
  if [[ -n "$backup" && -d "$backup/previous" ]]; then
    if [[ ! -e "$destination" && ! -L "$destination" ]] \
      && mv -T -- "$backup/previous" "$destination"; then
      rmdir -- "$backup"
    else
      echo "export.sh: previous export preserved at $backup/previous" >&2
    fi
  elif [[ -n "$backup" && -d "$backup" ]]; then
    rmdir -- "$backup"
  fi
  [[ -z "$output" ]] || rm -rf -- "$output"
  exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
trap 'exit 129' HUP

target_dir="${CARGO_TARGET_DIR:-$repo/target}"
zed_target_dir="${CARGO_TARGET_DIR:-$repo/editors/zed/target}"
mkdir -p "$output/linux-x86_64/bin" "$output/windows-x86_64/bin" \
  "$output/windows-aarch64/bin" "$output/vscode" \
  "$output/zed/grammar-repo/src" "$output/typst"

build_target() {
  local target="$1"
  echo "==> building bgraph for $target"
  cargo build --release --target "$target" --target-dir "$target_dir"
}

build_target x86_64-unknown-linux-gnu
for binary in bgraph bgraph-lsp; do
  cp "$target_dir/x86_64-unknown-linux-gnu/release/$binary" \
    "$output/linux-x86_64/bin/$binary"
done

windows_targets=(
  "x86_64-pc-windows-msvc:windows-x86_64"
  "aarch64-pc-windows-msvc:windows-aarch64"
)
host="$(rustc -vV | sed -n 's/^host: //p')"

for target_spec in "${windows_targets[@]}"; do
  windows_target="${target_spec%%:*}"
  windows_dir="${target_spec##*:}"
  windows_source=""

  if [[ -n "$windows_bin_dir" ]]; then
    for candidate in "$windows_bin_dir/$windows_target" "$windows_bin_dir/$windows_dir"; do
      if [[ -f "$candidate/bgraph.exe" && -f "$candidate/bgraph-lsp.exe" ]]; then
        windows_source="$candidate"
        break
      fi
    done
  fi

  if [[ -z "$windows_source" ]]; then
    # Drain the producer: grep -q can trigger SIGPIPE, which pipefail treats as failure.
    rustup target list --installed | grep -x "$windows_target" >/dev/null \
      || { echo "==> installing Rust target $windows_target"; rustup target add "$windows_target"; }

    if [[ "$host" == "$windows_target" ]]; then
      echo "==> building bgraph for $windows_target with the native MSVC toolchain"
      cargo build --release --target "$windows_target" --target-dir "$target_dir"
    else
      if ! command -v cargo-xwin >/dev/null 2>&1; then
        if [[ "${BGRAPH_SKIP_XWIN_INSTALL:-0}" == 1 ]]; then
          echo "export.sh: cargo-xwin is missing and automatic installation is disabled." >&2
          echo "             Install it with: cargo install --locked cargo-xwin" >&2
          echo "             Or provide --windows-bin-dir with both target directories." >&2
          exit 1
        fi
        echo "==> installing cargo-xwin (first Windows export only)"
        cargo install --locked cargo-xwin
      fi
      echo "==> building bgraph for $windows_target with cargo-xwin"
      cargo xwin build --release --target "$windows_target" --target-dir "$target_dir"
    fi
    windows_source="$target_dir/$windows_target/release"
  fi

  for binary in bgraph bgraph-lsp; do
    [[ -f "$windows_source/$binary.exe" ]] || {
      echo "export.sh: missing $windows_source/$binary.exe" >&2
      exit 1
    }
    cp "$windows_source/$binary.exe" "$output/$windows_dir/bin/$binary.exe"
  done
done

cp "$repo/LICENSE" "$repo/README.md" "$repo/docs/reference.md" \
  "$output/linux-x86_64/"
cp "$repo/LICENSE" "$repo/README.md" "$repo/docs/reference.md" \
  "$output/windows-x86_64/"
cp "$repo/LICENSE" "$repo/README.md" "$repo/docs/reference.md" \
  "$output/windows-aarch64/"

echo "==> building the Typst module"
rustup target list --installed | grep -x wasm32-unknown-unknown >/dev/null \
  || { echo "export.sh: install wasm32-unknown-unknown with rustup" >&2; exit 1; }
cargo build --lib --release --target wasm32-unknown-unknown \
  --target-dir "$target_dir" --no-default-features --features all-archetypes,json,typst
cp "$repo/typst/typst.toml" "$repo/typst/bgraph.typ" "$repo/typst/README.md" \
  "$output/typst/"
cp "$target_dir/wasm32-unknown-unknown/release/bgraph.wasm" "$output/typst/"
cp "$repo/LICENSE" "$output/typst/"

echo "==> packaging the VS Code extension"
command -v node >/dev/null || { echo "export.sh: node is required for VS Code packaging" >&2; exit 1; }
command -v zip >/dev/null || { echo "export.sh: zip is required for VS Code packaging" >&2; exit 1; }
(cd "$repo/editors/vscode" && ./package.sh)
mv "$repo/editors/vscode/bgraph-$version.vsix" "$output/vscode/bgraph-$version.vsix"
cp "$repo/LICENSE" "$output/vscode/"

echo "==> building the Zed extension"
zed_target="wasm32-wasip2"
rustup target list --installed | grep -x "$zed_target" >/dev/null \
  || { echo "export.sh: install $zed_target with rustup" >&2; exit 1; }
cargo build --manifest-path "$repo/editors/zed/Cargo.toml" --release --target "$zed_target" \
  --target-dir "$zed_target_dir"
cp "$zed_target_dir/$zed_target/release/zed_bgraph.wasm" \
  "$output/zed/extension.wasm"

command -v tree-sitter >/dev/null || {
  echo "export.sh: tree-sitter is required to build the Zed grammar" >&2
  exit 1
}
tree_sitter_wasm="$output/zed/grammar-repo/bgraph.wasm"
(cd "$repo/tree-sitter-bgraph" && tree-sitter build --wasm -o "$tree_sitter_wasm")
cp "$repo/tree-sitter-bgraph/tree-sitter.json" "$output/zed/grammar-repo/"
cp "$repo/tree-sitter-bgraph/src/grammar.json" \
  "$repo/tree-sitter-bgraph/src/node-types.json" \
  "$repo/tree-sitter-bgraph/src/parser.c" "$output/zed/grammar-repo/src/"
cp -r "$repo/editors/zed/languages" "$output/zed/"
cp "$repo/LICENSE" "$repo/editors/zed/README.md" "$output/zed/"

# Zed requires a git revision for a grammar. This repository contains only
# generated parser artefacts, never the grammar source. The installer creates
# the local revision after extraction and rewrites extension.toml accordingly.
git -C "$output/zed/grammar-repo" init -q
git -C "$output/zed/grammar-repo" add .
git -C "$output/zed/grammar-repo" -c user.name=bgraph -c user.email=local@bgraph.invalid \
  commit -q -m "bgraph generated grammar $version"
zed_rev="$(git -C "$output/zed/grammar-repo" rev-parse HEAD)"
cat > "$output/zed/extension.toml" <<EOF
id = "bgraph"
name = "bgraph"
description = "Syntax highlighting and language server for the bgraph diagram DSL"
version = "$version"
schema_version = 1
authors = ["Bunchyearth23"]

[grammars.bgraph]
repository = "file://__BGRAPH_GRAMMAR_REPO__"
rev = "$zed_rev"
path = "."

[language_servers.bgraph-lsp]
name = "bgraph"
languages = ["bgraph"]
EOF

cat > "$output/zed/install.sh" <<'INSTALL'
#!/usr/bin/env bash
set -euo pipefail
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
manifest="$here/extension.toml"
repo="$here/grammar-repo"
sed "s|__BGRAPH_GRAMMAR_REPO__|$repo|" "$manifest" > "$manifest.tmp"
mv "$manifest.tmp" "$manifest"
printf 'Zed export ready. Install it with: zed: install dev extension → %s\n' "$here"
INSTALL
chmod +x "$output/zed/install.sh"

cat > "$output/zed/install.ps1" <<'INSTALL_PS1'
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$manifest = Join-Path $here "extension.toml"
$grammarRepo = Join-Path $here "grammar-repo"
$content = Get-Content -Raw -Path $manifest
$content = $content.Replace("__BGRAPH_GRAMMAR_REPO__", $grammarRepo.Replace('\', '/'))
Set-Content -Path $manifest -Value $content -NoNewline
Write-Host "Zed export ready. Install it with: zed: install dev extension -> $here"
INSTALL_PS1

cat > "$output/README.md" <<EOF
# bgraph $version — local export

This directory contains compiled artifacts only; no project source is included.
You can draw and export diagrams without installing Rust or Node.js.

## Choose your command-line tools

- Linux: linux-x86_64/bin/bgraph and bgraph-lsp
- Windows x86_64: windows-x86_64/bin/bgraph.exe and bgraph-lsp.exe
- Windows ARM64: windows-aarch64/bin/bgraph.exe and bgraph-lsp.exe

Keep the matching pair of executables together. Add their bin folder to your
user PATH and open a new terminal. Check with:

    bgraph --version

You should see bgraph followed by its version. If it is not found, run the
executable from its own folder: ./bgraph on Linux or .\bgraph.exe in Windows
PowerShell. Linux files may need: chmod +x bgraph bgraph-lsp.

## Make a first diagram

Open a terminal in a new working folder where you can keep your files:

    bgraph init free -o architecture.bd
    bgraph check architecture.bd
    bgraph architecture.bd -o architecture.svg
    bgraph watch architecture.bd --open --port 0

Open architecture.bd in a plain text editor. It is the editable source.
architecture.svg is the generated image, viewable in a browser. The last
command opens a live browser preview; leave its terminal running and save
source changes to update the drawing. If the browser does not open, use the
printed URL. Press Ctrl+C in that terminal to stop the preview.

Render again to refresh an exported image. Use architecture.pdf after -o
for PDF output. Run bgraph init to list other available diagram types.

## Optional editor support

- VS Code: choose Install from VSIX in the Extensions menu and select
  vscode/bgraph-$version.vsix. Open a .bd file and press Ctrl+Shift+V
  (Cmd+Shift+V on macOS). The pane updates while typing, without saving.
- Zed: run zed/install.sh (Linux) or zed/install.ps1 (Windows), then use
  zed: install dev extension and select zed/. Open the diagram's containing
  folder as a project, then run task: spawn -> bgraph: Preview.
  See zed/README.md for the shortcut entry and shell-specific setup.
- Typst: copy typst/ to ~/.local/share/typst/packages/local/bgraph/$version/.
  See typst/README.md for usage.

The language-server binary bgraph-lsp supplies editor suggestions and errors.
Configure its absolute path in the editor when it is not on PATH. VS Code's
preview can work without it, but the Zed browser task requires bgraph.
The VS Code pane cannot read include or external from: files; use the browser
watcher for diagrams composed from several files. Read LICENSE for usage terms.
EOF

printf '%s\n' 'bgraph source-free export v1' > "$output/.bgraph-export"
check_output "$destination"
if [[ -e "$destination" ]]; then
  backup="$(mktemp -d "$(dirname "$destination")/.bgraph-export-backup.XXXXXXXX")"
  mv -T -- "$destination" "$backup/previous"
fi
mv -T -- "$output" "$destination"
output=""
[[ -z "$backup" ]] || rm -rf -- "$backup"
backup=""

echo
echo "Export ready: $destination"
find "$destination" -type f -not -path '*/.git/*' -printf '  %P\n' | sort
