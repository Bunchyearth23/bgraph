# Résoudre un problème

## `bgraph` ou `bgraph-lsp` est introuvable

Vérifiez que le dossier des exécutables est dans votre `PATH`, puis ouvrez
un nouveau terminal. En compilation locale, essayez directement
`./target/release/bgraph --version` ou
`.\target\release\bgraph.exe --version` sous PowerShell.

Si seul l'éditeur échoue, indiquez le chemin absolu du serveur dans ses
[réglages](editeurs.md).

## Le fichier ne se rend pas

```sh
bgraph check architecture.bd
```

Commencez par la première erreur indiquée. Vérifiez notamment :

- les guillemets autour des libellés ;
- les commentaires en `//` ;
- les identifiants des nœuds et les classes `.style` ;
- une propriété placée au bon niveau (`graph`, élément, lien ou genre) ;
- les accolades et les séparations entre les instructions.

Les membres d'une classe, comme `- id: Uuid`, doivent être sur leur propre
ligne ; ne placez pas l'accolade de fermeture sur la même ligne.

## Mon ancien placement n'est plus accepté

Utilisez les déclarations et le bloc `layout` séparément :

```bgraph
node a "A"
node b "B"
layout {
  place a at (0, 0)
  place b right-of a gap 40
}
```

Les placements directement ajoutés à une déclaration `node` ne sont plus
la syntaxe courante. `fmt --migrate` ne réalise pas cette conversion.

## `layout` est refusé dans mon graphique

Les genres tels que `pie`, `packet`, `bar` ou `gantt` calculent leurs positions
à partir des données. Retirez le bloc et utilisez les réglages du genre.
La [table des géométries](graphiques.md#deux-facons-de-controler-la-geometrie)
indique ceux qui acceptent les placements déclarés.

## Des boîtes se chevauchent ou des relations se contredisent

bgraph signale les collisions et les contraintes incompatibles.
Il ne choisit pas de déplacer vos éléments pour masquer le problème.
Augmentez un `gap`, changez un ancrage ou retirez la relation contradictoire.

Utilisez `bgraph explain architecture.bd api` et l'inspecteur de la preview
pour examiner les dimensions et les relations qui concernent l'élément.

## Le thème ne change pas certaines couleurs

Les couleurs écrites dans le fichier prennent la priorité sur `--theme`.
Retirez les valeurs que vous voulez déléguer au thème. Pour comprendre
la priorité de chaque réglage, consultez [la cascade](styles.md#lire-la-cascade).

## Le PNG n'est pas disponible ou le texte change de police

Le PNG demande une [compilation avec `png`](installation.md#activer-lexport-png).
Le rendu dépend des polices disponibles ; utilisez une police installée
ou `--embed-font /chemin/Police.ttf` pour fournir le fichier voulu.
Pour les destinataires sans cette police, un SVG avec police embarquée
permet aussi de préserver le texte.

## La preview ne se charge pas

Gardez `bgraph watch architecture.bd` actif dans son terminal.
Consultez l'adresse et les diagnostics qu'il affiche. Si le port est déjà
occupé, choisissez-en un autre avec `--port 7879` et ouvrez cette adresse.

Pour un template, fournissez ses valeurs avec
`bgraph watch trafic.bd --data trafic.json`.

## Zed n'affiche pas de preview avec Ctrl+Shift+V

La preview bgraph intégrée n'est pas présente dans l'extension actuelle.
Utilisez `bgraph watch` dans le navigateur ; voir [la section Zed](editeurs.md#zed).

## Un `include` fonctionne en CLI, mais pas dans le navigateur ou Typst

Ces rendus wasm prennent du texte autonome. Ils ne peuvent pas résoudre
un chemin sur disque. Rendez le fichier avec le CLI, puis insérez le SVG
produit dans la page ou le document.
