# bgraph — index

**The one file that answers "what is left".** One line per item the project refers to by
identifier. Documents describe things; this says what state they are in and where they are.

```
grep " open \| planned \| blocked " docs/INDEX.md
```

| Prefix | What it is | States |
|---|---|---|
| `I-nnn` | **Idea** — raw; most die, and that is the point | `parked` → `promoted` → `discarded` |
| `Q-nnn` | **Question** — undecided, and *needs a human* | `open` → `answered` → `dropped` |
| `D-nnn` | **Decision** — taken, dated, carries its why | `active` → `amended` → `reversed` |
| `W-nnn` | **Work** — something to build | `planned` → `building` → `landed`, or `blocked` / `dropped` |
| `X-nnn` | **Risk** — a way this goes wrong | `open` → `mitigated` → `realised` → `retired` |

An identifier never changes and is never reused. State is explicit. Numbers come from this file,
never from a position in a document. Updated in the same commit as the work.

Section references (`§N.M`), founding principles and the dated lessons in `AGENTS.md` stay out on
purpose — an identifier is earned by being cited.

---

## Open — the answer to "what is left"

**Nothing is planned and nothing is blocked.** Sixteen archetypes ship, the
library is embeddable, and the two questions that needed a human are answered
(`D-020`, `D-021`). What remains are three open **risks** — known ways the
renderer is imperfect, each with no work item because none has been worth the
cost yet.

| id | state | title | notes |
|---|---|---|---|
| `X-009` | mitigated | Editor queries can name node types the grammar has dropped | It broke once — `W-029` renamed `node_declaration` and Zed stopped recognising `.bd` entirely. `the_editor_queries_match_the_grammar` compiles every `.scm` against the generated parser, but only when the `tree-sitter` CLI is on PATH. |
| `X-002` | mitigated | The two grammars can drift apart | `W-008` cross-checks them on every `cargo test` — but only when the `tree-sitter` CLI is on PATH; without it the test says SKIPPED and passes. |
| `X-010` | mitigated | `tree-sitter test`'s summary line counts parses, not expectation matches | It read "failed parses: 0" while seven corpus expectations were stale across the `W-029` rename. `the_grammar_corpus_expectations_still_hold` now asserts the **exit code** on each `cargo test`, but only when the CLI is on PATH. |
| `X-006` | mitigated | Edge labels avoid lines, not each other's readability | `W-010` scores candidates; `W-022` made the node and label penalties proportional to the overlapped area and distance a real cost, which stopped labels fleeing three rows from their own edge. Still local: nothing guarantees the best global arrangement. |
| `X-004` | open | Orthogonal routing ignores obstacles | An `ortho` edge can pass straight through a node. No diagnostic. |
| `X-008` | open | `D-016` removed `opacity`, which is a breaking change | Any file using `opacity: 0.5` now fails with `unknown property`. The repo's own files carried none, and the message names the fix — but nothing migrates it automatically. |
| `X-005` | open | Text width is estimated, not measured | `0.56 × font-size` per character. Auto-sized boxes are slightly wrong for narrow or wide fonts, and edge-label plates with them. |

---

## Settled

