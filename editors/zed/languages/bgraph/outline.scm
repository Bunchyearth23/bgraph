(style_declaration
  "style" @context
  name: (identifier) @name) @item

(node_declaration
  "node" @context
  name: (identifier) @name
  label: (string)? @context) @item

(graph_declaration
  "graph" @context @name) @item
