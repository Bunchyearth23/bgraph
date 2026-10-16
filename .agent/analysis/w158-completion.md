# W-158 — preview et provenance LSP

État : terminé et prouvé après stabilisation math W-156. Consolidation visuelle
W-164 incluse ; gates globaux et hôtes finaux centralisés par root.

## Architecture et périmètre

Projection de `PlacementPlan` dans `src/explain/inspect.rs`, partagée par preview,
hover et navigation. Aucun nouveau champ du solveur, aucune dépendance, aucun
changement des SVG exportés : surbrillance et poignées sont ajoutées seulement
au DOM de la preview. Le serveur watch utilise l'analyse composée commune et
sa `SourceMap`, avec le buffer rempli lorsque `--data` est employé.

Le périmètre D-047 est le placement V2 des neuf archetypes Declared. Les genres
Managed n'acceptent pas de layout (W-159). Le panneau expose leur politique et
leur cascade ; certains renderers produisent des primitives 1:n/sans ID source.
Il n'invente donc pas une bijection ni des bounds individuels pour pie/packet.
La source reste sélectionnable, y compris la provenance `style slice` de pie.

## Livré

- Bouton **Inspect layout**, liste nœuds/cadres, bounds absolus, cinq ancres
  nommées et coordonnées, parent cliquable, raison lisible et cascade paint.
- Bounds sélectionnés bleus, parents/ancres/entités liées ambre ; vrai clic dans
  le dessin et navigation bidirectionnelle ligne source ↔ sélection.
- Relations X/Y/largeur/hauteur avec texte et fichier/ligne d'origine. Conflits
  consultables sans géométrie, diagnostics liés aux déclarations complètes et
  aux relations, erreurs dans les propriétés comprises.
- Source d'un include affichée dans son propre fichier et avec ses propres
  coordonnées ; fichiers Unicode/emoji conservés. Rechargement sur contenu
  composé et données, y compris création d'un include vide auparavant manquant.
- LSP : définition déclaration → mention de placement prioritaire, mention →
  déclaration, groupes et includes namespacés compris. Nouveau provider
  références structurelles (déclarations optionnelles, layouts et edges).
- Hover : bounds, parent, placement/paint, liens aux fichiers source, relations
  et conflits. Les hovers de documentation et de paramètres template restent
  prioritaires. Outline enrichi des relations enfants des blocs layout.
- `explain` comprend les cadres et leurs translations ; une coordonnée issue
  d'un alignement explicite n'est plus attribuée au centrage implicite.
- Le contexte `style node` est traité avant la documentation du mot-clé `node`,
  pour que le hover public expose le thème en BPMN/XY aussi. Régression routée
  et test JSON-RPC indépendant, plutôt qu'un appel direct contournant le routeur.
- Décodage URI LSP corrigé : les octets percent-encodés sont décodés comme UTF-8,
  plutôt que transformés un par un en caractères. Positions LSP en UTF-16 ;
  positions humaines de la preview en scalaires Unicode, comme la CLI.
- Documentation utilisateur : `docs/language.md`. Driver navigateur durable :
  `tests/runtime/inspector.mjs` (Playwright et Chromium existants, configurables
  avec `BGRAPH_PLAYWRIGHT` et `BGRAPH_CHROMIUM`).

## Preuves ciblées exécutées

Chaque commande ci-dessous a fini avec code 0, sauf régression BPMN explicitement
signalée dans la section suivante.

| Commande | Résultat |
|---|---|
| `cargo check --lib` | compilation verte |
| `cargo check --locked --no-default-features --lib` | compilation verte |
| `cargo test --locked --lib watch::tests` | 14 tests verts |
| `cargo test --locked --lib lsp::` | 75 tests verts après correctifs, dont 6 inspection dédiés |
| `cargo test --locked --lib explain::` | 4 tests verts |
| `cargo test --lib the_preview_fills_a_template_from_its_data` | 1 test vert, ancien contrat préservé |
| `node --check src/watch_inspector.js` | syntaxe JS valide |
| `node --check tests/runtime/inspector.mjs` | syntaxe driver valide |
| `git diff --check` | propre |

