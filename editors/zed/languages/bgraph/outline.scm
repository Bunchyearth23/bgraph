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

(legend_declaration
  "legend" @context @name) @item

; A component is a declaration like any other, and its placings are what the
; reader scrolls looking for.
(component_declaration
  "component" @context
  name: (identifier) @name) @item

(use_declaration
  "use" @context
  component: (identifier) @context
  prefix: (identifier) @name) @item

; What a file pulls in belongs at the top of its outline: it is the first
; thing that explains a name the file never declares.
(include_declaration
  "include" @context
  path: (string) @name) @item
