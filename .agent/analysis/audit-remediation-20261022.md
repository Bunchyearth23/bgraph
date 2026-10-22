# Suivi des corrections — 22 octobre 2026

Les six étapes proposées ont été exécutées en parallèle après votre accord. **Dix-neuf points de l’audit ont reçu un correctif local et des vérifications ; le point 20 a été amélioré sur les titres, mais reste partiellement ouvert.** Les constats historiques ci-dessous décrivent l’état avant ces changements. Cette section décrit le candidat actuel, encore non commité et non publié.

## Ce qui a changé

| Point | État du candidat | Correction et preuve principale |
|---|---|---|
| 1 — Dossier d’export | Corrigé | La distribution est construite dans un dossier temporaire. Les destinations dangereuses, liens et dossiers inconnus sont refusés. Un ancien export reconnu n’est remplacé qu’après succès ; un échec de remplacement restaure la sauvegarde. Tests isolés de succès, panne initiale, panne tardive et restauration. |
| 2 — Source écrasé | Corrigé | Le CLI, le diff et la compilation d’un dossier protègent leurs sources et dépendances. Les fichiers sont écrits à côté de la destination puis remplacés ; les autres liens durs conservent leur contenu. Les permissions existantes sont conservées avant écriture. Tests sur source, include, CSV, liens, permissions privées et sortie précédente intacte après erreur. |
| 3 — Format des modèles | Corrigé | Après substitution complète, un modèle suit la même composition, le même thème, la même incorporation de police et le même choix de format que les autres sources. Les signatures SVG, PDF, PNG et JSON sont contrôlées ; les options incompatibles sont refusées. |
| 4 — Champs JSON manquants | Corrigé | Chaque champ requis est vérifié dans chaque ligne, avec le nom du champ et le numéro de ligne en cas d’erreur. Des listes de lignes imbriquées dans une valeur sont également refusées. Une liste vide reste un cas volontaire. |
| 5 — Contenu des composants | Corrigé avec limites explicites | Les notes et légendes relatives, leurs classes, leurs positions source et les points de passage sont transférés. Les déclarations imbriquées non supportées et les annotations sans ancre relative sont refusées, au lieu de disparaître ou d’apparaître au mauvais endroit. |
| 6 — Références préfixées | Corrigé | La qualification couvre aussi les ancres de notes/légendes et les points de passage ; les diagnostics restent rattachés aux bons fichiers. Le risque historique X-032 sur les styles de certains includes imbriqués reste distinct. |
| 7 — Contexte des trous | Corrigé | La substitution distingue noms, valeurs, coordonnées, ancres, classes et préfixes d’include. La vérification emploie une valeur de sonde adaptée à chaque occurrence. Les régressions sur accolades littérales et noms hostiles sont couvertes. |
| 8 — CSV incohérent | Corrigé | Les deux parcours utilisent le même lecteur : virgules citées, guillemets échappés et cellules multilignes. En-têtes vides ou dupliqués et largeurs de ligne invalides sont refusés. |
| 9 — Aperçus périmés | Corrigé | Les dépendances réellement lues ou tentées sont suivies, y compris CSV, fichiers absents et chemins choisis par un modèle rempli. La compilation d’un dossier observe leur contenu et ne réécrit pas une image identique. |
| 10 — Diagnostics parents | Corrigé | Ouverture, modification, sauvegarde, fermeture et notifications de fichiers provoquent le recalcul des documents ouverts, avec le contenu non sauvegardé des includes. |
| 11 — URI Windows | Corrigé, exécution native à vérifier | Conversion commune des chemins Unix, lecteurs Windows et chemins réseau UNC ; tests portables d’aller-retour, Unicode et échappements invalides. Ces tests ne remplacent pas une session Windows réelle. |
| 12 — Commit de grammaire Zed | Corrigé | Le script repère les modifications de grammaire déjà indexées et limite le commit aux chemins de grammaire. Tests dans des dépôts temporaires avec modifications tierces indexées ; aucun commit dans le vrai dépôt. |
| 13 — Mauvais dossier de build | Corrigé | Les scripts compilent et copient depuis la même destination explicite, y compris avec CARGO_TARGET_DIR. Une course supplémentaire a été trouvée : une détection de cible pouvait produire un faux échec via SIGPIPE. Les contrôles lisent maintenant toute la sortie. Le test déterministe et 225 exécutions répétées du lot livraison passent. |
| 14 — Unicode PDF altéré | Corrigé | Un caractère absent de l’encodage ou de la police embarquée produit une erreur avant écriture, avec conseil de police adaptée. Le symbole décoratif non portable de gitgraph est remplacé par « tag: » ; l’exemple s’exporte sans police additionnelle. |
| 15 — Tailles de page invalides | Corrigé | Dimensions finies, positives et supérieures au chevauchement, chevauchement non négatif et plafond de 10 000 feuilles sont vérifiés. Un dessin tenant exactement sur une page n’en produit plus deux. |
| 16 — Connexion inactive | Corrigé | Lecture et écriture réseau ont un délai limité et la ligne de requête est bornée. Le test ouvre une connexion inactive puis vérifie que la suivante reçoit une réponse. |
| 17 — Saisie remplacée au chargement | Corrigé | La restauration initiale précède le chargement du moteur. L’exemple par défaut n’écrase plus une saisie intervenue pendant ce chargement. |
| 18 — Brouillon perdu | Corrigé | Sauvegarde locale avec état visible et téléchargement du source .bd. Un lien explicite, même vide, est prioritaire sur le brouillon ; refus de stockage et quota préservent la saisie. |
| 19 — Clavier et noms accessibles | Corrigé, contrôle utilisateur à compléter | Boutons et sélecteur nommés ; déplacement par flèches, zoom et ajustement avec Home. L’ajustement tient compte de la taille réelle du diagramme. Tests JavaScript et compilation Svelte ; lecteur d’écran non essayé. |
| 20 — Mesure typographique | Partiellement corrigé | Les cartouches de groupes et les limites du canevas des titres SVG/PNG utilisent les avances de la police embarquée lorsqu’elle est explicitement sélectionnée. Les cartouches PDF utilisent les avances de la police embarquée. Aucun nœud ni cadre résolu n’est déplacé par le rendu. X-005 reste ouvert pour dimensionnement général, retours à la ligne et mise en forme complexe du texte. |

