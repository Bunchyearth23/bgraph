# W-160 — font portability exposed by hosted CI

The first hosted run `38895893757` failed only two unit tests. Every native
platform job and the strict five-platform comparison passed, as did runtime
and feature jobs. The test failure log is `/tmp/bgraph-ci-failed-38895893757.log`.

## Observed causes

`a_subset_font_is_smaller_and_still_a_font` picked the first installed TTF.
On the hosted runner this was NotoColorEmoji, with bitmap color glyphs rather
than the TrueType glyf/loca outlines the conservative subsetter reduces. The
font was valid and intentionally returned unchanged; the test's assumed input
was unsuitable.

`representative_png_snapshots_are_byte_stable` used the machine's available
fonts. The hosted labels hash was `8ecbbf6662c1541f`, unlike the local reference.
A fixed DejaVu font reproduces the hosted hash exactly. This identifies font
selection as the cause, rather than geometry or coordinate drift.

The investigation found a production issue too: PNG's usvg font database did
not load the bytes supplied by the existing `embed_font` option. SVG @font-face
alone does not make those bytes available to the usvg font database.

## Correction

The test font is the unmodified DejaVu Sans Book 2.37, 759720 bytes, SHA-256
`6038a160b491e121c1f12c7bccb4a9c8730296e3adc1086a059404ed84b7451c`.
`tests/fixtures/fonts/` contains its license and provenance. The subset test now
has an explicit glyf/loca input. PNG snapshot cases supply this exact font.
No assertion, raster threshold or source case was removed.

PNG now loads the provided font bytes under the same explicit CSS family alias
used by SVG, with priority over an installed face of that alias. The independent
review requested preserving other localized family aliases on the existing face;
that correction now removes only the requested name and reinserts a face with
its other family names and source/style metadata intact. The new regression
checks both aliases, distinct IDs, original source Arc/index and bold/italic/
condensed metadata. The final focused pass succeeds **6/6**, zero ignored.

Three initial private regressions prove rendering without installed system
fonts, priority over an installed alias, and error propagation for invalid font
bytes. Together with the two original tests, the focused pass succeeded 5/5.
The initial corrected full suite passed **713 tests** (692 library plus 21
other tests), with Clippy and format checks green. Final alias-preservation and
hosted CI results are tracked in the overall completion report.

## Image review

The implementer and an independent UI reviewer inspected the actual PNGs.
Dimensions remain 328×124 (shapes), 367×128 (labels), and 453×364 (charts).
Shapes are byte-identical (`a14676b474c3a993`). Labels and pie retain geometry,
colors, values, percentages and all text; their intended font resource is now
fixed. Only those two hashes changed after inspection: labels
`8ecbbf6662c1541f`, pie `c7b79283142cd658`.

The pie title appears lighter with the supplied Book face: usvg selects the
available face and does not synthesize a missing bold face. Style weight remains
unchanged. This is explicit test-font selection; the implementation does not
substitute another machine font to make it look heavier.

Images: `/tmp/bgraph-{fixed,system}-font-{labels,charts}.png`. The standalone
shape image was also inspected and its bytes were unchanged. These artifacts
are visual evidence, not replacements for the strict PNG hash assertions.

## Final local integration after alias preservation

`CARGO_TARGET_DIR=target/goal-root cargo test --locked --all-features` exited 0:
693 library tests plus 21 other tests, **714 passed**, one intentionally ignored
historical measurement. `/tmp/bgraph-goal-font-alias-allfeatures-final.log`.
Clippy all-targets/all-features with `-D warnings`, format check and diff check
also exited 0. Independent final review accepted alias preservation and the
reviewed images. This fixes the hosted failures without skipping either test.
The corrected hosted rerun remains required before W-160 closure.
