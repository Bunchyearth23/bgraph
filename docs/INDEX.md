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

**Nothing is planned. No question is open and nothing is blocked.**
Twenty-eight genres plus `free` ship, both editors have an extension, and the
library renders SVG and PDF. (Earlier lines in this file said "thirty"; the
table in `props.rs` has twenty-nine `ArchetypeDef` entries, one of them `free`.) The parked ideas were
reviewed on 2026-08-10 and every one of them moved: what was worth building
became a work item, what was not was discarded, and the one language question
the review raised was answered the same day (`D-023`). One idea is still parked,
on purpose — `I-021`, because nobody has chosen which part of BPMN is the useful
part, and that has to be decided before it can be costed.

### Work planned

Nothing. The sixteen chantiers opened on 2026-08-10 all landed the same day.


### Risks

Known ways this goes wrong. `X-005` now has a chantier; the rest do not, because none has been worth the cost yet.

| id | state | title | notes |
|---|---|---|---|
| `X-009` | mitigated | Editor queries can name node types the grammar has dropped | It broke once — `W-029` renamed `node_declaration` and Zed stopped recognising `.bd` entirely. `the_editor_queries_match_the_grammar` compiles every `.scm` against the generated parser, but only when the `tree-sitter` CLI is on PATH. `W-038` did **not** double it: VS Code takes a TextMate grammar, which cannot name a tree-sitter node type at all — it has its own guard instead. |
| `X-002` | mitigated | The two grammars can drift apart | `W-008` cross-checks them on every `cargo test` — but only when the `tree-sitter` CLI is on PATH; without it the test says SKIPPED and passes. |
| `X-010` | mitigated | `tree-sitter test`'s summary line counts parses, not expectation matches | It read "failed parses: 0" while seven corpus expectations were stale across the `W-029` rename. `the_grammar_corpus_expectations_still_hold` now asserts the **exit code** on each `cargo test`, but only when the CLI is on PATH. |
| `X-006` | mitigated | Edge labels avoid lines, not each other's readability | `W-010` scores candidates; `W-022` made the node and label penalties proportional to the overlapped area and distance a real cost, which stopped labels fleeing three rows from their own edge. Still local: nothing guarantees the best global arrangement — `W-037` adds the escape hatch (`label-offset`) rather than a better solver, because the author can see what the cost function cannot. |
| `X-017` | open | There is no PNG or other raster output | `W-042` ships PDF because it is vector and needs nothing; a raster format needs glyph rasterisation and path filling, which is a real renderer. `D-027` records that the honest options were a large dependency tree in a crate that has five, or months of work — and that a PDF opens everywhere a PNG does. Anyone who needs a bitmap can rasterise the PDF or the SVG with a tool built for it. |
| `X-004` | mitigated | Orthogonal routing ignores obstacles | It still does — the router turns corners and avoids nothing. What changed is that it no longer does so *in silence*: `check::after_layout` reports an `ortho` edge whose path crosses a box that is not one of its endpoints, and names the ways out. A router that avoided obstacles is a different piece of work; a drawing that is wrong without saying so was the part worth fixing first. |
| `X-008` | open | `D-016` removed `opacity`, which is a breaking change | Any file using `opacity: 0.5` now fails with `unknown property`. The repo's own files carried none, and the message names the fix — but nothing migrates it automatically. |
| `X-005` | open | Text width cannot be measured, only estimated | **Narrowed by `W-040`, and it cannot be closed.** The within-typeface error is gone: advances are per character now, so `l` and `W` are no longer charged the same. What remains is unfixable by construction — the SVG names a font *family* and whoever renders it decides what that resolves to, so a box sized against Helvetica metrics is wrong by however much the reader's font differs. `D-026` records why the alternatives are worse. |

---

## Settled

