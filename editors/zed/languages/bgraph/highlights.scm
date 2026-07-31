; Every identifier position is captured explicitly rather than relying on a
; broad fallback, so a node reference never borrows a style's color.

; ---------------------------------------------------------------- keywords

[
  "graph"
  "style"
  "extends"
  "node"
  "at"
  "gap"
  "align"
] @keyword

(direction) @keyword
(alignment) @constant

; ---------------------------------------------------------------- names

; A node declaration is a definition; references to it are plain variables.
(node_declaration name: (identifier) @variable.special)

(edge_declaration from: (identifier) @variable)
(edge_declaration to: (identifier) @variable)
(relative_position anchor: (identifier) @variable)

; Styles read as types: declared once, applied by name.
(style_declaration name: (identifier) @type)
(style_declaration parent: (identifier) @type)
(class_reference (identifier) @type)

; ---------------------------------------------------------------- values

; The capture includes the trailing `:` — it is part of the token.
(property_name) @property

; Bare words in value position are enum members: cylinder, ortho, dashed…
(property value: (identifier) @constant)

(string) @string
(number) @number
(color) @string.special

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
