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

  // Keyword extraction: `type`, `node`, … are lexed as identifiers first and
  // reclassified, so a node may still be named after one where the parser
  // expects an identifier — matching the pest grammar's backtracking.
  word: ($) => $.identifier,

  conflicts: ($) => [],

  rules: {
    // Statements must be separated. Without it `slice a b` is ambiguous —
    // is `b` a positional value or the start of the next statement?
    source_file: ($) =>
      seq(
        repeat($._newline),
        optional(
          seq(
            $._statement,
            repeat(seq(repeat1($._newline), $._statement)),
            repeat($._newline),
          ),
        ),
      ),

    _statement: ($) =>
      choice(
        $.type_declaration,
        $.graph_declaration,
        $.style_declaration,
        $.group_declaration,
        $.edge_declaration,
        $.item_declaration,
      ),

    _newline: (_) => choice('\n', '\r\n', ';', ','),

    comment: (_) => token(seq('//', /[^\n]*/)),

    // ---------------------------------------------------------- statements

    // The archetype: `type pie { … }`. Absent means `free`.
    type_declaration: ($) =>
      seq('type', field('name', $.identifier), optional(field('body', $.block))),

    graph_declaration: ($) => seq('graph', field('body', $.block)),

    // A container. Its block holds properties *and* statements, so a group
    // nests. Its rectangle is derived from its members, never declared.
    // `activate` is the same declaration under another word: its identifier
    // names an existing item rather than declaring one (D-024).
    group_declaration: ($) =>
      seq(
        field('keyword', choice('group', 'activate')),
        field('name', $._name),
        optional(field('label', $.string)),
        // No positional data: a group has no slots, it is a frame.
        repeat(field('class', $.class_reference)),
        optional(field('placement', $._placement)),
        field('body', $.group_block),
      ),

    // Entries are separated, for the same reason statements are: without it
    // `padding: 22 node a` cannot be told from a property with three values.
    group_block: ($) =>
      seq(
        '{',
        repeat($._newline),
        optional(
          seq(
            $._group_entry,
            repeat(seq(repeat1($._newline), $._group_entry)),
            repeat($._newline),
          ),
        ),
        '}',
      ),

    _group_entry: ($) => choice($.property, $._statement),

    style_declaration: ($) =>
      seq(
        'style',
        field('name', $.identifier),
        optional(seq('extends', field('parent', $.identifier))),
        field('body', $.block),
      ),

    // The leading identifier is the archetype's element keyword — `node`,
    // `slice`, `field`. Open here, closed by `check`, so adding an archetype
    // touches neither grammar.
    item_declaration: ($) =>
      seq(
        field('keyword', $.item_keyword),
        field('name', $._name),
        // "The first string is always the label" is encoded here rather than
        // left to the reader: with a label, later strings are data; without
        // one, data cannot begin with a string. Exactly one parse either way.
        choice(
          seq(
            field('label', $.string),
            repeat(choice(field('data', $._value), field('class', $.class_reference))),
          ),
          // Data and `.class` may interleave: the leading `.` keeps them apart.
          repeat(choice(field('data', $._bare_value), field('class', $.class_reference))),
        ),
        // Either order reads naturally, and `{` cannot be mistaken for a
        // placement keyword.
        optional(
          choice(
            seq(field('placement', $._placement), optional(field('body', $.block))),
            seq(field('body', $.block), optional(field('placement', $._placement))),
          ),
        ),
      ),

    // A hole may stand where a name goes (see the pest grammar for why not
    // where the archetype goes).
    _name: ($) => choice($.identifier, $.placeholder),

    item_keyword: ($) => $.identifier,

    edge_declaration: ($) =>
      seq(
        field('from', $._name),
        field('operator', $.arrow),
        field('to', $._name),
        // Same rule as an item: the first string is the label, and what
        // follows is positional data for archetypes that declare edge slots.
        choice(
          seq(
            field('label', $.string),
            repeat(choice(field('data', $._value), field('class', $.class_reference))),
          ),
          repeat(choice(field('data', $._bare_value), field('class', $.class_reference))),
        ),
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
        field('anchor', $._name),
        optional($.gap_clause),
        optional($.align_clause),
      ),

    direction: (_) => choice('right-of', 'left-of', 'below', 'above'),

    gap_clause: ($) => seq('gap', field('amount', $.number)),

    align_clause: ($) => seq('align', field('alignment', $.alignment)),

    alignment: (_) => choice('left', 'right', 'center', 'middle', 'top', 'bottom'),

    point: ($) => seq('(', field('x', $.number), ',', field('y', $.number), ')'),

    // ---------------------------------------------------------- blocks

    // A line opening with `- ` or `+ ` is a member — a class's field, a
    // table's column — and runs to the end of the line. The space is what
    // keeps `-4` a number.
    block: ($) => seq('{', repeat(choice($.property, $.member, $._newline)), '}'),

    member: (_) => token(seq(choice('- ', '+ '), /[^\n]*/)),

    property: ($) => seq(field('name', $.property_name), field('value', repeat1($._value))),

    // The trailing `:` is part of the token on purpose. Without it the lexer
    // cannot tell the key of the next property from a bare-word value, and
    // `{ fill: #111 stroke: #222 }` on one line becomes ambiguous — the same
    // problem the pest grammar solves with a `!(ident ~ ":")` lookahead.
    property_name: (_) =>
      token(seq(/[a-zA-Z_][a-zA-Z0-9_]*(-[a-zA-Z0-9_]+)*/, /[ \t]*/, ':')),

    // `date` and `duration` come before `number`, or `2026-07-31` would lex
    // as three numbers and `12d` as a number followed by an identifier.
    // `{{name}}` is a hole the library fills at render time. It parses so a
    // template stays editable — the editor would otherwise light up every line.
    _value: ($) =>
      choice($.string, $.color, $.date, $.duration, $.number, $.placeholder, $.identifier),

    placeholder: (_) => token(seq('{{', /[^}]*/, '}}')),

    date: (_) => token(seq(/\d{4}/, '-', /\d{2}/, '-', /\d{2}/)),

    duration: (_) =>
      token(seq(/\d+/, optional(seq('.', /\d+/)), choice('mo', 'w', 'd', 'h'))),

    _bare_value: ($) =>
      choice($.color, $.date, $.duration, $.number, $.placeholder, $.identifier),

    // ---------------------------------------------------------- tokens

    // A trailing hyphen is excluded so `a->b` splits into `a` `->` `b`.
    identifier: (_) => /[a-zA-Z_][a-zA-Z0-9_]*(-[a-zA-Z0-9_]+)*/,

    number: (_) => token(seq(optional('-'), /\d+/, optional(seq('.', /\d+/)), optional(choice('px', '%')))),

    // Any run of hex digits; `check` accepts only 3, 4, 6 or 8 of them.
    // Tree-sitter regexes have no lookahead, so a length pinned here would
    // split `#1234567` into a six-digit color and a stray number.
    color: (_) => token(seq('#', /[0-9a-fA-F]+/)),

    // One token, so `extras` cannot leak whitespace or a `//` into a label.
    string: (_) => token(seq('"', repeat(choice(/[^"\\]/, seq('\\', /./))), '"')),
  },
});
