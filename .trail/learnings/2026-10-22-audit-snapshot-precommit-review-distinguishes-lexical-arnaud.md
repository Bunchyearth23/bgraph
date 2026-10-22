---
title: Audit snapshot precommit review distinguishes lexical warnings from regressions
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - ".github/workflows/ci.yml"
  - AGENTS.md
  - docs/API.md
  - docs/INDEX.md
  - docs/goldens.txt
  - docs/guide/commandes.md
  - docs/guide/composition.md
  - docs/guide/editeurs.md
  - editors/README.md
  - editors/vscode/README.md
---

## Decided
The user authorized committing and pushing the entire current bgraph worktree on 2026-10-22, including audit fixes, editor improvements, local tool configuration migration and team learnings. Parent work states stay unchanged; this is not an installation or release decision.

## Tried and ruled out
trail check exits 1 on twenty phrase overlaps in this snapshot. Reviewing the matching lines shows documentation of known font limitations, recorded test counts, actual measured editor geometry, VS Code file selectors, and an assertion that webview HTML stays unchanged. Those do not reinstate the old font-measurement claim, unsafe Zed shell interpolation or per-edit HTML replacement. The commit message records this rationale; no prior learning was edited to suppress matches.

## Watch out
Keep the distinction between passing local tests and remote CI. Linux editor hosts are verified; native Windows/macOS and screen readers are not. X-005 and the documented nested-style include limitation remain open work even when the fixes are committed.
