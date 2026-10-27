# X-005 — estimated advances can leave text outside its intended bounds

Work item: the only open risk in `docs/INDEX.md` (`X-005`), reopened by `W-172`.
Date: 2026-10-27. Scope of this file: **analysis and one contained change** in
`src/subset.rs`. Everything else is handed over as a diff or a plan.

---

## 1. How text is measured today, from the code

### The estimate

| What | Where |
|---|---|
| The advance table — 95 printable ASCII characters, in thousandths of an em | `src/layout.rs:232` |
| `advance(text, font_size)` — ASCII from the table, **everything else a flat `0.56` em** | `src/layout.rs:251`, the flat guess at `src/layout.rs:258` |
| `text_width(lines, size)` — the widest line | `src/layout.rs:267` |
| A node's box: `text_width + 2 × padding` (padding defaults to 14, `src/style.rs:402`) | `src/layout.rs:371-377` |
| Wrapping: the same `advance` decides where a line breaks | `src/layout.rs:498`, `:535`, `:544` |
| Legends, edge-label clearance, frame labels | `src/layout.rs:1689`, `:1406`, `src/constraint_layout/frames.rs:226` |
| Caption and plate rectangles | `src/labels.rs:6`, `:34`, `:68` |
| Every chart archetype's labels | `src/charts.rs:621`, `:840`, `:1382`, and twenty more |
| The validator's "the text does not fit" check — it compares the estimate with the estimate | `src/check.rs:446` |

So **one table decides every box, plate, wrap point and diagnostic** in the
project. Nothing else measures text.

### The embedded-font path

| What | Where |
|---|---|
| `subset::text_advance` — the face's own `hmtx` advances, scaled by `unitsPerEm` | `src/subset.rs:137` (was `:43`) |
| `subset::Face` — the parsed face (new; see §4) | `src/subset.rs:48` |
| The only caller: `render_svg::embedded_width`, used for **group-title plates only** | `src/render_svg.rs:1004`, used at `:981` and `:1198` |
| `--embed-font` names the family after the **font file's stem** | `src/main.rs:585-600` |
| The embedded metrics are used **only when the diagram's primary family equals that stem** | `src/render_svg.rs:1006-1009` |
| PDF: the embedded face measures the text and the group-title plate | `src/render_pdf.rs:111-115`, `:149-155`, `:399-426` |
| PDF: with no embedded font, the base-14 fonts `/F1` Helvetica and `/F2` Helvetica-Bold | `src/render_pdf.rs:889-917`, `:1066-1073` |
| PDF: with no embedded font, the width is `layout::advance` — the estimate | `src/render_pdf.rs:902` |
| JSON publishes the estimate-based geometry | `src/render_json.rs:18` (`D-028`: coordinates are not promised) |
| PNG renders the SVG, so it inherits the SVG's text | `src/render_png.rs:35`, `:114` |

**Consequence:** the embedded face measures *group titles* and nothing else.
Node boxes, edge-label plates, captions, chart labels and the JSON export are
all sized by the Helvetica table, whatever the diagram embeds.

### `D-027` — "the advance table is *exact* for PDF"

**Confirmed for the base-14 regular path, refuted for the rest.**

* The table **is** Adobe Helvetica: compared against URW's NimbusSans-Regular
  (a metric-compatible clone) with fontTools, **0 of the 95 characters differ**.
* PDF `/F1` is Helvetica, so for regular-weight Latin the rendered text width
  equals the measured width exactly (0.00 px error).
* **But `/F2` is Helvetica-Bold**, and the table is the *regular* one. Measured
  against NimbusSans-Bold: `illlllllllllllll` at 32px is 71.17px in the face
  and 56.83px in the table (**+14.34px, +25.2%**); `Auth gateway` at 32px is
  208.03 vs 193.89 (**+14.14px, +7.3%**). The default node weight is `500`
  (`src/style.rs:402`), and the PDF writer picks `/F2` for any weight ≥ 600
  (`src/render_pdf.rs:895-896`) — and the SVG asks the viewer for `font-weight:
  500` on every node (`src/style.rs:405`).
* With an embedded font the PDF draws the **real** advances into a box the
  **estimate** sized, so the PDF inherits the SVG's error.

So `D-027` holds for one case out of three.

---

## 2. The risk, measured

