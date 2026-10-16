#!/usr/bin/env bash
# Run the actual VS Code extension host in an isolated profile and X display.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/../.."
for tool in cargo code xvfb-run npm; do
  command -v "$tool" >/dev/null || { echo "VS Code gate: missing $tool" >&2; exit 1; }
done
gate_host_dir="$(mktemp -d)"
trap 'rm -rf "$gate_host_dir"' EXIT
cargo build --locked --bins --target-dir "$PWD/target"
cp -R editors/vscode "$gate_host_dir/extension"
if [[ ! -d "$gate_host_dir/extension/node_modules/vscode-languageclient" ]]; then
  (cd "$gate_host_dir/extension" && npm install --omit=dev --no-audit --no-fund)
fi
export BGRAPH_RUNTIME_SERVER="$PWD/target/debug/bgraph-lsp"
export BGRAPH_RUNTIME_CLI="$PWD/target/debug/bgraph"
export BGRAPH_RUNTIME_FIXTURES="$PWD/tests/runtime"
export BGRAPH_RUNTIME_RESULT="$gate_host_dir/result"
env -u VSCODE_IPC_HOOK_CLI -u VSCODE_PID -u ELECTRON_RUN_AS_NODE \
  XDG_CONFIG_HOME="$gate_host_dir/config" \
  timeout 60s xvfb-run -a code --wait --ozone-platform=x11 --disable-gpu \
  --disable-workspace-trust --skip-welcome --skip-release-notes \
  --user-data-dir "$gate_host_dir/user" --extensions-dir "$gate_host_dir/extensions" \
  --extensionDevelopmentPath "$gate_host_dir/extension" \
  --extensionTestsPath "$PWD/tests/runtime/vscode-host.cjs"
[[ -f "$BGRAPH_RUNTIME_RESULT" ]] \
  || { echo "VS Code exited without running the extension-host assertions" >&2; exit 1; }
cat "$BGRAPH_RUNTIME_RESULT"
grep -qx 'VS Code host: 7 fixtures passed' "$BGRAPH_RUNTIME_RESULT"
