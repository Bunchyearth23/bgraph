# Per-archetype vocabulary — design note

Cited by `D-015` and `W-029` in [`INDEX.md`](INDEX.md). **Implemented.** This
records what was decided and why; [`reference.md`](reference.md) describes what
the binary does.

## The problem

`D-008` had every archetype reuse the same statements: a `node` was a box, a
slice or a bitfield depending on the genre. That kept the cascade, the
validator, the two grammars and the editor written once — but it made a pie
read like this:

```
node organic "Organic search" { value: 4820 }
```

It says *node* when it means *slice*, and buries the one number that matters
inside a property block. A language that reads wrong is a language people
write wrong.

## The shape

One statement form, for every archetype:

```
<keyword> <id> ["label"] <positional data…> [.class …] [<placement>] { props }
```

```
type pie
slice organic "Organic search" 4820
slice direct  "Direct"         2410
slice paid    "Paid"            410 { fill: #f43f5e }

type packet
field sport "Source port"      0 15
field dport "Destination port" 16 31

type wave
signal clk "Clock" "p......"
signal bus "Data"  "x.=.=x" { data: "head" "body" }

type free
node a "A" at (0, 0)
```

This keeps `D-008`'s benefit — **one grammar rule, one validator, one
completion engine** — while giving each genre its own words. `props.rs` gains
a per-archetype declaration of the item keyword and the shape of its
positional slots; nothing else learns about archetypes.

## The seven rules

1. **The keyword names the element.** `node`, `slice`, `field`, `signal`,
   `series`, `task`, `participant`, `state`, … declared per archetype.
2. **Positional data is sugar for the named property.** `slice a 4820` and
   `slice a { value: 4820 }` produce the same thing. The property is the
   model; the position is the shorthand. Edges take slots too, by the same
   rule — `a -> b 40` in a sankey, where the datum rides on the link rather
   than on what it joins.
3. **`node` is a universal synonym**, and so is `edge`. Nothing that works
   today stops working. The genre keyword is idiomatic, not mandatory.
4. **Identifier first, label second.** `slice organic "Organic search" 4820`.
   Without a label the identifier is displayed: `slice referral 1180`.
5. **The first string after the identifier is always the label.** So an
   archetype whose data is a string obliges you to write the label —
   `signal clk "Clock" "p......"`. `check` says so when it is missing.
6. **A range is two numbers**, not `0..15`: `..` is already a dotted-line
   arrow, and `size: 100 60` / `cell: 20 46` / `dash: 6 3` already read this
   way.
7. **The implicit theme follows the keyword.** `style slice { }` in a pie,
   `style signal { }` in a waveform — with `style node` and `style edge`
   remaining universal aliases.

Relations stay universal (`->`, `-->`, `<->`, …); the archetype decides
whether that means a flow, a message or a transition, and per-genre nuance
rides on properties (`{ guard: … }`, `{ from-card: one to-card: many }`).

## The one judgement call not put to a vote

**The keyword set is open in the grammar and closed in `check`.**

```pest
item_decl = { ident ~ ident ~ string? ~ (pos_value | class_ref)* ~ placement? ~ block? }
pos_value = _{ !(place_kw) ~ value }
```

(`pos_value` excludes the placement keywords, or `at` and `right-of` would be
eaten as bare-word data before `placement?` ever looked. Data and `.class`
interleave freely — the leading `.` keeps them apart, so there is no order to
remember.)

The parser accepts any `<word> <word>`; `check` rejects a keyword the current
archetype does not declare, and suggests the right one. The alternative — an
alternation of literal keywords — would mean editing *both* grammars and
regenerating the tree-sitter parser every time an archetype is added, which
is exactly the cost this design exists to avoid.

The price is that a line like `alpha beta` moves from a syntax error to a
semantic one (`type free declares nodes, not alphas`). The
message is better; the category is different. `tests/grammar/reject/no-arrow.bd`
moved to `accept/`, and that is not a regression — but it is the kind of thing
that should be noticed on purpose rather than discovered.

Recorded as `X-007`, and it happened exactly once —
`tests/grammar/accept/bare-words-parse-as-items.bd` pins the new category.

