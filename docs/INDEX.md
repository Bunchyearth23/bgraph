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

| id | state | title | notes |
|---|---|---|---|
| `W-007` | planned | Re-pin the Zed grammar | `W-006` has landed, so this is unblocked. `extension.toml` pins `rev = d6997b1`, which predates the pest-side `type` grammar; run `./editors/zed/install.sh`. |
| `Q-002` | open | Which archetype after `pie` and `packet`? | The Archmagos said "pie, packet, ce genre de chose" — the list is open. Candidates parked as `I-005`. |
| `X-002` | mitigated | The two grammars can drift apart | `W-008` cross-checks them on every `cargo test` — but only when the `tree-sitter` CLI is on PATH; without it the test says SKIPPED and passes. |
| `X-006` | open | Edge labels avoid lines, not each other's readability | `W-010` scores candidate positions and picks the least-obstructed. Two labels on the same crossing still both move; nothing guarantees the result is the *best* arrangement. |
| `X-004` | open | Orthogonal routing ignores obstacles | An `ortho` edge can pass straight through a node. No diagnostic. |
| `X-005` | open | Text width is estimated, not measured | `0.56 × font-size` per character. Auto-sized boxes are slightly wrong for narrow or wide fonts, and edge-label plates with them. |

---

## Settled

### Decisions

| id | state | date | title | was |
|---|---|---|---|---|
| `D-001` | active | 2026-07-31 | Placement is declared with **relative anchors plus absolute** — `at`, `right-of`, `gap`, `align` — not auto-layout | |
| `D-002` | active | 2026-07-31 | Styling is a **cascade of named classes**: defaults → implicit `node`/`edge` → `.classes` → inline | |
| `D-003` | active | 2026-07-31 | Ship a **CLI plus a live preview** (`bgraph watch`), not export formats | |
| `D-004` | active | 2026-07-31 | Comments are `//`. `#` is reserved for hex colors — pest applies `COMMENT` everywhere and the two cannot coexist | |
| `D-005` | active | 2026-07-31 | Unknown property / class / shape / router names are **hard errors**, never silent no-ops | |
| `D-006` | active | 2026-07-31 | `check.rs` is the **single validator**. `layout::solve` reports its first error; the LSP reports all of them | |
| `D-007` | active | 2026-07-31 | `type` selects the **genre** of diagram, once per file; `free` is the default | |
| `D-008` | active | 2026-07-31 | Archetypes **reuse the same statements** — a `node` is a box, a slice or a bitfield — so the cascade, the validator and the editor are written once | |
| `D-009` | active | 2026-07-31 | The SVG surface is **explicit** (`#ffffff` by default); `background: none` opts into transparency. Preview and export must agree | |
| `D-010` | active | 2026-07-31 | The reference manual is **generated** from the `props.rs` tables; a test fails on drift | |
| `D-011` | active | 2026-07-31 | Completion is **quiet by answer, not by refusal**: space still triggers, but the server returns nothing where a suggestion would be in the way | |
| `D-012` | active | 2026-07-31 | The git remote uses **SSH**; the HTTPS URL had no stored credentials | |
| `D-013` | active | 2026-07-31 | The four statement keywords (`type`, `graph`, `style`, `node`) are **reserved** and cannot name a node — found by `W-008`, which caught pest accepting `style -> a` where tree-sitter could not | |

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
| `W-008` | 2026-07-31 | Cross-check the two grammars over a shared corpus (`tests/grammar/`) | `4b4c548` |
| `W-009` | 2026-07-31 | Draw self-loops — a sampled arc over the node's top edge | `4b4c548` |
| `W-010` | 2026-07-31 | Place edge labels clear of the lines they would cross | `4b4c548` |

Nothing is marked `landed` without a commit to point at.

### Questions answered

| id | date | title | became | was |
|---|---|---|---|---|
| `Q-001` | 2026-07-31 | What does `type` select — a placement strategy or a diagram genre? | `D-007` | |

### Risks retired

| id | date | title | how |
|---|---|---|---|
| `X-001` | 2026-07-31 | The Zed extension had never been exercised inside Zed | The Archmagos installed it and reported live completion and rendering behaviour, which drove `D-011` and the `font-color` fix |
| `X-003` | 2026-07-31 | Self-loops were declared but not drawn | `W-009` draws them; the stale "not drawn yet" warning is gone |

### Ideas

| id | state | title |
|---|---|---|
| `I-001` | discarded | Absolute coordinates only — rejected in favour of `D-001`; every insertion would renumber its neighbours |
| `I-002` | discarded | Grid layout as the core model — too rigid for organic diagrams |
| `I-003` | discarded | Auto-layout with per-node overrides — reintroduces the very fight with the engine the project exists to end |
| `I-004` | parked | Auto-placement archetypes (`flow` layered from edges, `tree`, `grid`) — proposed, then set aside when `type` turned out to mean genre, not layout |
| `I-005` | parked | Further chart genres: xy / bar, sequence, gantt, state — candidates for `Q-002` |
| `I-006` | parked | Containers and subgraphs |
| `I-007` | parked | PNG and PDF export |
| `I-008` | parked | Real font metrics instead of the estimate in `X-005` |
