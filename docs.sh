#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

docs_command="${1:-serve}"
if (( $# )); then
  shift
fi

case "$docs_command" in
  serve|build) ;;
  -h|--help|help)
    cat <<'EOF'
Usage: ./docs.sh [serve|build] [MkDocs options]

  ./docs.sh                         Serve the site at http://127.0.0.1:8000
  ./docs.sh serve --dev-addr 127.0.0.1:8001
  ./docs.sh build                   Build the static site in site/

The Python environment and dependencies are prepared automatically.
EOF
    exit 0
    ;;
  *)
    echo "Unknown command: $docs_command. Use ./docs.sh --help." >&2
    exit 2
    ;;
esac

docs_python=".tools/docs-venv/bin/python"
if [[ ! -x "$docs_python" ]]; then
  python3 -m venv .tools/docs-venv
fi
"$docs_python" -m pip install --disable-pip-version-check --quiet -r requirements-docs.txt

if [[ "$docs_command" == build ]]; then
  set -- --strict "$@"
fi
exec "$docs_python" -m mkdocs "$docs_command" "$@"