### Decisions
| id | state | date | title | was |
|---|---|---|---|---|
| `D-027` | active | 2026-08-10 | **PDF is written by hand; PNG is not written at all.** A PDF is a vector format — rectangles, béziers and `Tj` — so `W-042` emits one from the same `Layout` the SVG writer consumes, sharing `render_svg::canvas` so the page box and the viewBox cannot drift. That keeps the dependency list at five, matching the SVG writer and the watch server. A raster format cannot be done that way: it needs glyph rasterisation and path filling, which is either a large dependency tree or months of work, and a PDF already opens everywhere a PNG does. Recorded as `X-017` rather than left as an unstated gap. One thing falls out of it: a viewer must have Helvetica, so `W-040`'s advance table is *exact* for PDF and `X-005` does not apply to that output | |
| `D-026` | active | 2026-08-10 | **Text is measured per character against one typeface's metrics, and the remaining error is accepted.** `W-040` replaces the flat `0.56 × font-size` with a Helvetica-class advance table, which removes the four-to-one variation between `l` and `W`. The *cross-family* error cannot be removed: an SVG names a font family and the renderer decides what it resolves to, so no measurement here can be right by anything but luck. The two ways out were both worse — embedding a font in every SVG costs hundreds of kilobytes per file and raises a licensing question the project has no answer to, and shipping metrics for a fixed set of families is the same estimate with a longer table and a promise it cannot keep. Non-ASCII keeps the flat guess, because a Latin table has nothing to say about CJK. `X-005` is therefore narrowed, never retired | |
| `D-025` | active | 2026-08-10 | **A pin is a member line; a wire carries its pins as positional data.** `W-045` addresses a connector's pin with the two mechanisms that already ship — `- 1: +12V` is a member (`W-013`), and `j1 -> j2 "power" 1 3` puts the pin at each end in the edge's positional slots (`W-015`). No grammar changes, and it is `D-015` read literally: an archetype's positional data is its own. The dotted form `j1.3` was rejected on evidence rather than taste — `ident` forbids `.` and `3` is not an identifier, so it does not parse at all; `j1.p3` *does* parse, but as `ident` + `class_ref`, which means the prettiest syntax is also the one that silently means something else. A cable with N wires is N edges between the same pair, which is already legal | |
| `D-024` | active | 2026-08-10 | **An activation bar is a block, not an inferred pairing.** `W-047` writes `activate api { … }` — a `group` whose `edge_members` are the messages the bar spans, rendered as a bar down that participant's lifeline instead of a frame. The mechanism is already built and already correct: a group records `members` **and** `edge_members`, and a sequence frame spans message *rows*, which is exactly a bar's extent. Nesting comes free because groups nest. Inferring the bar from a call and its matching reply was refused because "matching" is a guess as soon as two calls are in flight, and a silent guess is what `D-005` exists to prevent | |
| `D-023` | active | 2026-08-10 | **One keyword per slot form.** An archetype that needs a second positional shape declares a second word for it — in `xy`, `series s 4 8 15` spreads across the axis and `scatter s 1 4 3 8` reads coordinates — rather than picking a shape from how many values were written, or from a mode set elsewhere in the file. Arity-dispatch cannot tell two forms of the same length apart and leaves the error message unable to say which was meant; a mode property makes one line mean two things depending on a setting written somewhere else, which is what `D-005` refuses everywhere else. This is `D-015` applied *inside* one archetype instead of only between them, so it costs a word of vocabulary and nothing in either grammar. Answers `Q-004`; `W-048` is the first to use it | |
| `D-022` | active | 2026-07-31 | **Every data property lives with the genre that reads it.** The thirteen archetype-specific properties left on the shared table (`value`, `bits`, `x`, `y`, `wave`, `data`, `points`, `start`, `length`, `kind`, `guard`, `from-card`, `to-card`) moved into `item_props`. Completion is then right by construction rather than by a filter someone has to remember, and a property written in the wrong genre is an unknown property whose message names the genre that does read it. Completes `D-019` | |
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
| `D-015` | amended | 2026-07-31 | Each archetype gets its **own keyword and positional data** — `slice organic "Organic search" 4820` — over one shared statement form, so the grammars, the validator and the editor stay written once. Seven rules and the reasoning in [`vocabulary.md`](vocabulary.md). Amended by `D-023`: an archetype may declare **more than one** keyword, one per positional shape | |
| `D-014` | active | 2026-07-31 | The target is **four audiences — stats, dev, embedded and data** — and, at the end of it, an **embeddable Rust library** that other programs call to generate documents on the fly. The archetype survey in *Open* is scoped to that, and `W-027` is the library itself | |

