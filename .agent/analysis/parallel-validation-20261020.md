# W-166 / W-167 / W-168 — validation intégrée du 20 octobre 2026

Un agent par chantier, puis validation commune par le coordinateur.
Les trois états restent `building` : aucune validation utilisateur ni clôture
n'a été déduite des contrôles automatisés.

## Résultat

- W-166 : réservation des grands libellés et titres implicites des cadres de
  séquence ; 11 nouveaux tests d'intégration, dont 5 exécutés aussi sans
  fonctionnalités par défaut.
  [Preuves ciblées](w166-validation-20261020.md).
- W-167 : neuf nouveaux tests des captions, offsets explicites, annotations,
  titres et empreintes multilignes ; deux attentes historiques ajustées au
  wrapping et à un warning réel, sans supprimer les autres diagnostics.
  [Preuves ciblées](w167-validation-20261020.md).
- W-168 : origines locales des groupes BPMN cohérentes entre solveur et
  validation, centrage de hauteurs différentes possible, canaux orthogonaux
  partagés évités quand une autre voie existe, rejet immédiat d'un port situé
  dans la clearance d'un obstacle. Cinq nouveaux tests unitaires et un test
  d'intégration du routage.
  [Preuves ciblées et rejeu](w168-validation-20261020.md).

## Contrôles finaux

Tous les codes de sortie sont zéro. Commandes et durées :
[relevé intégré](parallel-validation-20261020.json).

| Contrôle | Résultat |
| --- | --- |
| `cargo test --locked --all-features` | 740 réussis, aucun échec, un test ignoré |
| `cargo fmt --check` | Réussi |
| Clippy, toutes cibles et fonctionnalités, warnings refusés | Réussi |
| Compilation sans fonctionnalités par défaut | Réussie |
| Compilation `wasm32-unknown-unknown`, wasm/tous genres/PDF/JSON/CSV | Réussie |
| `cargo bench --locked --bench pipeline -- --check` | Réussi |
| MkDocs `build --strict` | Réussi |

Le test ignoré préexistait : comparaison manuelle de performance du solveur
de cadres avec l'ancien chemin. Tree-sitter était disponible dans le PATH
pendant la suite et les tests de grammaire ont réussi.

Le premier lancement avait 15 échecs. Les corrections fonctionnelles, les
attentes devenues obsolètes et les artefacts décrits ci-dessous les résolvent.
La suite finale inclut 26 nouveaux tests par rapport au début de cette session.

## Artefacts vérifiés

- Les 30 exemples rendent. Comparaison avant/après inspectée pour les deux
  SVG modifiés, `examples/network.svg` et `examples/bpmn.svg` ; galerie et
  empreintes SVG régénérées. `examples/sequence.svg` reste identique.
- Le cliché PNG `labels` a été produit avec le même nom de famille et les
  mêmes octets de police incorporée que le test, inspecté, puis son empreinte
  mise à jour : `14046175f4ed513d`. Les deux autres clichés restent identiques.
- `docs/reference.md` régénéré par un compilateur reconstruit : il lui manquait
  la documentation existante de `label-position`.
- Le résumé de licence des notes 1.0.0 reprend la formulation « pass on a
  build » attendue par sa garde, sans changer les conditions de licence.
- 25 reproductions originales inchangées, 75 commandes finales réussies.
  SVG, JSON et diagnostics sont identiques avant/après la dernière optimisation.
  Les 12 avertissements sur huit cas restent visibles.

## Performance du grand processus BPMN

Le benchmark standard passait mais ne représentait pas ce cas. Une arrivée
de lien à 0,08 unité à l'intérieur de la clearance d'un libellé fixé entraînait
une recherche complète infructueuse d'environ 29 000 points, répétée pendant
les passes du pipeline. Le rejet préalable réutilise la règle exacte de
collision ; le tracé de secours et l'avertissement restent identiques.

Cinq paires alternées de binaires release figés donnent une médiane CPU de
0,736 à 0,0796 seconde, soit environ 9,24× plus rapide, et une médiane réelle
de 0,742 à 0,0808 seconde sur cette machine.
[Mesures et empreintes des deux exécutables](w168-performance-final-20261020.json).
Ce ratio compare les deux versions corrigées de cette session, pas une
révision supposée du binaire précédemment installé.

## Reprise

Validation utilisateur des rendus et des avertissements restants, source
originale de la capture de séquence et exemples d'autres genres concernés.
La CI distante et les hôtes éditeur n'ont pas été relancés.

Le binaire release corrigé est construit dans `target/release/bgraph`.
L'installation existante dans `~/.cargo/bin` n'a pas été remplacée.
Aucun commit, push ou publication ; les changements d'outillage préexistants
ont été préservés.
