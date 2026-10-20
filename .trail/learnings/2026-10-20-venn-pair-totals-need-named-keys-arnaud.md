---
title: Venn pair totals need named keys and data-aware circle geometry
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
  - docs/goldens.txt
  - docs/guide/commandes.md
  - docs/guide/composition.md
---

## Decided
Venn circles retain area-proportional set totals. Two-set lens areas remain exact. For three sets, derive each pair distance with the existing lens bisection and use a triangle when those distances can form one; coincident centers have a separate finite path. Incompatible distances retain the schematic fallback and the visible indicative-area warning. Omitted links mean zero, as they already did for two sets.

## Tried and ruled out
An equilateral arrangement ignores all three pair counts. A bare pair count placed between circle centers can land in the triple region, outside a small lens, or imply an exclusive pair region. The input never supplies a triple count. Keep exact pair totals in a named key, with A/B/C matching colored set keys and in-circle letters where they fit. Do not infer exclusive or triple counts.

## Watch out
Long names wrap in the key, and each row reserves its own font height. Columns need space for both names and counts at their resolved font sizes. Circle letters maximize clearance from outlines and other letters; a tiny or empty circle keeps its identifier only in the key. Update the generated reference through props/docgen, regenerate the Venn gallery pair from init, inspect the image, and update only its golden hash. Evidence: .agent/analysis/venn-validation-20261020.md. The installed user CLI/extensions are separate from local rebuilt artifacts.
