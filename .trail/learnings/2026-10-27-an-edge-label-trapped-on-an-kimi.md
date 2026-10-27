---
title: An edge label trapped on an elbow walks along its own route
author: kimi
date: 2026-10-27
branch: main
id: l-x034elbow
scope: repo
status: active
touches:
  - src/render_svg.rs
  - src/tests.rs
  - src/examples.rs
---

## Decided

When the edge-label avoider's least-bad candidate still clips a node, it no
longer settles: it walks the edge's **other segments** and takes the cheapest
candidate that clears every box, preferring one on the wire itself (`X-034`).
The search stays on the route — a label with no free stretch anywhere on its
line is reported by `check`, not teleported off into free space it cannot be
read from.

The trap was the anchor, not the cost function: `label_anchor` takes the
midpoint of the middle *segment by index*, and spread edges (`D-033`) land a
short elbow jog exactly there — ten pixels pressed against the target box,
where every perpendicular step either clips the box or crosses the frame
boundary. Widening the gap changed nothing because the room was never near
the jog; it was on the long run of the same wire.

## Tried and ruled out

- Ranking node overlap as a first sort key in the *primary* grid: it picks a
  frame-crossing candidate over a node-clipping one, and the better answer
  (on the wire, further along) lives outside that grid anyway.
- Widening the perpendicular stride until a label escapes in any direction:
  a label that flees a hundred pixels off its line is the failure `W-022`
  already fixed once — distance is a cost because attribution is the point.
- Fixing `label_anchor` to prefer the longest segment: right instinct, wrong
  blast radius — it moves every multi-segment label in every golden for the
  sake of the one case in thirty that traps.

## Watch out

- Geometry code may not call `.powi()` — `D-045` pins portable math, and
  `geometry_calls_the_portable_kernels_instead_of_platform_math` greps for
  it. Use `crate::math::square` and std `sqrt`.
- Removing the `network` example's `label-offset` workaround touched
  `src/examples.rs`, `examples/network.bd`, `examples/network.svg`,
  `docs/goldens.txt` and the generated `docs/reference.md` together — the
  drift test is what names the last one.
- The regression test
  (`an_edge_label_trapped_on_an_elbow_walks_along_its_own_route`) needed the
  full trap — spread edges, margins, the frame and the edge font. A two-edge
  miniature routes diagonally and never elbows, so it passes with and without
  the fix; a guard that passes against the old code is not a guard.
