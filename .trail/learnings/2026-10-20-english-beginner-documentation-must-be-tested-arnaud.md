---
title: English beginner documentation must be tested as a real workflow
author: Arnaud
date: 2026-10-20
branch: main
touches:
  - AGENTS.md
  - README.md
  - docs.sh
  - docs/INDEX.md
  - docs/TASKLIST.md
  - docs/WORKLIST.md
  - docs/btyp/btyp.typ
  - docs/guide/commandes.md
  - docs/guide/composition.md
  - docs/guide/demarrage.md
---

## Decided
The user requires the project documentation in English and usable by someone who has never seen bgraph. Keep the existing guide URLs while making their content, navigation, search, gallery and launcher help English. Start with installation, source/output terminology and a complete first diagram with expected results, then editor previews, exports and troubleshooting. D-055 records this language policy.

## Tried and ruled out
A successful manual build does not prove printed code examples execute. The old printable manual still taught removed inline placements, had an incomplete orgchart and a misplaced bytefield style class. The reference outline was also stale: edit src/docgen.rs and regenerate it through Cargo. Historical release notes keep an explicit notice rather than pretending old syntax is current. Run complete examples with the CLI and replay multifile examples from their stated files.

## Watch out
A namespaced diagram that itself includes a shared palette can report an unknown original class. The verified workaround is to include that palette at the root as well; the English composition guide and troubleshooting show it, and X-032 tracks the remaining resolver issue. Do not promise a native Zed pane or unsaved-file browser updates. The final evidence is .agent/analysis/documentation-english-20261020.md: 25 complete examples, seven composition commands, English search, 30 gallery sources, 1124 local targets and a visually checked 22-page manual. Native Windows/macOS installers were not run.
