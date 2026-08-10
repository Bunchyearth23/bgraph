; Every identifier position is captured explicitly rather than relying on a
; broad fallback, so a node reference never borrows a style's color.

; ---------------------------------------------------------------- keywords

[
  "type"
  "graph"
  "style"
  "group"
  "activate"
  "extends"
  "at"
  "gap"
  "align"
] @keyword

; Each archetype names its own elements — `node`, `slice`, `field`. The word
; is an identifier in the grammar, so it is captured by rule, not by literal.
(item_keyword) @keyword

(direction) @keyword
(alignment) @constant

; ---------------------------------------------------------------- names

; An item or group declaration is a definition; references are plain variables.
(item_declaration name: (identifier) @variable.special)
(group_declaration name: (identifier) @variable.special)

(edge_declaration from: (identifier) @variable)
(edge_declaration to: (identifier) @variable)
(relative_position anchor: (identifier) @variable)

; The archetype name is a fixed vocabulary word, not a user identifier.
(type_declaration name: (identifier) @constant.builtin)

; Styles read as types: declared once, applied by name.
(style_declaration name: (identifier) @type)
(style_declaration parent: (identifier) @type)
(class_reference (identifier) @type)

; ---------------------------------------------------------------- values

; The capture includes the trailing `:` — it is part of the token.
(property_name) @property

; Bare words in value position are enum members: cylinder, ortho, dashed…
(property value: (identifier) @constant)

; Positional data after the label — `slice a "A" 4820`, and on an edge too:
; `order -> line "contains" one many`.
(item_declaration data: (identifier) @constant)
(edge_declaration data: (identifier) @constant)

; A member line — `- id: uuid` on a class, an entity or anything with a body.
(member) @string.special.symbol

(string) @string
(number) @number
(color) @string.special

; A calendar day and a length of time are their own tokens, so a `gantt` or a
; `timeline` reads as data rather than as unhighlighted text.
(date) @number
(duration) @number

; `{{hole}}` — bound at render time, so it is neither a value nor an
; identifier. It reads as the interpolation it is.
(placeholder) @embedded

; ---------------------------------------------------------------- syntax

(arrow) @operator

[
  "{"
  "}"
  "("
  ")"
] @punctuation.bracket

[
  ","
  ";"
  "."
] @punctuation.delimiter

(comment) @comment
