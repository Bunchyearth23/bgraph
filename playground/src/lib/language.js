// Syntax highlighting for bgraph, as a CodeMirror stream tokenizer.
//
// Hand-written, and a third grammar after the TextMate one and the
// tree-sitter one — which is a cost worth naming. The alternative is compiling
// the tree-sitter grammar to wasm and shipping a second module beside the
// renderer, for a page whose whole job is a two-pane preview. Twenty lines of
// tokenizer is the cheaper honest answer; the *language* is still defined
// once, in `src/syntax.pest`.

import { StreamLanguage, LanguageSupport, HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'

const ARROWS = ['<->', '-->', '..>', '<-', '--', '->', '..']

// The statement words. Kept short on purpose: an item's word is the genre's,
// so it cannot be listed, and it lights up as a name rather than a keyword.
const KEYWORDS = new Set([
  'type', 'graph', 'style', 'group', 'activate', 'node', 'extends',
  'include', 'component', 'use', 'as', 'note', 'legend', 'constraint',
  'align-group', 'same-width', 'same-height',
])

const PLACEMENT = new Set(['at', 'right-of', 'left-of', 'above', 'below', 'gap', 'align', 'via'])

const PARSER = {
  name: 'bgraph',

  token(stream, state) {
    if (stream.eatSpace()) return null

    if (stream.match('//')) {
      stream.skipToEnd()
      return 'comment'
    }

    // A hole in a template: `{{ name }}`. Read whole, because the braces and
    // the name are one thing to a reader.
    if (stream.match('{{')) {
      stream.eatWhile((c) => c !== '}')
      stream.match('}}')
      return 'meta'
    }

    if (stream.peek() === '"') {
      stream.next()
      let escaped = false
      while (!stream.eol()) {
        const c = stream.next()
        if (escaped) escaped = false
        else if (c === '\\') escaped = true
        else if (c === '"') break
      }
      return 'string'
    }

    if (stream.match(/^#[0-9a-fA-F]{3,8}\b/)) return 'colour'
    if (stream.match(/^-?\d+(\.\d+)?/)) return 'number'

    for (const arrow of ARROWS) {
      if (stream.match(arrow)) return 'operator'
    }

    // `.class` — the dot belongs to the name.
    if (stream.match(/^\.[A-Za-z_][\w-]*/)) return 'class'

    const word = stream.match(/^[A-Za-z_][\w-]*/)
    if (word) {
      const text = word[0]
      // `name:` is a property wherever it appears, which is the one thing a
      // reader most needs told apart from a bare word.
      if (stream.peek() === ':') {
        state.property = true
        return 'property'
      }
      if (KEYWORDS.has(text)) return 'keyword'
      if (PLACEMENT.has(text)) return 'placement'
      return 'name'
    }

    stream.next()
    return null
  },

  startState() {
    return { property: false }
  },

  languageData: {
    commentTokens: { line: '//' },
    closeBrackets: { brackets: ['{', '(', '"'] },
    indentOnInput: /^\s*\}$/,
  },
}

const MAP = {
  comment: t.lineComment,
  string: t.string,
  number: t.number,
  colour: t.color,
  operator: t.operator,
  keyword: t.keyword,
  property: t.propertyName,
  placement: t.modifier,
  class: t.className,
  meta: t.meta,
  name: t.variableName,
}

// The tokenizer names its tokens and this decides what they mean, which keeps
// one file about syntax and the other about colour.
const language = StreamLanguage.define({ ...PARSER, tokenTable: MAP })

export const highlight = HighlightStyle.define([
  { tag: t.lineComment, color: 'var(--syntax-comment)', fontStyle: 'italic' },
  { tag: t.string, color: 'var(--syntax-string)' },
  { tag: t.number, color: 'var(--syntax-number)' },
  { tag: t.color, color: 'var(--syntax-number)' },
  { tag: t.operator, color: 'var(--syntax-arrow)' },
  { tag: t.keyword, color: 'var(--syntax-keyword)', fontWeight: '600' },
  { tag: t.propertyName, color: 'var(--syntax-property)' },
  { tag: t.modifier, color: 'var(--syntax-placement)' },
  { tag: t.className, color: 'var(--syntax-class)' },
  { tag: t.meta, color: 'var(--syntax-hole)' },
  { tag: t.variableName, color: 'var(--syntax-name)' },
])

export function bgraph() {
  return new LanguageSupport(language, [syntaxHighlighting(highlight)])
}