All numbers below are mine, produced with fontTools 4.67 against
`tests/fixtures/fonts/DejaVuSans.ttf` (the project's own fixture),
`/usr/share/fonts/noto/NotoSans-{Regular,Bold}.ttf`, and
`/usr/share/fonts/gsfonts/NimbusSans-{Regular,Bold}.otf`, and by rendering with
the built binary and comparing the `<rect>` the renderer drew against the real
width of the text it put in it.

### The estimate against real faces (relative to the Helvetica table)

| Face | `WWW` | `User service` | `Auth gateway` | `illlllllllllllll` |
|---|---|---|---|---|
| DejaVu Sans (fixture) | +4.7% | +11.2% | +14.5% | +25.2% |
| Noto Sans Bold | — | +7.1% | +7.3% | +25.2% |
| Helvetica-Bold (= PDF `/F2`) | +0.0% | +7.1% | +7.3% | +25.2% |

### End to end: the drawn box against the drawn text

`bgraph` output, one node per row, box width from the SVG, text width from the
face that will actually render it:

| Label | Size | Box | Text (DejaVu) | Past the box | Past the text area |
|---|---|---|---|---|---|
| `Auth gateway` | 64 | 415.78 | 444.09 | **+28.31px** | +56.32px |
| `Auth gateway` | 64 (Noto Bold) | 415.78 | 446.34 | **+30.56px** | +58.56px |
| `Auth gateway` | 32 | 221.89 | 222.05 | +0.16px | +28.16px |
| `Auth gateway` | 32 (Noto Bold) | 221.89 | 223.17 | +1.28px | +29.28px |
| `Request — retry` | 32 | 246.88 | 261.25 | **+14.37px** | +42.37px |
| `日本語のラベル` | 32 | 153.44 | 224.00 | **+70.56px** | +98.56px |

* The em dash alone is 1.0 em in Helvetica, Noto Sans and DejaVu; the estimate
  charges 0.56 → **14.08px of the 14.37px** above.
* The ideographs are full-width (1.0 em) in every CJK face; the estimate
  charges 0.56 → the label is measured **44% short**. (No CJK font is
  installed on this machine, so that row uses the full-width convention, not a
  measured face — the only number in this report that is not measured.)
* The **worst overshoot I could produce**: **70.56px past the drawn box**
  (46% of the box) for an ideograph label at 32px; **30.56px** for Latin at
  64px. There is no upper bound on `font-size` (`src/props.rs:439`), so the
  Latin overshoot grows linearly with the size the author asks for.

### The damage is a label on a wire, not a tidy box

An edge label is drawn on a plate sized by the estimate
(`src/render_svg.rs:892-899`, `text_width + 10`). For
`retry after timeout` at 12px the plate is **104.03px** and the text is
104.71px (Noto Regular), 112.05px (DejaVu) or 113.93px (Noto Bold) — so up to
**9.9px of the text sits outside the plate**, over the wire the plate exists
to hide.

### The residual error in the *exact* path

`subset::text_advance` sums unshaped advances. A browser kerns: DejaVu Sans
has 2727 kern pairs, and `To` at 14px is 2.38px **narrower** than the sum of
its advances (`User service` has none). So the embedded measurement is exact
for the advances and off by ≤ ~2.4px at 14px for kerned pairs.

### Why nothing catches it

`src/check.rs:446` compares `text_width(node.lines, font)` with
`node.rect.w` — the estimate against the box the estimate built. It cannot
see this class of error, by construction.

---

## 3. The three questions

### (a) The cheapest change that would materially reduce the risk

**Make the estimate right where it is knowably wrong, and exact where the user
has already paid for exactness.**

1. **A script-aware estimate** (`src/layout.rs`, ~25 lines, Diff A below).
   The flat `0.56` is wrong by a known factor for full-width characters
   (1.0 em) and for the em dash and the ellipsis (1.0 em). This removes the
   worst case I measured — the 70.56px overshoot — and changes **nothing**
   for ASCII, so the gallery and its goldens do not move.
2. **Measure with the embedded face, everywhere** — not only for group titles.
   This is the only route to *zero* error, and the user has already paid for
   it (`--embed-font` embeds the whole face). It needs the face threaded into
   the sizing paths (§5, Diff C).
3. A **bold table** for a declared weight ≥ 500 (Diff B) closes a 4–25% slice
   of the error, but only for families that have a 400/700 pair; a family with
   a real 500 (Inter) would be over-reserved.

The cheapest of these that a decision can be taken on is **1**; it is small,
safe, dependency-free, and it attacks the worst measured case.

### (b) What it costs, and does it break the project's stance

* **Dependencies: none.** Diff A is a table and a `match`. Diff C reuses
  `subset::Face`, which is already in the tree. AGENTS.md's stance — "SVG,
  PDF, the watch server, base64 and the TrueType subsetter are all hand-rolled
  — no dependency for any of them" — is about *not adding a dependency tree*,
  and neither diff adds one.
* **Licence: none.** No font is bundled; the metrics come either from a table
  written by hand (Adobe's Helvetica widths, which are facts about a typeface)
  or from the user's own font file.
* **Effort:** Diff A is ~25 lines and a golden-free change. Diff B is a
  95-value table plus the weight threaded through the sizing paths (the
  wrapping at `src/layout.rs:822` and the chart call sites). Diff C is a
  real refactor: the face has to reach `natural_size`, the wrapping, the
  caption plates, the edge-label plates and the chart labels — roughly the
  30-odd `text_width` call sites, plus the JSON export if it is to stay
  truthful.
* **What I am recommending *against*:** adding a font-metrics crate
  (`ttf-parser`, `fontdue`, `rustybuzz`). It would break the stance, and it
  would not fix the problem: the error is not that the project cannot read a
  font, it is that **the project does not know which font the reader's viewer
  will resolve** (`D-026`). A metrics crate would measure *a* font exactly
  and still be wrong about the viewer's.

### (c) The cost of accepting the risk unchanged

* The text leaves the **bounds the layout reserved for it** in almost every
  diagram drawn in a face other than Helvetica: 5–25% of the text's width, at
  every size (measured: `Auth gateway` at 14px — 97.15px of text in a 84.83px
  text area).
* In the worst cases it leaves the **shape**: +1.28px at 32px, +30.56px at
  64px (Noto Sans Bold), +70.56px for an ideograph label at 32px.
* An **edge label can sit on the wire it labels** — up to 9.9px of text
  outside the plate that exists to hide the wire.
* It is **silent**: `check` compares the estimate with itself.
* It is **cosmetic, not structural**: the boxes are still right, the picture
  still reads, and the padding absorbs most of it. A user would say "the label
  is a bit tight", not "the file is broken".

---

## 4. The change I made: `src/subset.rs` only

**What changed.** `subset::text_advance` used to return `None` if **any**
character in the string had no glyph in the face — so one ideograph in a title
threw away the measurement of every other character, and the caller fell back
to the crude Helvetica estimate for the whole string. Now the face is parsed
once into a `Face` (`src/subset.rs:48`) and measured **per character**: the
face's own advances for the characters it has, and the project's own estimate
for the characters it does not (the reader will draw those from whatever font
it falls back to, which nobody here can know).

Nothing else changed: the same function, the same signature, the same value for
a string the face can measure completely.

**The test** (`src/subset.rs:464`, inside the file, `#[cfg(test)]`):

```rust
#[test]
fn a_character_the_face_lacks_does_not_void_the_measurement_of_the_rest() {
    // DejaVu Sans has no CJK ideograph. One missing character used to
    // throw the whole measurement away, and the caller fell back to the
    // estimate for every character in the string.
    let width = text_advance(DEJAVU, "Deploy 日本", 12.0).expect("the face measures the rest");
    let latin = text_advance(DEJAVU, "Deploy ", 12.0).expect("measured");
    let fallback = crate::layout::advance("日本", 12.0);
    assert!((width - (latin + fallback)).abs() < 1e-9, ...);
    assert!((width - 59.272).abs() < 0.01, "measured {width}");
    let whole = crate::layout::advance("Deploy 日本", 12.0);
    assert!(width > whole, ...);
}
```

**Proven to fail before.** With the old behaviour restored temporarily (the
old `None` contract), the test panics at `src/subset.rs:469` — *"the face
measures the rest"* — and passes with the change. The other three new tests
(`the_face_measures_with_its_own_advances`, `a_face_this_cannot_read_measures_nothing`,
`a_subset_face_measures_the_same_as_the_face_it_came_from`) pin the face's
advances (97.15px for `Auth gateway` at 14px), the refusal to measure a font it
cannot read, and the fact that subsetting does not change a single advance.

**The measured improvement**, end to end with the built binary — a group title
`Deploy 日本` in an embedded DejaVu Sans:

| | plate width | shortfall against the truth (DejaVu's 45.83px + two full-width ideographs = 69.83px) |
|---|---|---|
| before | 60.12px | 9.71px |
| after | **65.27px** | **4.56px** (−53%) |

The Latin part of the plate is now the face's own advance, exactly.

**Scope note.** This is a real but **narrow** improvement: it touches the
embedded path, and only the group titles. It does **not** reduce the risk in
§2, which is the *default* path. I am not claiming otherwise.

---

## 5. What I did not change, and the exact diffs for whoever owns it

### Diff A — `src/layout.rs`: the script-aware estimate (**my recommendation**)

`git apply --check` verified against the current tree: applies cleanly.
Changes nothing for ASCII, so the gallery and `docs/goldens.txt` do not move.

```diff
--- a/src/layout.rs
+++ b/src/layout.rs
@@ -253,14 +253,48 @@
         .chars()
         .map(|c| match c as u32 {
             code @ 0x20..=0x7e => ADVANCE[(code - 0x20) as usize] as f64 / 1000.0,
-            // Anything outside printable ASCII keeps the old flat guess: a
-            // table for one typeface has nothing to say about CJK or emoji.
-            _ => 0.56,
+            // Anything outside printable ASCII is charged what it is worth
+            // in the faces a diagram is drawn in.
+            _ => wide_advance(c),
         })
         .sum();
     em * font_size
 }
 
+/// What a character outside printable ASCII is charged.
+///
+/// The flat guess of 0.56 was less than half of what a full-width character
+/// costs: an ideograph is one em wide in every face that has it, and so are
+/// an em dash and an ellipsis. A label of ideographs was measured 44% short —
+/// at 32px, `日本語のラベル` was given 125.44px and the reader drew it in
+/// 224px, 70.56px past the box it was in.
+fn wide_advance(ch: char) -> f64 {
+    match ch as u32 {
+        // Full-width forms: ideographs, kana, Hangul, full-width punctuation.
+        0x1100..=0x115f
+        | 0x2e80..=0x303e
+        | 0x3041..=0x33ff
+        | 0x3400..=0x4dbf
+        | 0x4e00..=0x9fff
+        | 0xa000..=0xa4cf
+        | 0xac00..=0xd7a3
+        | 0xf900..=0xfaff
+        | 0xfe10..=0xfe19
+        | 0xfe30..=0xfe6f
+        | 0xff00..=0xff60
+        | 0xffe0..=0xffe6
+        | 0x1f300..=0x1faff
+        | 0x20000..=0x3fffd => 1.0,
+        // The em dash and the ellipsis are one em wide in Helvetica, in Noto
+        // Sans and in DejaVu Sans alike.
+        0x2014 | 0x2026 => 1.0,
+        // An arrow, a minus sign, a tick, a bullet: about four fifths.
+        0x2190..=0x21ff | 0x2212 | 0x25cf | 0x2713 | 0x2714 => 0.8,
+        // Everything else keeps the flat guess.
+        _ => 0.56,
+    }
+}
+
 /// The widest of a set of lines. Character *count* used to stand in for width
 /// here, which meant the longest line and the widest line could be different
 /// lines — and the box was sized for the wrong one.
```

### Diff B — `src/layout.rs`: a bold table (the weight slice)

The table, from Adobe Helvetica-Bold (verified against NimbusSans-Bold, 0 of
95 differ), to sit beside `ADVANCE` at `src/layout.rs:232`:

```rust
const ADVANCE_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278,
    278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584,
    584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611,
    833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333,
    278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278,
    556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556,
    500, 389, 280, 389, 584,
];
```

with `advance_bold(text, size)` beside `advance` and a `Style::text_width`
that picks the table from `style.font_weight` (≥ 500 or `"bold"` → the bold
table). **This one needs a decision before it is written**: it changes every
bold box in the project, so the gallery moves and `docs/goldens.txt` must be
regenerated; and it is an estimate *about the viewer's face*, so it is only
right for families that have a 400/700 pair. I have left it unapplied on
purpose.

### Diff C — the exact path: measure with the embedded face, everywhere

Not a mechanical patch: it is a plan, because it touches every sizing path.

* `subset::Face` (now in the tree, `src/subset.rs:48`) is the API: `Face::parse`
  once, `Face::width(text, size)` per string.
* `GraphOpts.embed_font` (`src/style.rs:906`) already carries the bytes; the
  face would be parsed once in `layout::solve` and passed alongside the
  options.
* Every `text_width`/`advance` call that sizes something the reader will see
  would take the face when the diagram's primary family is the embedded one —
  `natural_size` (`src/layout.rs:371`), `wrap_width` (`:495`), the legend
  widths (`:1689`), `labels::caption_rect` (`src/labels.rs:62`),
  `render_svg::label_size` (`src/render_svg.rs:892`), and the chart labels
  (`src/charts.rs`, ~20 call sites).
* `render_svg::embedded_width` (`src/render_svg.rs:1004`) would become the
  single place that answers "how wide is this text", and
  `render_pdf.rs:149-155` and `:902` would use the same answer, so the three
  writers cannot disagree.
* The PDF writer already measures with the face (`src/render_pdf.rs:399`), so
  this change would make the PDF's *boxes* match its *text*.

### Diff D — `textLength` on every `<text>` (optional, **not** recommended)

`textLength` + `lengthAdjust="spacing"` would force every renderer to draw
the text in exactly the width the layout reserved, whatever face it resolved —
the only fix that closes the *default* path completely. It is not recommended:
it changes the letter spacing of every label in every output, `resvg` (the PNG
path) may ignore it, and it papers over the estimate rather than improving it.

---

## 6. Recommendation

**Accept `D-026` — the family mismatch is not removable — and fix the two
things that are removable, in this order.**

1. **Take Diff A** (`src/layout.rs`, ~25 lines, no dependency, no golden
   churn). It removes the worst measured overshoot — 70.56px for an ideograph
   label at 32px, and the 14.37px the em dash costs — at the cost of nothing.
   Evidence: the em dash and the ellipsis are 1.0 em in Helvetica, Noto Sans
   and DejaVu Sans alike (measured), and the ideographs are full-width by
   construction; the flat guess charges 0.56 for both.
2. **Decide on Diff B** (the bold table). It is worth 4–25% on a bold label,
   but it is an estimate about the viewer's face, it moves every bold box, and
   it needs the gallery regenerated. **My recommendation: defer it**, and only
   take it together with Diff C, which makes the weight question moot for the
   embedded case.
3. **Then decide on Diff C** (measure with the embedded face everywhere). It
   is the only route to exactness, it is the reason `--embed-font` exists, and
   `subset::Face` — the hard half — is now in the tree. It is a real refactor
   of the sizing paths and should not be smuggled in.
4. **Do not add a font-metrics dependency.** It does not fix the problem
   (`D-026`), and it breaks the stance in AGENTS.md.

**And record `X-005` as narrowed, not closed**, with the bound stated
honestly: *the estimate is exact for the base-14 PDF path (regular weight,
Latin), exact for an embedded face's advances, and within 5–25% of the
rendered width for the default SVG path — the residue being the viewer's
choice of face, which the file cannot know.*

**If the risk is accepted unchanged** (option 4 alone), the user-visible
consequences are the §3(c) numbers: a label 5–25% wider than the space
reserved for it, a label that touches or crosses its box at 32px and
visibly crosses it at 64px, and an edge label that can sit on the wire it
labels. That is a cosmetic defect in a picture that is otherwise correct —
which is a defensible thing to accept, and is what `D-026` already decided.

---

## 7. Assumptions, and what I could not measure

* **The CJK row is the only number not measured against a face**: no CJK font
  is installed here, so `日本語のラベル` is computed with the full-width
  convention (1.0 em per ideograph) rather than from a face's `hmtx`. Every
  other number in this report comes from a font file or from the renderer.
* The **CSS weight-matching rule** (a request of 500 picks the 700 face when
  the family has no 500) is what makes Noto Sans Bold the right reference for
  a default node label. I did not run a browser; I used the rule and the two
  faces it selects between.
* The `render_svg.rs` / `render_pdf.rs` / `layout.rs` diffs above are
  **proposals**: Diff A is verified to apply, Diffs B and C are described
  because they need decisions this work item is not mine to take.
* The `--embed-font` family name is the font file's **stem**
  (`src/main.rs:596`), so the embedded metrics only apply when the diagram
  asks for that exact family — the default stack
  (`Inter, ui-sans-serif, system-ui, sans-serif`) never matches it. That is
  worth a line of its own in the manual.

## 8. What I ran

* `cargo test` — 728 lib tests + every integration suite, **0 failed**
  (1 ignored: the manual performance test).
* `cargo clippy --all-targets` — **clean** (the two warnings the other
  agents' in-progress edits had in `src/labels.rs` and `src/spacing.rs` are no
  longer there).
* `cargo fmt -- --check src/subset.rs` — **clean**.
* `cargo test --lib subset` — 5 passed, 0 failed; and the new test was
  **proven to fail** with the old behaviour restored temporarily.
