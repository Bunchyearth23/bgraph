; Indentation: one level inside a pair of brackets, and the closer comes back
; out to the line that opened it.
;
; Matched on the **brackets** rather than on the node types that hold them.
; Naming the nodes was the first answer and it named only `block`, so a
; `group { … }`, a `component { … }` and a `use … { … }` — all of which parse
; as `group_block` — indented not at all, while `node a { … }` beside them did.
; A list of node types written by hand is a list that stops being true the day
; the grammar gains one, which is the same lesson `X-014` and `W-101` taught
; about the highlighter and the corpus (`W-129`).
(_ "{" "}" @end) @indent

(_ "(" ")" @end) @indent
