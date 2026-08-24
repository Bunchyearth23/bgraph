# Analyse de l'utilisabilité du placement dans bgraph

**Date :** 2026-08-24  
**Portée :** syntaxe, AST, validation, solveur, groupes, contraintes, archetypes,
LSP, commandes `explain` / `layout`, wasm, preview et stratégie de refactor.  
**Nature :** analyse et recommandation; aucun changement de moteur n'est inclus.

> **Décisions postérieures au rapport — `D-042` à `D-050` (2026-08-24).**
> L'auteur a choisi Cassowary comme cœur du placement dès la refonte et un bloc
> `layout` séparé/scoped comme forme canonique du DSL v2. La migration est stricte :
> rendu identique pour les fichiers cohérents, refus sans écriture pour les cas
> ambigus ou dépendant d'un défaut actuel. Le solveur doit être strictement
> déterministe sur toutes les plateformes : mêmes sources, mêmes coordonnées et
> mêmes sorties. Une contradiction doit citer le conflit minimal, les lignes et
> valeurs incompatibles, proposer des corrections et rester identique dans toutes
> les interfaces. La v2 n'est livrable qu'avec blocks locaux, navigation
> bidirectionnelle, provenance, outline, `explain` et surbrillance preview; aucun
> placement inline concurrent n'est réintroduit. Les archetypes déclarent leur
> capacité `Declared`, `ArchetypeManaged` ou `Hybrid`; toute relation non supportée
> est refusée. Les collisions sont diagnostiquées sans déplacement; un éventuel
> `arrange` reste une transformation DSL séparée du render. Toutes les cibles sont
> un gate du premier prototype. Kasuari 0.4.12 est le candidat retenu pour ce spike,
> avec une adaptation locale minimale : les collections parcourues du tableau
> sont ordonnées, tandis que la table de lookup des contraintes reste hachée car
> elle n'est jamais itérée. Le même cas sous-contraint est désormais stable entre
> processus natifs et instances wasm; l'adoption finale attend encore le double
> run sur le véritable IR bgraph. Les passages comparant
> un palier initial limité aux égalités exactes restent la trace de l'analyse,
> mais ne décrivent plus la direction retenue.

## Résumé exécutif

Le ressenti « il est difficile de placer ce qu'on veut où on le veut » est fondé.
Il vient moins d'un manque de mots-clés que d'une contradiction entre la promesse
du produit et son modèle géométrique actuel.

bgraph promet un layout déclaré. Son noyau sait bien poser une boîte à une
coordonnée, ou sur un des quatre côtés d'une autre boîte. Mais cette relation
n'est pas conservée comme un invariant. Le moteur calcule d'abord les positions,
puis d'autres passes déplacent les rectangles : alignements, cadres de groupes,
attachements BPMN. Les dépendants ne sont pas toujours déplacés avec eux, les
cadres ne sont pas toujours recalculés, et le résultat final n'est pas comparé
aux relations écrites par l'auteur.

Le modèle est aussi trop peu expressif pour la composition précise. Une boîte ne
peut avoir qu'une seule clause de placement, avec une seule référence. On ne peut
pas dire naturellement « son X vient de A, son Y de B », « aligne son bord haut
sur C avec 12 px de décalage », « centre-le entre A et B », ou « distribue ces
quatre boîtes ». L'auteur compense avec des coordonnées absolues, des chaînes
d'ancres fragiles et des contraintes globales qui agissent aujourd'hui comme des
mutations tardives.

Enfin, les différentes interfaces n'utilisent pas exactement la même sémantique.
Le solveur fait suivre un élément sans placement au frère précédent de son groupe;
le cycle-check, `bgraph layout` et `bgraph explain` utilisent le nœud précédent
dans la liste globale. Le LSP ne propose le placement que pour `free`, alors que
huit autres archetypes passent par le même solveur. Le wasm du playground omet
les diagnostics géométriques post-layout disponibles dans le CLI et le LSP.

Le DSL amplifie cette fragmentation : placement inline et contraintes globales,
données positionnelles et propriétés nommées, mot d'archetype et synonyme `node`,
`graph` et `type`, entités placées mais parfois sans identifiant. Étendre ce langage
sans définir une forme canonique rendrait le projet plus puissant mais moins
apprenable. L'unification de surface doit donc être conçue en même temps que l'IR
géométrique, pas après.

La recommandation est donc un refactor important, mais progressif :

1. **Rétablir la confiance** avec un plan de placement normalisé et partagé par
   validation, résolution, explication et transformation de source.
2. **Unifier le DSL** : une seule grammaire mentale, un mot canonique par concept,
   un sous-langage de layout commun et une migration v1 → v2 vérifiée.
3. **Rendre les relations vérifiables** : toute relation déclarée doit encore
   être vraie dans le layout final, ou produire un diagnostic précis.
4. **Étendre le modèle par axes et ancres nommées**, en conservant la syntaxe
   actuelle comme sucre compatible.
5. **Remplacer les passes impératives par un cœur Cassowary**, conformément à
   `D-042`, avec contraintes requises, priorités, stays, provenance et départages
   déterministes dès la refonte.

Le grand refactor est justifié. En revanche, remplacer tout le moteur d'un coup
par un auto-layout ou un optimiseur général serait contraire à `D-001`, plus
difficile à expliquer et plus risqué pour le déterminisme.

## 1. Ce que le moteur fait aujourd'hui

### 1.1 Le modèle visible dans la langue

Le langage expose une clause de placement par déclaration :

```bgraph
node a "A" at (40, 60)
node b "B" right-of a gap 32 align top
```

La grammaire limite cette clause à deux familles :

- `at (x, y)` ou une cellule de grille;
- `left-of`, `right-of`, `above`, `below`, avec un `gap` et un alignement.

