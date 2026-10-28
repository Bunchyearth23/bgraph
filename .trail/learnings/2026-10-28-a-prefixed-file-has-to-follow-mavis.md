---
title: A prefixed file has to follow a style that came from its own include
author: mavis
date: 2026-10-28
branch: main
id: l-41d34487
scope: repo
status: active
touches:
  - src/compose.rs
---

## Decided

`compose::qualify` now takes the styles the included files brought in, and a
class reference follows a style into them. A file included as `backend` writes
`.service`; the style came from the `house.bds` that file includes, so the
compiler looks for `backend.service`, which is where the style is. The
documented workaround — including the shared file at the root as well — is
gone, because it would now leave the top-level copy of the style unused.

## Tried and ruled out

- Qualifying the class with the prefix: the style is already qualified by the
  time the parent looks, so the parent has to be told which name means what,
  not what the name would be.
- Leaving the class alone when the file does not declare the style: that rule
  is right for a *forward* reference into the including file, and wrong for a
  style that came in through this file's own include. The two were the same
  case until now.

## Watch out

- The map travels upward one level at a time: a file reports the styles *it*
  declared, and the parent collects the reports of its own children.
- A component body is a text, not a file, so it cannot include anything and
  has no nested style to follow.
- The regression test fails with `unknown style .service` when the inherited
  branch is removed, which is the shape the bug had.
