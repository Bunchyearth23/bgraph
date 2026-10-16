# Placement, analyse commune et styles — travail parallèle

Date : 2026-10-16. Chantiers demandés : W-153, W-154, W-155, W-156,
W-157, W-160 et W-164 (styles). Sur instruction utilisateur et après vérification,
W-154 et W-155 passent à `landed`; les cinq autres restent `building`.

## Résultat par chantier

| Chantier | Changements observables | Limite ou prochaine condition |
|---|---|---|
| W-153 | Cassowary résout nœuds, frames et attaches BPMN ensemble jusqu'à 512 entités. Les contraintes des frames sont partitionnées par composantes indépendantes, sans perdre constantes, provenance ni ordre canonique. Les événements sont placés avant leurs dépendants, aussi dans le fallback acyclique >512; le déplacement BPMN tardif est supprimé. Les placements explicites restent requis en plus de l'attache. | Optimiser les grandes composantes connectées et lever le seuil. Les cycles impliquant une attache ne peuvent être dénoués symboliquement que dans le solveur simultané. Les nouvelles branches BPMN et partitions attendent une validation fonctionnelle. |
| W-154 | Suppression de `canonical_source` et `split_inline_placement`, convertisseurs V1 cachés dans les tests. Littéraux, constantes, chaînes dynamiques et fixtures transitives utilisent les blocs `layout`; V1 ne subsiste que dans les rejets explicites. Le fallback du playground utilise aussi V2. | Garder les deux grammaires et les éditeurs alignés lors des extensions. |
| W-155 | Analyse commune des rendus bibliothèque, CLI, LSP, wasm, Typst et preview. `FileAnalysis` conserve source map et diagnostics même après erreurs de parsing/composition. Les erreurs des includes imbriqués, absents ou répétés et des composants gardent leur localisation. Le LSP relaie au bon include avec `relatedInformation` vers le fichier enfant, puis efface les diagnostics corrigés. | Les includes exigent un contexte fichier; les interfaces autonomes navigateur/Typst ne prétendent pas fournir un filesystem. Les unités de colonnes sont documentées dans `docs/API.md`. |
| W-156 | Allocation et tri canoniques; choix sous-contraint épinglé sur 32 processus natifs et instances wasm fraîches. Les coordonnées fractionnaires restent dans le solveur; seul le SVG sérialise au centième. Parité SVG des 30 archétypes sur CLI, navigateur, WASI et Typst. | L'ordre déclaré garde son sens pour flow implicite et peinture. Les runtimes natifs Windows, macOS et ARM ne sont pas exécutés ici. |
| W-157 | Conflits typés avec équations et spans; chaînes equal-width/equal-height et nouvelles relations attach portent leur provenance. Seule UnsatisfiableConstraint produit un conflit source. DuplicateConstraint est idempotent pour le même objet; les erreurs internes Kasuari restent typées, y compris pendant la minimisation et dans les partitions. | Le noyau reste minimal par inclusion, sans minimum de cardinalité garanti. Les contradictions non linéaires de bornes de frame gardent leur diagnostic géométrique distinct. |
| W-160 | Le gate commun et les hôtes réels ont passé leurs fixtures au passage précédent. La continuation ajoute une matrice de compilation native Linux/macOS/Windows et compile localement les surfaces natives croisées et wasm. | La matrice CI distante n'a pas été exécutée ici. Les nouveaux changements ne sont pas couverts par une nouvelle exécution des suites runtime ou des hôtes. Les interactions UI ne sont pas assertées. |
| W-164 styles | Résolution et `explain` partagent les couches implicites; un override de même valeur appartient à la dernière couche. Hover distingue thème automatique et classe. Matrice validation/complétion/hover des formes BPMN et XY; quinze tests thèmes et deux `explain`. | Conserver la couverture lors de l'ajout d'un mot d'archétype. |

## Clôture W-154 et W-155 — 2026-10-16

Instruction utilisateur : « W-154 et 155 si prouvé alors passe les en done ».
Les deux chantiers sont classés `landed` selon la convention de l'index, sur le
code du commit `5d0320c`. Aucune modification d'implémentation n'est nécessaire
pour cette clôture.

Preuves reconfirmées sur ce code, toutes commandes terminées avec exit 0 :

- `CARGO_TARGET_DIR=target/pipeline-work cargo test --locked --all-features --lib analysis_tests::` :
  15 tests réussis (sept analyse source, huit multifichiers), dont localisation
  des erreurs, récupération, Unicode, includes répétés et effacement des relais LSP.
- Même commande avec filtre `inline` : quatre tests réussis, dont rejet V1 par
  le parseur, refus de migration du placement inline et conflit V1/layout.
- Gardes `the_tree_sitter_grammar_agrees_with_pest` et
  `the_grammar_corpus_expectations_still_hold` : une réussite chacune;
  Tree-sitter 0.26.11 présent sur PATH.