Références : [`src/syntax.pest`](../../src/syntax.pest#L92),
[`src/ast.rs`](../../src/ast.rs#L386).

Dans l'AST, une relation contient une direction, une seule ancre, un décalage sur
l'axe principal et un alignement discret sur l'axe transverse. Cela rend le cas
simple agréable, mais enferme le placement dans un arbre de dépendances.

Le parser réduit par ailleurs `left` et `top` à `Align::Start`, puis `right` et
`bottom` à `Align::End`. L'intention d'axe disparaît dès le parsing; le validateur
ne peut plus dire qu'un mot d'alignement a été utilisé sur le mauvais axe.
Référence : [`src/parser.rs`](../../src/parser.rs#L650).

### 1.2 Le pipeline réel

Pour les archetypes à layout déclaré, le flux est approximativement :

```text
source
  → parse + AST
  → check sémantique
  → styles et tailles intrinsèques
  → contraintes de taille
  → placement des nœuds et groupes
  → align-group (mutation tardive)
  → arêtes, annotations et marques spécialisées
  → déplacement BPMN boundary (mutation encore plus tardive)
  → check géométrique partiel
  → routage / labels / rendu
```

Ce n'est donc pas un solveur de contraintes au sens déclaratif. C'est une suite
de transformations dont certaines peuvent invalider le résultat des précédentes.

Le point d'entrée principal se trouve dans [`src/layout.rs`](../../src/layout.rs#L469).
La fonction `solve` dépasse mille lignes et mélange mesure, placement, groupes,
contraintes, détails d'archetypes, préparation des arêtes et annotations. Cette
taille n'est pas un défaut en soi; elle révèle surtout que les phases ne possèdent
pas de contrat intermédiaire vérifiable.

### 1.3 Deux familles de layout sous un même langage

Sur les trente définitions d'archetype, neuf utilisent réellement le solveur de
placement commun : `free`, `class`, `block`, `er`, `state`, `harness`, `bpmn`,
`network` et `requirements`. Les vingt-et-un autres passent dans leurs moteurs
de charts spécialisés avant ce chemin générique.

Un camembert ou un gantt doit naturellement garder une géométrie sémantique; il
ne faut pas les forcer dans un placement manuel. Le problème est l'absence d'une
capacité explicite dans `ArchetypeDef`. La grammaire accepte une forme commune,
puis certaines familles avertissent seulement que le placement est ignoré.
Références : [`src/layout.rs`](../../src/layout.rs#L493),
[`src/props.rs`](../../src/props.rs#L1441),
[`src/check.rs`](../../src/check.rs#L1027).

La promesse produit devrait distinguer clairement :

| Capacité | Sens |
|---|---|
| `Declared` | L'auteur place les entités; le moteur résout ses relations. |
| `ArchetypeManaged` | La géométrie découle des données et du genre. |
| `Hybrid` | Le genre impose une structure, l'auteur contrôle certains guides ou régions. |

Cette capacité doit piloter validation, documentation, complétion et preview.

## 2. Défauts de confiance : une déclaration peut ne plus être vraie

### 2.1 Les alignements cassent les ancres déjà résolues — priorité P0

`align-group` est appliqué après `place_all`. Il déplace uniquement les rectangles
nommés et ne propage pas ce mouvement aux éléments qui en dépendent.

```bgraph
node a "A" at (0, 0)
node b "B" at (100, 50)
node c "C" right-of b
align-group top a b
```

`c` est d'abord placé à la hauteur initiale de `b`. Ensuite `b` remonte, mais
`c` reste où il était. La relation finale `c right-of b align center` n'est plus
respectée.

Références : [`src/layout.rs`](../../src/layout.rs#L843),
[`src/layout.rs`](../../src/layout.rs#L1711).

Ce défaut est plus grave qu'un résultat peu esthétique : la source et l'image
ne racontent plus la même chose.

### 2.2 Les contraintes sont impératives et dépendent de leur ordre — P0

Deux contraintes de taille ne sont pas résolues simultanément :

```bgraph
same-width a b
same-width b c
```

Si `c` est la plus large, la deuxième passe agrandit `b`, sans revenir sur `a`.
La première contrainte peut donc être fausse à la fin. Deux alignements partageant
des membres ont le même problème.

Le sens d'`align-group` n'est pas déclaré non plus. Le code choisit l'axe à partir
de la géométrie courante; lorsque les étendues sont égales, le `>=` tranche
arbitrairement. Une modification de taille ou de position peut faire changer
l'axe d'une contrainte sans changer sa ligne source.
Référence : [`src/layout.rs`](../../src/layout.rs#L1734).

Le mot écrit ne sauve pas ce cas : `align-group top a b` peut aligner les bords
gauches lorsque la dispersion verticale est plus grande. Le parser a déjà ramené
`top` et `left` à la même valeur `Start`; le solveur reconstruit ensuite l'axe à
partir des rectangles. Ce comportement a été reproduit avec `a at (0, 0)` et
`b at (20, 100)` : `b` finit à X = 0, Y = 100 malgré le mot `top`.

Une contrainte doit être un invariant ou un conflit, pas une instruction de
mutation dont le résultat dépend de l'ordre du fichier.

### 2.3 Le contrat de `at` se contredit — P0

`D-031` et la référence générée disent qu'un `at` explicite gagne, et qu'une
contrainte contradictoire produit un avertissement. Le test actuel affirme au
contraire qu'`align-group` déplace un nœud portant `at`, sans avertissement.

Références : [`docs/INDEX.md`](../../docs/INDEX.md),
[`src/props.rs`](../../src/props.rs#L2844),
[`src/tests.rs`](../../src/tests.rs#L8725).

Il faut prendre une décision explicite puis la rendre vraie partout. Pour rester
fidèle au projet, la recommandation est :

- `at` est une contrainte **requise**;
- une relation requise contradictoire est une erreur de placement;
- un futur `prefer at` peut exister pour une préférence déplaçable;
- aucun mécanisme anti-overlap ne déplace silencieusement une position requise.

### 2.4 Les groupes peuvent cesser de contenir leurs membres — P0

Le cadre d'un groupe est calculé pendant le placement. Une passe d'alignement peut
ensuite déplacer un membre sans recalculer le cadre. Des contraintes entre
éléments de groupes différents peuvent également détacher les membres de leur
cadre.

Le `padding` géométrique du groupe est lu depuis les propriétés inline, tandis
que le style complet du groupe — thème, classe, héritage — est résolu plus tard.
Un `style group { padding: 40 }` peut donc être accepté et peint sans que les
bounds aient réservé les 40 pixels attendus. La taille du titre utilise aussi
des constantes avant la résolution de sa police.

Références : [`src/layout.rs`](../../src/layout.rs#L1928),
[`src/layout.rs`](../../src/layout.rs#L2168).

Un groupe doit être une entité géométrique du même système : style mesuré avant
ses bounds, containment vérifié après résolution, et translation propagée à tout
son sous-arbre.

### 2.5 L'attachement BPMN arrive trop tard — P0

Un événement `boundary` est d'abord placé comme une boîte ordinaire, puis déplacé
sur son hôte après les groupes et contraintes. Un nœud ancré sur cet événement,
ou un groupe construit autour de lui, conserve l'ancienne géométrie.

Références : [`src/layout.rs`](../../src/layout.rs#L1536),
[`src/bpmn.rs`](../../src/bpmn.rs#L67).

`attach` est une relation de placement et doit être normalisé avec les autres,
avant tout dépendant. Une mutation sémantique après le solveur ne peut pas fournir
ce contrat.

### 2.6 Le résultat final ne prouve pas ses invariants — P0

`check::after_layout` sait détecter certains labels cachés, waypoints dans une
boîte et routes orthogonales traversant une boîte. Il ne vérifie pas :

- qu'une relation d'ancre est satisfaite après toutes les passes;
- que deux nœuds ne se superposent pas;
- que chaque membre reste dans son groupe;
- qu'un `at` requis n'a pas bougé;
- que toutes les contraintes de taille ou d'alignement sont vraies.

Référence : [`src/check.rs`](../../src/check.rs#L191).

`D-001` implique de **diagnostiquer**, pas de réparer. Un overlap déclaré peut
être volontaire; il devrait au minimum être signalable, avec une option explicite
si l'auteur veut l'accepter.

## 3. Défauts de cohérence entre les interfaces

### 3.1 Le flux implicite possède quatre interprétations — P0

Sans placement explicite, le solveur place le premier frère à l'origine et fait
suivre les suivants selon `graph.flow`. Dans un groupe, « précédent » signifie
le frère précédent du même conteneur.

Cette règle est réimplémentée ailleurs avec le nœud précédent dans le tableau
global :

| Consommateur | Interprétation actuelle |
|---|---|
| solveur | frère précédent du même conteneur |
| détection de cycles | nœud global précédent |
| `bgraph layout` | nœud global précédent |
| `bgraph explain` | nœud global précédent |

Références : [`src/layout.rs`](../../src/layout.rs#L1822),
[`src/check.rs`](../../src/check.rs#L902),
[`src/relayout.rs`](../../src/relayout.rs#L43),
[`src/explain.rs`](../../src/explain.rs#L115).

Conséquences : un groupe valide peut recevoir un faux cycle; `bgraph layout`
peut proposer ou refuser une ancre incorrecte; `explain` peut donner une raison
factuellement fausse. Le test du solveur fixe déjà la sémantique par frère, mais
la matrice équivalente manque pour ces trois consommateurs.

Ces conséquences ont été reproduites : avec deux groupes ayant chacun leur flux
implicite, `explain` attribue le premier élément du second groupe au dernier nœud
global du premier; `layout` refuse ensuite d'écrire parce que son round-trip
changerait le dessin. Une autre composition valide pour le solveur est rejetée
par le cycle-check comme un cycle entre trois éléments.

La règle ne doit exister qu'une fois, sous la forme d'un `PlacementPlan`
normalisé consommé par tous.

### 3.2 Les contraintes ne sont pas validées — P0

Le validateur ne parcourt pas systématiquement `diagram.constraints`. Le layout
suppose que les références inconnues et les groupes ont déjà été signalés, puis
les ignore. En pratique, `same-width a typo` peut devenir un no-op silencieux,
et une contrainte réduite à un seul membre disparaître.

Référence : [`src/layout.rs`](../../src/layout.rs#L1660).

Cela contredit la convention du projet : un nom inconnu est une erreur dure. Une
contrainte invalide doit être arrêtée avant le solveur et pointer chaque nom avec
son span.

Le contrat public de `check` est lui-même trop fort : il indique qu'une liste vide
signifie que le diagramme peut être dessiné. Un groupe placé relativement à un
de ses membres peut pourtant recevoir « no problems found », puis échouer au
render sur un cycle de groupe. Le CLI et le LSP tentent un solve après le check,
mais n'en convertissent pas toutes les erreurs en diagnostic localisé. La propriété
à garantir devrait être testable : **toute erreur de `layout::solve` doit avoir un
diagnostic issu de l'analyse**, sauf invariant interne qui constitue un bug.

### 3.3 Les nombres géométriques peuvent retomber silencieusement sur un défaut — P1

Le check générique valide bien les énumérations, mais pas uniformément le type et
le domaine des propriétés numériques. Par exemple, `graph { gap: wide }` peut
laisser `GraphOpts` conserver le gap par défaut; une largeur non numérique peut
retomber sur l'auto-dimensionnement. Les tailles négatives ou une grille non
positive ne possèdent pas toutes un contrôle central.

Références : [`src/check.rs`](../../src/check.rs#L2981),
[`src/style.rs`](../../src/style.rs#L873).

Dans un outil de placement, une valeur géométrique ignorée sans message détruit
rapidement la confiance. Le vocabulaire de `props.rs` devrait porter un schéma de
valeur (`number`, domaine, unité, autorisation de `auto`/`none`) consommé par le
check et le style.

### 3.4 L'éditeur n'aide que `free` — P1

Le moteur de complétion propose les mots de placement seulement si
`archetype == "free"`. Pourtant huit autres archetypes suivent le même chemin
déclaré. Un utilisateur de `bpmn`, `class`, `network` ou `requirements` doit
connaître la syntaxe sans assistance.

Référence : [`src/lsp/completion_engine.rs`](../../src/lsp/completion_engine.rs#L254).

La capacité `Declared` d'`ArchetypeDef` doit remplacer ce test spécial. Les tests
doivent couvrir toute la matrice des archetypes, pas seulement `free` contre
`pie`.

La forme de la complétion est aussi trop plate. En `free`, la liste mélange les
débuts valides (`at`, `right-of`), des modificateurs (`gap`, `align`) et même
`extends`, qui n'est pas un placement. Le fallback de snippets traite plusieurs
statements comme des déclarations d'item et peut produire des formes invalides
pour `align-group`, `same-width`, `include` ou `note`. Les contraintes ne
complètent pas leurs identifiants.

Le LSP devrait utiliser un petit automate dérivé de la grammaire canonique :

```text
début de relation  → at | right-of | left-of | above | below
après direction    → identifiants ancrables
après ancre        → gap | align | fin
après gap           → nombre, puis align | fin
après align         → valeurs compatibles avec l'axe
```

Un test structurel doit parser chaque snippet émis dans chacun de ses contextes.

### 3.5 CLI, LSP, wasm et preview n'affichent pas les mêmes problèmes — P1

Le CLI `check` et le LSP ajoutent `check::after_layout`. L'export wasm
`bgraph_check`, utilisé par le playground, ne lance que le check sémantique. Le
serveur de preview résout et rend, mais ne présente pas ces warnings lorsque le
render réussit.

Références : [`src/main.rs`](../../src/main.rs#L682),
[`src/lsp/diagnostics.rs`](../../src/lsp/diagnostics.rs#L38),
[`src/wasm.rs`](../../src/wasm.rs#L110),
[`src/watch.rs`](../../src/watch.rs#L77).

Une API unique `analyze_source` devrait produire parse, validation, plan,
géométrie, diagnostics et éventuellement rendu. Chaque surface choisirait ce
qu'elle affiche sans réinterpréter les phases.

### 3.6 Le preview montre l'image, pas les raisons — P1

Le watch viewer et le composant playground offrent pan, zoom et ajustement. Ils
ne permettent pas de sélectionner une entité, voir sa boîte, ses ancres, son
parent, la relation qui a fixé X ou Y, ni les conflits. Le SVG ne porte pas non
plus d'identifiant stable de nœud destiné à l'inspection.

Références : [`src/watch.rs`](../../src/watch.rs#L145),
[`playground/src/lib/Viewer.svelte`](../../playground/src/lib/Viewer.svelte#L1).

Pour un langage de placement, l'inspecteur géométrique a probablement plus de
valeur qu'un drag-and-drop complet. Il faut d'abord rendre le modèle visible :
boîtes, guides, points d'ancrage, chaînes de dépendance et contraintes en conflit.

### 3.7 `bgraph layout` ne connaît pas la politique de l'archetype — P1

La commande matérialise seulement le flux implicite en ajoutant une ancre vers le
nœud global précédent. Elle ne traite pas correctement les conteneurs et ne
positionne ni notes, ni légendes, ni instances. Plus révélateur : elle peut ajouter
`right-of` à une `slice` de `pie`, puis le validateur avertit que le placement est
ignoré par cet archetype.

L'ajout d'une capacité `Declared | ArchetypeManaged | Hybrid` doit donc précéder
toute évolution de la commande. Sur un genre intrinsèque, `layout` doit refuser
clairement. Sur un genre déclaré, deux intentions gagneraient à être distinguées :

- `freeze` matérialise exactement la disposition implicite courante;
- un éventuel `arrange` propose une disposition initiale et écrit du DSL déclaré,
  sans jamais s'exécuter silencieusement au render.

## 4. Limites d'expression : pourquoi les coordonnées absolues reviennent

### 4.1 Une relation unique lie X et Y au même objet

`right-of a align top` fixe les deux axes par rapport à `a`. Les compositions
réelles demandent souvent deux sources indépendantes :

```text
b.left = a.right + 40
b.top  = c.top
```

Le langage actuel oblige à placer par rapport à A puis à espérer qu'une passe
d'alignement sur C ne casse rien. C'est exactement la composition qui échoue
aujourd'hui.

### 4.2 Pas d'ancres géométriques de première classe

Les quatre directions et trois alignements cachent les équations réelles. Il
manque des références explicites comme :

```text
a.left   a.right   a.top   a.bottom
a.center-x         a.center-y
```

Le couple d'ancres doit être clair. `right-of` peut rester un excellent sucre :

```text
b.left = a.right + gap
b.center-y = a.center-y
```

Il faut aussi distinguer explicitement distance bord-à-bord et pas centre-à-centre.
Cette distinction est centrale dans TikZ et évite qu'un changement de taille
modifie une intention mal définie.

### 4.3 Pas d'offset transverse précis

L'axe transverse ne possède que start/center/end. On ne peut pas demander
« aligne les hauts, puis descends de 12 ». Cela pousse vers `at` ou vers un nœud
intermédiaire artificiel. Un offset X/Y après l'ancre suffit à couvrir ce besoin
sans inventer une nouvelle direction.

### 4.4 Pas de lignes, colonnes, distribution ni guides

Une chaîne de `right-of` exprime une rangée, mais chaque élément dépend de la
taille et du nom du précédent. Insérer un élément, changer l'ordre ou égaliser
les espacements demande de réécrire plusieurs lignes. Les abstractions utiles
sont :

- rangée / colonne avec alignement et gap;
- distribution exacte ou minimale;
- grille native avec cellules vides et spans;
- guide X ou Y nommé;
- centrage entre deux références;
- coordonnées locales à un groupe.

Ces formes doivent être des **macros vers le même système de relations**, pas des
mini-solveurs parallèles. C'est ce qui préserve le routage ortho, l'explication
et les diagnostics.

### 4.5 Les coordonnées `at` ne sont pas une origine de page stable

`at` utilise la géométrie interne, puis le renderer translate l'ensemble des
bounds vers le padding du canvas. Un nœud seul à `(100, 50)` apparaît donc près
du padding de l'image, pas à 100 px du bord du fichier SVG.

Référence : [`src/render_svg.rs`](../../src/render_svg.rs#L223).

Ce comportement permet un viewBox ajusté et cohérent avec le projet. Il doit
toutefois être expliqué comme « coordonnées dans l'espace du diagramme » plutôt
que « coordonnées de page ». Si une origine stable est utile, elle devrait être
un repère explicite, pas un effet de bord du canvas.

### 4.6 Un waypoint accepte un alignement que le solveur jette

La syntaxe d'un waypoint relatif réutilise `rel_pos`, donc accepte `align`.
Lors de sa résolution, le champ est ignoré et le point reste centré sur l'axe
transverse.

Référence : [`src/layout.rs`](../../src/layout.rs#L1029).

Il faut soit implémenter la sémantique, soit refuser la forme avec un diagnostic.
Un mot de placement silencieusement sans effet est particulièrement coûteux.

## 5. Unifier le DSL est une condition du refactor

Ajouter un meilleur solveur sous le DSL actuel ne suffirait pas. Cela risquerait
même d'aggraver le problème : plus de relations, plus de syntaxes concurrentes et
plus d'endroits où chercher la cause d'une position.

Le projet a déjà une excellente intention — « chaque archetype lit la même forme
de statement » — mais cette unité est surtout interne. Pour l'auteur, plusieurs
concepts possèdent deux ou trois formes de surface :

| Concept | Formes actuelles |
|---|---|
| mot d'un élément | mot propre à l'archetype, mot alternatif, ou `node` universel |
| données d'archetype | valeurs positionnelles ou propriétés nommées dans un block |
| géométrie | placement inline sur la déclaration ou contrainte globale séparée |
| alignement | modificateur `align`, statement `align-group`, synonymes `center` / `middle` |
| configuration | `graph { … }` et `type X { … }`, avec deux portées à apprendre |
| block | propriétés seules pour un item, propriétés **et** statements pour un groupe |
| entité placée | items et groupes nommés, mais `note` et `legend` non nommables |
| ordre de la queue | block avant ou après placement; classes intercalables avec les données |

Chacune de ces décisions est explicable isolément. Ensemble, elles empêchent de
former une règle mentale courte. Le LSP en fournit la preuve pratique : une liste
plate de mots ne sait pas à quelle étape de la déclaration ils deviennent valides,
et finit par proposer `gap`, `align` ou des snippets génériques à des positions où
la grammaire ne les accepte pas.

### 5.1 L'unification visée

Unifier ne signifie pas transformer tous les diagrammes en une syntaxe générique
et verbeuse. Cela signifie fixer les invariants de langue suivants :

1. **Un concept, un mot canonique.** Les aliases restent lisibles pendant la
   migration, mais ne sont ni documentés comme forme principale, ni complétés,
   ni émis par le formatter.
2. **Une seule grammaire de placement.** `right-of`, alignement, égalité de taille,
   grille, rangée et attachement spécialisé abaissent tous vers le même langage de
   relations; il n'y a plus « placement d'item » d'un côté et « contraintes » de
   l'autre.
3. **Toutes les entités positionnables ont une identité.** Nœud, groupe, instance,
   note et légende peuvent être référencés, expliqués et inspectés de la même façon.
4. **Un ordre canonique des clauses.** Le parser de compatibilité peut rester
   tolérant; documentation, init, completions et formatter n'émettent qu'un ordre.
5. **Les données et la géométrie restent séparées.** Les slots d'un archetype
   décrivent ce que l'objet est; le sous-langage de layout décrit où il se trouve.
6. **Le schéma pilote tout.** `props.rs` — ou son successeur — déclare mots,
   propriétés, types, domaines, slots, capacité de placement et snippets; check,
   LSP, docs et migration ne maintiennent aucune liste parallèle.

### 5.2 Forme v2 retenue : un bloc `layout` scoped

`D-043` retient la variante qui sépare les déclarations des relations géométriques
dans un block `layout` lexicalement scoped.

```bgraph
diagram network {
  background: #0b1120
  gap: 48
}

host client "Client" .external
host api    "API"    .service
host db     "DB"     .store
note db_note "Replica, not primary" .caption

layout {
  client at (0, 0)
  api right-of client gap 64
  align top client api
  db below api gap 48
  align left api db
  db_note right-of db gap 18
  equal width api db
}

client -> api "HTTPS"
api -> db "SQL"
```

Les bénéfices sont importants :

- les déclarations disent ce qui existe;
- un seul endroit et une seule grammaire disent où cela se trouve;
- plusieurs relations par objet deviennent naturelles;
- `align top`, `equal width`, `row`, `column` et `at` ont tous la forme d'une
  relation de layout, au lieu de mélanger suffixes et statements spéciaux;
- le LSP connaît son contexte dès qu'il entre dans `layout`;
- `explain` peut pointer directement la ou les lignes responsables de X et Y;
- un groupe peut contenir son propre `layout`, ce qui donne enfin un flow et un
  gap locaux sans nouveau mécanisme :

```bgraph
group services "Services" {
  host api "API"
  host worker "Worker"
  host db "DB"

  layout {
    column api worker db gap 24 align left
  }
}

layout {
  services right-of client gap 72
}
```

Dans cette proposition, `right-of` fixe l'axe principal. L'alignement transverse
est explicite par une deuxième relation ou un modificateur canonique. La migration
doit développer l'ancien alignement implicite en `align center` afin de conserver
exactement l'image; la v2 évite ainsi de cacher une deuxième contrainte dans un
mot qui semble ne parler que de X.

### 5.3 Ce qui resterait compact

Les mots propres aux archetypes (`slice`, `field`, `task`, `host`) sont utiles :
ils portent le domaine et les slots attendus. Ils doivent rester la forme
canonique. En revanche, `node` utilisé comme synonyme universel dans un genre ne
devrait plus être émis ni complété en v2; il peut rester accepté en compatibilité.

Les données positionnelles restent également légitimes pour les formes très
fréquentes :

```bgraph
slice organic "Organic" 4820
```

Ce n'est pas une seconde géométrie, mais la signature compacte de `slice`. La
forme nommée `{ value: 4820 }` reste utile lorsque la lisibilité ou la génération
de données l'exige. Pour éviter que les deux deviennent deux dialectes, chaque
archetype doit déclarer une **forme canonique d'émission** et les outils doivent
la respecter.

### 5.4 Simplifications recommandées

- Fusionner le couple `type X { … }` / `graph { … }` dans un en-tête canonique
  comme `diagram X { … }`; le schéma sait quelles propriétés sont générales ou
  propres au genre.
- Remplacer `align-group top a b c` par `align top a b c` dans `layout`.
- Remplacer `same-width` / `same-height` par `equal width` / `equal height`, une
  même famille extensible.
- Garder un seul mot canonique pour un sens (`center`, pas `center` et `middle`).
- Donner un identifiant aux notes et légendes; une entité que l'on place doit
  pouvoir servir d'ancre et apparaître dans `explain`.
- Interdire en v2 les deux ordres équivalents « block puis placement » et
  « placement puis block », puisque le placement n'est plus dans la queue de
  déclaration.
- Réserver les propriétés `{ … }` aux données/style d'une déclaration et les
  blocks `layout { … }` aux relations géométriques. Un block n'a ainsi qu'un
  rôle mental.
- Faire de `row`, `column`, `grid` et `distribute` des macros du block `layout`,
  jamais de nouvelles propriétés spéciales dans les objets.

### 5.5 Compatibilité et migration

`D-044` impose que la migration ne laisse ni deux DSL vivants indéfiniment, ni
un changement silencieux du dessin. La stratégie retenue est :

1. les fichiers sans en-tête d'édition restent v1;
2. `edition 2` — ou l'équivalent retenu — active la grammaire canonique;
3. `bgraph fmt --migrate --to 2` transforme la v1, préserve commentaires et
   vérifie le rendu avant/après;
4. le parser continue à lire la v1 pendant une période définie, mais `init`, docs,
   snippets et completions n'émettent que la v2;
5. toute ancienne forme acceptée en v2 produit une correction automatique vers
   la forme canonique, pas une nouvelle branche de documentation;
6. une matrice de tests prouve l'équivalence sémantique et visuelle de chaque
   sucre migré.

Le point essentiel est que l'IR du placement et la v2 du DSL doivent être conçus
ensemble. Concevoir d'abord un solveur, puis lui ajouter une syntaxe, recréerait
la fragmentation actuelle à un niveau plus puissant.

### 5.6 Décision prise et travail de spécification restant

Le besoin d'unification et la famille syntaxique sont désormais établis. Deux
familles ont été comparées :

- **block `layout` séparé**, retenu pour la lisibilité, les relations multiples
  et le LSP;
- **clauses `place` attachées aux déclarations**, plus locales mais plus difficiles
  à étendre proprement à plusieurs axes, groupes de contraintes et distributions.

Le choix est fermé; il reste à prototyper le block retenu sur cinq fichiers
représentatifs — architecture libre, BPMN, réseau groupé, diagramme avec notes et
composants, chart intrinsèque — afin de fixer la grammaire détaillée, les scopes,
l'ordre canonique et la qualité des diagnostics avant la modification générale.

## 6. Ce qui est déjà solide et doit être conservé

Le diagnostic ne remet pas en cause tout le projet. Plusieurs choix sont de
bonnes fondations :

- `D-001` est un positionnement produit distinctif : l'auteur décide, le moteur
  résout et signale;
- le gap bord-à-bord rend les chaînes simples lisibles;
- le solveur fixe et déterministe convient bien aux goldens;
- les bounds pilotent le viewBox au lieu d'un canvas codé en dur;
- groupes, grille, waypoints, côtés d'arête, lanes et routeur ortho couvrent déjà
  une grande partie du vocabulaire nécessaire;
- `explain`, `layout`, le LSP et le JSON géométrique sont les bons points
  d'intégration, même s'ils doivent partager une source de vérité;
- la suite actuelle est large : **571 tests ont passé** lors de cette analyse;
- l'AST/layout interne est volontairement hors de la promesse semver, ce qui
  permet un refactor profond derrière les API stables.

Le problème se trouve surtout dans les interactions entre couches et les cas de
composition absents de la matrice de tests, pas dans une absence générale de
discipline.

## 7. Mesures de référence

`cargo test` passe intégralement : 566 tests de bibliothèque, 1 test du binaire,
1 test d'intégration et 3 doctests, soit 571 au total.

Le benchmark existant, exécuté en mode vérification, donne sur cette machine :

| Cas | Temps total observé |
|---|---:|
| grand graphe, 250 nœuds + 249 arêtes étiquetées | 2,406 s |
| routage ortho | 109 ms |
| template | 200 ms |
| chart | 7,8 ms |
| composition | 1,2 ms |

Sur le grand graphe, parse ≈ 182 ms, layout ≈ 146 ms et render ≈ 2,079 s. Le
layout n'est donc pas le goulot principal du benchmark actuel; le rendu domine.
Cela laisse de la marge pour une normalisation et des vérifications sérieuses.

Réserve : ce benchmark couvre une chaîne forward simple. Il ne mesure ni les
références inversées, ni les conflits, ni de nombreux groupes et contraintes.
Avant un nouveau solveur, il faut ajouter des cas adverses qui pincent précisément
ces axes.

## 8. Options de refactor

### Option 0 — Correctifs locaux sans IR commun

Corriger séparément la propagation d'`align-group`, le recalcul des groupes, les
completions et le wasm apporterait un soulagement rapide, mais laisserait quatre
interprétations du flux et plusieurs passes mutables. Chaque nouveau mot de
placement recréerait le même risque.

**Verdict :** utile seulement comme hotfix, insuffisant comme direction.

### Option 1 — Étendre le solveur de dépendances actuel

Ajouter ancres nommées, offsets X/Y, rangées et colonnes tout en gardant un DAG
est possible. La syntaxe historique resterait naturelle et la dépendance externe
nulle.

Avantages :

- changement incrémental;
- déterminisme simple;
- faible coût initial;
- bon pont de migration.

Limites :

- plusieurs relations simultanées rendent le DAG artificiel;
- cycles géométriques valides et contraintes redondantes restent difficiles;
- distribution, containment et inégalités font proliférer les passes;
- le diagnostic d'un conflit reste ad hoc.

**Verdict :** bon palier, mauvais cœur final si l'ambition comprend contraintes
riches et placement interactif.

### Option 2 — Cœur hybride de contraintes linéaires — recommandée

Chaque boîte possède au minimum `left`, `top`, `width`, `height`; la mesure fixe
d'abord width/height, puis les relations déterminent X et Y. La syntaxe actuelle
est compilée vers des équations exactes.

Exemple conceptuel :

```text
right-of b a gap 40
  → b.left - a.right = 40
  → b.center-y - a.center-y = 0
```

Les relations actuelles sont pour l'essentiel des égalités de différence par axe.
Elles peuvent être résolues par un graphe de potentiels / union-find pondéré,
déterministe, sans dépendance et avec détection de contradiction. Cette première
étape suffit pour :

- plusieurs ancres exactes par objet;
- alignements simultanés;
- coordonnées absolues requises;
- propagation correcte;
- cycles cohérents et redondance;
- conflits citant les relations source.

Un solveur de style Cassowary ne devient nécessaire que pour :

- inégalités (`gap >= 24`);
- priorités (`required`, `prefer`, `stay`);
- distributions ou dimensions flexibles;
- édition incrémentale où l'on veut conserver la solution précédente.

Cette séparation respecte la philosophie de dépendances minimales : ne pas
introduire un simplex incrémental avant d'avoir un besoin que les égalités exactes
ne couvrent pas.

**Verdict :** meilleur équilibre entre contrôle auteur, puissance, diagnostics,
déterminisme et migration.

### Option 3 — Optimisation générale / auto-layout

Un moteur de type Penrose peut exprimer overlaps, distances, angles et objectifs
visuels. Graphviz ou D2 peuvent décider rapidement d'une disposition entière.
Mais ces familles introduisent choix implicites, minima locaux ou variations
difficiles à expliquer. Elles déplaceraient le produit de « je déclare le layout »
vers « je guide un moteur de layout ».

**Verdict :** à écarter pour le noyau. On peut reprendre l'idée de phases de
Penrose, pas son optimiseur général.

## 9. Architecture cible recommandée

### 9.1 `PlacementPlan` : une seule vérité avant le solveur

Introduire une représentation normalisée, interne, comprenant :

- identifiant qualifié et span d'origine;
- parent et ordre des frères;
- taille intrinsèque et style nécessaire à la mesure;
- variables géométriques exposées;
- relations X/Y normalisées avec provenance complète;
- dureté éventuelle (`required`, puis plus tard `prefer`/`stay`);
- capacité d'archetype;
- relations spécialisées abaissées, dont `attach` BPMN;
- groupes comme entités géométriques et règles de containment.

La normalisation désucre une seule fois :

- le flux implicite vers le frère précédent;
- `right-of` et autres directions;
- cellules de grille;
- `align-group`, `same-width`, `same-height`;
- groupes, composants et noms qualifiés;
- attachements spécialisés.

`check`, `solve`, `explain`, `relayout` et le LSP consomment ensuite ce plan ou
sa provenance, sans relire chacun l'AST avec leur propre règle.

### 9.2 Phases à contrats explicites

```text
parse / compose
→ validation des noms et valeurs
→ résolution des styles nécessaires à la mesure
→ mesure intrinsèque
→ normalisation du PlacementPlan
→ résolution exacte X/Y et tailles contraintes
→ bounds des groupes + vérification de containment
→ vérification de toutes les relations source
→ routage des arêtes
→ placement des labels et annotations
→ diagnostics géométriques finaux
→ Layout final immuable
→ SVG / PDF / JSON / PNG
```

Après la résolution, aucune phase ne devrait modifier un rectangle de nœud. Une
phase qui a besoin de le faire doit produire une relation avant le solveur.

### 9.3 Déterminisme

Le nouveau cœur doit préciser ses départages :

- contraintes requises satisfaites exactement ou diagnostic;
- origine canonique pour une composante libre;
- ordre source pour départager des composantes indépendantes;
- arrondi stable à la frontière de rendu seulement;
- aucune table de hash dont l'ordre influence la solution;
- même entrée, mêmes nombres, mêmes bytes quand le renderer ne change pas.

### 9.4 Diagnostics de conflit

Un conflit doit être formulé dans le vocabulaire source :

```text
Impossible de placer `b` :
  `b right-of a gap 40` exige b.left = 180
  `b at (120, 60)` exige b.left = 120
```

Le solveur devrait retourner un petit ensemble de relations incompatibles, avec
spans. Sans provenance, un système de contraintes plus puissant serait moins
utilisable que l'actuel.

### 9.5 API finale immuable

Conserver `Layout` — ou une évolution interne équivalente — comme frontière
commune des renderers limite le rayon du refactor. SVG, PDF, JSON et ortho ne
doivent pas connaître le solveur. Le routage reçoit des rectangles finaux; il ne
les déplace jamais.

## 10. Proposition de langage géométrique, illustrative et non décidée

L'objectif n'est pas d'imposer cette orthographe, mais de montrer le niveau
d'expression nécessaire. La forme historique reste valable :

```bgraph
node b "B" right-of a gap 40 align top
```

Elle pourrait être équivalente à des relations explicites :

```text
place b.left = a.right + 40
place b.top = a.top
```

Cas actuellement difficiles :

```text
place b.left = a.right + 40
place b.top = c.top + 12

place title.center-x = panel.center-x
place title.bottom = panel.top - 16

place hub.center-x = midpoint(a.center-x, c.center-x)

row api db cache gap 32 align center-y
column title api footer gap 24 align left
distribute a b c d between frame.left frame.right
```

Principes de surface recommandés :

- ancres explicites et peu nombreuses;
- équations par axe plutôt qu'une explosion de directions;
- bord-à-bord par défaut pour `right-of`/`above`;
- macros structurantes compilées vers les mêmes relations;
- `at` requis, préférence souple seulement avec un mot distinct;
- coordonnées locales à un groupe si elles sont introduites;
- pas d'anti-overlap implicite qui décide un ordre pour l'auteur.

Toute nouvelle syntaxe devra naturellement être reflétée dans pest,
tree-sitter, corpus, queries Zed, TextMate, completions et documentation générée.

## 11. Ergonomie de l'éditeur et du preview

### 11.1 Avant le drag-and-drop : rendre la géométrie inspectable

Ajouter un mode debug au preview :

- identifiant et bounds au survol;
- points d'ancrage visibles;
- guides X/Y et grille;
- parent de groupe et containment;
- flèches vers les relations qui ont déterminé X et Y;
- surbrillance du plus petit conflit;
- coordonnées internes et coordonnées canvas clairement nommées.

Le SVG rendu pourrait exposer `data-bgraph-id`, ou le preview consommer le JSON
géométrique parallèlement au SVG. La seconde option évite de faire du SVG public
une API d'édition implicite.

### 11.2 LSP

Améliorations utiles une fois le plan partagé :

- completions fondées sur la capacité d'archetype;
- hover montrant X et Y résolus et leur provenance;
- code action « matérialiser le flux implicite »;
- code action pour convertir un `at` en relations quand le résultat est évident;
- diagnostic direct sur une relation non satisfaite;
- aperçu des ancres valides pour la cible courante.

### 11.3 Déplacement interactif comme proposition, pas comme vérité cachée

Un drag peut devenir utile plus tard si le modèle fournit des contraintes
d'édition. Il ne devrait pas modifier silencieusement la source ou remplacer des
relations par un `at`. Le preview peut :

1. montrer la position proposée;
2. expliquer quelles relations seraient ajoutées ou remplacées;
3. produire un diff DSL;
4. laisser l'auteur l'accepter.

Cela préserve `D-001` : l'interface aide à écrire le layout déclaré, elle ne
devient pas un second moteur invisible.

## 12. Plan de réalisation

Les durées ci-dessous sont des ordres de grandeur, à recalibrer après un spike.

### Phase 0 — Contrat, prototype du DSL retenu et tests de caractérisation, 3 à 5 jours

- décider définitivement la priorité de `at`;
- prototyper le block `layout` scoped de `D-043` sur cinq fichiers réels;
- fixer l'en-tête d'édition, les règles d'émission et les scopes;
- écrire les invariants finaux sous forme de tests;
- ajouter des reproductions pour contraintes partagées, dépendants déplacés,
  groupes, BPMN, flux implicite et surfaces CLI/LSP/wasm;
- ajouter des benchmarks adverses de placement;
- déclarer les capacités des archetypes sans changer encore la sortie.

### Phase 1 — Fiabilisation du moteur actuel, 3 à 7 jours

- centraliser le flux implicite et la provenance;
- valider toutes les références de contraintes;
- centraliser type et domaine des valeurs géométriques;
- corriger ou refuser l'`align` des waypoints;
- intégrer `attach` avant les dépendants;
- recalculer les groupes après les relations;
- ajouter la vérification finale des ancres, contraintes, overlaps et containment;
- aligner CLI, LSP, wasm et preview sur la même analyse;
- étendre les completions aux archetypes `Declared`.

Cette phase ne règle pas toute l'expressivité, mais rend les erreurs visibles et
le comportement digne de confiance.

### Phase 2 — DSL canonique et squelette de migration, 1 à 2 semaines

- introduire l'édition v2 sans retirer la lecture v1;
- faire émettre uniquement la forme canonique par `init`, docs, formatter et LSP;
- migrer synonymes, ordre des clauses et entités positionnables sans id;
- transformer placements inline et contraintes globales vers le sous-langage
  commun, avec preuve de rendu avant/après;
- générer l'assistance depuis le schéma d'archetype;
- conserver une table explicite des sucres v1 et de leur développement v2.

### Phase 3 — `PlacementPlan` et axes explicites, 1 à 2 semaines

- construire l'IR normalisé avec spans;
- faire consommer le même plan par check/solve/explain/relayout;
- compiler la syntaxe historique vers des relations exactes X/Y;
- conserver les goldens cohérents inchangés;
- retirer les mutations tardives de rectangles;
- stabiliser une API interne de diagnostic de conflit.

### Phase 4 — Nouveau cœur exact, 2 à 4 semaines

- implémenter les équations de différence par axe;
- gérer cycles cohérents, redondance et contradictions;
- résoudre groupes et tailles contraintes dans le même modèle;
- définir départages et origine canonique;
- ajouter ancres nommées et offsets indépendants;
- migrer `align-group`, `same-width`, `same-height` en macros.

### Phase 5 — Structures et outillage, 1 à 3 semaines

- rangées, colonnes, distribution, guides et grille enrichie;
- inspecteur géométrique dans watch/playground;
- hovers et code actions LSP;
- éventuellement préférences et stays;
- évaluer Cassowary sur des besoins démontrés, pas par anticipation.

Estimation globale : **5 à 10 semaines** pour le cœur, la migration et une UX
éditeur convaincante, selon l'étendue retenue. Un premier gain de fiabilité est
réaliste en une semaine concentrée.

## 13. Critères d'acceptation

Le refactor devrait être jugé sur des propriétés, pas seulement des images :

1. Toute relation déclarée est vraie dans le layout final, ou un diagnostic cite
   les relations incompatibles.
2. Réordonner des contraintes indépendantes ne change pas la solution.
3. Un `at` requis ne bouge jamais silencieusement.
4. Une chaîne dépendante suit une ancre déplacée par une autre relation.
5. Chaque membre reste dans les bounds résolus de son groupe.
6. `attach` BPMN participe au même graphe que les autres placements.
7. `check`, `solve`, `explain` et `layout` partagent le même flux implicite.
8. CLI, LSP, wasm et preview exposent le même ensemble de diagnostics.
9. Une completion de placement est offerte si et seulement si la capacité de
   l'archetype l'autorise.
10. Un overlap est signalé, jamais réparé par un ordre inventé.
11. Les fichiers historiques cohérents conservent leur rendu ou bénéficient
    d'une migration explicitement documentée.
12. Le résultat reste déterministe entre exécutions et ordres de hash.
13. Les performances du layout restent secondaires face au rendu sur le grand
    benchmark, sans régression asymptotique sur les cas adverses.
14. Documentation, `init`, formatter et completions n'émettent qu'une forme DSL
    canonique par concept.
15. Chaque snippet proposé par le LSP parse et check dans son archetype.
16. La migration v1 → v2 préserve commentaires et rendu, ou refuse sans écrire.
17. Toute entité positionnable possède un id stable et la même surface d'ancrage.
18. La compatibilité v1 est un adaptateur vers l'IR v2, jamais un second chemin
    de validation ou de résolution.

## 14. Comparaison avec les modèles externes

### TikZ : le meilleur modèle de surface

TikZ fait des ancres des coordonnées de première classe et distingue placement
bord-à-bord et centre-à-centre. Sa bibliothèque de calcul compose offsets,
interpolations et coordonnées mixtes. C'est le modèle mental le plus proche du
contrôle précis recherché ici.

Sources : [ancres TikZ](https://tikz.dev/tikz-shapes#sec-17.5.1),
[positionnement et `on grid`](https://tikz.dev/tikz-shapes#sec-17.5.3),
[calcul de coordonnées](https://tikz.dev/tikz-coordinates#sec-13.5).

### Cassowary : le cœur adapté quand les relations deviennent simultanées

Cassowary traite égalités et inégalités linéaires, priorités, stays et édition
incrémentale. Il correspond aux alignements, espacements, centrages et dimensions
flexibles. Il ne choisit pas naturellement une branche d'anti-overlap ni les
relations non linéaires; ce sont des limites plutôt utiles pour bgraph, qui ne
doit pas inventer un ordre.

Sources : [rapport technique original](https://badros.com/greg/papers/cassowary-tr.pdf),
[historique du projet](https://constraints.cs.washington.edu/cassowary/).

#### Kasuari : bon socle de spike, pas encore conforme au contrat

Kasuari 0.4.12 est un fork Rust maintenu de `cassowary-rs`, porté par le projet
Ratatui. Sa surface est volontairement bas niveau, incrémentale, compatible
`no_std` et sans abstraction de rectangle imposée : c'est une bonne forme pour
abaisser l'IR géométrique de bgraph sans lui déléguer le modèle produit.

L'audit isolé a passé les builds natif, `--no-default-features`,
`wasm32-unknown-unknown` et `wasm32-wasip1`, avec et sans features par défaut.
Cela ne prouve encore ni le protocole Typst ni l'intégration dans bgraph.

Le blocage initial était `D-045`. La documentation de Kasuari prévient qu'un système
sous-contraint peut choisir non déterministement entre plusieurs solutions. Le
probe a reproduit exactement ce cas : 200 processus identiques ont produit les
deux solutions possibles, 101 fois l'une et 99 fois l'autre. Les tables de hachage
du solveur participent à l'ordre de sélection des symboles et pivots. Kasuari doit
donc être adapté avec ordres canoniques et tests cross-target avant toute
migration. Le spike a depuis introduit un fork local minimal utilisant des
collections ordonnées partout où l'itération peut choisir un pivot. Le probe
donne maintenant la même solution sur 32 processus natifs et 100 instances
wasm; la preuve doit encore être répétée sur le véritable IR avant la bascule.

Sources : [dépôt Kasuari](https://github.com/ratatui/kasuari),
[documentation 0.4.12](https://docs.rs/kasuari/latest/kasuari/),
[manifest](https://github.com/ratatui/kasuari/blob/main/Cargo.toml),
[changelog](https://github.com/ratatui/kasuari/blob/main/CHANGELOG.md).

### Graphviz : le piège entre position proposée et position exigée

Graphviz distingue `pos`, `pin` et le mode `-n`, mais certaines passes comme le
retrait d'overlap ou le ratio peuvent encore transformer les coordonnées. bgraph
devrait adopter une distinction explicite entre requis et préféré, avec un
contrat plus strict : un requis ne bouge pas.

Sources : [`pos` et `pin`](https://graphviz.org/doc/info/attrs.html#d:pos),
[`-n`](https://graphviz.org/doc/info/command.html#-n),
[`overlap`](https://graphviz.org/docs/attrs/overlap/).

### D2 : structures pratiques, mais moteur encore décisionnaire

D2 montre la valeur des grilles, mais aussi les limites d'une structure parallèle
au routeur : fillers invisibles et routage dégradé. Dans bgraph, rangées et
grilles doivent compiler vers la géométrie commune.

Sources : [positions D2](https://d2lang.com/tour/positions/),
[grilles D2](https://d2lang.com/tour/grid-diagrams/).

### Penrose : garder les phases, pas l'optimisation générale

Les stages de Penrose confirment l'intérêt de séparer formes, groupes, labels et
arêtes. Son optimisation non linéaire, ses minima locaux et ses conflits entre
objectifs conviennent moins aux goldens déterministes et aux diagnostics précis
de bgraph.

Sources : [contraintes Penrose](https://penrose.cs.cmu.edu/docs/ref/constraints),
[staged layout](https://penrose.cs.cmu.edu/blog/staged-layout).

La synthèse décidée est : **TikZ en surface, Cassowary dès le nouveau cœur,
Kasuari comme premier spike sous gate déterministe et multiplateforme, Penrose
pour l'idée des phases, D2 et Graphviz comme garde-fous contre le contrôle
implicite.**

## Conclusion

bgraph n'a pas besoin de devenir un auto-layout plus intelligent. Il a besoin de
devenir un compilateur plus strict de relations géométriques déclarées.

Le refactor prioritaire n'est donc pas « ajouter plus de directions », mais
**rendre le placement unique, composable et vérifiable** : une représentation
normalisée, plusieurs relations par objet, des ancres par axe, des groupes et
attachements dans le même système, puis une vérification finale qui garantit que
l'image honore encore la source.

Cette unité doit être visible dans la langue : une forme canonique, un block de
layout commun à toutes les relations, un schéma unique pour les outils et une
migration d'édition explicite. Le vieux DSL peut être lu pendant la transition;
il ne doit pas survivre comme une deuxième manière officiellement enseignée de
faire la même chose.

Cette direction est ambitieuse mais cohérente avec l'architecture et la promesse
du projet. Elle protège l'existant, rend les erreurs explicables et ouvre ensuite
la voie aux rangées, colonnes, guides et outils interactifs sans créer un second
moteur de placement caché.
