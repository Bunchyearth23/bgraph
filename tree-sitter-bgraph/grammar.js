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
        $.include_declaration,
        $.note_declaration,
        $.legend_declaration,
        $.layout_declaration,
        $.component_declaration,
        $.use_declaration,
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

    // `include "shared.bds"` merges another file whole; with `as` its names
    // come in under a prefix (`D-029`).
    include_declaration: ($) =>
      seq('include', field('path', $.string), optional(seq('as', field('prefix', $.identifier)))),

    // Text that belongs to no box. It declares no name, which is what stops
    // anything referring to it.
    note_declaration: ($) =>
      seq(
        'note',
        field('text', $.string),
        repeat(field('class', $.class_reference)),
        optional(field('placement', $._placement)),
        optional(field('body', $.block)),
      ),

    // Without entries, the key uses classes with a `legend:` label.
    legend_declaration: ($) =>
      seq(
        'legend',
        repeat(field('class', $.class_reference)),
        optional(field('placement', $._placement)),
        optional(field('body', $.legend_block)),
      ),

    legend_block: ($) =>
      seq(
        '{',
        repeat($._newline),
        optional(
          seq(
            $._legend_item,
            repeat(seq(repeat1($._newline), $._legend_item)),
            repeat($._newline),
          ),
        ),
        '}',
      ),

    _legend_item: ($) => choice(repeat1($.property), $.legend_entry),

    legend_entry: ($) =>
      seq(
        'entry',
        field('text', $.string),
        repeat(field('class', $.class_reference)),
        optional(field('body', $.block)),
      ),

    // v2 placement is held apart from declarations. A `place` line names its
    // target and then uses the ordinary anchor vocabulary.
    layout_declaration: ($) => seq('layout', field('body', $.layout_block)),

    layout_block: ($) =>
      seq(
        '{',
        repeat($._newline),
        optional(
          seq(
            $.layout_statement,
            repeat(seq(repeat1($._newline), $.layout_statement)),
            repeat($._newline),
          ),
        ),
        '}',
      ),

    layout_statement: ($) =>
      choice(
        seq('place', field('target', $._name), field('placement', $._placement)),
        seq('align', field('alignment', $.layout_alignment), repeat1(field('member', $._name))),
        seq('equal-width', repeat1(field('member', $._name))),
        seq('equal-height', repeat1(field('member', $._name))),
        seq('row', repeat1(field('member', $._name)), optional($.gap_clause)),
        seq('column', repeat1(field('member', $._name)), optional($.gap_clause)),
      ),

    layout_alignment: (_) =>
      choice('left', 'right', 'top', 'bottom', 'center-x', 'center-y'),

    // A component is a file you did not have to make a file of: declared
    // once, placed under a name, prefixed exactly as an included file is.
    component_declaration: ($) =>
      seq('component', field('name', $.identifier), field('body', $.group_block)),

    use_declaration: ($) =>
      seq(
        'use',
        field('component', $.identifier),
        'as',
        field('prefix', $.identifier),
        optional(field('bindings', $.block)),
      ),

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
        // Placement is declared in a scoped `layout` block; this optional
        // branch is only the inline property/member block.
        optional(
          choice(
            field('body', $.block),
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
        // `via` and the property block come in either order.
        optional(
          choice(
            seq(field('via', $.via_list), optional(field('body', $.block))),
            seq(field('body', $.block), optional(field('via', $.via_list))),
          ),
        ),
      ),

    // The places a line has to go through, in order (`D-034`). Each is
    // parenthesised, which is what lets the list need no separator — `,` is
    // already a statement separator — and what lets a point, a grid cell and a
    // place beside a box all be written the same way.
    via_list: ($) => seq('via', repeat1($._waypoint)),

    _waypoint: ($) => choice($.cell, $.point, $.relative_point),

    relative_point: ($) => seq('(', $.relative_position, ')'),

    class_reference: ($) => seq('.', $.identifier),

    arrow: (_) => token(choice('<->', '<-', '-->', '..>', '--', '->', '..')),

    // ---------------------------------------------------------- placement

    _placement: ($) => choice($.absolute_position, $.relative_position),

    absolute_position: ($) => seq('at', field('point', choice($.cell, $.point))),

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

    alignment: (_) => choice('left', 'right', 'center-x', 'center-y', 'center', 'middle', 'top', 'bottom'),

    point: ($) => seq('(', field('x', $.number), ',', field('y', $.number), ')'),

    // `at (col 3, row 2)` — still `at`, in the units `graph { grid }` declares.
    // A coordinate system and not a solver, which is why it cannot decide an
    // order (`D-031`).
    cell: ($) =>
      seq('(', 'col', field('col', $.number), ',', 'row', field('row', $.number), ')'),

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
    //
    // A qualified name — `core.api` — is one token, which is what lets a
    // leading `.box` stay unambiguously a class reference: the lexer takes the
    // longest match, so the dot in `core.api` is never seen on its own
    // (`D-029`). A dot must be followed by a name part.
    identifier: (_) =>
      /[a-zA-Z_][a-zA-Z0-9_]*(-[a-zA-Z0-9_]+)*(\.[a-zA-Z_][a-zA-Z0-9_]*(-[a-zA-Z0-9_]+)*)*/,

    number: (_) => token(seq(optional('-'), /\d+/, optional(seq('.', /\d+/)), optional(choice('px', '%')))),

    // Any run of hex digits; `check` accepts only 3, 4, 6 or 8 of them.
    // Tree-sitter regexes have no lookahead, so a length pinned here would
    // split `#1234567` into a six-digit color and a stray number.
    color: (_) => token(seq('#', /[0-9a-fA-F]+/)),

    // One token, so `extras` cannot leak whitespace or a `//` into a label.
    string: (_) => token(seq('"', repeat(choice(/[^"\\]/, seq('\\', /./))), '"')),
  },
});
