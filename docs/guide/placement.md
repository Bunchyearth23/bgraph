# Placer les éléments

## Déclarer, puis placer

Les déclarations décrivent ce qui existe. Un bloc `layout` décrit les
positions. Il est accepté dans les types `free`, `class`, `block`, `er`,
`state`, `harness`, `bpmn`, `network` et `requirements`.

```bgraph
node ui "Interface"
node api "API"
node db "Stockage"

layout {
  place ui at (0, 0)
  place api right-of ui gap 48 align top
  place db below api gap 32
}

ui -> api
api -> db
```

`at (x, y)` désigne le coin supérieur gauche de l'élément. Les coordonnées
augmentent vers la droite et vers le bas. Les distances sont exprimées dans
les unités du dessin ; le [mode grille](language.md#a-grid) est décrit
dans le tour complet du langage.

| Relation | Effet |
|---|---|
| `right-of api` | À droite de `api` |
| `left-of api` | À gauche |
| `below api` | En dessous |
| `above api` | Au-dessus |
| `gap 40` | Espace entre les bords des éléments |
| `align top` | Alignement transversal pour un placement horizontal |
| `align left` | Alignement transversal pour un placement vertical |

Sans précision, l'alignement transversal est centré. Un élément sans
placement suit le flux implicite du fichier (`graph { flow: right gap: 40 }`).
Déclarez son ancrage quand sa position a un sens particulier.

## Faire une rangée ou une colonne

```bgraph
node api "API"
node cache "Cache"
node db "Base"
node audit "Journal"

layout {
  place api at (0, 0)
  row api cache db gap 32
  column db audit gap 24
}
```

Le premier élément sert d'ancrage. Les suivants reçoivent leur placement
de la rangée ou de la colonne : ne leur ajoutez pas un second `place`.

## Harmoniser les dimensions

Ajoutez les relations dans le même bloc `layout` :

```text
equal-width api cache db
equal-height api cache db
align top api cache db
```

Les tailles égales préservent la place nécessaire au plus grand libellé.
Pour un alignement, `top`, `bottom` et `center-y` concernent l'axe vertical ;
`left`, `right` et `center-x` concernent l'axe horizontal.

## Regrouper une partie du schéma

```bgraph
group backend "Backend" {
  padding: 24
  node api "API"
  node worker "Worker"
  layout {
    place api at (0, 0)
    place worker below api gap 32
  }
}

node db "Base"
layout {
  place backend at (0, 0)
  place db right-of backend gap 60
}

api -> db
```

Le groupe s'ajuste à son contenu. Le déplacer déplace son sous-arbre.
Un bloc `layout` local adresse les membres directs de son groupe.

## Comprendre un placement inattendu

```sh
bgraph explain architecture.bd api
```

La commande explique la géométrie et l'origine du style de l'élément.
La preview expose aussi ces informations dans son inspecteur.
Un chevauchement est signalé ; bgraph ne décide pas à votre place dans
quelle direction déplacer les boîtes.

!!! note "Les graphiques calculent leur géométrie"
    Les types comme `pie`, `packet` ou `gantt` refusent les blocs `layout`.
    Utilisez leurs données et leurs réglages de genre pour agir sur le rendu.
    Voir [choisir un type](graphiques.md).
