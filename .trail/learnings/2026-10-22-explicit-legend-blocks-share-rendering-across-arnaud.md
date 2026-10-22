---
title: Explicit legend blocks share rendering across all archetypes
author: Arnaud
date: 2026-10-22
branch: main
touches:
  - docs/language.md
  - docs/reference.md
  - editors/zed/extension.toml
  - editors/zed/languages/bgraph/highlights.scm
  - editors/zed/languages/bgraph/outline.scm
  - src/ast.rs
  - src/check.rs
  - src/compose.rs
  - src/docgen.rs
  - src/layout.rs
---

## Decided
- The user requested a descriptive block like graph: legend { title: "Status"; entry "Available" .live; entry "Planned" { fill: #fef3c7 } }. Keep entry contextual so existing node/group identifiers and treeview items remain valid.
- LegendDecl and LegendEntry have dedicated AST records. Properties stay raw; LEGEND_PROPS and LEGEND_ENTRY_PROPS feed both diagnostics and editor suggestions. Entries override the legacy class-derived list; a legend with no explicit entries still uses styles with legend text.
- All archetypes pass through the same legend renderer. An omitted position places the key below solved content, including shifted labels; absolute positions work everywhere. Relative node/group anchors remain declared-layout-only because managed charts do not consistently retain named entity geometry. Native chart keys stay independent.

## Tried and ruled out
- Merely extending the old legend loop misses all managed charts: their branch returns before declared placement. Move the shared decoration into both exits, after BPMN caption refinement.
- Starting legend typography from node_defaults unchanged makes dark-background text dark; clear the base ink and use the theme fallback while preserving explicit style overrides.
- Registering entry as a global keyword invalidates previously legal identifiers and interferes with editor references; keep its completion and hover inside the legend block.

## Watch out
- Composition must qualify entry classes and shift/pin their spans as well as the enclosing legend. Both grammars and Zed queries must follow the new legend_block and legend_entry nodes.
- Legend entries use the class cascade without the implicit node theme, otherwise diagram node paint can override the key. Only swatch-relevant fields are copied; classes may carry other node settings.
- Existing placementless-legend warning tests describe the old contract; automatic placement replaces that warning. The generated reference comes from docgen, and Zed's existing grammar revision is deliberately not advanced until the generated parser is committed and installation is requested.