## Vérifications du candidat final

| Contrôle | Résultat observé |
|---|---|
| Suite Rust standard | **796 tests réussis**, 1 test de performance manuel ignoré |
| Suite Rust avec PNG | **805 tests réussis**, le même test manuel ignoré |
| Construction sans fonctions optionnelles | Réussie ; 13 tests ciblés de composition, polices et écriture passent également sans fonctions par défaut |
| Analyse stricte du code et formatage | Clippy sur toutes les cibles/fonctions, rustfmt et contrôle des espaces du diff réussis |
| Cohérence entre interfaces | 22 cas exécutés dans les interfaces disponibles, 30 types et 4 contextes multifichiers ; extensions incluses |
| Playground | 5 tests JavaScript réussis, compilation Svelte/Vite réussie ; tests ajoutés à la CI |
| Extension VS Code | 6 tests JavaScript réussis ; paquet temporaire vérifié par le contrôle d’intégration |
| Livraison | 9 tests isolés réussis et 25 répétitions de ce lot, soit 225 exécutions supplémentaires |
| Documentation | Construction stricte MkDocs réussie |
| Image gitgraph | SVG régénéré, aperçu PNG inspecté, empreinte attendue et galerie vérifiées |

Ces nombres sont des exécutions de tests : les suites standard et PNG se recouvrent. Ils ne désignent pas 1 601 cas indépendants. Les codes de sortie des commandes ont été contrôlés ; les premiers échecs ont conduit aux corrections de modèles, de caractères PDF, de permissions et de scripts avant cette validation finale.

Le contrôle `trail check` a aussi été exécuté : il retourne 17 correspondances lexicales avec d’anciennes notes, examinées comme des faux positifs dans cette passe. Par exemple, la nouvelle mesure des cartouches traite précisément l’ancienne observation selon laquelle incorporer une police ne suffisait pas. Il ne faut donc pas présenter ce contrôle comme « vert ». La justification est conservée dans une nouvelle note de projet, sans modifier les anciennes.

## Ce que ces résultats permettent de conclure

Les quatre défaillances prioritaires ont désormais des protections testées. Les erreurs de données et de caractères cessent de produire un export trompeur ; les échecs de génération conservent les sorties précédentes. Les parcours quotidiens disposent d’une source récupérable et de retours plus à jour.

Les correctifs restent localisés dans les frontières de fichiers, la substitution, la composition et les interfaces existantes. Aucune dépendance n’a été ajoutée. Les tests du playground sont raccordés à la CI, et les nouveaux tests Rust sont découverts par Cargo.

## Limites et suite utile

- **Typographie :** incorporer une police ne relance pas le calcul de disposition. La mesure ajoutée somme les avances horizontales ; elle ne couvre pas le crénage, les ligatures, les scripts complexes, les axes variables ni tous les effets de graisse synthétique. Le contrat est documenté dans docs/API.md.
- **Composants :** notes et légendes doivent utiliser une ancre relative. Les formes imbriquées refusées doivent être déclarées à l’extérieur. C’est une restriction explicite, testée, qui empêche une perte silencieuse.
- **Éditeurs :** la fraîcheur des diagnostics est corrigée ; hover/navigation des includes gardent leur comportement fondé sur le disque. Le recalcul de tous les documents ouverts n’a pas fait l’objet d’un nouveau test de charge.
- **Plateformes et usage réel :** pas de nouvelle session native Windows/macOS, de vrai hôte Zed/VS Code ni de lecteur d’écran. Les tests d’URI et le contrôle du module de l’extension ne prouvent pas ces parcours manuels.
- **Distribution :** les tests simulent les commandes de livraison sans installer ni publier. Une interruption non interceptable entre les renommages peut laisser l’ancien export dans son dossier de sauvegarde ; il n’est pas supprimé avant placement du nouveau.
- **État du dépôt :** les modifications préexistantes ont été conservées. Aucun commit, push, changement d’installation ou publication n’a été effectué. Les états des travaux W-166 à W-172 n’ont pas été clos.

La suite pertinente est de valider ce candidat dans les éditeurs et sur Windows, puis de traiter X-005 comme un travail de mesure typographique explicite. Le rapport initial reste ci-dessous pour conserver les reproductions et leur contexte.


## Evidence files

Editable report: https://chatgpt.com/space/page_24e4faa833608191bed9b99f4e1399ca

Temporary validation logs from this run:

- /tmp/bgraph-fixes-default.log
- /tmp/bgraph-fixes-png-final.log
- /tmp/bgraph-fixes-clippy-final.log
- /tmp/bgraph-fixes-minimal.log and /tmp/bgraph-fixes-minimal-tests.log
- /tmp/bgraph-fixes-runtime-final.log
- /tmp/bgraph-fixes-playground.log, /tmp/bgraph-fixes-vscode.log and /tmp/bgraph-fixes-svelte.log
- /tmp/bgraph-fixes-docs.log, /tmp/bgraph-fixes-fmt.log and /tmp/bgraph-fixes-trailcheck.log

Baseline commit: 79d2d8094bef9e14128ceb9dd7f56f2dded86f0a. No commit made by this task.
