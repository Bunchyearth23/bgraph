---
title: Parallel audit remediation and output boundaries
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
  - editors/README.md
  - editors/vscode/package.sh
  - editors/vscode/src/extension.js
---

## Decided
- The 2026-10-22 full audit was followed by six authorized remediation phases run in parallel, without committing, installing or publishing. The editable audit Page preserves initial findings and records implementation evidence.
- Complete template rendering uses fill_checked, including per-row required fields and rejecting nested Rows. Partial fill remains available for editing and --emit. Filled sources enter the normal composition/theme/format path.
- Output writes protect source/dependency aliases, write a sibling temporary, preserve existing permissions before content, then replace the destination. Directory preview builds use the same safeguards. Export bundles stage the whole build and preserve/restore prior recognized exports on failure.
- SourceMap tracks attempted dependencies, including CSV and missing includes. Watch fingerprints the bound source and actual dependencies. LSP diagnostics recompose open parents against dirty includes; URI conversion is centralized.
- Component expansion transfers relative notes, legends and waypoints with spans/classes. Nested declarations and absolute component annotations that cannot retain their semantics are rejected explicitly.
- PDF export returns an error for absent characters, invalid page geometry and excessive sheet counts. Gitgraph uses the portable text tag: instead of an unsupported decorative glyph. Embedded group-title plates use font advances; SVG title bounds use those advances when the explicit family matches.

## Tried and ruled out
- A single global probe value cannot validate a template containing both identifiers and coordinates. Probe per occurrence and include earlier substitutions in the statement context, including class and include-as positions.
- Embedding alone does not fix text measurement. The new limited cmap/hmtx measurement changes title decorations/canvas, never solved node placement; X-005 remains open for node sizing, wrapping, shaping, kerning and installed fonts.
- Basic script tests exposed a pipefail race: grep -q can close rustup output early and turn an installed target into exit 141. Full-consuming grep -x redirected to /dev/null preserves the producer status.
- trail check reported 17 lexical matches against earlier learnings. They are not semantic reversions: private preview paths are still data, exported VSIX uses supplied WASM, and measured title decoration explicitly addresses rather than denies the earlier overflow. No prior learning was edited.

## Watch out
- Native Windows/macOS execution, actual editor hosts and screen-reader interaction are not established by the portable URI, JS and local runtime tests. A Windows-only output test exists but was not run on Windows.
- LSP refresh recomputes all open documents; large-workspace performance is unmeasured. Hover/navigation include reads retain their existing disk behavior.
- SIGKILL or power loss between bundle-directory renames can leave the previous export in a sibling backup. It is retained, not deleted before the new export is placed.
- Do not mark parent W-166 through W-172 landed or publish a release as a consequence of these fixes. The user asked to run the audit phases, not to change their recorded work states or distribution channels.