### Decisions
| id | state | date | title | was |
|---|---|---|---|---|
| `D-020` | active | 2026-07-31 | **Proprietary, all rights reserved, and not published.** `LICENSE` grants nothing; `Cargo.toml` carries `license-file` and `publish = false`, so `cargo publish` refuses on that line rather than on a comment. The Archmagos chose AGPL and then changed it to proprietary the same day | |
| `D-021` | active | 2026-07-31 | **Repetition in templates is one line, one list.** A line holding `{{row.field}}` is written once per row — the line is the unit because a statement already ends at a newline, so it needs no block syntax and cannot nest wrongly. No conditions, no expressions, no nesting: that would be a template language, and a much larger promise. A line naming two lists is refused when the template is read | |
| `D-019` | active | 2026-07-31 | **An archetype may refine a shared property.** `ArchetypeDef::item_props` is consulted before the global table, so `kind` carries `state`'s words in a machine and `block`'s in an architecture view, and `tech` exists in neither anywhere else. Without it the two word lists would have to be merged, and completion would offer `initial` while writing a C4 diagram — the exact noise `W-007` was asked to remove | |
| `D-018` | active | 2026-07-31 | **The embedding surface is narrow on purpose.** Semver covers `render_source`, `template`, `parser` and `check` — nothing else. `ast`, `layout`, `style`, `charts`, `render_svg` and `props` stay public to be inspected but are not a promise; pinning them would mean the diagrams could never be improved. The vocabulary's stable form is `docs/reference.md`, generated from those tables | |
| `D-001` | active | 2026-07-31 | Placement is declared with **relative anchors plus absolute** — `at`, `right-of`, `gap`, `align` — not auto-layout | |
| `D-002` | active | 2026-07-31 | Styling is a **cascade of named classes**: defaults → implicit `node`/`edge` → `.classes` → inline | |
| `D-003` | active | 2026-07-31 | Ship a **CLI plus a live preview** (`bgraph watch`), not export formats | |
| `D-004` | active | 2026-07-31 | Comments are `//`. `#` is reserved for hex colors — pest applies `COMMENT` everywhere and the two cannot coexist | |
| `D-005` | active | 2026-07-31 | Unknown property / class / shape / router names are **hard errors**, never silent no-ops | |
| `D-006` | active | 2026-07-31 | `check.rs` is the **single validator**. `layout::solve` reports its first error; the LSP reports all of them | |
| `D-007` | active | 2026-07-31 | `type` selects the **genre** of diagram, once per file; `free` is the default | |
| `D-008` | amended | 2026-07-31 | Archetypes **reuse the same statements** — a `node` is a box, a slice or a bitfield — so the cascade, the validator and the editor are written once. Amended by `D-015`: the *form* stays one, the *words* become per-genre | |
| `D-009` | active | 2026-07-31 | The SVG surface is **explicit** (`#ffffff` by default); `background: none` opts into transparency. Preview and export must agree | |
| `D-010` | active | 2026-07-31 | The reference manual is **generated** from the `props.rs` tables; a test fails on drift | |
| `D-011` | active | 2026-07-31 | Completion is **quiet by answer, not by refusal**: space still triggers, but the server returns nothing where a suggestion would be in the way | |
| `D-012` | active | 2026-07-31 | The git remote uses **SSH**; the HTTPS URL had no stored credentials | |
| `D-013` | active | 2026-07-31 | The four statement keywords (`type`, `graph`, `style`, `node`) are **reserved** and cannot name a node — found by `W-008`, which caught pest accepting `style -> a` where tree-sitter could not | |
| `D-017` | active | 2026-07-31 | The library is embedded by **templating**: a `.bd` file holds the diagram's shape and the calling program binds `{{holes}}`. Not a typed builder — a program that needs to build a diagram from nothing can already write the source text, and holes earn their keep exactly where the shape is fixed and only values vary | |
| `D-016` | active | 2026-07-31 | **Alpha rides on the color**, as its fourth channel, defaulting to `ff`. The `opacity` property is removed: transparency belongs to the paint, which is what lets a shape keep an opaque border over a translucent fill. Emitted as a separate SVG attribute, since several SVG-to-PDF paths drop inline eight-digit hex | |
| `D-015` | active | 2026-07-31 | Each archetype gets its **own keyword and positional data** — `slice organic "Organic search" 4820` — over one shared statement form, so the grammars, the validator and the editor stay written once. Seven rules and the reasoning in [`vocabulary.md`](vocabulary.md) | |
| `D-014` | active | 2026-07-31 | The target is **four audiences — stats, dev, embedded and data** — and, at the end of it, an **embeddable Rust library** that other programs call to generate documents on the fly. The archetype survey in *Open* is scoped to that, and `W-027` is the library itself | |

The full reasoning for each lives in `AGENTS.md` and `docs/reference.md`; these lines are pointers,
not copies.

### Work landed

