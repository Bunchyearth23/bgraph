---
title: BPMN local frame origins and congested orthogonal channels
author: Arnaud
date: 2026-10-20
branch: main
touches:
  - AGENTS.md
  - docs/INDEX.md
  - docs/reference.md
  - docs/release-notes/1.0.0.md
  - opencode.json
  - src/charts.rs
  - src/check.rs
  - src/constraint_layout.rs
  - src/constraint_layout/frames.rs
  - src/explain.rs
---

## Decided
- BPMN frame content has a local origin even when the frame has no explicit placement. Post-layout absolute placement checks must not compare those coordinates to the global canvas.
- The BPMN origin cannot precede padding/title, but containment may move it inward when a member extends to negative local coordinates (for example a 60px task centered beside a 40px event). Positive explicit offsets remain meaningful, including the 420px nested-lane case.
- Exact-port orthogonal routing must include channels beside previously occupied segments. A valid local route that still overlaps a previous run retries the complete channel grid; unrelated obstacles outside the local window can provide the needed parallel channel.

## Tried and ruled out
- A REQUIRED equality between a BPMN content origin and the frame's padded corner makes an event at local y=0 followed by a taller centered task insoluble.
- Restoring used-run channels alone does not fix the three-route wall/floor reproduction: the useful channel belongs to the floor, outside the local search window. Changing the overlap penalty is unnecessary.

## Watch out
- Keep width/height and placement constraints required. The origin preference must not relax source contradictions or shift geometry after solving.
- Freeze the CLI binary before a multi-case replay while agents build concurrently. Reading target/debug/bgraph throughout a long run can mix old and new binaries.
- Regression coverage lives in src/constraint_layout.rs, src/tests.rs and tests/routing.rs. Production changes touch src/constraint_layout/frames.rs, src/check.rs, src/explain.rs and src/ortho.rs.
