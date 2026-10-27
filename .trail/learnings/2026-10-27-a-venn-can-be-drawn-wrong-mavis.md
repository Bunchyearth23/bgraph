---
title: A Venn can be drawn wrong and still pass check
author: mavis
date: 2026-10-27
branch: main
id: l-f1764f6d
scope: repo
status: active
touches:
  - src/charts.rs
  - src/check.rs
---

## Decided

`check` now refuses Venn pair totals that cannot all be true: inclusion-exclusion
gives `|A∩B| ≥ |A∩C| + |B∩C| − |C|`, so a third set that is in two of them means
those two cannot be disjoint. It is an **error**, not a warning — the schematic
fallback drew a lens of ~100 people beside a key that said "A and B = 0", and a
picture that contradicts its own data is `D-005` applied to the numbers.

Separately, the letter of a set could sit **inside another circle's stroke**
(29.9 px) because the exclusive-region search ran on a 33×33 grid and stepped
over the crescent it was looking for. The search now finds the exclusive region
when one exists.

## Tried and ruled out

- Warning instead of error: the draw would still contradict the key, and a
  warning is what a reader ignores.
- Keeping the old grid: it is the resolution, not the rule, that misses.

## Watch out

- One degenerate test source was itself contradictory under the new rule
  (`a->b 99`, `a->c 99`, `b->c 1`); it was replaced, not weakened.
- The Venn letter moved, so `examples/venn.svg` and `docs/goldens.txt` changed
  together. A golden is a picture of a decision, not a picture of a file.
