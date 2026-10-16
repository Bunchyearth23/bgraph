# Choisir un type de diagramme

Un fichier choisit son genre une seule fois, au début, avec `type`.
Sans cette déclaration, il utilise `free`.

```sh
bgraph init
bgraph init packet -o entete.bd
```

La première commande liste les genres disponibles dans votre binaire.
La seconde écrit un exemple complet. La [galerie](galerie.md) montre
les 30 genres de la compilation complète, avec leurs sources téléchargeables.

## Quel genre pour quel besoin ?

| Besoin | Genres |
|---|---|
| Architecture, services, réseau | `free`, `block`, `network` |
| Modèles logiciels et données | `class`, `er`, `state`, `requirements` |
| Processus et interactions | `bpmn`, `sequence`, `journey`, `kanban` |
| Parts et comparaisons | `pie`, `bar`, `xy`, `radar`, `quadrant`, `treemap` |
| Flux et intersections | `sankey`, `venn` |
| Dates et historique | `gantt`, `timeline`, `gitgraph` |
| Hiérarchies | `orgchart`, `mindmap`, `treeview` |
| Électronique et infrastructure | `packet`, `bytefield`, `wave`, `harness`, `rack` |

## Un camembert en quelques lignes

```bgraph
type pie {
  title: "Sources de trafic"
  donut: 0.55
  labels: percent
}

slice organic "Recherche" 4820
slice direct "Accès direct" 2410
slice referral "Sites référents" 1180
```

Le nombre après le libellé est la valeur de la part. Le genre calcule la
géométrie : n'ajoutez pas de `layout`. Les couleurs sont choisies dans la
palette par défaut, sauf si vous écrivez un `fill`.
Au-delà de huit catégories, regroupez la fin de la distribution ; consultez
le réglage `top` dans la [référence de pie](reference.md#type-pie).

## Un paquet binaire

```bgraph
type packet { title: "Petit en-tête" width: 16 }

field version "Version" 0 3
field flags "Drapeaux" 4 7
field length "Longueur" 8 15
```

Les bornes de bits sont **inclusives** : `0 3` représente quatre bits.
Les champs sont organisés par le genre, selon la largeur déclarée.

## Deux façons de contrôler la géométrie

| Géométrie | Genres | Votre contrôle |
|---|---|---|
| Déclarée | `free`, `class`, `block`, `er`, `state`, `harness`, `bpmn`, `network`, `requirements` | Positions et relations dans `layout` |
| Calculée par le genre | Les 21 autres | Données, ordre et réglages du genre |

Un genre à géométrie calculée refuse les blocs `layout` et les `note`.
Les styles restent communs aux deux familles, mais une propriété sans effet
pour un genre produit un diagnostic.

Chaque genre dispose de son vocabulaire : `slice`, `field`, `task`, `host`…
Le mot `node` reste un synonyme universel. Pour commencer, gardez les mots et
la forme des données de l'exemple fourni par `bgraph init`.
