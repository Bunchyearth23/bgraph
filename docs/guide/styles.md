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

## Partager une palette

Placez les déclarations `style` dans un fichier `maison.bds`, puis ajoutez
`include "maison.bds"` dans vos diagrammes. Un `.bds` contient uniquement
des styles. Voir [composer plusieurs fichiers](composition.md).

Pour toutes les propriétés et leurs valeurs acceptées, utilisez la
[référence générée](reference.md#property-reference). `bgraph explain fichier.bd id`
permet de retrouver quelle couche a fourni une valeur.
