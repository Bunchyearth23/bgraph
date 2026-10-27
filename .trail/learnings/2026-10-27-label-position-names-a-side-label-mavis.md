---
title: "label-position names a side, label-offset names a point"
author: mavis
date: 2026-10-27
branch: main
id: l-86e6ad67
scope: repo
status: active
touches:
  - src/labels.rs
---

## Decided

`label-position` pins the **side** a BPMN event or gateway caption belongs on;
`label-offset` is what pins the **point**. A caption whose ink lies across a
wire is a defect, so the engine may slide it along its declared side within a
bounded range. A declared `label-offset` is never overridden, zero included.

## Tried and ruled out

- Reading a declared side as the author's whole declaration: the generated
  reference says the property "pins the side" and that `label-offset` "then
  adjusts that position", so the two are different things.
- Moving a caption off its declared side to dodge a wire: that overrides the
  declaration, which is the thing `D-005` refuses.

## Watch out

- The opposite reading was argued by a parallel replay of the same fixtures and
  is revertible by dropping the side filter and the slide. The disagreement was
  real; the reference settled it.
- The cost function must separate the two cases: a caption whose *box* grazes a
  box is a warning, a caption whose *ink* crosses a wire is a defect. Treating
  them the same makes the avoidance either timid or reckless.
