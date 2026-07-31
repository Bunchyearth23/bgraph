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

## Keywords, as currently planned

| Archetype | Keyword | Positional slots |
|---|---|---|
| `free` | `node` | — (placement is separate) |
| `pie` | `slice` | `value` — one number ✔ |
| `packet` | `field` | `bits` — two numbers ✔ |
| `wave` | `signal` | `wave` — one string ✔ |
| `bar` | `series` | `points` — a number list ✔ |
| `xy` | `series` | `points` — a number list ✔ |
| `quadrant` | `point` | `x`, `y` — two numbers ✔ |
| `sankey` | `node` | — the value is an **edge** slot: `a -> b 40` ✔ |
| `state` (`W-022`) | `state` | — |
| `sequence` (`W-021`) | `participant` | — |
| `class` / `er` (`W-023`) | `class` / `entity` | — (members are `W-013`) |
| `treemap` | `box` | `value` — one number ✔ |
| `gantt` / `timeline` (`W-026`) | `task` / `event` | start and duration (`W-014`) |
| `block` (`W-025`) | `block` | — |

✔ marks what is implemented. The rest is the shape the survey suggests, to be
confirmed as each archetype is built — and each costs one `item` and one
`slots` line, with neither grammar touched.
