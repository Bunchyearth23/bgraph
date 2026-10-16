# Des diagrammes que vous maîtrisez

**bgraph transforme un fichier texte `.bd` en diagramme SVG, PDF ou PNG.**
Vous décrivez les éléments, leurs liens et leurs styles. Pour les schémas
d'architecture, vous déclarez aussi leur placement ; pour les graphiques,
les données déterminent la géométrie.

![Architecture avec groupes et services](examples/free.svg)

## Commencer en trois commandes

Une fois [bgraph installé](installation.md) :

```sh
bgraph init free -o architecture.bd
bgraph architecture.bd -o architecture.svg
bgraph watch architecture.bd
```

Ouvrez l'adresse indiquée, par défaut **<http://127.0.0.1:7878>**.
Modifiez le fichier et enregistrez : la preview se recharge.

**[Créer son premier diagramme →](demarrage.md)**

## Trouver le bon guide

| Je veux… | Aller à |
|---|---|
| Installer les outils | [Installation](installation.md) |
| Comprendre `node`, les flèches et `layout` | [Premier diagramme](demarrage.md) |
| Aligner des boîtes, créer une rangée ou un groupe | [Placement](placement.md) |
| Choisir les couleurs et partager une palette | [Styles](styles.md) |
| Faire un camembert, un schéma réseau, un Gantt… | [Types de diagrammes](graphiques.md) |
| Partir d'un exemple complet | [Galerie des 30 types](galerie.md) |
| Exporter ou automatiser le rendu | [Commandes](commandes.md) |
| Travailler dans Zed ou VS Code | [Éditeurs](editeurs.md) |
| Générer un diagramme depuis des données | [Composition et données](composition.md) |
| Trouver une propriété précise | [Référence générée](reference.md) |

## Deux autres exemples

### Des données : `pie`

![Sources de trafic en anneau](examples/pie.svg){ .overview-image }

Créez ce fichier avec `bgraph init pie -o trafic.bd`.

### Un processus : `bpmn`

![Processus BPMN](examples/bpmn.svg){ .overview-image }

Créez ce fichier avec `bgraph init bpmn -o processus.bd`.

Les guides de ce site sont en français. La référence exhaustive, le tour
complet du langage et le contrat d'API restent en anglais, dans leur version
maintenue par le projet.