- `./tests/runtime/gate.sh --extensions` : 17 fixtures, parité SVG des 30
  archétypes, quatre contextes multifichiers. Comparaisons CLI/LSP, navigateur,
  WASI, preview, Typst et renderer VSIX, incluant diagnostics Unicode.
- Recherche exhaustive dans le code indexé : les définitions de
  `canonical_source` et `split_inline_placement` sont absentes; les interfaces
  de rendu et diagnostic appellent l'analyse commune.

Traces temporaires : `/tmp/bgraph-close-w155-analysis.log`,
`/tmp/bgraph-close-w154-inline.log`, `/tmp/bgraph-close-w154-grammar.log`,
`/tmp/bgraph-close-w154-corpus.log`, `/tmp/bgraph-close-w154-w155-runtime.log`.
Les tests et fixtures correspondants restent dans le dépôt.

Portée : cette clôture prouve le noyau V2 et le partage de l'analyse.
Elle ne clôt pas les grands graphes/cycles BPMN (W-153), le déterminisme natif
sur les autres plateformes (W-156), la minimisation des conflits (W-157), ni
les dernières preuves des hôtes réels et de la CI distante (W-160). La suite
complète et les hôtes VS Code/Zed ne sont pas relancés dans cette clôture.

## Continuation : compilation et performance

Lors de la continuation du placement/BPMN, les changements ont été relus et
compilés; aucun test n'avait alors été ajouté ni exécuté. Les appels existants des tests ont seulement été
adaptés au retour `Result` du minimiseur. Les résultats fonctionnels ci-dessous
précèdent le partitionnement et le nouveau placement BPMN.

Compilations finales réussies après la correction des cycles :

- Binaires Linux x64 hôte et Linux ARM64 (cross avec
  `CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc`).
- Binaires Windows x64 et ARM64 via `cargo xwin build --locked --target … --bins`.
- Bibliothèque `--no-default-features`, puis `--no-default-features --features bpmn`.
- Bibliothèques navigateur `wasm32-unknown-unknown`, WASI `wasm32-wasip1` et
  Typst; features `wasm,all-archetypes,pdf,json,csv`, avec `typst` à la place de
  `wasm` pour le module Typst.
- `CARGO_TARGET_DIR=target/pipeline-work cargo check --all-targets --all-features`.
- `cargo fmt --all -- --check` et `git diff --check`.

Les neuf builds utilisent `--locked`; leurs traces sont sous
`/tmp/bgraph-continuation-{native,arm64,windows-x64,windows-arm64,nodefault,bpmn,wasm,wasi,typst}-build.log`.
Les builds natifs croisés utilisent `target/native-continuation`; les autres
`target/runtime-gate`. Ils n'exécutent ni les binaires cibles ni les assertions
des tests. Les warnings Kasuari préexistants restent présents; les builds debug
Windows signalent aussi la collision préexistante de nom PDB bibliothèque/CLI.

La CI ajoute des builds release natifs Linux, macOS et Windows, ainsi qu'un
build feature `bpmn` seul. Ces jobs n'ont pas été lancés à distance pendant
cette session. Le macOS natif et le déterminisme runtime des autres plateformes
restent donc sans nouvelle preuve d'exécution.

Le benchmark de performance existant a reçu quatre cas de frames, hors harness
de tests. Commande : `CARGO_TARGET_DIR=target/solver-work cargo bench --bench pipeline`.
Les nombres suivants sont la moyenne par appel; la colonne `layout=` du log
additionne dix appels :

| Cas | Moyenne de layout |
|---|---:|
| 50 frames sœurs | 2,52 ms |
| 100 frames sœurs | 4,80 ms |
| 50 frames imbriquées | 17,13 ms |
| 100 frames, en deux chaînes de 50 | 40,16 ms |

Trace : `/tmp/bgraph-frame-final-partition.log`, commande réussie. La machine
n'est pas isolée; le baseline sous contention ne permet pas un ratio fiable.
Le seuil de 512 reste en place car une grande composante connectée n'est pas
accélérée par son seul partitionnement.

Le rattachement BPMN conserve l'ordre de distribution déclaré et la cascade de
style. Il supprime le flow implicite de l'événement; un `place` explicite reste
une exigence supplémentaire. Le contrôle final des placements et alignements
s'applique désormais aussi à BPMN. Seul le chevauchement d'un événement avec
son propre hôte est autorisé. Les cycles ordinaires de placement restent
refusés; les cycles contenant une attache peuvent être traités symboliquement
par le chemin simultané, puis validés par Cassowary. Le fallback >512 conserve
le refus explicite de ces cycles, sans refuser tous les diagrammes BPMN.

## Validation fonctionnelle avant cette continuation

- `cargo test --all-features --quiet` : 645 tests de bibliothèque réussis,
  un benchmark comparatif ignoré par défaut, un test du binaire, neuf tests
  Kasuari, un test LSP JSON et trois doctests réussis.
