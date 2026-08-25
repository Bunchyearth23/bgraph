# Audit transversal de cohérence du projet

**Date :** 2026-08-25  
**Portée :** DSL V2, grammaires, documentation, Typst, API library, wasm,
LSP/éditeurs, exemples, features Cargo et CI.

## Résumé

L’audit n’a pas révélé de seconde divergence critique dans le pipeline de rendu après la correction des targets de style. Il a toutefois trouvé une régression concrète sur l’interface Typst : le wrapper avait été renommé en `bg-diagram`, mais plusieurs imports, exemples et le wrapper fichier utilisaient encore `diagram`. Le manuel échouait avec `unexpected argument: caption` ou une fonction non définie selon le point d’entrée.

Cette incohérence est corrigée dans `d934da8`, avec propagation du nouveau nom dans le README principal, la documentation Typst, l’exemple, `typst.toml`, le manuel et `bg-diagram-file`. Le même passage a corrigé des commentaires de grammaire qui décrivaient encore un placement inline V1 et un compteur de dépendances devenu faux après l’ajout de Kasuari.

## Constats vérifiés

### Typst — corrigé

* `typst/bgraph.typ` exposait `bg-diagram` et `bg-diagram-file`, mais le second appelait encore `diagram` (`typst/bgraph.typ:40-52`).
* `typst/README.md`, `README.md`, `typst/example/demo.typ`, `typst/typst.toml` et l’import du manuel ne nommaient pas tous les mêmes fonctions.
* Le manuel utilise `diagram` comme fonction de figure de son thème (`docs/btyp/btyp.typ`). La correction conserve donc `#diagram(...)` pour la figure et importe `bg-diagram as render` pour l’image bgraph.

Vérifications :

```text
typst compile --root typst typst/example/demo.typ /tmp/bgraph-demo.pdf  ✅
./compile.sh docs/manual.typ                                             ✅
```

### DSL V2 et grammaires — cohérent après nettoyage documentaire

La production n’accepte plus le placement inline des items : `item_decl` ne contient que le mot, le nom, les données, les classes et le bloc de propriétés (`src/syntax.pest:105-111`, `tree-sitter-bgraph/grammar.js:190-207`). Les placements sont dans `layout` et les routes d’arêtes gardent `via`, ce qui est un contrôle de route et non une seconde position de boîte.

Les commentaires des deux grammaires affirmaient encore que l’item pouvait recevoir placement et bloc dans n’importe quel ordre. Ils ont été réalignés avec la grammaire effective. Les placements inline des `note`/`legend` restent intentionnels : ces objets ne sont pas des boîtes nommées et leur syntaxe est encore documentée dans `docs/language.md:100-120`.

### API, wasm et features — cohérent

`render_source` refuse les `include` et `render_file` les résout (`src/lib.rs:201-236`), ce qui correspond à la contrainte filesystem du wasm (`src/wasm.rs:20-28`). `bgraph_check` exécute le second passage géométrique quand le premier n’a pas d’erreur (`src/wasm.rs:105-122`), comme le CLI et le LSP.

Le nombre actuel de dépendances directes non optionnelles est cinq (`anyhow`, `pest`, `pest_derive`, `serde_json`, `kasuari` dans `Cargo.toml:158-168`). Les textes qui en annonçaient quatre ont été corrigés; la mention historique de `W-096` dans l’index n’a pas été réécrite, car elle décrit l’état de cette date.

### Documentation et exemples — divergences corrigées

* `AGENTS.md` montrait encore `node a ... at (0, 0)` alors que la V2 exige un bloc `layout`.
* `docs/language.md` parlait de “both chart archetypes” alors que le projet en contient beaucoup plus.
* Les descriptions des themes d’archétype sont désormais générées avec la règle multi-forme introduite par la correction précédente.

## Vérification finale

```text
cargo test --all-features --quiet       606 tests passés
cargo fmt --all -- --check              OK
cargo build --no-default-features      OK
typst compile .../demo.typ              OK
./compile.sh docs/manual.typ            OK
```

L’arbre Git est propre après `d934da8`. `W-164` reste marqué `building` dans `docs/INDEX.md` conformément à la règle du dépôt : son passage à `landed` attend la confirmation explicite de l’auteur, même si le code du premier jalon et les preuves associées sont présents.

## Reste à surveiller

Le prochain audit utile est la preuve de parité des diagnostics et de la provenance du placement entre CLI, LSP, wasm, preview et Typst (`W-155` à `W-160`). Les risques correspondants restent déjà listés dans `docs/INDEX.md`; aucun nouveau risque indépendant n’a été découvert ici.
