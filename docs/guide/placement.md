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

## Éviter le chevauchement des libellés

En BPMN, les événements et les losanges portent leur texte à l'extérieur.
Le placement cherche une position libre près de la forme, en évitant les
flux, les autres formes, les libellés et les titres de groupes. Un flux
vertical sous un losange fait ainsi passer sa légende à côté du trait.

Les placements relatifs réservent aussi l'empreinte de cette légende, en
plus de `gap` et des marges. Les cadres englobent le texte à sa position
retenue. Les coordonnées `at` et les alignements explicites continuent à
désigner les bords visibles des formes.

Les libellés de liens cherchent une place libre près du milieu de leur
segment, aussi bien le long du trait que de chaque côté. Ils prennent en
compte les annotations et les titres des groupes dans tous les genres
qui utilisent le routeur commun.

Pour choisir une position précise :

```bgraph
gateway decision "Valider ?" exclusive { label-offset: 24 0 }
```

Pour choisir simplement le côté d'une légende BPMN :

```bgraph
event recu "Message reçu" start message { label-position: right }
```

`label-position` accepte `top`, `right`, `bottom` ou `left`, uniquement sur
les événements et les gateways BPMN. Ce côté reste fixe ; `label-offset`
peut ajouter un ajustement. Sans ces propriétés, le placement reste automatique.

Un `label-offset` explicite fixe le libellé BPMN ou le texte du lien. Si
l'espace disponible reste insuffisant, un avertissement signale la collision :
augmentez le `gap`, ajoutez des marges ou ajustez ce décalage.

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
`equal-width` et `equal-height` acceptent également des groupes, et peuvent
mélanger nœuds et groupes. Leur taille doit toujours contenir leurs éléments.
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

Vous pouvez imposer `width`, `height` ou `size` à un groupe. Par exemple,
`group pool "Pool" { width: 600 … }` réserve réellement 600 unités. Un
contenu trop grand provoque une erreur de contraintes.

En BPMN, un groupe dont les membres directs sont tous des groupes forme une
pool de lanes. Ces lanes partagent sa largeur intérieure, après déduction
du `padding` de la pool et de leurs marges. Le parent est dessiné avant ses
enfants. La couche commune `style node` s'applique aussi aux groupes ;
utilisez `style group { fill: none }` pour des cadres transparents, et
`style task { size: 176 74 }` pour dimensionner seulement les tâches.

## Flux BPMN et ports

Les flux de séquence sortant d'un événement de bordure s'éloignent d'abord
de la tâche hôte. Les ports automatiques d'une tâche évitent ses événements
attachés ; les associations et messages ne décalent pas ses ports de séquence.
Les avertissements sur les débuts et fins de processus comptent uniquement
les flux de séquence, y compris `default` et `conditional`.

Le routeur `ortho` évite les boîtes, les légendes extérieures et les titres de
groupes, même au-delà de 60 obstacles. Les ports et les points `via` font
partie de sa recherche. Une route impossible reste visible et s'accompagne
d'un avertissement ; un `via` impossible ne devient pas une diagonale.

Une largeur fixe fait passer les étiquettes de nœuds à la ligne. Si une hauteur
fixe ou une largeur trop petite empêche encore le texte de tenir, un
avertissement indique l'élément concerné.

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
