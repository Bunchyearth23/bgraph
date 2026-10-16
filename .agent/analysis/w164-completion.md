# W-164 — multi-form themes, notation and public provenance

## Scope completed

The original implicit-theme fixes are preserved: the written form selects its
canonical style target; `node` is the primary-word synonym; `Registry` shares its
implicit theme layers with explain; equal-value overrides belong to the last
explicit layer. Validation, completion and hover recognize the BPMN and XY item
words as themes.

The independent proof found two additional defects rather than treating uniform
output as sufficient:

1. **Unqualified BPMN words lost their notation.** `event e`, `gateway g` and
   `data d` without optional positional values did not set `stage`, `gate` or
   `artifact`. `element_of` therefore treated them as tasks. A new semantic
   rendering test failed before the fix: a bare event had `Shape::Rect` instead
   of `Shape::Ellipse`. The browser owner independently reproduced a 76px event
   instead of 40px (`/tmp/bgraph-w158-before-bpmn-fix.log`).
2. **Public hover on `style node` returned declaration documentation.** Private
   `describe_style` tests passed, but a real JSON-RPC hover request reached the
   earlier generic keyword branch. The hover owner corrected the priority for
   the style declaration context. The node theme's shared vocabulary text now
   explicitly states the cascade order, so the public hover explains it too.

## Implementation

`style::item_defaults` supplies BPMN notation below every author style:

| Written form | Builtin notation defaults |
|---|---|
| task / node | kind plain |
| event | stage intermediate, trigger plain |
| gateway | gate exclusive |
| data | artifact object |

No previous fallback stage existed: `element_of` depended on a property being
present. Intermediate is the neutral event default; the new behavior is documented
in the vocabulary and generated reference. Shape, router and head remain unset by
this helper, allowing the archetype to choose them and author properties to override.
The genre guard prevents the unrelated timeline word `event` from receiving BPMN
semantics.

`Registry::resolve_node` applies these defaults before common node theme, written
item theme, classes in order and inline/positional properties. Layout, semantic
validation and boundary-attachment extraction use that same helper. Explain uses
the same builtin defaults and reports notation properties as well as paint, with
separate provenance for builtin/default, theme, class and inline values. Preview
inspection and node hover consume this explanation; no parallel style resolver
was introduced there.

Docs were edited in `props.rs` and regenerated using a freshly built binary;
`docs/reference.md` was not edited manually.

## Executed proof

Final log: `/tmp/bgraph-w164-proof.log`.

- `cargo test --locked --test style_provenance -- --nocapture` — **exit 0, 6 tests**.
  - 40 render/registry/explain cases: BPMN task/event/gateway/data/node and XY
    series/scatter/node, each at five override levels including identical values.
    Explicit class ordering and inherited stroke provenance are asserted.
  - All five bare BPMN spellings, without optional semantic slots, assert actual
    shape and SVG markup. Events assert 40×40 ellipse and double ring; gateways
    assert 48×48 diamond/polygon; data asserts the emitted folded-page polyline.
  - Builtin/common-theme/item-theme/class/inline precedence for stage, gate and
    artifact; author shape/size override for all BPMN spellings; node synonym;
    no BPMN defaults leaking into timeline/XY; unset shape/router/head in base.
  - Real `bgraph-lsp` subprocesses exchange framed JSON-RPC: 40 node hover cases
    name the same winning paint layer; all item theme contexts offer node-side
    properties and reject edge-only context; global theme hovers are recognized;
    builtin notation versus identical explicit theme/inline values is distinguished.
- `cargo test --locked --lib --all-features theme -- --nocapture` — **exit 0, 16 tests**.
- `cargo test --locked --lib --all-features explain -- --nocapture` — **exit 0, 15 tests**.
- `cargo test --locked --lib --all-features styles_cascade_defaults_then_class_then_inline -- --nocapture`
  — **exit 0, 1 test**.
- `cargo run --locked --bin bgraph -- docs -o docs/reference.md` — **exit 0** after
  editing the shared vocabulary, using a newly compiled binary.
- `cargo test --locked --lib --all-features the_reference_manual_is_up_to_date -- --nocapture`
  — **exit 0, 1 test**.
- `cargo check --locked --no-default-features --lib` — **exit 0**.
- `git diff --check` — **exit 0**.

All Cargo commands above used `CARGO_TARGET_DIR=target/pipeline-work`. Filters may
overlap; the counts are per command, not a count of distinct tests. An initial
`--locked` run crossed the concurrent portable-math lock update and failed; the
final successful run followed the completed update.

## Visual evidence / final integration

The parent confirmed the final portable-math Chromium proof: **6/6 scenarios**,
zero JavaScript errors, and six screenshots visually inspected. Structured evidence:
`/tmp/bgraph-inspector-ctfmjn/result.json`. This includes the strengthened bare-BPMN
event/gateway assertions that failed before the fix, not merely differently colored
rectangles. The browser driver also checks pie source-to-slice/theme interaction
and packet one-to-many geometry.

The parent's final all-feature suite also exited **0**: 689 library tests, the six
style integration tests and three doctests, recorded in
`/tmp/bgraph-goal-allfeatures-complete.log`. The parent additionally confirmed the
22 text fixtures, 30-genre runtime gate, four file-context fixtures and final real
VS Code/Zed host checks after the portable-math change. These are parent-executed
integration evidence, distinguished from the focused commands executed here.

No commit or publication. The parent owns the final full suite, cross-platform
native comparison and INDEX update.

Graft for this W-164 inspection: 6 retrievals, approximately 165,585 source-reading
tokens saved. A separate read-only W-157 minimizer audit found the exact-cardinality
search logically sound; its notes were sent to the parent.
