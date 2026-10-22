---
title: "Legend documentation must reach installed docs, the site and the printable manual"
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - README.md
  - docs/INDEX.md
  - docs/guide/commandes.md
  - docs/guide/graphiques.md
  - docs/guide/index.md
  - docs/guide/styles.md
  - docs/language.md
  - docs/manual.typ
  - docs/reference.md
  - editors/zed/extension.toml
---

## Decided
Document legend titles as title: inside the legend block, with entry strings as row descriptions. The beginner route is docs/guide/styles.md#add-a-legend, linked from the home page and diagram-type guide. Canonical syntax is in docs/language.md; the generated manual belongs in src/docgen.rs, then cargo run --locked --bin bgraph -- docs -o docs/reference.md. The installed CLI embeds that generator too: rebuilding the site alone does not update bgraph docs in PATH.

## Tried and ruled out
A broad standalone snippet replay reaches canonical examples requiring CSV or included files, so it needs those documented fixtures. For this documentation change, run all complete README/guide examples and the changed canonical legend examples, then the existing documentation tests. A strict MkDocs build plus local-link/anchor checks verifies site assembly; inspect the actual legend page in a browser as well.

## Watch out
The printable manual reads docs/diagrams/*.bd through the Typst plugin, which must be rebuilt after renderer changes. Its test requires one read("diagrams/...") per source file. When printing source and rendering its image side by side, bind that read once and reuse the variable. The legend page now does this; duplicating the read fails the existing manual test. A new PDF must be inspected after compilation. Keep the user-facing documentation English (D-055) and do not change work-item states during documentation refreshes.
