---
title: A convention fixes some widths; a fudge factor does not
author: mavis
date: 2026-10-28
branch: main
id: l-d6bb114e
scope: repo
status: active
touches:
  - src/layout.rs
  - src/tests.rs
---

## Decided

A character outside printable ASCII is charged what a **convention** fixes, and
the flat guess for the rest (`D-060`). An ideograph, a kana, a Hangul syllable
and the full-width forms are one em square by the design of the script; the em
dash is one em wide because that is what the name says. The flat 0.56 was not
a better guess but a worse one: at 32px it left `日本語のラベル` 70.56px outside
its box, and it under-charged the em dash in the project's own gallery.

## Tried and ruled out

- Charging every emoji one em: a joined sequence is **one glyph** whatever the
  characters in it are, and the colour-emoji faces advance their own way. The
  per-character guess cannot be right for both a single emoji and a family.
- A factor of 0.8 for arrows, minus, ticks and bullets: a bullet is a third of
  an em in Helvetica, and a fudge wearing the clothes of a measurement is
  worse than admitting the guess.
- Dropping the em dash because it "is Latin": it is one em in Helvetica, in
  Noto Sans and in DejaVu Sans alike, and the gallery shipped two of them.

## Watch out

- `D-026` is untouched: a face the reader does not have is still not a reason
  to measure a character differently. The cross-family error stands.
- The subset test that fell back to the estimate now compares the Latin parts
  only: the two sides must differ by exactly the Latin difference and nothing
  else.
- Two gallery drawings moved because of the em dash, and the goldens followed.
  A machine with no CJK font installed draws nothing for an ideograph whatever
  the estimate says — the PNG cannot be the proof there, the geometry is.
