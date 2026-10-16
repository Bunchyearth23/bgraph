# Zed et VS Code

Le serveur **`bgraph-lsp`** est commun aux éditeurs. Il apporte diagnostics,
complétion contextuelle, survol, navigation vers les déclarations et plan du
document. Installez-le avec le CLI et rendez-le accessible dans le `PATH`.

## Saisir plus vite

- Dans `layout`, la complétion propose les six instructions de placement,
  puis les identifiants disponibles dans le groupe courant.
- Les snippets proposent des choix d'identifiants et des coordonnées
  numériques, comme `at (0, 0)`. Utilisez `Tab` pour parcourir les champs.
- Après un placement relatif, complétez l'ancre, `gap` et `align`.
  Les axes proposés dépendent de la direction du placement.
- L'aide aux paramètres accompagne les coordonnées, les rangées, les
  colonnes et les alignements, même pendant une instruction incomplète.
- La commande de formatage du document de l'éditeur utilise le formateur
  bgraph. Un source qui ne se parse pas reste inchangé.

Les suggestions restent silencieuses dans les chaînes et les commentaires.
Après une mise à jour de `bgraph-lsp`, redémarrez le serveur de langage dans
l'éditeur pour charger le nouvel exécutable et ses capacités.

## Zed

### Depuis le dépôt

À la racine du dépôt :

```sh
./editors/zed/install.sh
```

Le script installe le serveur et prépare la grammaire. Il peut créer un
commit des fichiers de grammaire régénérés et actualise la révision épinglée
dans le manifeste de l'extension.

Dans Zed, ouvrez la palette des commandes (`Ctrl+Shift+P` ou `Cmd+Shift+P`),
lancez **zed: install dev extension** et choisissez le dossier `editors/zed`.
Ouvrez ensuite un fichier `.bd`.

Pour un dossier d'export reçu séparément, utilisez l'installateur Zed fourni
avec ce paquet (`install.sh` ou `install.ps1`), puis les instructions qu'il
affiche.

### Preview et Ctrl+Shift+V

L'extension actuelle apporte le langage et le LSP. Elle n'ajoute pas de
volet de preview bgraph à Zed, ni de rendu des blocs bgraph dans son
aperçu Markdown. **`Ctrl+Shift+V` n'ouvre donc pas une preview `.bd` intégrée.**

Le flux disponible consiste à garder la preview navigateur à côté de Zed :

```sh
bgraph watch architecture.bd
```

Ouvrez <http://127.0.0.1:7878> et enregistrez vos modifications dans Zed.
L'API d'extension de Zed n'expose actuellement pas le volet personnalisé
nécessaire à une preview native de ce type.

### Le serveur est introuvable

Ajoutez son chemin dans les réglages Zed :

```json
{
  "lsp": {
    "bgraph-lsp": {
      "binary": { "path": "/chemin/vers/bgraph-lsp" }
    }
  }
}
```

Sous Windows, utilisez le chemin vers `bgraph-lsp.exe`.
`zed --foreground` permet de consulter les erreurs de chargement.

## VS Code

### Préparer et installer le VSIX

Depuis la racine du dépôt, avec Node/npm, `zip` et la cible Rust wasm :

```sh
rustup target add wasm32-unknown-unknown
./editors/vscode/package.sh
code --install-extension editors/vscode/bgraph-0.1.0.vsix
```

Adaptez le nom du VSIX à la version produite par le script. Si vous avez
reçu un VSIX déjà construit, installez directement ce fichier.
Le réglage `bgraph.server.path` permet de préciser le chemin du LSP
lorsqu'il n'est pas sur le `PATH`.

### Dessiner dans une page Markdown

Dans un fichier `.md`, ajoutez un bloc balisé `bgraph` :

````markdown
```bgraph
node a "Départ"
node b "Arrivée"
layout { place b right-of a gap 48 }
a -> b
```
````

Ouvrez l'aperçu Markdown de VS Code, par exemple avec `Ctrl+Shift+V`.
L'extension utilise son module wasm embarqué pour dessiner le diagramme.
Cette preview concerne les blocs d'un fichier Markdown ; pour travailler
directement dans un `.bd`, utilisez `bgraph watch`.

Les blocs doivent être autonomes : le rendu wasm ne résout pas les
`include` et les fichiers `from:`. Les diagnostics LSP d'un vrai fichier
`.bd`, eux, savent résoudre ses inclusions.

## Autres éditeurs

`bgraph-lsp` parle LSP sur l'entrée et la sortie standard. Configurez
votre client LSP pour lancer cette commande sur les fichiers `.bd`.
La grammaire tree-sitter du dépôt et les requêtes de `editors/zed/languages/bgraph`
fournissent la coloration pour les clients compatibles.

Les configurations détaillées Neovim, Helix et Emacs sont conservées dans
[le guide des éditeurs du dépôt](https://github.com/Bunchyearth23/bgraph/blob/main/editors/README.md).
