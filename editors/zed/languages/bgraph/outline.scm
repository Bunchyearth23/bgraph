(style_declaration
  "style" @context
  name: (identifier) @name) @item

(item_declaration
  keyword: (item_keyword) @context
  name: (identifier) @name
  label: (string)? @context) @item

; `group` and `activate` are the same declaration under two words, so the
; outline captures the keyword field rather than the literal.
(group_declaration
  keyword: _ @context
  name: (identifier) @name
  label: (string)? @context) @item

(graph_declaration
  "graph" @context @name) @item
