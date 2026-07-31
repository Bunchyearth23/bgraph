(style_declaration
  "style" @context
  name: (identifier) @name) @item

(item_declaration
  keyword: (item_keyword) @context
  name: (identifier) @name
  label: (string)? @context) @item

(group_declaration
  "group" @context
  name: (identifier) @name
  label: (string)? @context) @item

(graph_declaration
  "graph" @context @name) @item
