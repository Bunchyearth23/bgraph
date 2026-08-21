# Modularisation — tasklist

Objectif : réduire les responsabilités de `src/lsp.rs` sans changer le comportement du serveur.

- [x] Établir une baseline : `cargo test --all-targets` — 555 tests passent.
- [x] Extraire l'index UTF-8/UTF-16 dans `src/lsp/positions.rs`.
- [x] Extraire le framing JSON-RPC et les capacités dans `src/lsp/protocol.rs`.
- [x] Extraire les diagnostics et publications de notifications dans `src/lsp/diagnostics.rs`.
- [x] Extraire la complétion et les snippets dans `src/lsp/completion.rs`.
- [x] Extraire hover, symboles, définitions et références dans `src/lsp/navigation.rs`.
- [x] Extraire le contexte lexical et les helpers de déclaration dans `src/lsp/context.rs`.
- [x] Déplacer les tests dans les modules concernés, en conservant les tests d'intégration du serveur.
- [x] Vérifier après chaque extraction : `cargo test --all-targets`, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] Vérifier les builds réduits : `cargo build --no-default-features` et les combinaisons de features pertinentes (`pie`, `packet,json,csv,pdf`, `wasm`, `png`).
- [x] Mesurer la taille et les dépendances entre modules avant de considérer `W-141` terminé.

Ordre de démarrage : `positions` → `protocol` → `diagnostics` → `completion` → `navigation` → `context`.

## Prochains cinq lots préparés

### 1. Complétion et snippets — `src/lsp/completion.rs`

- Périmètre : `completion`, `items_for`, `completion`, `snippet`, `item_snippet`, `completion` helpers.
- Dépendances conservées : `Context`, `declared`, `declared_archetype`, `item_words`, `props`.
- Garde-fou : aucun changement dans l'ordre, les filtres ou le contenu des suggestions.
- Critère de fin : tests de complétion verts, notamment chaînes/commentaires, trous de template, styles, aliases et snippets.

### 2. Navigation — `src/lsp/navigation.rs`

- Périmètre : hover, document symbols, go-to-definition, références et document highlights.
- Dépendances conservées : `LineIndex`, `diagnostics`, `word_at`, `declaration_span`, `text_of`.
- Garde-fou : conserver les plages UTF-16 et les réponses `null` lorsqu'aucune cible n'existe.
- Critère de fin : mêmes réponses JSON sur les tests de hover, symboles, définitions et références.

### 3. Contexte lexical et déclarations — `src/lsp/context.rs`

- Périmètre : `Block`, `Context::at`, mots d'items, déclarations, identifiants, directions et helpers lexicaux.
- Dépendances conservées : `props`, `ast::Span`, `LineIndex` indirectement.
- Garde-fou : rester purement lexical pendant les fichiers invalides ; ne pas introduire de second parser.
- Critère de fin : les tests de contexte, complétion en groupe et mots d'archetype restent verts.

### 4. Tests et nettoyage — `src/lsp/*` et `src/tests.rs`

- Déplacer les tests unitaires proches du module qu'ils couvrent ; laisser dans `src/tests.rs` les tests de contrat traversant le serveur.
- Supprimer les imports devenus inutiles et rendre les interfaces internes explicites (`pub(super)` ou `pub(crate)`).
- Ajouter un test de non-régression par module extrait avant de supprimer l'ancien code.
- Critère de fin : aucun test ne dépend d'un détail privé uniquement parce qu'il est resté dans `lsp.rs`.

### 5. Validation finale — features, taille et dépendances

- Rejouer `cargo test --all-targets`, `cargo fmt --check` et clippy strict.
- Rejouer `cargo build --no-default-features`, puis les combinaisons `pie`, `packet,json,csv,pdf`, `wasm` et `png` si l'environnement les supporte.
- Comparer `wc -l src/lsp.rs`, `cargo tree --no-default-features` et les dépendances des nouveaux modules avant/après.
- Critère de fin : comportement identique, aucun warning, aucun module extrait qui force une dépendance inutile.
