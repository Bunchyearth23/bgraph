# Fichiers, composants et données

## Partager des styles

Créez `maison.bds` :

```text
style service { fill: #dbeafe stroke: #2563eb 1.5 radius: 8 }
```

Puis `application.bd`, dans le même dossier :

```text
include "maison.bds"

node api "API" .service
node jobs "Traitements" .service
layout { place jobs below api gap 40 }
api -> jobs
```

```sh
bgraph application.bd -o application.svg
```

Les chemins sont relatifs au fichier qui écrit l'`include`.
Une inclusion cyclique ou un nom déclaré deux fois produit une erreur.
Un fichier `.bds` ne contient que des styles ; un `.bd` peut contenir
un diagramme complet.

## Inclure un diagramme sous un préfixe

Pour inclure le fichier précédent depuis `ensemble.bd` :

```text
include "application.bd" as backend

node ui "Interface"
layout { place ui above backend.api gap 48 }
ui -> backend.api
```

`backend.api` désigne le nœud `api` du fichier inclus. Ses références internes
sont préfixées elles aussi. Le préfixe évite les collisions entre diagrammes.

## Réutiliser un composant

```bgraph
component service {
  node app "{{label}}"
  node db "Base"
  layout { place db below app gap 32 }
  app -> db
}

use service as web { label: "Web" }
use service as jobs { label: "Traitements" }

layout {
  place web at (0, 0)
  place jobs right-of web gap 80
}

web.app -> jobs.app "déclenche"
```

Chaque instance est un cadre que vous pouvez placer. Les paramètres sont
substitués par le moteur de templates, avec l'échappement adapté au contexte.

## Remplir un template

Créez `trafic.bd` :

```text
type pie { title: "Trafic {{year}}" }
slice organic "Recherche" {{organic}}
slice direct "Direct" {{direct}}
slice referral "Référents" {{referral}}
```

```sh
bgraph trafic.bd -o trafic.svg --set year=2026 --set organic=4820 --set direct=2410 --set referral=1180
```

Pour relire les mêmes valeurs depuis un objet JSON, créez `trafic.json` :

```json
{"year": 2026, "organic": 4820, "direct": 2410, "referral": 1180}
```

```sh
bgraph trafic.bd --data trafic.json -o trafic.svg
bgraph trafic.bd --data trafic.json --emit
bgraph watch trafic.bd --data trafic.json
```

`--emit` affiche le source rempli. `watch --data` recharge aussi les données
lorsqu'elles changent. Un paramètre manquant donne une erreur nommant le trou
à remplir.

## Alimenter un genre depuis un CSV

Pour un camembert, créez `ventes.csv` :

```csv
name,value
Recherche,4820
Direct,2410
Référents,1180
```

Puis `ventes.bd`, dans le même dossier :

```text
type pie { title: "Ventes" from: "ventes.csv" }
```

```sh
bgraph ventes.bd -o ventes.svg
```

Les colonnes requises dépendent du genre ; consultez sa section dans la
[référence](reference.md). Pour des templates qui répètent une ligne par
enregistrement, consultez `--rows` dans `bgraph --help` et la
[syntaxe des répétitions](reference.md#repetition).

!!! note "Fichiers et rendu embarqué"
    `include` et `from:` demandent un chemin sur disque. Utilisez le CLI ou
    `render_file` en Rust. Le rendu texte autonome dans le navigateur ou le
    plugin Typst ne dispose pas de ce contexte de fichiers.
