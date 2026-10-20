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

Le script installe le CLI et le serveur, puis prépare la grammaire. Il peut
créer un commit des fichiers de grammaire régénérés et actualise la révision épinglée
dans le manifeste de l'extension.

Dans Zed, ouvrez la palette des commandes (`Ctrl+Shift+P` ou `Cmd+Shift+P`),
lancez **zed: install dev extension** et choisissez le dossier `editors/zed`.
Ouvrez ensuite un fichier `.bd`.

Pour un dossier d'export reçu séparément, utilisez l'installateur Zed fourni
avec ce paquet (`install.sh` ou `install.ps1`), puis les instructions qu'il
affiche.

### Preview et Ctrl+Shift+V

L'extension fournit la tâche **bgraph: Preview** pour le fichier `.bd`
courant. Ouvrez dans Zed le dossier qui contient ce fichier, puis le `.bd` :
sur l'hôte vérifié, une fenêtre contenant seulement un fichier isolé ne
propose pas la tâche de langue. La tâche sauvegarde ce fichier, démarre la
preview sur un port libre et ouvre le navigateur. Lancez-la avec **task: spawn**,
puis sélectionnez **bgraph: Preview**. Pour lui associer `Ctrl+Shift+V`, ajoutez l'entrée de
`editors/zed/keymap.json` à votre keymap Zed (**zed: open keymap**).
Le raccourci est limité aux éditeurs bgraph.

La tâche fournie utilise la syntaxe des shells Linux/macOS. Pour PowerShell,
le README de l'extension Zed donne l'argument à adapter dans une tâche de
projet ; cette variante n'a pas été testée dans un hôte Windows.

La commande équivalente est :

```sh
bgraph watch architecture.bd --open --port 0
```

L'adresse choisie est affichée dans le terminal de la tâche. Les sauvegardes
actualisent le dessin, y compris celles des fichiers inclus ; une inclusion
non sauvegardée conserve sa version sur disque. Arrêtez la tâche depuis son
terminal pour fermer le serveur. Relancer une tâche déjà active ne crée pas
une deuxième instance concurrente.

La preview s'affiche dans le navigateur. L'API d'extension de Zed n'expose
actuellement pas le volet personnalisé nécessaire à une preview native ;
l'aperçu Markdown de Zed n'est pas étendu.

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
code --install-extension editors/vscode/bgraph-1.0.0.vsix --force
```

Adaptez le nom du VSIX à la version produite par le script. Si vous avez
reçu un VSIX déjà construit, installez directement ce fichier.
Le réglage `bgraph.server.path` permet de préciser le chemin du LSP
lorsqu'il n'est pas sur le `PATH`.

### Preview directe d'un fichier .bd

Dans un éditeur bgraph, pressez **`Ctrl+Shift+V`** (`Cmd+Shift+V` sur macOS),
ou lancez **bgraph: Open Preview** depuis la palette. Un volet s'ouvre à côté
du source et s'actualise pendant la saisie, sans enregistrer le fichier.
Les buffers bgraph sans nom sont aussi acceptés. Rappeler la commande sur
le même document réutilise son volet ; chaque document garde sa propre preview.
Les erreurs de syntaxe ou de rendu s'affichent dans le volet.

Le renderer wasm est fourni dans le VSIX, sans CLI à installer pour cette
preview. Le LSP reste indépendant. Les sources doivent être autonomes :
`include` et les fichiers `from:` ne sont pas résolus par ce renderer.
Pour une preview multifichier, utilisez `bgraph watch fichier.bd --open`.

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
Le raccourci conserve son comportement Markdown dans un fichier `.md` ;
il n'est associé à la commande bgraph que dans un éditeur bgraph.

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