| id | date | title | commit |
|---|---|---|---|
| `W-001` | 2026-07-31 | Core language: pest grammar, spanned AST, `check`, anchor solver, style cascade | `1b53ec3` |
| `W-002` | 2026-07-31 | SVG renderer: 6 shapes, straight and ortho routing, arrow heads, content-derived viewBox | `1b53ec3` |
| `W-003` | 2026-07-31 | `bgraph watch` — dependency-free live preview | `1b53ec3` |
| `W-004` | 2026-07-31 | `bgraph-lsp`: diagnostics, completion, hover, go-to-definition, outline | `1b53ec3` |
| `W-005` | 2026-07-31 | Tree-sitter grammar and Zed extension | `1b53ec3`, regenerated `d6997b1` |
| `W-006` | 2026-07-31 | `type` archetypes (`pie`, `packet`), quiet completion, explicit surface, generated reference manual | `4b4c548` |
| `W-033` | 2026-07-31 | Templating: `{{holes}}` that parse, typed bindings, context-aware escaping, `--set` on the CLI | 6d166eb |
| `W-028` | 2026-07-31 | One cargo feature per archetype; an archetype not compiled in is *unknown*, not broken. `rlib` 3439 KB → 2566 KB with none | this commit |
| `W-035` | 2026-07-31 | Templating: repetition. A line holding a `{{row.field}}` hole is written once per row | this commit |
| `W-032` | 2026-07-31 | `type timeline` — events on a date line, labels packed into staggered rows either side | this commit |
| `W-025` | 2026-07-31 | `type block` — C4 architecture views: typed elements, a technology line, a wrapped description, relations that carry a protocol | this commit |
| `W-031` | 2026-07-31 | `type er` — entities, columns and crow's-foot cardinality at each end of a relation | this commit |
| `W-027` | 2026-07-31 | **The embedding promise**: hostile input cannot bring down the host, and the public surface says what semver covers | this commit |
| `W-014` | 2026-07-31 | Date and duration literals — `2026-08-01`, `12d`, `2w`, `6h`, with civil-calendar arithmetic | this commit |
| `W-026` | 2026-07-31 | `type gantt` — bars on a calendar, sections from groups, a today line | this commit |
| `W-013` | 2026-07-31 | Member lists — a `- ` or `+ ` line in a block, kept with its marker | this commit |
| `W-023` | 2026-07-31 | `type class` — a name, a rule, and its members; placement stays declared | this commit |
| `W-022` | 2026-07-31 | `type state` — declared placement kept, pseudo-states as marks, guards in brackets, nesting via groups | this commit |
| `W-021` | 2026-07-31 | `type sequence` — participants, lifelines, ordered messages, self-messages, frames spanning message rows | this commit |
| `W-030` | 2026-07-31 | Pan and zoom in the watch preview, kept across the reload a save triggers | this commit |
| `W-017` | 2026-07-31 | `type xy` — line, area and dot marks over a shared axis, categorical or numeric | this commit |
| `W-012` | 2026-07-31 | Plot frame — `nice_scale`, axis lines, gridlines, tick and category labels, legend | this commit |
| `W-016` | 2026-07-31 | `type bar` — grouped or stacked series across named categories | this commit |
| `W-015` | 2026-07-31 | `value` on edges — and, past it, positional slots for edges: `a -> b 40` | this commit |
| `W-018` | 2026-07-31 | `type sankey` — layered flows, ribbons sized by magnitude and coloured by source | this commit |
| `W-024` | 2026-07-31 | `type wave` — timing lanes in the WaveDrom alphabet, clocks, buses with named segments, gaps | this commit |
| `W-019` | 2026-07-31 | `type quadrant` — points on two axes, four named regions; a Wardley map is the same archetype with other axis names | this commit |
| `W-020` | 2026-07-31 | `type treemap` — squarified tiles, hierarchy from group nesting, one hue per top-level branch | this commit |
| `W-011` | 2026-07-31 | Containers: `group` with nested statements, sized by its members, anchorable, translating its subtree when placed | this commit |
| `W-029` | 2026-07-31 | Per-archetype vocabulary: `slice a "A" 4820`, `field b "B" 0 15` — one statement form, keyword and slots declared in `props.rs` | this commit |
| `W-007` | 2026-07-31 | Re-pin the Zed grammar — `extension.toml` now points at `288f785`, which carries the `type` parser; `bgraph-lsp` reinstalled | this commit |
| `W-008` | 2026-07-31 | Cross-check the two grammars over a shared corpus (`tests/grammar/`) | `4b4c548` |
| `W-009` | 2026-07-31 | Draw self-loops — a sampled arc over the node's top edge | `4b4c548` |
| `W-010` | 2026-07-31 | Place edge labels clear of the lines they would cross | `4b4c548` |