One thing the design did not anticipate: **a statement has to run to its
separator**. Without that rule `slice a b` is ambiguous — is `b` data or the
next statement? — and tree-sitter refuses to generate a parser at all. Both
grammars now require a separator, which also means `node a node b` on one line
is a single item with two stray values rather than two items.

## An archetype may declare a second word

Written after the fact, when `xy` needed one (`D-023`).

Seven rules were not quite enough. An archetype sometimes has **one picture and
two shapes of data**: an `xy` plot whose numbers are spread across the axis, and
the same plot whose numbers are coordinates. Rule 2 says positional data is
sugar for the named property — but which property, when the same statement
could fill either?

The answer is a second **word**, not a second reading of the same one:

```
series north 4 8 15        // spread across the axis
scatter north 1 4  3 8     // read two at a time, as coordinates
```

`ArchetypeDef::alt_items` carries it. **The word decides the shape, never the
number of values written** — two shapes of the same length would be
indistinguishable, and the error message could not say which one it thought you
meant. That is the same reasoning as rule 1, applied inside an archetype rather
than only between them, which is why `D-015` is `amended` rather than replaced.

Neither grammar moved for it: `item_keyword` is an identifier in both, so a new
word is not a new token.

## A hole may stand where a name goes

Also after the fact (`W-058`). A template writes the id of the statement it
fills:

```
slice {{row.id}} "{{row.name}}" {{row.value}}
```

So a name is `ident | placeholder` in both grammars, everywhere a name is
declared or referenced — an item's id, both ends of an edge, an anchor, a
group. **Not where the archetype goes**: the archetype decides which properties
exist and which word names an item, so nothing else in the file is checkable
until it is known.

This one is only visible from an editor. Substitution happens before parsing,
so the renderer never sees a hole in a name position — the grammars refused it
while the command line was perfectly happy, and the two disagreed about what a
file *is* for as long as templating had existed.

## Keywords, as they actually are

Generated from the same table the compiler reads; a test asserts this document
still lists every one.

| Archetype | Keyword | Positional data |
|---|---|---|
| `free` | `node` | — |
| `pie` | `slice` | `value` |
| `gantt` | `task` | `start` — 2 values |
| `class` | `class` | — |
| `timeline` | `event` | `date` |
| `block` | `block` | `kind` |
| `er` | `entity` | on the **edge**: `from-card`, `to-card` |
| `state` | `state` | — |
| `sequence` | `participant` | — |
| `xy` | `series` · `scatter` | `points` — a list, `pairs` — a list |
| `bar` | `series` | `points` — a list |
| `sankey` | `node` | on the **edge**: `value` |
| `wave` | `signal` | `wave` |
| `quadrant` | `point` | `x`, `y` |
| `treemap` | `box` | `value` |
| `packet` | `field` | `bits` — 2 values |
| `gitgraph` | `commit` | `branch` |
| `orgchart` | `position` | — |
| `mindmap` | `topic` | — |
| `treeview` | `entry` | — |
| `harness` | `connector` | on the **edge**: `wire` — 2 values |
| `venn` | `set` | `count`; on the **edge**: `shared` |
| `network` | `host` | `role`; on the **edge**: `speed` |
| `rack` | `device` | `u` — 2 values |
| `radar` | `series` | `points` — a list |
| `journey` | `step` | `scores` — a list |
| `kanban` | `card` | — |
| `requirements` | `requirement` | on the **edge**: `link` |
| `bytefield` | `field` | `bytes` — 2 values |
| `bpmn` | `task` · `event` · `gateway` · `data` | `kind`; `stage` + `trigger`; `gate`; `artifact`; on the **edge**: `flow` |

`node` remains a universal synonym in every genre, and every positional value
can be written as its named property instead. 29 specialised genres, plus `free`.

`bpmn` is the one with **four** words rather than one or two, and it is the
clearest case for `D-023`: a task, an event, a gateway and a data artefact are
four different things carrying four different shapes of data, and choosing
between them by counting the values after the label could not tell a task's one
word from a gateway's one word.
