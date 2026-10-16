# Créer son premier diagramme

## 1. Créer un fichier

Créez `architecture.bd` avec ce contenu complet :

```bgraph
graph {
  title: "Une petite application"
  background: #ffffff
  padding: 32
}

style service { fill: #dbeafe stroke: #2563eb 1.5 radius: 8 }
style database extends service { shape: cylinder fill: #d1fae5 stroke: #059669 1.5 }

node client "Navigateur"
node api "API" .service
node db "Base de données" .database

layout {
  place client at (0, 0)
  place api right-of client gap 60
  place db below api gap 48
}

client -> api "HTTPS"
api -> db "SQL" { router: ortho }
```

Le fichier sans `type` utilise le genre **`free`**, adapté aux boîtes et aux
flèches. Une autre façon de commencer est
`bgraph init free -o architecture.bd`, qui écrit un exemple plus détaillé.

## 2. Comprendre les lignes

| Forme | Rôle |
|---|---|
| `graph { … }` | Titre, fond et réglages du diagramme |
| `style service { … }` | Style nommé, réutilisable |
| `node api "API" .service` | Élément d'identifiant `api`, libellé « API », style `service` |
| `layout { … }` | Positions et relations entre les boîtes |
| `client -> api "HTTPS"` | Lien dirigé avec un libellé |

L'identifiant sert aux références dans le fichier. Le texte entre guillemets
est ce que voit le lecteur. Un commentaire commence par `//` ; `#` sert aux
couleurs hexadécimales.

## 3. Produire l'image

```sh
bgraph architecture.bd -o architecture.svg
```

Ouvrez le SVG dans un navigateur ou insérez-le dans un document.
Pour un PDF, changez simplement l'extension :

```sh
bgraph architecture.bd -o architecture.pdf
```

## 4. Dessiner avec la preview ouverte

```sh
bgraph watch architecture.bd
```

Ouvrez **<http://127.0.0.1:7878>**, puis changez un libellé, une couleur ou
un `gap` et enregistrez le fichier. La preview reflète le nouveau rendu.
L'inspecteur permet d'examiner les éléments et leur style résolu.
Le source reste édité dans votre éditeur de texte.

## 5. Lire un diagnostic

```sh
bgraph check architecture.bd
```

En cas d'erreur, le diagnostic indique le fichier et la position à corriger.
Les noms de propriétés et de styles sont vérifiés : une faute de frappe
ne devient pas un réglage silencieusement ignoré.

**Suite : [placer les éléments](placement.md), puis [travailler les styles](styles.md).**
