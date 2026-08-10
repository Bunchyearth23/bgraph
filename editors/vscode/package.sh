#!/usr/bin/env bash
# Build a .vsix for the bgraph VS Code extension.
#
# A VSIX is a zip with three things in it: the extension under `extension/`,
# an OPC content-types part, and a manifest VS Code reads to decide what it has
# been handed. So this builds one with `zip` rather than pulling `@vscode/vsce`
# off a registry — the same call this project makes for its SVG writer, its
# HTTP server and its PDF writer.
#
#   ./package.sh              # with the language client, if node_modules is there
#   ./package.sh --no-server  # highlighting only, no npm needed
#
# Install the result with:
#   code --install-extension bgraph-<version>.vsix

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$here"

with_server=1
[[ "${1:-}" == "--no-server" ]] && with_server=0

command -v zip >/dev/null || { echo "package.sh: needs \`zip\` on PATH" >&2; exit 1; }
command -v node >/dev/null || { echo "package.sh: needs \`node\` on PATH" >&2; exit 1; }

# Everything the manifest says comes out of package.json, so the two cannot
# disagree — a version in two files is a version that is wrong in one of them.
read -r name version publisher engine description < <(node -e '
  const p = require("./package.json");
  // The trailing newline is load-bearing: `read` returns non-zero at EOF
  // without one, and `set -e` would take the script down with it.
  // The engine range comes from here too: hardcoding it in the manifest
  // would be a second place for it to be wrong.
  process.stdout.write([p.name, p.version, p.publisher, p.engines.vscode, p.description].join(" ") + "\n");
')
vsix="$name-$version.vsix"

if (( with_server )); then
  if [[ ! -d node_modules/vscode-languageclient ]]; then
    echo "package.sh: installing the language client (npm install --omit=dev)…"
    npm install --omit=dev --no-audit --no-fund
  fi
  if [[ ! -d node_modules/vscode-languageclient ]]; then
    echo "package.sh: vscode-languageclient is still missing." >&2
    echo "            Run \`npm install\` here, or build with --no-server for" >&2
    echo "            a highlighting-only package." >&2
    exit 1
  fi
fi

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
mkdir -p "$staging/extension"

cp -r package.json language-configuration.json README.md src syntaxes "$staging/extension/"
(( with_server )) && cp -r node_modules "$staging/extension/"
cp ../../LICENSE "$staging/extension/LICENSE"

# The OPC part. `.json` has to be declared or VS Code will not read the grammar
# or the manifest's own companion files.
cat > "$staging/[Content_Types].xml" <<'XML'
<?xml version="1.0" encoding="utf-8"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="json" ContentType="application/json" />
  <Default Extension="js" ContentType="application/javascript" />
  <Default Extension="md" ContentType="text/markdown" />
  <Default Extension="vsixmanifest" ContentType="text/xml" />
  <Default Extension="LICENSE" ContentType="text/plain" />
</Types>
XML

# XML is not a place to paste a description from a file, so it goes through an
# escaper rather than straight in.
escape() { node -e 'process.stdout.write(process.argv[1].replace(/[&<>"]/g, c => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"'"'"'"'"':"&quot;"}[c])))' "$1"; }

cat > "$staging/extension.vsixmanifest" <<XML
<?xml version="1.0" encoding="utf-8"?>
<PackageManifest Version="2.0.0" xmlns="http://schemas.microsoft.com/developer/vsx-schema/2011">
  <Metadata>
    <Identity Language="en-US" Id="$(escape "$name")" Version="$version" Publisher="$(escape "$publisher")" />
    <DisplayName>bgraph</DisplayName>
    <Description xml:space="preserve">$(escape "$description")</Description>
    <Tags>bgraph,diagram,dsl</Tags>
    <Categories>Programming Languages</Categories>
    <GalleryFlags>Public</GalleryFlags>
    <Properties>
      <Property Id="Microsoft.VisualStudio.Code.Engine" Value="$(escape "$engine")" />
      <Property Id="Microsoft.VisualStudio.Code.ExtensionDependencies" Value="" />
    </Properties>
  </Metadata>
  <Installation>
    <InstallationTarget Id="Microsoft.VisualStudio.Code" />
  </Installation>
  <Dependencies />
  <Assets>
    <Asset Type="Microsoft.VisualStudio.Code.Manifest" Path="extension/package.json" Addressable="true" />
    <Asset Type="Microsoft.VisualStudio.Services.Content.Details" Path="extension/README.md" Addressable="true" />
  </Assets>
</PackageManifest>
XML

rm -f "$here/$vsix"
(cd "$staging" && zip -q -r -X "$here/$vsix" '[Content_Types].xml' extension.vsixmanifest extension)

echo "$vsix — $(du -h "$here/$vsix" | cut -f1)$( (( with_server )) || echo ', highlighting only')"
echo "install with: code --install-extension $here/$vsix"
