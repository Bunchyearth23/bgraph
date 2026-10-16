# Commandes et exports

Les commandes ci-dessous supposent que `architecture.bd` existe.
L'aide de votre binaire reste accessible avec `bgraph --help` ou
`bgraph <commande> --help`.

## Choisir le format

```sh
bgraph architecture.bd -o architecture.svg
bgraph architecture.bd -o architecture.pdf
bgraph architecture.bd -o architecture.json
```

L'extension de sortie choisit le format. Sans `-o`, le fichier de sortie
est `out.svg`.

| Format | Usage |
|---|---|
| SVG | Image vectorielle pour navigateur, documentation ou édition graphique |
| PDF | Document vectoriel pour impression ou partage |
| PNG | Image raster ; nécessite une compilation avec la feature `png` |
| JSON | Géométrie résolue, destinée à l'inspection et aux intégrations |

Le JSON de géométrie expose des détails d'inspection ; ne supposez pas que
ses valeurs numériques ou l'ordre des éléments constituent un contrat d'API.

### PNG et résolution

Après [activation du PNG](installation.md#activer-lexport-png) :

```sh
bgraph architecture.bd -o architecture.png --scale 2
```

`--scale` multiplie les dimensions raster. Il ne sert pas pour les sorties
vectorielles.

### Polices et impression

```sh
bgraph architecture.bd -o architecture.svg --embed-font /chemin/Police.ttf
bgraph architecture.bd -o architecture.pdf --page a4
```

Le premier exemple embarque une police pour éviter qu'une autre machine
ne lui substitue une police différente. Le second répartit un grand dessin
sur des feuilles A4. `--page` accepte aussi `a3`, `letter` ou des dimensions
en points, comme `600x800`.

## Preview pendant l'édition

```sh
bgraph watch architecture.bd
bgraph watch architecture.bd --port 7879
bgraph watch architecture.svg
```

Pour un fichier `.bd`, la preview reconstruit le diagramme à l'enregistrement.
Pour un SVG, elle affiche l'image existante. Ouvrez l'adresse imprimée par
la commande. Le viewer propose zoom, déplacement et, pour le source bgraph,
inspection de la géométrie et du style.

## Construire tout un dossier

```sh
bgraph watch schemas --out images
bgraph watch schemas --out images --once
```

En mode dossier, `watch` est une boucle de génération des SVG.
`--once` construit puis termine, avec un code de sortie non nul si un
fichier échoue. Cette forme convient à une étape de build.

## Vérifier et formater

```sh
bgraph check architecture.bd
bgraph check architecture.bd --ci
bgraph check architecture.bd --json
bgraph fmt architecture.bd
bgraph fmt architecture.bd --check
```

`check --ci` produit des lignes `fichier:ligne:colonne: …`.
`check --json` produit un tableau JSON stable pour l'automatisation.
Une erreur donne un code de sortie non nul ; les avertissements seuls
ne font pas échouer `check`.

`fmt` réécrit le fichier. `fmt --check` signale les différences sans modifier
le source. `fmt --migrate` traite l'ancienne propriété `opacity` ; il ne
convertit pas les anciens placements en ligne vers le langage `layout`.

## Comprendre et comparer

```sh
bgraph explain architecture.bd api
bgraph diff avant.bd apres.bd -o changements.svg
bgraph layout architecture.bd --check
```

`explain` décrit un élément et la provenance de son style.
`diff` représente les ajouts, retraits et déplacements en rapprochant
les identifiants des deux versions.
`layout` peut expliciter les placements implicites dans le fichier ;
sa variante `--check` ne l'écrit pas.

## Générer la référence et le vocabulaire

```sh
bgraph docs -o reference.md
bgraph schema -o schema.json
bgraph completions bash > bgraph.bash
```

La référence et le schéma sont générés à partir du vocabulaire du compilateur.
La commande `completions` demande la feature du même nom, activée par défaut.