The full reasoning for each lives in `AGENTS.md` and `docs/reference.md`; these lines are pointers,
not copies.

### Work landed

| id | date | title | commit |
|---|---|---|---|
| `W-062` | 2026-08-10 | `--rows name=file.csv`, and `template::rows_from_csv`. A CSV is one table and maps exactly onto repetition — a header names the fields, every other line writes its statement once. Hand-rolled, like everything else here: the dialect is twenty lines | this commit |
| `W-061` | 2026-08-10 | `bgraph watch --data` — the live preview fills a template from its data, re-read on every request, so editing the *numbers* reloads too. Without data the preview draws the shape holes and all, which is the opposite of a one-shot render and deliberately so: the shape is what you are editing before the data is wired up | this commit |
| `W-060` | 2026-08-10 | `docs/vocabulary.md` rewritten from the tables, a GitHub Actions workflow, and a golden test for the thirty-three checked-in SVG | 152f834 |
| `W-068` | 2026-08-10 | The window's wheel zoom, which was wrong in both directions: the backends disagree about the size of one notch — X11 sends ±1, Wayland the axis in fractional pixels, about ±15 — and treating that as a factor made one notch a 2.8× zoom one way and a *negative* scale the other, shown by the clamp as 5%. Only the sign is read now, and the zoom is multiplicative: 5% a notch, 10% with shift, and a notch each way returns exactly | this commit |
| `W-067` | 2026-08-10 | A **native window** for `watch`, behind an off-by-default `window` feature. `minifb` + `resvg` rather than a webview or `winit`: the *windowing* is the expensive half, so 30 crates become 108 where `winit` + `softbuffer` would make them 249, Tauri 428. No WebKitGTK, no server, no port, no browser — and no HTML, since the SVG is rasterised directly. The HTTP preview stays, chosen by `--server` or when there is no display, because that is what a remote machine wants rather than a lesser thing | this commit |
| `W-066` | 2026-08-10 | `watch` takes a picture as readily as a source: an `.svg` is shown as it is, so the viewer works on a file bgraph never wrote, and since the reload follows the file's timestamp it keeps up with whatever is regenerating it. The extension decides which it is, the same rule `-o` uses. (Shipped with a second command, `view`, for the same job; removed the same day — two words for one thing is the duplication this project refuses everywhere else) | this commit |
| `W-065` | 2026-08-10 | Eleven rendering defects found by reading every example on screen: treeview rows drawn over each other, a gitgraph merge diagonal crossing its own commit message, harness wires all turning in one column, a rack's unit numbers under the frame's border, a compartment title adrift in a box far wider than itself, a chart title sitting on its top gridline, a gantt name touching its bar, a group title drawn through its own frame, a hexagon's slant cutting a second line of text, a sequence frame's name across a lifeline, and a three-set venn whose circles never met. Also **non-deterministic SVG**: the venn's overlap labels came out of a `HashMap`, so the same file rendered to different bytes — the golden test is what caught that, and nothing else would have | this commit |
| `W-064` | 2026-08-10 | `check::after_layout` — the validator's second pass, for the diagnostics that need the boxes placed. First of them: an `ortho` edge crossing a box that is not one of its endpoints, which mitigates `X-004`. `PlacedEdge` carries its span so the warning can point at a line | this commit |
| `W-063` | 2026-08-10 | `bgraph fmt` — indentation, trailing whitespace and runs of blank lines. Lexical, so comments and blank lines survive, and **interior alignment is left alone**: indentation is structural and the formatter's business, column alignment is taste and is not. `--check` exits non-zero, so one command serves a person and a build. All thirty-three examples already passed it, which is the evidence it followed the house style rather than inventing one | this commit |
| `W-059` | 2026-08-10 | The PDF draws the edges the SVG draws — routed, clipped, with heads and labels | see commit |
| `W-058` | 2026-08-10 | **Placeholders, in the grammars and in the editor.** A hole may stand where a *name* goes — `slice {{r.id}} "…"`, `{{a}} -> b`, `right-of {{a}}`, `group {{g}}` — in both grammars, with a corpus case holding them to it. Not where the archetype goes: that is what makes the rest of the file checkable. The language server gained the template's own rules as spanned diagnostics (a line repeating over two lists was a surprise at render time), hover on a hole, completion of the holes a file already uses, and the holes in its outline | this commit |
| `W-057` | 2026-08-10 | **A template is filled from data, not from flags.** `Bindings::from_json` and `bgraph <template> --data <file\|->`, so the binary can bind what the library always could — lists and rows, which is the case templating exists for and the one `--set` could not express. `serde_json` was already a dependency, so this gates code and nothing else. It also fixed a real hole: substitution knew whether it was inside a string but not whether it was in an **identifier** position, so a name from data came out quoted and the file stopped parsing | this commit |
| `W-056` | 2026-08-10 | `editors/vscode/package.sh` — builds a `.vsix` with `zip` rather than fetching `@vscode/vsce`, since a VSIX is a zip with an OPC part and a manifest. Every fact the manifest repeats is read back out of `package.json`, and a test fails if one is written twice. `--no-server` builds a highlighting-only package with no npm and no network, which works because the client is now required *inside* `activate` | this commit |
| `W-055` | 2026-08-10 | Four capabilities the language server was missing, all of them things it *knew* and did not say: **signature help** for positional data — the idiomatic form, and the one shape nothing could tell you while typing it; **document colours**, so `#38bdf8` gets a swatch and a picker; **document highlight**, so an id lights up everywhere it is written and only whole; and **snippet completions**, so accepting `slice` writes the statement rather than the word. Hover also answers on an archetype's own word, and reports the paint the cascade settled on rather than only the geometry | this commit |
| `W-054` | 2026-08-10 | A harness wire lands **on its pin**. Retires `X-016` | this commit |
| `W-042` | 2026-08-10 | **PDF export**, hand-written, from the CLI (`-o out.pdf`) and the library (`render_source_pdf`, covered by `D-018`). Vector output needs no rasteriser, so the dependency list is still five — and a PDF viewer must have Helvetica, which is the table `W-040` added, so text there is exact rather than estimated. PNG is *not* delivered: `D-027` says why, and `X-017` carries it | this commit |
| `W-038` | 2026-08-10 | A VS Code extension. **TextMate rather than WASM tree-sitter**: VS Code reads TextMate natively, so nothing is compiled, pinned or cached, and the semantic half comes from `bgraph-lsp` — the same binary Zed talks to. The failure mode changes with the choice, so the guard did too: a stale `.scm` breaks Zed loudly, a stale TextMate grammar silently stops colouring a word, and `the_textmate_grammar_knows_every_word_the_compiler_does` re-derives the list from `props.rs` on every run | this commit |
| `W-040` | 2026-08-10 | Per-character advance widths in place of the flat `0.56`. Removes the larger half of `X-005` — the four-to-one variation *within* a typeface — and makes `text_width` measure the **widest** line rather than the longest, which had been a different line. Answers the question the chantier carried with `D-026`: the cross-family error cannot be removed at all, so `X-005` is narrowed, not retired | this commit |
| `W-049` | 2026-08-10 | `type gitgraph` — commits on rails. Parentage is the edges and a commit with two parents is a merge, landing one step past its **latest** parent. Only the rails are computed, which is where this stays on the right side of `D-001`: the author declares every commit, its branch and what came before it. A parent declared after its child is refused, because the picture would show history running backwards | this commit |
| `W-041` | 2026-08-10 | A tree layout engine, and the three genres over it — `orgchart`, `mindmap`, `treeview`. Hierarchy from **edges**, not group nesting: an org chart is written `ceo -> cto`, and nesting a manager in a group *and* declaring them as a box would be two statements for one fact. The engine is the subtree-width pass — measure bottom-up, hand each child a disjoint slice — so siblings cannot overlap by construction and there is nothing to repair. One cargo feature each, per `W-028` | this commit |
| `W-045` | 2026-08-10 | `type harness` — connectors whose pins are member lines, wires that carry the pin they land on at each end (`D-025`). No grammar moved. A wire is checked against the pins that are really there and against being wired twice — the two mistakes that make a harness drawing useless to whoever is holding the crimp tool. The line joins the connectors rather than the pin rows: `X-016` | this commit |
| `W-053` | 2026-08-10 | `type venn` — overlapping sets. With **two** the centre distance is solved by bisection so the lens area is the number written; with **three** the overlaps are labelled but arranged, and `check` says so out loud; **four is refused**, naming an upset plot as the honest picture. Circles scale by area, not radius | this commit |
| `W-043` | 2026-08-10 | `type network` — topology on the free path, per the `W-022` precedent: the archetype supplies the vocabulary, the shapes and the diagnostics, and the anchors still place. A `role` picks the outline and deliberately **not** the fill; `addr` is a line of the box, `speed` a line of the link; a `group` is a segment, and a host on no link and in no segment is reported | this commit |
| `W-047` | 2026-08-10 | Activation bars for `type sequence`, per `D-024`. `activate` is `group` under a second word, in both grammars — its identifier **references** a participant instead of declaring one, which is the only reason it needed its own word: a group id and an item id share one namespace. `frame_names` keeps an activation out of the frame namespace everywhere, so a message touching that participant stays a message | this commit |
| `W-044` | 2026-08-10 | `type rack` — a rack elevation: a numbered frame, devices occupying inclusive spans of units, counted from **1 at the bottom** as a rack is labelled. Overlaps and devices past the top of the frame are errors; empty units are silent, because a rack with gaps has room where a packet with gaps does not add up | this commit |
| `W-046` | 2026-08-10 | `type radar` — axes radiating from a centre, one closed polygon per series. Reuses `bar`'s `categories` and `series` verbatim rather than inventing a synonym for the same idea. Every axis shares one scale and the rings start at zero: a per-axis scale, or rings starting at the smallest value, is how this chart is normally made to lie | this commit |
| `W-052` | 2026-08-10 | `type journey` — steps in the order they are taken, one line per actor. Scores are positional against `actors`, exactly as `bar`'s points are against `categories`; `group`s become the sections banded above the steps they hold. The axis is labelled at every whole score rather than by `nice_scale`, because a satisfaction score has no meaning between two of them | this commit |
| `W-050` | 2026-08-10 | `type kanban` — a board. Columns are `group`s in declaration order, cards stack inside them, and a column may carry a work-in-progress `limit` that `check` reports when exceeded — the one thing a board is kept for. The count and the limit are drawn in the column header | this commit |
| `W-051` | 2026-08-10 | `type requirements` — requirements and their typed SysML links. A `class` box plus a relation vocabulary drawn in guillemets; `risk` and `verify` say how likely it is to go unmet and how it will be shown to be met. Made the ruled-compartment flag declarative (`ArchetypeDef::members`) instead of a name list in the layout, and closed a `D-005` hole: positional **edge** data was never validated, so `a -> b nonsense` passed `check` in `er` too | this commit |
| `W-039` | 2026-08-10 | `type bytefield` — a wire-format diagram at byte granularity. One layout shared with `packet` (`packet_like`), parameterised by a `Granularity`: 16 cells per row instead of 32, a ruler stepping by four instead of eight, and `bytes` in place of `bits`. Seventeen archetypes ship | this commit |
| `W-048` | 2026-08-10 | True scatter for `type xy`. `scatter s 1 4 3 8` reads coordinates two at a time where `series s 4 8 15` spreads across the axis — the first consumer of `D-023`, and the change that made the *written keyword*, not the archetype's primary form, decide the positional shape everywhere (`NodeDecl::props_for`) | this commit |
| `W-037` | 2026-08-07 | `label-offset` — the manual escape hatch out of automatic label placement. Moves an edge label, a node's text or a group's title; on an edge it also pins the label, so the solver stops second-guessing a position the author declared. The viewBox follows a label moved outside its shape | this commit |
| `W-001` | 2026-07-31 | Core language: pest grammar, spanned AST, `check`, anchor solver, style cascade | `1b53ec3` |
| `W-002` | 2026-07-31 | SVG renderer: 6 shapes, straight and ortho routing, arrow heads, content-derived viewBox | `1b53ec3` |
| `W-003` | 2026-07-31 | `bgraph watch` — dependency-free live preview | `1b53ec3` |
| `W-004` | 2026-07-31 | `bgraph-lsp`: diagnostics, completion, hover, go-to-definition, outline | `1b53ec3` |
| `W-005` | 2026-07-31 | Tree-sitter grammar and Zed extension | `1b53ec3`, regenerated `d6997b1` |
| `W-006` | 2026-07-31 | `type` archetypes (`pie`, `packet`), quiet completion, explicit surface, generated reference manual | `4b4c548` |
| `W-033` | 2026-07-31 | Templating: `{{holes}}` that parse, typed bindings, context-aware escaping, `--set` on the CLI | 6d166eb |
| `W-036` | 2026-07-31 | Verification pass over the editor, the language server and the documentation — and the four defects it found | 37c4b94 |
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
| `Q-002` | 2026-07-31 | Which archetype after `pie` and `packet`? | answered by the work — `W-016` … `W-026` all landed, so the ordering question stopped having a subject | **Restored 2026-08-10.** This line had been *deleted* rather than closed, which is exactly what the rule at the top of this file forbids: an identifier never changes and is never reused, and state is explicit. A deleted question reads as a question that was never asked. Its last recorded note said the survey had answered it in part and what remained was the order, naming `W-019`, `W-024`, `W-018` and `W-016` as the ones with no prerequisite |
| `Q-003` | 2026-07-31 | What shape should the embeddable API take? | `D-017` | |
| `Q-004` | 2026-08-10 | How does an archetype declare a **second** slot form? | `D-023` | Raised by the idea review the same day: a true scatter for `xy` wants `(x, y)` pairs, which `D-015` had no way to express inside one archetype. Asked before it was built, because the answer lands in `props.rs`, both grammars and completion at once |

