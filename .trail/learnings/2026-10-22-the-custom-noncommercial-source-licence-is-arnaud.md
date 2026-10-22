---
title: The custom noncommercial source licence is English and replaces PolyForm
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - ".github/workflows/release.yml"
  - AGENTS.md
  - Cargo.toml
  - LICENSE
  - README.md
  - docs/INDEX.md
  - docs/guide/commandes.md
  - docs/guide/graphiques.md
  - docs/guide/index.md
  - docs/guide/styles.md
---

## Decided
On 2026-10-22 the user first proposed PolyForm Noncommercial for future public source availability, then explicitly replaced that proposal with their own six-section Non-Commercial Source License, version 0.1. The user then required the licence to be in English. LICENSE is a faithful English translation, names the existing holder Bunchyearth23, and is identified as LicenseRef-bgraph-noncommercial-source-0.1. D-056 supersedes the former confidential-source/no-modifications policy and the internal-only source policy of D-041. The README remains English and presents bgraph as source-available.

## Tried and ruled out
Do not restore PolyForm or imply this is a standard SPDX licence. The user's terms allow source study, noncommercial compilation/execution and private or internal noncommercial modifications; they restrict redistribution and commercial use without prior written authorization, with the stated exception for hosting-platform technical copies/forks. Do not infer general redistribution rights from public repository visibility. Keep third-party component licences intact.

## Watch out
The licence requires a separate agreement for accepted contributions, including the rights necessary for commercial use and distribution. No such agreement has been supplied or drafted by this task. License metadata spans the core crate, editor packages, Typst, playground and the project-owned Tree-sitter grammar. Tracked stage package copies must match the root licence too; the coherence test covers them. The release playground archive now carries LICENSE. Old release notes and historical journal entries retain their old terms as history. Changing repository visibility, publishing an artifact or enabling another registry is a separate action; none was performed here.
