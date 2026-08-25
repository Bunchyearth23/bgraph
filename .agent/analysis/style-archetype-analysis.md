# Audit de cohérence du DSL et des styles par archétype

**Date :** 2026-08-25  
**Portée :** vocabulaire des archétypes, cascade des styles, validation,
complétion/hover LSP, `explain`, exemples et documentation.  
**Nature :** diagnostic et recommandations; aucun changement de moteur n'est
inclus dans cet audit.

## Résumé exécutif

Le projet a fait un gros travail d'unification, mais il reste une incohérence
centrale qui peut faire perdre beaucoup de temps à un nouvel utilisateur : le
mot d'un archétype est présenté comme une *theme* implicite, alors que le
compilateur, le validateur et le LSP ne reconnaissent réellement que le mot
principal de l'archétype. Les formes alternatives sont même proposées comme
des themes par la complétion, mais ne sont pas appliquées comme telles.

Le cas le plus visible est BPMN : `task`, `event`, `gateway` et `data` sont
quatre formes déclarées par l'archétype (`src/props.rs:1933-1990`). Pourtant
la résolution des nœuds utilise toujours `def.item` (`task`) dans
`src/layout.rs:636-650`, et les moteurs de charts font la même chose
(`src/charts.rs:314-325`). `style event { … }` est donc suggéré comme theme,
mais ne devient pas la couche automatique des événements. Dans le même temps,
`check_dead_styles` considère tous les mots alternatifs comme des themes
(`src/check.rs:564-575`), donc il ne signale pas ce style comme inactif.

Le problème est amplifié par deux divergences d'outillage :

* `check_styles` ne transforme que les cinq themes globaux en contexte
  spécialisé (`src/check.rs:488-519`); `style task`/`style event` est validé
  comme une classe générique et peut recevoir des propriétés d'arête sans le
  diagnostic attendu.
* `Context::at` du LSP ne reconnaît que `props::theme_style` (`src/lsp/context.rs:45-54`).
  Après `style task` ou `style event`, la complétion propose donc le vocabulaire
  générique des deux côtés, alors que la complétion de `style node` est filtrée.

Enfin, `explain` n'attribue pas toute la cascade réelle : il liste la couche
`style <def.item>` mais pas `style node`, alors que `Registry::resolve` applique
les deux (`src/style.rs:772-800`, `src/explain.rs:232-238`). Une explication peut
donc être factuellement fausse précisément quand un utilisateur cherche à
comprendre pourquoi une boîte est colorée.

Le reste du modèle est globalement cohérent : `props.rs` est bien la source du
vocabulaire, les propriétés inconnues sont refusées, les exemples couvrent les
archétypes, et la documentation explique correctement la différence entre
`graph { grid: ... }` et `style grid`. Le risque n'est donc pas une refonte
totale du DSL, mais l'absence d'une notion unique de **cible de style effective**
réutilisée par le compilateur, le check, les éditeurs et `explain`.

## Classement des constats

| ID | Gravité | Constat | Effet utilisateur |
|---|---|---|---|
| S-01 | P0 | Les mots alternatifs sont suggérés comme themes mais seuls les mots principaux sont résolus automatiquement. | Un fichier BPMN semble accepter `style event`, mais le style ne s'applique pas à tous les événements. |
| S-02 | P1 | Validation et complétion traitent `style <item>` comme classe générique. | Propriétés d'arête proposées/acceptées dans une theme de nœuds; l'éditeur enseigne une règle différente du renderer. |
| S-03 | P1 | `explain` omet la couche `style node` et n'affiche pas exactement la cascade résolue. | Le diagnostic “pourquoi cette boîte est ainsi” peut accuser la mauvaise couche. |
| S-04 | P1 | La documentation formule “un mot d'archétype = une theme” alors que BPMN en a quatre formes et que le code n'en résout qu'une. | Le lecteur ne sait pas s'il doit écrire `style task`, `style node`, ou une classe `.event`. |
| S-05 | P2 | Le nom `grid` désigne à la fois le système de coordonnées de `graph` et le mobilier visuel de `style grid`. | La distinction est documentée, mais reste facile à rater dans l'autocomplétion et les recherches textuelles. |
| S-06 | P2 | La répartition des propriétés est correcte dans `props.rs`, mais l'explication “style générique = les deux côtés” n'indique pas que les themes d'items sont un cas distinct. | Les utilisateurs peuvent choisir une classe générique alors qu'ils veulent une palette globale. |

## Analyse technique

### 1. Deux modèles concurrents des themes

`props::THEME_STYLES` contient `node`, `edge`, `group`, `grid` et `title`
(`src/props.rs:108-195`). La résolution ajoute ensuite dynamiquement
`def.item` :