### Risks retired

| id | date | title | how |
|---|---|---|---|
| `X-001` | 2026-07-31 | The Zed extension had never been exercised inside Zed | The Archmagos installed it and reported live completion and rendering behaviour, which drove `D-011` and the `font-color` fix |
| `X-007` | 2026-07-31 | `W-029` was going to move some syntax errors to semantic ones | It did, exactly once: `alpha beta` is now an item with an unknown keyword. The message names the archetype's word, and `tests/grammar/accept/bare-words-parse-as-items.bd` pins the new category |
| `X-015` | 2026-07-31 | Prose documentation could not be checked, so it rotted | `language.md` still called a slice a `node` five archetypes after the word changed. Snippets in a ```` ```bgraph ```` fence are now parsed and checked by the suite, and every archetype's section of the reference carries an `include_str!`d example the suite renders |
| `X-014` | 2026-07-31 | The highlighter captured only what existed when it was written | `date`, `duration`, `placeholder`, `member` and an edge's positional data were parsed by the grammar and coloured by nothing. `the_highlighter_captures_every_kind_of_value` names each node type that carries text a reader looks at |
| `X-013` | 2026-07-31 | The language server matched item keywords against a hard-coded list of five | So every archetype's own word — `slice`, `box`, `signal`, `task`, `field`, `series`, `event`, `entity`, `point`, `class`, `state`, `block` — read as an *edge*, and its block was offered `arrow` and `router` and denied `fill`, `shape` and `size`. Broken since `W-029`. It now asks whether the statement carries an arrow, which is what distinguishes the two and needs no list |
| `X-012` | 2026-07-31 | Deeply nested groups aborted the host process | `pest` descends recursively, so ~1500 nested `{` overflowed the stack — `SIGABRT`, which a caller cannot catch, in a library meant to be embedded. A lexical depth scan now runs *before* pest and refuses past `MAX_NESTING` (64). Proven by removing the guard and watching the suite abort |
| `X-011` | 2026-07-31 | Two tests shelling out to `tree-sitter` in parallel could catch the parser library mid-rebuild | It fired twice before being fixed. The calls now take a mutex; serialising them costs nothing, and a flaky guard stops being believed |
| `X-016` | 2026-08-10 | A harness wire joined the connectors, not the pin rows | `W-054` draws it to the pin. The way in was not a new endpoint mechanism: the compartment pass already computes where each member row lands, so it now **records** those positions and the wire is a `Polyline` from one row to the other. The pin pair came off the label — the geometry says it better than text beside a line ever did |
| `X-003` | 2026-07-31 | Self-loops were declared but not drawn | `W-009` draws them; the stale "not drawn yet" warning is gone |

### Ideas

| id | state | title |
|---|---|---|
| `I-001` | discarded | Absolute coordinates only — rejected in favour of `D-001`; every insertion would renumber its neighbours |
| `I-002` | discarded | Grid layout as the core model — too rigid for organic diagrams |
| `I-003` | discarded | Auto-layout with per-node overrides — reintroduces the very fight with the engine the project exists to end |
| `I-004` | discarded | Auto-placement archetypes (`flow` layered from edges, `tree`, `grid`) — set aside when `type` turned out to mean genre, then **discarded 2026-08-10**: it reintroduces the very fight with the engine `D-001` exists to end, which is what killed `I-001` … `I-003` too |
| `I-005` | promoted | Further chart genres — surveyed against Mermaid, Kroki, WaveDrom and Vega-Lite, then split into `W-016` … `W-026` |
| `I-006` | promoted | Containers and subgraphs — became `W-011`, landed |
| `I-007` | promoted | PNG and PDF export — became `W-042`, which absorbed `I-015`: exporting from the CLI and exporting from the library are one job |
| `I-008` | promoted | Real font metrics instead of the estimate in `X-005` — became `W-040` |
| `I-009` | promoted | `type bytefield` — `packet` with a byte-granular ruler — became `W-039` |
| `I-010` | promoted | Infrastructure genres (the nwdiag / rackdiag / WireViz territory) — split by genre, since they share a subject and not an engine: `W-043` network, `W-044` rack, `W-045` harness |
| `I-011` | promoted | Tree genres: mindmap, treeview, org chart — one tree-layout engine, became `W-041` |
| `I-012` | promoted | `type radar` and `type venn` — split 2026-08-10: radar reuses `W-012`'s scales and the series notion, and became `W-046`; venn is circle geometry with nothing in common with a plot frame, and stayed an idea as `I-018` |
| `I-013` | discarded | Process and planning genres — a catch-all, and a catch-all never gets decided because one member always holds the others back. C4 had already shipped as `W-025`; the rest split into `I-019` … `I-022` on 2026-08-10 |
| `I-014` | promoted | `type gitgraph` — commits, branches and merges — became `W-049`. The layout being its own problem is the cost, not a reason to refuse: unlike `I-004`, the author still declares *what* is there, and only the rails are computed |
| `I-015` | discarded | Raster and PDF export from the library — the same work as `I-007` seen from the embedding end; merged into `W-042` rather than tracked twice |
| `I-016` | promoted | Activation bars for `type sequence` — became `W-047` |
| `I-017` | promoted | True scatter for `type xy` — became `W-048` once `Q-004` was answered by `D-023`. What held it back was a language decision, not effort |
| `I-018` | promoted | `type venn` — became `W-053`. Split out of `I-012` first, which is what made it visible that it is a geometry problem and not a chart |
| `I-019` | promoted | `type kanban` — became `W-050` |
| `I-020` | promoted | `type journey` — became `W-052` |
| `I-021` | parked | `type bpmn` — the notation is large and mostly ceremony. **The only idea still parked**, and deliberately: which part of it is the useful part has to be chosen before the work can be costed, and nobody has chosen |
| `I-022` | promoted | `type requirements` — became `W-051` |
