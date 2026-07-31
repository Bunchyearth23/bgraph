/**
 * Tree-sitter grammar for the bgraph diagram DSL.
 *
 * Mirrors `src/syntax.pest`. Two rules of the language shape this file:
 *   - newlines separate statements, so `extras` must NOT swallow them;
 *   - `#` starts a hex color, never a comment — comments are `//`.
 */

module.exports = grammar({
  name: 'bgraph',

  // Horizontal space and comments only: newlines are significant.
  extras: ($) => [/[ \t]/, $.comment],

  conflicts: ($) => [],

  rules: {
    source_file: ($) => repeat(choice($._statement, $._newline)),

    _statement: ($) =>
      choice($.graph_declaration, $.style_declaration, $.node_declaration, $.edge_declaration),

    _newline: (_) => choice('\n', '\r\n', ';', ','),

    comment: (_) => token(seq('//', /[^\n]*/)),

    // ---------------------------------------------------------- statements

    graph_declaration: ($) => seq('graph', field('body', $.block)),

    style_declaration: ($) =>
      seq(
        'style',
        field('name', $.identifier),
        optional(seq('extends', field('parent', $.identifier))),
        field('body', $.block),
      ),

    node_declaration: ($) =>
      seq(
        'node',
        field('name', $.identifier),
        optional(field('label', $.string)),
        repeat(field('class', $.class_reference)),
        optional(field('placement', $._placement)),
        optional(field('body', $.block)),
      ),

    edge_declaration: ($) =>
      seq(
        field('from', $.identifier),
        field('operator', $.arrow),
        field('to', $.identifier),
        optional(field('label', $.string)),
        repeat(field('class', $.class_reference)),
        optional(field('body', $.block)),
      ),

    class_reference: ($) => seq('.', $.identifier),

    arrow: (_) => token(choice('<->', '<-', '-->', '..>', '--', '->', '..')),

    // ---------------------------------------------------------- placement

    _placement: ($) => choice($.absolute_position, $.relative_position),

    absolute_position: ($) => seq('at', field('point', $.point)),

    relative_position: ($) =>
      seq(
        field('direction', $.direction),
        field('anchor', $.identifier),
        optional($.gap_clause),
        optional($.align_clause),
      ),

    direction: (_) => choice('right-of', 'left-of', 'below', 'above'),

    gap_clause: ($) => seq('gap', field('amount', $.number)),

    align_clause: ($) => seq('align', field('alignment', $.alignment)),

    alignment: (_) => choice('left', 'right', 'center', 'middle', 'top', 'bottom'),

    point: ($) => seq('(', field('x', $.number), ',', field('y', $.number), ')'),

    // ---------------------------------------------------------- blocks

    block: ($) => seq('{', repeat(choice($.property, $._newline)), '}'),

    property: ($) => seq(field('name', $.property_name), field('value', repeat1($._value))),

    // The trailing `:` is part of the token on purpose. Without it the lexer
    // cannot tell the key of the next property from a bare-word value, and
    // `{ fill: #111 stroke: #222 }` on one line becomes ambiguous — the same
    // problem the pest grammar solves with a `!(ident ~ ":")` lookahead.
    property_name: (_) =>
      token(seq(/[a-zA-Z_][a-zA-Z0-9_]*(-[a-zA-Z0-9_]+)*/, /[ \t]*/, ':')),

    _value: ($) => choice($.string, $.color, $.number, $.identifier),

    // ---------------------------------------------------------- tokens

    // A trailing hyphen is excluded so `a->b` splits into `a` `->` `b`.
    identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_]*(-[a-zA-Z0-9_]+)*/,

    number: (_) => token(seq(optional('-'), /\d+/, optional(seq('.', /\d+/)), optional(choice('px', '%')))),

    color: (_) => token(seq('#', /[0-9a-fA-F]{3,8}/)),

    // One token, so `extras` cannot leak whitespace or a `//` into a label.
    string: (_) => token(seq('"', repeat(choice(/[^"\\]/, seq('\\', /./))), '"')),
  },
});
