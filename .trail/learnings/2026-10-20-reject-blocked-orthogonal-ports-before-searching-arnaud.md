---
title: Reject blocked orthogonal ports before searching the complete grid
author: Arnaud
date: 2026-10-20
branch: main
touches:
  - AGENTS.md
  - docs/INDEX.md
  - docs/goldens.txt
  - docs/png-goldens.txt
  - docs/reference.md
  - docs/release-notes/1.0.0.md
  - examples/bpmn.svg
  - examples/network.svg
  - opencode.json
  - src/charts.rs
---

## Decided
- Before constructing an orthogonal path grid, test both exact endpoints with the same enters predicate, EPS and clearance as the search. An endpoint inside an inflated obstacle has no traversable incident step. A point exactly on the clearance boundary remains reachable. Keep the existing visible fallback and diagnostics.
- The Audiospy BPMN context case had a goal 0.08 units inside an explicitly offset caption's clearance. Its impossible search repeatedly built a grid of about 29,000 points. The six-line guard reduced paired release CPU medians from 0.735810s to 0.079620s (five alternating runs); all 25 saved reproductions retained identical SVG, JSON and diagnostics. Evidence: .agent/analysis/w168-performance-final-20261020.json and w168-observations-20261020.json.

## Tried and ruled out
- Restricting used-run channels to the local window gave only about 20% improvement and was removed. Instrumentation showed 31 local searches cost about 0.0013s total, while complete searches cost about 0.415s, dominated by this blocked goal. Do not refactor the general search to solve a precondition that can be checked exactly.

## Watch out
- The standard small orthogonal benchmark did not catch this process-shaped input. Measure a real context case too, freeze executables, alternate before/after runs, and record CPU time as well as wall time on a busy machine.
- This optimization changes neither candidate channels nor overlap prices. Tests for endpoint limits, explicit ports/waypoints with more than 60 obstacles, and impossible-route warnings remain required. Production change: src/ortho.rs.