- `cargo clippy --all-targets --all-features -- -D warnings` : réussi.
  Quatre dépréciations préexistantes du fork Kasuari restent affichées comme
  warnings de dépendance; aucun lint du projet ne fait échouer la commande.
- `cargo fmt --all -- --check` et `git diff --check` : réussis.
- `./tests/runtime/gate.sh --extensions` : 17 fixtures ciblées, 30 archétypes,
  quatre contextes fichier réussis. Les fixtures incluent contradictions de
  tailles, bornes de frames, Unicode et includes en erreur. Les builds utilisent
  `target/runtime-gate`, isolé des builds de développement.
- `./tests/runtime/vscode-host.sh` : sept fixtures réussies dans VS Code réel,
  avec profil et affichage temporaires. Le marqueur de fin est requis par le script.
- `python3 tests/runtime/zed-host.py --grammar /tmp/bgraph-grammar.wasm` : six
  fixtures réussies dans Zed 1.24.2+stable. Composant `wasm32-wasip2` et grammaire
  WASI frais; recherche du serveur par l'extension, sans override du binaire LSP.
  Le proxy transparent enregistre URI, plages, messages et multiplicité.
- Huit tests multifichiers dédiés réussis, avec tests includes/composants et
  correction des diagnostics; le gate exerce aussi CLI, preview et LSP ensemble.
- `pnpm build` du playground, build no-default et check wasm/Typst : réussis.
- `cargo bench --bench pipeline -- --check` : réussi; grand graphe ~972 ms,
  ortho ~32 ms, template ~35 ms, chart ~2,6 ms et composition ~1,8 ms.
- Benchmark comparatif des frames exécuté séparément en release : 50 frames
  sœurs ~28 ms, 50 imbriquées ~49 ms, 100 sœurs ~90 ms, deux chaînes de 50
  ~166 ms. L'ancien chemin prenait ~45–335 µs : ce coût motive le seuil actuel.
  La limite de profondeur du parseur est 64; 100 frames directement imbriquées
  ne constituent donc pas un cas valide.

Traces de cette session (temporaires, pas une archive durable) :

- `/tmp/bgraph-continue-tests-complete.log`, `...-clippy-complete.log`,
  `...-runtime-gallery.log`, `...-vscode-complete.log`, `...-bench.log`.
- `/tmp/bgraph-zed-host-x9rnfoas/result.json`, transcriptions JSONL, captures et
  empreintes des binaires/composant/grammaire. CLI SHA256
  `2cef9cb77522e5154f0a8b347259f2d25173f049de19b459601b93bbd13bb8f1`;
  serveur `b570d3579ed83b1b34e2570a270bd956c41943f03fef0c3a69f7f3a6360da1c6`.
- `/tmp/bgraph-frame-snapshots/report.json` et rendus avant/après.

Les scripts et fixtures sous `tests/runtime/` sont les preuves reproductibles.
Le cache graft est régénéré après les changements.

## Snapshots et reprise

Les snapshots déjà périmés dans HEAD après les marqueurs SVG ont été régénérés,
rasterisés et inspectés : `free`, `sequence`, `block`, `er`, `requirements`,
`class`, `network`, `state`, `bpmn`. Les XML sont valides. Le lowering des frames
a ensuite déplacé seulement des arrondis SVG de 0,01 px dans `free` et `state`;
leurs nouveaux rendus ont été comparés visuellement à taille et contenu égaux.
Hashes finaux : `free` = `a4753b3276af91fa`, `state` = `735c9941cc0fa050`.
Le PNG `shapes` devient `a14676b474c3a993`; `labels` et `charts` sont inchangés.
Les snapshots PNG gardent leur dépendance préexistante aux fontes système.

Les artefacts d'éditeurs sont construits dans des dossiers temporaires ou
figés pour éviter les collisions avec les builds simultanés. Les modifications
locales étrangères au chantier sont préservées. Aucun commit ni publication
n'a été effectué.

Collision d'identifiant préexistante : `W-164` désigne les styles en cours et
l'export sans sources livré. Aucun id n'est réutilisé ou renuméroté.
X-020, X-027 et X-028 passent à `mitigated`, rejoignant X-021 et X-031;
W-154 et W-155 sont ensuite classés `landed` sur instruction utilisateur et
preuves de clôture ci-dessus; les cinq autres états parents restent inchangés.

Le gate runtime et les tests ciblés ont ensuite été reconfirmés pour la clôture
W-154/W-155. Les snapshots, la suite complète et les hôtes réels n'ont pas été
revalidés. À couvrir : dépendant d'un événement attaché,
placement explicite compatible/contradictoire, distribution de plusieurs
événements, cycles d'attache et cas >512; puis parité des runtimes et éditeurs.

Reprise : validation fonctionnelle des nouvelles branches, performance et
suppression du fallback >512, preuve runtime native sur les autres plateformes,
cardinalité des conflits et bornes non linéaires. W-158 et W-159 restent planifiés.
