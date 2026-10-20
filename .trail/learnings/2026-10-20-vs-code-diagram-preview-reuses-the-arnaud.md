---
title: VS Code diagram preview reuses the bundled renderer and packaging must rebuild it
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
The .bd preview in editors/vscode reuses the bundled synchronous WASM renderer already used for Markdown. Register commands and lifecycle listeners before requiring the optional language client, so the no-server package can still preview. Read TextDocument.getText() for unsaved and untitled buffers, keep one panel per URI, debounce edits, and release the WASM input allocation in finally. File composition remains an explicit watch-server fallback.

## Tried and ruled out
The former packaging freshness check watched src/lib.rs only and could silently bundle an old renderer after changes elsewhere. A full repository runs the incremental wasm build when its target is installed; a standalone export must instead package its supplied module. The runtime gate stages such an export, so requiring a missing repository build script there breaks an otherwise valid package. Packaging tests exercise that real exported directory and the missing-module failure without overwriting an existing VSIX.

## Watch out
Keep JS behavior tests using the real WASM and no language client, plus the separate real VS Code host scenario. A webview tab can be open before its iframe finishes painting: the first 250 ms screenshot captured the previous frame, while a later screenshot showed both updated nodes. No worker, filesystem support, or pan/zoom was added. The integrated evidence is .agent/analysis/editor-previews-20261020.md; installer/profile changes were not part of this work.
