---
title: Nested frame readiness and font embedding limits in 1.0.1
author: Arnaud
date: 2026-10-21
branch: main
touches:
  - src/placement.rs
  - src/layout.rs
  - src/check.rs
  - src/constraint_layout.rs
  - src/render_svg.rs
  - src/render_pdf.rs
  - src/render_json.rs
  - src/docgen.rs
  - tests/patch_101.rs
  - docs/INDEX.md
---

## Decided
- A frame anchored to a sibling within the same placed ancestor needs that sibling's local coordinates, not its final translated position. Keep nested frames unsettled until their placed ancestor is settled, so external anchors still cannot depend on unfinished translations. The exact n/m/inner/outer false cycle is covered in tests/patch_101.rs; true cycle tests remain necessary.
- Paint sequence frames before message polylines in SVG and PDF. Empty edge labels never enter shared label placement. Cylinder text occupies the body below two top-cap radii and above one bottom-cap radius; block annotations need the same half-cap sink as ordinary labels.
- JSON canvas.layout_origin is an additive version-1 field. Subtract it to recover solved global coordinates; it does not undo group-local or bounded-grid conversions. Relative node/group via stops already existed and are documented.

## Tried and ruled out
- A negative frame x is not evidence of clipping: the reported sequence frame at -175.04 is translated by +211.04 and starts visibly at 36. Canvas bounds already include it.
- Restoring bend: 12 in the supplied context and sampling a 3x PNG with multiple rounded waypoints and an empty label did not reproduce the reported corner gaps. Do not claim a bend fix without the original case.
- Embedding a matching DejaVuSans family does not fix short title plates. The 246.95-unit plate still trails roughly 275.67 units of regular glyph advances. Embedding happens after solving; the advance table does not read the selected family or weight. X-005 is reopened, and the previous exact-measured-font claim is corrected.

## Watch out
- Larger automatic cylinders move solved geometry and can invalidate hand-tuned absolute waypoints. The unchanged architecture context gains one label-over-node warning; its exports still succeed. Do not silently change the user's diagram to hide this.
- font with two numeric sizes is warned about without changing its existing interpretation. Group dimension errors attribute equal-value style overrides to the last declaring layer.
- 760 Rust tests, six JS tests, strict docs, minimal/WASM/Typst/Zed builds and the complete runtime/extension gate passed. Five gallery SVGs were inspected and repinned. No publication or installation was performed.
