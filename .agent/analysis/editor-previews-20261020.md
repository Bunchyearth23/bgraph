# Previews des éditeurs — W-169 / W-170

2026-10-20. Un agent par éditeur, puis validation intégrée. Modifications
locales non commitées ; aucune installation dans les profils utilisateur ni
publication. Les changements préexistants de W-166 à W-168 sont conservés.

## Comportement livré

- **VS Code** : `bgraph: Open Preview`, `Ctrl+Shift+V` (`Cmd+Shift+V` sur
  macOS), volet à côté du source. Le wasm embarqué rend le texte courant
  après 150 ms sans nouvelle frappe, y compris les buffers non sauvegardés
  ou sans nom. Un volet par document, erreurs lisibles, nettoyage à la
  fermeture. Commande disponible indépendamment du LSP ; aperçu Markdown
  conservé. Aucun accès au système de fichiers depuis ce renderer : utiliser
  `bgraph watch fichier.bd --open` pour `include` ou les fichiers `from:`.
- **Zed** : tâche de langue `bgraph: Preview`, sauvegarde courante puis
  `bgraph watch fichier.bd --open --port 0`. Le navigateur reçoit le port
  réellement réservé et le watcher actualise le dessin à la sauvegarde.
  L'entrée fournie dans `editors/zed/keymap.json` associe `Ctrl+Shift+V`
  aux fichiers `.bd`. Ouvrir leur dossier comme projet pour exposer la tâche.
  Une relance remplace le watcher actif. Le passage du nom de fichier via
  l'environnement conserve espaces et caractères de shell sans les exécuter.

Les modes d'emploi se trouvent dans [le guide des éditeurs](../../docs/guide/editeurs.md),
avec les instructions propres à [VS Code](../../editors/vscode/README.md)
et [Zed](../../editors/zed/README.md).

## Preuves observées

| Vérification | Résultat |
|---|---|
| Suite Rust complète, toutes fonctionnalités | 745 réussis ; un ancien test de performance manuelle ignoré |
| Format, Clippy sans avertissement, build sans fonctionnalités par défaut | Réussis |
| Tests JS VS Code, avec le vrai wasm et sans client LSP | Six réussis, dont empaquetage réel d'un export |
| VSIX complet et variante sans LSP | Construits et contenus vérifiés ; paquet complet restauré |
| Gate des interfaces avec extensions | 22 fixtures, 30 genres, quatre contextes de fichiers réussis |
| Hôte VS Code réel | Sept fixtures, deux scénarios d'inspection et scénario preview réussis |
| Hôte Zed 1.24.3 réel | Tâche native, raccourci, sauvegarde, nom hostile, URL, reload et relance réussis |
| Hôte Zed LSP standard | Six fixtures réussies |
| Extension Zed wasm32-wasip1 | Compilation réussie |
| Binaires CLI/LSP release finaux | Compilation réussie ; le CLI expose `watch --open` |
| Documentation MkDocs stricte | Réussie |

Le scénario VS Code ouvre deux buffers sans nom, réutilise leurs volets,
modifie le source et vérifie fermeture/réouverture. Une capture après la
mise à jour montre les deux nœuds rendus :
[capture VS Code](editor-previews-vscode-20261020.png).

Le scénario Zed utilise un profil isolé, Xvfb et le vrai CLI. Un ouvreur de
navigateur d'enregistrement reçoit l'URL ; les réponses HTTP réelles prouvent
le rendu et son actualisation. Ce contrôle ne lance pas de navigateur
graphique par défaut. Le nom de test contient espaces, apostrophe, `$()`,
backticks et `$file` ; un seul watcher reste actif après relance.

Métadonnées : [validation commune](editor-previews-validation-20261020.json),
[hôte Zed et empreintes des fichiers](zed-preview-host-20261020.json).
Rapports détaillés : [agent VS Code](vscode-preview-20261020.md),
[agent Zed](zed-preview-20261020.md).
La validation précédente des marges, libellés et corrections BPMN reste dans
[le bilan parallèle](parallel-validation-20261020.md).

`trail check` retourne 1 avec 20 correspondances de vocabulaire aux rubriques
« Tried and ruled out » des learnings. Relecture des zones concernées : ce
sont des faux positifs. La tâche Zed utilise bien l'environnement puis une
expansion entre guillemets ; le paquet distingue dépôt complet et export ;
l'origine BPMN a une borne REQUIRED et une préférence d'égalité STRONG ; le
routeur conserve tous les segments déjà utilisés et son essai global. Les
mentions de preview dans les guides, manifestes et tests ne réintroduisent
aucune des variantes écartées. La justification est conservée ici pour le
prochain commit ou la prochaine PR, non créés pendant ce travail.

Avant le commit demandé ensuite par l'utilisateur, le contrôle sur les
fichiers indexés retourne 1 avec 12 correspondances lexicales, principalement
dans les rapports et métadonnées. Il annonce aussi une limite de 400 Ko de
diff inspecté : ce contrôle ne constitue donc pas une revue exhaustive du
commit. Les mêmes alternatives acceptées et les tests ci-dessus justifient
le changement ; la justification est reprise dans le message du commit.
Les PDF de preuve sont déclarés binaires pour préserver leurs octets, y
compris les espaces obligatoires des entrées xref.

## Reprise

1. VS Code : installer `editors/vscode/bgraph-1.0.0.vsix` avec `--force`, puis
   ouvrir un `.bd` et utiliser le raccourci.
2. Zed : installer le CLI et l'extension à jour selon le guide, fusionner
   l'entrée du keymap, ouvrir le dossier et son `.bd`, puis lancer le raccourci.
3. Valider les rendus côté utilisateur et lancer la CI lors de la publication
   des changements dans le dépôt. Les états W-169/W-170 restent `building`.

Le volet VS Code utilise le renderer synchrone existant : un très grand
diagramme peut occuper le processus d'extension pendant son rendu. Aucun
zoom, contrôle d'export ou worker n'a été ajouté. Les hôtes exécutés sont
Linux ; l'ouverture native macOS/Windows et la variante de tâche PowerShell
documentée ne sont pas vérifiées sur ces systèmes.