Nothing is marked `landed` without a commit to point at.

### Questions answered

| id | date | title | became | was |
|---|---|---|---|---|
| `Q-001` | 2026-07-31 | What does `type` select — a placement strategy or a diagram genre? | `D-007` | |
| `Q-003` | 2026-07-31 | What shape should the embeddable API take? | `D-017` | |

### Risks retired

| id | date | title | how |
|---|---|---|---|
| `X-001` | 2026-07-31 | The Zed extension had never been exercised inside Zed | The Archmagos installed it and reported live completion and rendering behaviour, which drove `D-011` and the `font-color` fix |
| `X-007` | 2026-07-31 | `W-029` was going to move some syntax errors to semantic ones | It did, exactly once: `alpha beta` is now an item with an unknown keyword. The message names the archetype's word, and `tests/grammar/accept/bare-words-parse-as-items.bd` pins the new category |
| `X-012` | 2026-07-31 | Deeply nested groups aborted the host process | `pest` descends recursively, so ~1500 nested `{` overflowed the stack — `SIGABRT`, which a caller cannot catch, in a library meant to be embedded. A lexical depth scan now runs *before* pest and refuses past `MAX_NESTING` (64). Proven by removing the guard and watching the suite abort |
| `X-011` | 2026-07-31 | Two tests shelling out to `tree-sitter` in parallel could catch the parser library mid-rebuild | It fired twice before being fixed. The calls now take a mutex; serialising them costs nothing, and a flaky guard stops being believed |
| `X-003` | 2026-07-31 | Self-loops were declared but not drawn | `W-009` draws them; the stale "not drawn yet" warning is gone |

### Ideas

| id | state | title |
|---|---|---|
| `I-001` | discarded | Absolute coordinates only — rejected in favour of `D-001`; every insertion would renumber its neighbours |
| `I-002` | discarded | Grid layout as the core model — too rigid for organic diagrams |
| `I-003` | discarded | Auto-layout with per-node overrides — reintroduces the very fight with the engine the project exists to end |
| `I-004` | parked | Auto-placement archetypes (`flow` layered from edges, `tree`, `grid`) — proposed, then set aside when `type` turned out to mean genre, not layout |
| `I-005` | promoted | Further chart genres — surveyed against Mermaid, Kroki, WaveDrom and Vega-Lite, then split into `W-016` … `W-026` |
| `I-006` | promoted | Containers and subgraphs — became `W-011`, landed |
| `I-007` | parked | PNG and PDF export |
| `I-008` | parked | Real font metrics instead of the estimate in `X-005` |
| `I-009` | parked | `type bytefield` — `packet` with a byte-granular ruler; a variant of `W-005`, not a chantier of its own |
| `I-010` | parked | Infrastructure genres: network topology, server racks, wiring harnesses (the nwdiag / rackdiag / WireViz territory) |
| `I-011` | parked | Tree genres: mindmap, treeview, org chart — all one tree-layout engine, which `W-020` will need anyway |
| `I-012` | parked | `type radar` and `type venn` — both want `W-012`'s scales; radar needs a series notion |
| `I-013` | parked | Process and planning genres: kanban, user journey, BPMN, C4, requirement diagrams |
| `I-014` | parked | `type gitgraph` — commits, branches and merges; expressible as nodes and edges, but the layout is its own problem |
| `I-015` | parked | Raster and PDF export from the library (`W-027`), so an embedding program is not forced to ship an SVG renderer |
| `I-016` | parked | Activation bars for `type sequence` — the box down a lifeline while a call is outstanding. Needs a way to say where one opens and closes |
| `I-017` | parked | True scatter for `type xy` — arbitrary `(x, y)` pairs rather than points spread evenly across the axis. Needs a second slot form, which is its own decision |