Wire natif du LSP fraîchement figé : `initialize`, `didOpen` puis
`textDocument/documentSymbol` sur `frame-nested.bd` renvoie sept symboles
(outer, inner, a, b, trois layouts) et quatre relations enfants ; 7/7 acceptés par
le vrai `DocumentSymbol.is` du paquet client installé. Payload conservé
à `/tmp/bgraph-w158-outline-wire-math-final.json`. Le test hôte réel VS Code a d'abord rejeté l'outline des cadres : `detail: null`
violait `DocumentSymbol.is`, entraînant une conversion client en SymbolInformation.
Correction des champs optionnels en chaînes et test dédié. Vérification avec le
vrai validateur installé : premier symbole invalide avant correction, valide avec
`detail: ""`. La relance hôte réelle root a ensuite fini avec code 0 : sept fixtures
et deux scénarios inspection (définitions aller/retour, références exactes, hover,
outline scoped). Log : `/tmp/bgraph-goal-vscode-host-final.log`.

CLI réelle : `bgraph explain tests/runtime/frame-nested.bd inner` affiche
`inner — frame at 260, 150 · 30×30`, parent outer et translation locale (50,40).

## Revue navigateur et découverte W-164

La première passe prouvait les clics et les fills. La revue visuelle a découvert
une lacune indépendante de l'inspecteur : BPMN sans valeurs positionnelles
optionnelles dessinait event/gateway/data comme des tâches. La cascade fill
était correcte, les shapes builtin ne l'étaient pas. Le driver a été renforcé
avec event=40px et gateway=48px ; la régression a été exécutée rouge (`76 !== 40`,
`/tmp/bgraph-w158-before-bpmn-fix.log`). Après correction des defaults par mot
avant la cascade, le scénario passe sans affaiblir les assertions.

Preuve finale après math W-156 :
`/tmp/bgraph-inspector-ctfmjn/result.json`, six scénarios, zéro erreur JavaScript.
Les six captures de ce dossier ont été ouvertes et revues :

- `frames.png` : cadre inner traduit (260,150), parent outer, bounds bleus et
  cinq ancres. Coordonnées lisibles dans le panneau sans libellés superposés.
- `include.png` : entité p.a liée à é.bd ; sa relation référente sélectionne
  la ligne du fichier root. Emoji et texte `<script>` affichés littéralement.
- `conflict.png` : égalités de largeur et messages du noyau avec liens source,
  sans géométrie inventée ; la sélection reste exploitable.
- `bpmn-style.png` : événement à deux anneaux, gateway en losange avec croix,
  data à coin replié, task arrondie. Les quatre fills pastel et leurs sources
  canoniques concordent avec le dessin.
- `pie.png` : un clic sur la ligne source sélectionne ads, et la cascade montre
  `style slice`, sans rectangle source inventé pour la slice.
- `managed.png` : packet 1:n sur deux rangées, source du champ sélectionnée et
  frontière géométrique explicitement signalée.

Le driver teste aussi la sélection par vrai clic dans le SVG, le retour au
parent, les boutons déclaration/relation, le retour ligne→entité, la persistance
après reload, l'édition d'un include déclenchant le reload et les diagnostics.

Commandes finales (codes 0) : `cargo build --locked --bins`, copie des deux
exécutables dans `/tmp/bgraph-w158-final-math/`, puis
`node tests/runtime/inspector.mjs /tmp/bgraph-w158-final-math/bgraph`.
Log : `/tmp/bgraph-w158-browser-math-final.log`.
SHA-256 CLI : `de81e9a1f9840608c8eebebe25babe810be2c0a02736e599ec308c2f61a3d179`.
SHA-256 LSP : `f08899867f5ce965287056b4526a37e7fbc8708527d09d56f6524998c0c1d05a`.
Les exécutables de ce dossier restent figés. Aucune édition de production après
cette preuve.

## Limites déclarées

- Les includes sont chargés du disque ; le buffer d'entrée courant est analysé
  en mémoire. LSP ne prétend pas indexer tous les parents d'un fichier enfant
  ouvert isolément ni l'état non sauvegardé de tous les buffers d'un workspace.
- La preview est un inspecteur source en lecture seule, pas un éditeur qui
  réécrit les fichiers en déplaçant les boîtes.
- Les textes affichés après expansion template correspondent au buffer analysé
  et rendu, afin que leurs offsets restent cohérents.
- Pas de test hôte Windows/macOS dans cette passe ; preuve navigateur Linux et
  preuve VS Code réelle centralisée par root. Aucun profil utilisateur modifié.

## Coût et périmètre des outils

Graft a économisé environ 553 840 tokens sur 18 appels dans cette passe W-158
(estimation fournie par l'outil ; aucun coût dollar fourni). Aucun service
externe ni nouvelle dépendance pour l'inspecteur. INDEX laissé à root.
