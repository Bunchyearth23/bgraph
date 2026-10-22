---
title: Legend checks preserve geometry constraints despite lexical Trail conflicts
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - docs/INDEX.md
  - docs/language.md
  - docs/reference.md
  - editors/zed/extension.toml
  - editors/zed/languages/bgraph/highlights.scm
  - editors/zed/languages/bgraph/outline.scm
  - src/ast.rs
  - src/check.rs
  - src/compose.rs
  - src/docgen.rs
---

## Decided
- The 2026-10-22 legend implementation check reported ten conflicts; each is an unrelated lexical match, not a rejected design constraint. No commit or PR was created. docs/INDEX.md records this distinction rather than claiming the check passed.
- Legend placement uses solved geometry and pure moved-label measurements; the source node/frame coordinates remain unchanged across all 30 worked examples. No exact-font-layout claim or rounded-bend fix was made.
- Numeric font-weight values must reach Style.font_weight as text, just like word weights. The shared property interpreter now preserves 600 instead of discarding it; the explicit legend cascade test checks the emitted annotation weight.

## Tried and ruled out
- Trail matched words such as original/without against the old bend investigation, title/node/placement against limited font measurement, and test/preserve/api against a VS Code test harness learning. None of those mechanisms is replaced or reverted here.
- It also matched legacy class-key handling against Venn pair totals and generated parser tokens against include placeholder inference. Venn geometry/data keys and template slot inference remain unchanged.

## Watch out
- Keep Trail's nonzero result visible in validation records. Review semantic changes rather than rewriting correct code to evade vocabulary matching.
- Desktop plugin installation and advancing Zed's grammar revision require the generated parser in a commit; this task leaves both uninstalled and preserves the pre-existing manifest edit.
