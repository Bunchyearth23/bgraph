# W-167 — BPMN captions and edge labels

## Request and cause

The user's screenshot has a gateway caption `d` centred below the diamond,
exactly on its outgoing vertical flow. BPMN captions were always drawn below
shapes; routing did not influence their location. Edge-label avoidance also
ignored free annotations and group titles and only searched perpendicular to
one segment.

## Implementation

- Caption bounds include multiline text and a small clearance around glyphs.
  BPMN event/gateway captions consider nearby positions below, above and beside
  their own shape. The choice considers actual routed flows, other captions,
  node boxes, edge labels, annotations, group boundaries and group titles.
- At most three caption refinements feed the chosen offsets back through the
  same declared constraint solver. Relative gaps and containing frames include
  the chosen caption footprint in addition to external margins. Absolute `at`
  coordinates, alignments and boundary attachments still address visible shapes.
- Explicit node `label-offset` remains pinned, including a declared zero.
  Automatic offsets are stored in solved node styles; source declarations
  and style provenance are not rewritten.
- Edge labels search both along and perpendicular to their middle segment.
  Clear candidates take priority. Annotation rectangles, shifted node labels
  and group titles join the existing node/route/label obstacles. SVG and PDF
  use this shared routing, and PNG renders the same SVG geometry.
- Remaining BPMN caption/flow/node/label collisions and edge-label/text/title
  collisions are warnings in `check::after_layout`, shared with the LSP.
- Placement considers only declared captions while treating generated nodes
  (including note boxes) as obstacles; their indices are not caption indices.

## Observed local evidence

- Release CLI/LSP build with the `png` feature succeeded.
- Clippy with warnings denied succeeded for the default library and for
  `--no-default-features`; the minimal library also built successfully.
- Reconstructed the supplied picture: `/tmp/bgraph-bpmn-labels.bd`.
  Before/after comparison: `/tmp/bgraph-bpmn-labels-comparison.png`.
  The `d` caption moves beside the vertical flow. SVG, PDF and PNG generated.
- Rendered and inspected the complete BPMN example in SVG/PNG/PDF:
  `/tmp/bgraph-bpmn-gallery.*`. Gateway and boundary-event captions clear
  their flows and the existing captions remain readable.
- Rendered and inspected a grouped vertical process with zero/short gaps and
  long multiline labels: `/tmp/bgraph-bpmn-labels-long.*`.
- An explicit `label-offset: 0 0` retains the pictured collision;
  `bgraph check /tmp/bgraph-bpmn-labels-pinned.bd` reports its source warning.
  The automatic counterpart reports `no problems found`.
- Regenerated the gallery. Only `block`, `state` and `bpmn` changed on top of
  the previous sequence margin update; the three outputs were visually
  inspected before copying them into `examples/`. Their FNV-1a goldens are
  `065e04b17c38e785`, `5529e732304f06f8`, `f56404a47bfd514d` respectively.
- Reference regenerated from `src/docgen.rs`; strict MkDocs build succeeded.
- Rendered the reconstructed process with an additional note box:
  `/tmp/bgraph-bpmn-labels-note.svg`; generated boxes do not cause caption
  indexing failures.
- Installed updated release binaries at the existing `~/.cargo/bin` paths.
  Backups: `.tools/bgraph-before-labels-20261016` and
  `.tools/bgraph-lsp-before-labels-20261016`. Restart existing watch/LSP
  processes to load the new binaries.
  CLI SHA-256: `0af2793ef4b90b5c72bc5f6ac6ec6f5c433cb5f9219944a9d8b4cebcb173f4d2`.
  LSP SHA-256: `945bb0c20e6f294354559f8af639044b31f829f7f0383c17d572258eb5bfb956`.

## Minimal reconstruction

```bgraph
type bpmn
graph { background: #ffffff padding: 20 }
style edge { head: none }
data c "c" object
gateway d "d" exclusive
task a "a"
event b "b" intermediate
layout {
  place c at (0, 0)
  place d right-of c gap 40
  place a right-of d gap 40
  place b below d gap 64
}
c -> d
d -> a
d -> b
```

## Limits and resume

This is bounded local avoidance, not a guarantee that every dense or explicitly
pinned diagram admits a collision-free arrangement. Text widths retain the
existing font-estimation limitations. Performance and cross-platform parity
were not remeasured. No test was added and no test suite or CI job was run for
this change. W-166 and W-167 remain `building` pending the user's confirmation;
no commit or push was requested for these changes.
