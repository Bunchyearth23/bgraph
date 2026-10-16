# Maintenir cette documentation

Le site utilise [MkDocs](https://www.mkdocs.org/) avec son thème intégré
Read the Docs. Python et les dépendances de documentation sont séparés
du compilateur Rust.

## Avec le script docs.sh

Depuis la racine du dépôt, sous Linux ou macOS avec Bash et Python 3 :

```sh
./docs.sh
./docs.sh build
```

Le script crée l'environnement Python si nécessaire et installe les
dépendances déclarées. Sans argument, il lance la documentation sur
**<http://127.0.0.1:8000>**. Arrêtez le serveur avec `Ctrl+C` avant de
lancer la construction statique, ou utilisez un autre terminal.
`build` construit le dossier `site/` en mode strict.

Pour changer de port ou consulter l'aide :

```sh
./docs.sh serve --dev-addr 127.0.0.1:8001
./docs.sh --help
```

Les étapes ci-dessous donnent l'équivalent manuel, notamment pour PowerShell.

## Préparer l'environnement

Depuis la racine du dépôt, avec Python 3 :

```sh
python3 -m venv .tools/docs-venv
.tools/docs-venv/bin/python -m pip install -r requirements-docs.txt
```

Sous PowerShell :

```powershell
py -m venv .tools/docs-venv
.tools/docs-venv/Scripts/python.exe -m pip install -r requirements-docs.txt
```

## Lire et modifier le site

```sh
.tools/docs-venv/bin/python -m mkdocs serve
```

Ouvrez **<http://127.0.0.1:8000>**. Les modifications des pages,
des manuels réutilisés et de la galerie déclenchent une reconstruction.
Après une modification du hook Python, redémarrez `mkdocs serve`.

Sous PowerShell, remplacez le chemin du Python par
`.tools/docs-venv/Scripts/python.exe`.

## Construire le site statique

```sh
.tools/docs-venv/bin/python -m mkdocs build --strict
```

Le résultat est écrit dans `site/`. Pour le consulter par HTTP :

```sh
python3 -m http.server 8000 --directory site
```

La recherche utilise un index local et doit être consultée par HTTP.
Le dossier généré est ignoré par Git. La construction ne publie rien.

## Où modifier le contenu ?

| Contenu | Source |
|---|---|
| Guides français | `docs/guide/*.md` |
| Navigation et options | `mkdocs.yml` |
| Présentation | `docs/guide/stylesheets/extra.css` |
| Tour complet du langage | `docs/language.md` |
| Contrat d'API | `docs/API.md` |
| Référence exhaustive | `src/docgen.rs` et tables de `src/props.rs` |
| Exemples de la galerie | `src/examples.rs`, puis galerie générée `examples/` |

**Ne modifiez pas `docs/reference.md` à la main.** Pour le régénérer
après avoir modifié ses sources :

```sh
cargo run --locked --bin bgraph -- docs -o docs/reference.md
```

Cette commande recompile le générateur avant de l'exécuter.

Le hook `docs/site_hooks.py` expose les trois manuels existants au site
sans en maintenir une deuxième copie. Il assemble aussi la galerie à partir
des paires `.bd` / `.svg` existantes. Les vérifications de galerie du projet
assurent leur accord avec le compilateur ; MkDocs n'exécute pas le rendu Rust.

Le site prend uniquement les pages de `docs/guide/`, les trois manuels et
les exemples. L'index opérationnel et les journaux de travail ne font pas
partie de son contenu.
