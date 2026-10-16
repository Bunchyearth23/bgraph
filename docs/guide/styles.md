# Styles et couleurs

## Créer un style réutilisable

```bgraph
style service {
  fill: #dbeafe
  stroke: #2563eb 1.5
  radius: 8
  font: 14 #0f172a
}

style store extends service {
  shape: cylinder
  fill: #d1fae5
}

node api "API" .service
node db "Base" .store { stroke: #059669 2 }
layout { place db right-of api gap 60 }
api -> db
```

`fill` est le fond. `stroke` associe une couleur et, facultativement,
une épaisseur. `font` associe une taille et une couleur de texte.
`extends` hérite d'un style ; le bloc placé sur l'élément a la priorité.

## Lire la cascade

Du plus général au plus spécifique pour un élément :

1. Valeurs par défaut et thème choisi en ligne de commande.
2. `style node`, appliqué à tous les éléments.
3. Style du mot de déclaration, par exemple `style slice` ou `style task`.
4. Classes `.classe`, dans leur ordre d'application.
5. Propriétés écrites directement sur l'élément, dans `{ … }`.

`style edge` joue le rôle de base pour les liens. Un style nommé arbitrairement,
comme `style service`, s'applique là où sa classe `.service` est utilisée.

## Régler tous les éléments ou les liens

```bgraph
style node { fill: #f1f5f9 stroke: #334155 1 font: 14 #0f172a }
style edge { stroke: #64748b 1.5 router: ortho }

node a "Départ"
node b "Arrivée"
layout { place b right-of a gap 60 }
a -> b "requête"
```

Les formes usuelles sont `rect`, `stadium`, `ellipse`, `diamond`,
`hexagon` et `cylinder`. Le routage `ortho` cherche un passage à angles droits
autour des boîtes ; `straight` trace un lien direct.

## Marges extérieures

`padding` réserve de l'espace **dans** la boîte. `margin` réserve de l'espace
**autour** de la boîte ou du cadre, avec l'ordre des côtés de CSS :

```text
margin: 16
margin: 8 16
margin: 8 16 12
margin: 8 16 12 20
margin-top: 24
```

Ces formes correspondent à : tous les côtés ; vertical / horizontal ; haut /
horizontal / bas ; haut / droite / bas / gauche. Les côtés individuels
(`margin-top`, `margin-right`, `margin-bottom`, `margin-left`) se combinent
avec les styles, `extends` et les propriétés écrites sur l'élément.

```bgraph
style node { margin: 8 16 }
node api "API"
node db "Base"
layout {
  place api at (0, 0)
  place db right-of api gap 40
}
api -> db
```

Ici, l'espace entre les bords vaut **72** : 16 de marge à droite de `api`,
40 de `gap`, puis 16 à gauche de `db`. Les dimensions visibles des boîtes
ne changent pas. Un groupe englobe aussi les marges de ses enfants.
Les coordonnées absolues et les alignements explicites restent attachés aux
bords visibles ; une marge ne déplace pas un `at` imposé.

Pour une séquence :

```text
style participant { margin-bottom: 12 }
style group { margin: 16 8 }
```

Le calcul réserve la place des titres de cadres, des marges et des messages
avant de dessiner les lignes de vie. Cela évite qu'un premier cadre touche
les participants et garde les cadres imbriqués séparés.

Les marges sont disponibles dans les neuf genres à placement déclaré et
dans `sequence`. Les autres genres les refusent explicitement. Les valeurs
sont des distances finies positives ou nulles ; `auto`, les valeurs négatives
et la fusion des marges CSS ne sont pas pris en charge. Les suffixes d'unité
restent indicatifs : `50%` vaut 50 unités, sans calcul de pourcentage relatif.

## Appliquer un thème

```sh
bgraph architecture.bd --theme light -o clair.svg
bgraph architecture.bd --theme dark -o sombre.svg
bgraph architecture.bd --theme print -o impression.pdf
```

Un thème passe sous les styles du fichier. Si votre source fixe déjà le fond
et toutes les couleurs, ces valeurs continuent à gagner. Pour laisser le thème
choisir le fond, omettez `background` dans votre source.

Sans thème ni fond déclaré, le fond est blanc. Pour exporter un fond transparent :

```text
graph { background: none }
```

Pour un fond partiellement transparent, ajoutez l'alpha à la couleur :

```bgraph
graph { background: #ffffff80 }
```

Le format est `#RRGGBBAA` : les deux derniers chiffres contrôlent l'opacité
du fond, indépendamment des formes et des textes.

| Alpha | Opacité approximative |
|---|---|
| `00` | 0 % — transparent |
| `40` | 25 % |
| `80` | 50 % |
| `bf` | 75 % |
| `ff` | 100 % — opaque |

Cela fonctionne dans les exports SVG, PDF et PNG. La preview affiche un damier
derrière le fond pour rendre sa transparence visible. Le format court `#RGBA`
est également accepté, par exemple `#fff8` (environ 53 % d'opacité).

## Partager une palette

Placez les déclarations `style` dans un fichier `maison.bds`, puis ajoutez
`include "maison.bds"` dans vos diagrammes. Un `.bds` contient uniquement
des styles. Voir [composer plusieurs fichiers](composition.md).

Pour toutes les propriétés et leurs valeurs acceptées, utilisez la
[référence générée](reference.md#property-reference). `bgraph explain fichier.bd id`
permet de retrouver quelle couche a fourni une valeur.
