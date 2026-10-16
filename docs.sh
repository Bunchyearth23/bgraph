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
Usage : ./docs.sh [serve|build] [options MkDocs]

  ./docs.sh                         Lance le site sur http://127.0.0.1:8000
  ./docs.sh serve --dev-addr 127.0.0.1:8001
  ./docs.sh build                   Construit le site statique dans site/

L'environnement Python et les dépendances sont préparés automatiquement.
EOF
    exit 0
    ;;
  *)
    echo "Commande inconnue : $docs_command. Utilisez ./docs.sh --help." >&2
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
