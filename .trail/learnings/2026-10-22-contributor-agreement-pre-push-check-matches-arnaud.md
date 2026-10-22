---
title: Contributor agreement pre-push check matches wording rather than policy conflicts
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - AGENTS.md
  - CONTRIBUTOR_AGREEMENT.md
  - README.md
  - docs/INDEX.md
---

## Decided
The English contributor agreement and README link are ready to commit and push at the user's explicit request. The agreement preserves contributor ownership, requires explicit acceptance for identified revisions and grants the maintainer separate commercial and distribution rights. The current project LICENSE is unchanged.

## Tried and ruled out
The pre-commit trail check reported 16 lexical matches. Reviewing the full staged documents shows no policy reversal: pull requests and sign-offs alone are explicitly insufficient acceptance; the acceptance record identifies agreement and contribution revisions; the INDEX reference to PolyForm records its replacement, not its restoration. Other matches concern unrelated compiler layout, editor packaging, fonts and probe tests, none of which this documentation change modifies. Rewriting valid legal prose merely to avoid shared words would not fix a defect.

## Watch out
The commit message records why these checker findings are false positives. This is a documentation-only change; local links and document structure were checked previously and staged whitespace checks pass. No runtime test rerun or legal validation is implied by committing and pushing the draft. Future checks still require reviewing each finding on its substance.