```text
defaults → style node → style <def.item> → classes → inline
```

Cette règle est implémentée dans `Registry::resolve`
(`src/style.rs:772-800`) et est saine pour un archétype à un seul mot.

Mais `ArchetypeDef` permet explicitement plusieurs mots (`alt_items`), et
`ArchetypeDef::items()` les expose à la documentation et à la complétion
(`src/props.rs:1510-1535`). Le LSP les insère tous après `style`
(`src/lsp/completion_engine.rs:205-235`). Cela crée une promesse publique :
chaque mot proposé ressemble à une theme renderer-native.

Cette promesse est fausse pour BPMN. Le renderer demande toujours `def.item`
(`task`) au resolver, indépendamment de `node.keyword`. Le même motif est
visible dans `charts.rs`, où chaque entrée est résolue avec le mot principal.
Le code sait distinguer le mot écrit pour les slots et les propriétés
(`NodeDecl::props_for(def)`), mais cette information ne franchit pas la
frontière de la cascade de styles.

### 2. Validation incohérente avec le renderer

`check_styles` appelle `props::theme_style(style.name)` pour déterminer le
contexte d'une theme. Le mot principal et les mots alternatifs ne passent pas
par cette fonction. Ainsi `style task` et `style event` sont vus comme des
classes génériques, même si le resolver les utilise implicitement pour le mot
principal dans certains cas.

Le validateur dispose pourtant de toutes les informations nécessaires :
`def.items()` et `def.item_props`. Il lui manque une fonction canonique du type
`theme_target(def, name)` qui dirait :

* si le nom est une theme globale (`node`, `edge`, `group`, `grid`, `title`);
* si le nom est un item de l'archétype courant (contexte nœud, propriétés de
  l'item courant);
* sinon, classe libre (contexte des deux côtés).

Sans cette fonction, le renderer, `check`, le LSP et `explain` continuent à
réinventer la même décision avec des listes différentes.

### 3. `explain` ne reproduit pas la cascade effective

`Registry::resolve` applique `style node` avant la theme spécifique. Le code de
`explain::paint` construit pourtant une liste qui commence directement par
`style <def.item>` (`src/explain.rs:232-238`). Dans un `type rack`, par exemple,
un `style node` qui fournit la couleur de texte ne sera pas cité dans la
section `paint`, même si cette couleur est effectivement utilisée.

Le problème est sémantique, pas cosmétique : l'outil d'explication est censé
être une projection de la résolution, pas une approximation basée sur le nom
du style. Il doit consommer une trace de couches produite par le même resolver,
ou au minimum appeler la même fonction de sélection de themes et inclure
`node`, la theme item pertinente, les classes dans l'ordre, puis l'inline.

### 4. Documentation et exemples

La référence affirme : “Each genre adds one more theme: its own item word”
(`docs/reference.md:278-290`). Cette phrase est exacte pour les archétypes à un
mot (`slice`, `device`, `host`, etc.), mais incomplète pour BPMN. La section
BPMN documente bien quatre mots (`docs/reference.md:1552-1600`), sans préciser
lequel est une theme globale et lesquels sont des formes alternatives.

Les exemples BPMN utilisent `style node`, ce qui masque le défaut et donne une
bonne image par défaut, mais n'enseigne pas comment différencier visuellement
les tâches, événements, gateways et données. Les exemples `rack` et `harness`
montrent au contraire clairement la relation entre une theme item et des
classes (`examples/rack.bd:23-30`, `examples/harness.bd:18-21`). Il manque un
exemple BPMN qui force cette décision et un test qui vérifie le résultat.

La distinction `graph { grid: ... }` / `style grid` est bien expliquée dans
`docs/language.md:560-575` et `docs/reference.md:298-303`. Elle reste toutefois
un point de friction lexical : les deux mots apparaissent dans les mêmes menus
et recherches, avec des portées totalement différentes.

### 5. Éditeurs

Le LSP filtre correctement `style node` et `style edge`, mais
`Context::at` reconnaît une theme uniquement via `props::theme_style`
(`src/lsp/context.rs:45-54`). Les styles nommés après un item d'archétype
retombent donc dans `Block::Style(None)`, ce qui donne le tableau générique
des propriétés (`src/lsp/completion_engine.rs:92-140`).

Le TextMate et les queries Zed connaissent les mots syntaxiques, mais cette
coloration ne garantit pas la sémantique du style. C'est précisément pourquoi
la sélection de cible doit vivre dans `props.rs`/une API partagée, pas dans
chaque extension.

## Options évaluées

### Option A — Documenter que seul le mot principal est une theme

