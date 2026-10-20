---
title: Learning checker vocabulary matches need semantic review
author: Arnaud
date: 2026-10-20
branch: main
touches:
  - AGENTS.md
  - docs/INDEX.md
  - docs/goldens.txt
  - docs/guide/commandes.md
  - docs/guide/editeurs.md
  - docs/png-goldens.txt
  - docs/reference.md
  - docs/release-notes/1.0.0.md
  - editors/README.md
  - editors/vscode/README.md
---

## Decided
The final editor preview review is recorded in .agent/analysis/editor-previews-20261020.md. The learned alternatives remain enforced by real host and packaging tests. A nonzero memory-check result must be reported with its actual reason, separately from test results.

## Tried and ruled out
Do not infer a semantic regression from shared words alone. The check returned twenty matches in guides, manifests, tests and implementation spans that obey the corresponding learned constraint. Renaming valid product terms or weakening code to satisfy those matches would hide the issue rather than fix anything.

## Watch out
The source review confirms quoted environment expansion for Zed filenames, separate repository/export packaging, a required lower bound with a strong frame-origin equality preference, and complete used-run data plus a global routing fallback. No commit or pull request was requested; carry the documented rationale into one when requested. The checker remains nonzero and is not counted among passing validations.
