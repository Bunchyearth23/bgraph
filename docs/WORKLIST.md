# Chantiers W-140 à W-150 — tasklist

Ordre : sécuriser les surfaces configurables → terminer les contrats/outils →
mesurer et durcir → améliorer l'ergonomie et le rendu.

## W-140 — matrice de fonctionnalités

- [x] Ajouter la matrice de builds à la CI.
- [x] Vérifier localement les variantes minimale, `pie`, `packet`, JSON/CSV/PDF, WASM, PNG et CLI/completions.
- [x] Ajouter un test qui empêche la réduction accidentelle de cette matrice CI.

## W-141 — modularisation du LSP

- [x] Extraire positions, protocole, diagnostics et contexte lexical.
- [x] Extraire les méthodes de complétion et les snippets.
- [x] Extraire navigation, hover, symboles et références.
- [x] Rapprocher les tests unitaires de leurs modules.
- [x] Vérifier les interfaces internes et mesurer la réduction de `src/lsp.rs`.

## W-142 — layout global

- [x] Définir une fonction de coût globale pour labels et routes.
- [x] Ajouter les diagnostics de conflit résiduel au JSON de layout.
- [x] Ajouter des cas de contention de labels et de corridors orthogonaux.
- [x] Vérifier déterminisme et absence de régression sur les goldens.

## W-143 — fuzzing

- [x] Créer un corpus d'entrées hostiles versionné.
- [x] Fuzzer parsing, vérification, layout et rendus SVG/PDF/JSON.
- [x] Exécuter le corpus dans la CI.
- [x] Ajouter les régressions trouvées comme tests déterministes.

## W-144 — benchmarks

- [x] Définir des fixtures : gros graphe, compose, ortho, template et chart.
- [x] Mesurer parsing, layout et rendu séparément.
- [x] Conserver les résultats ou seuils de régression dans CI.

## W-145 — API publique

- [x] Répertorier les modules/types publics réellement consommables.
- [x] Réduire la visibilité des détails internes ou documenter leur absence de stabilité.
- [x] Vérifier les exemples d'intégration Rust contre cette surface.

## W-146 — diagnostics JSON

- [x] Ajouter `bgraph check --json` pour les fichiers simples et composés, y compris les erreurs de composition.
- [x] Ajouter des tests CLI du schéma et du code de sortie.
- [x] Documenter le schéma stable dans la référence.

## W-147 — onboarding

- [x] Refaire le README autour du premier diagramme et du preview.
- [x] Ajouter une progression styles → données → bibliothèque Rust.
- [x] Vérifier chaque commande et chaque extrait dans la suite.

## W-148 — cohérence documentaire

- [x] Choisir la formulation unique : « 29 genres spécialisés + free ».
- [x] Aligner README, manuel, release notes, playground et textes générés.
- [x] Ajouter un garde-fou de cohérence des comptes.

## W-149 — snapshots visuels

- [x] Sélectionner des fixtures représentatives des formes, labels et charts.
- [x] Générer des snapshots PNG reproductibles.
- [x] Comparer les images avec un seuil explicite en CI.

## W-150 — schéma du vocabulaire

- [x] Définir le format versionné des métadonnées.
- [x] Générer le schéma depuis `props.rs`.
- [x] Exposer `bgraph schema`.
- [x] Tester que le schéma reste synchronisé avec les tables du compilateur.