Corriger la documentation, retirer les mots alternatifs de la liste des themes
LSP et faire diagnostiquer `style event` comme classe non appliquée sauf si
`.event` est écrit.

*Avantage :* très petit changement, aucun changement de cascade.  
*Risque :* BPMN reste difficile à styliser par type; les quatre mots sont des
formes sémantiques visibles, donc l'utilisateur s'attend naturellement à
`style event`.

### Option B — Faire de chaque mot d'item une vraie theme (recommandée)

Introduire une cible de style structurée, dérivée de `ArchetypeDef::items()` et
utilisée partout. La résolution prend le mot déclaré (`node.keyword`) au lieu
de toujours prendre `def.item`. Le fallback `style node` reste commun, puis
`style <mot écrit>` devient la couche spécifique. Les chartes qui n'ont qu'un
mot ne changent pas.

*Avantage :* le DSL devient prédictible : ce que la complétion propose est ce
que le renderer applique; BPMN gagne enfin quatre themes utiles.  
*Risque :* il faut décider explicitement l'ordre entre `style node`,
`style task`, `style event` et les classes; les goldens BPMN peuvent changer si
des styles portant ces noms existent déjà comme classes.

### Option C — Séparer syntaxiquement theme et classe

Introduire, par exemple, `theme task { … }` et réserver `style` aux classes.

*Avantage :* aucune ambiguïté conceptuelle.  
*Risque :* rupture de DSL plus lourde, duplication grammaire/LSP/éditeurs et
aucun gain pour les cinq themes existantes; disproportionné au stade actuel.

## Recommandation

Choisir l'option B et traiter la cible de style comme une donnée de l'IR, pas
comme une convention de chaînes.

Ordre de chantier recommandé :

1. Ajouter dans `props.rs` une fonction unique de classification des noms de
   style pour un `ArchetypeDef` (global, item, classe), avec le contexte et les
   propriétés lisibles.
2. Modifier `check_styles`, `Context::at` et la complétion pour consommer cette
   classification. Une theme item doit proposer uniquement le vocabulaire
   nœud/alt-item; une classe doit garder le comportement générique.
3. Étendre `Registry::resolve` avec la cible réellement écrite
   (`node.keyword`), tout en gardant le fallback `node` commun. Vérifier les
   archétypes avec `alt_items`, en premier BPMN et XY.
4. Faire produire à `explain` la liste des couches par le resolver partagé,
   notamment `style node` et la theme item effective.
5. Ajouter une matrice de tests : pour chaque `def.items()`, `style <item>` doit
   modifier une propriété visible; une propriété d'arête dans cette theme doit
   produire un diagnostic; la complétion et l'hover doivent montrer le même
   contexte; `explain` doit citer les mêmes couches que le rendu.
6. Mettre à jour `docs/reference.md` (via `docgen`) et `docs/language.md` pour
   distinguer “mot principal” et “forme alternative”, puis ajouter au fichier
   BPMN un exemple `style event`, `style gateway` et `style data`.

## Chantiers proposés

* **W-S01 — StyleTarget canonique** (P0) : classification partagée et API de
  resolver; aucun changement de syntaxe.
* **W-S02 — Themes des formes alternatives** (P1) : appliquer les themes selon
  le mot déclaré; goldens BPMN/XY et règle de cascade documentée.
* **W-S03 — Explain/LSP alignés** (P1) : trace de cascade unique, completion et
  hover issus de la même cible.
* **W-S04 — Documentation et galerie de styles** (P1) : un exemple par type
  d'archétype (simple, multi-forme, chart furniture), tests de drift.
* **W-S05 — Clarification `grid`/`style grid`** (P2) : noms d'API internes et
  textes d'aide plus explicites; conserver la syntaxe publique tant qu'aucune
  décision de rupture n'est prise.

## Conclusion

La base est saine : le vocabulaire est centralisé, les erreurs silencieuses
ont été largement éliminées, et les exemples rendent le projet testable. Le
point qui reste incompréhensible n'est pas la quantité de propriétés, mais le
fait que la notion “theme implicite de l'archétype” existe sous quatre formes
différentes selon le sous-système. Tant que `props.rs`, `check`, le renderer,
le LSP et `explain` ne partagent pas une seule cible de style, un utilisateur
peut écrire une forme syntaxiquement correcte et obtenir un dessin qui ne
correspond pas à ce que l'éditeur lui a conseillé.

La correction recommandée est ciblée : rendre les formes alternatives de vrais
targets de style, puis faire dériver validation, complétion, rendu et
explication de cette même table. Cela apporte la cohérence recherchée sans
réintroduire une seconde grammaire ni un nouveau mécanisme de layout.
