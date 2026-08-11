<script>
  import { onMount, onDestroy } from 'svelte'
  import { EditorView, keymap, lineNumbers, highlightActiveLine, highlightActiveLineGutter, gutter, GutterMarker } from '@codemirror/view'
  import { EditorState, StateEffect, StateField, RangeSet, Compartment } from '@codemirror/state'
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
  import { indentUnit, bracketMatching } from '@codemirror/language'
  import { bgraph } from './language.js'

  let { text = $bindable(''), diagnostics = [], dark = false } = $props()

  let host
  let view

  // CodeMirror's own base theme paints the gutter, the cursor and the
  // selection, and it paints them for a light editor unless it is told
  // otherwise. Saying so through a theme rather than through `:global` CSS is
  // what makes this stick: a theme is injected by CodeMirror at a higher
  // priority than its base, so it wins without a specificity race that the
  // build order decides (`W-137`).
  const skin = new Compartment()

  function look(dark) {
    return EditorView.theme(
      {
        '&': { height: '100%', color: 'var(--ink)', backgroundColor: 'var(--surface)' },
        '.cm-content': { caretColor: 'var(--ink)' },
        '.cm-gutters': {
          backgroundColor: 'transparent',
          borderRight: '1px solid var(--line)',
          color: 'var(--faint)',
        },
        // The line numbers themselves. They are furniture: you look one up,
        // you do not read them, so they sit below the code rather than beside
        // it — and the current line is the one exception, because that one you
        // are looking for.
        '.cm-lineNumbers .cm-gutterElement': { color: 'var(--faint)' },
        // Two selectors deep, because `.cm-lineNumbers .cm-gutterElement`
        // above is, and the current line's number is the one you are actually
        // looking for.
        '.cm-lineNumbers .cm-activeLineGutter': { color: 'var(--muted)' },
        '.cm-activeLineGutter': { backgroundColor: 'var(--raised)' },
        '.cm-activeLine': { backgroundColor: 'var(--raised)' },
        '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--ink)' },
        '&.cm-focused .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection':
          { backgroundColor: 'var(--select)' },
        '.cm-selectionMatch': { backgroundColor: 'transparent' },
      },
      { dark },
    )
  }

  // A dot in the gutter, one per diagnostic. Deliberately not an underline in
  // the text: a span is a *statement's* span here, so underlining it would
  // paint whole lines and read as an error in the middle of typing.
  class Dot extends GutterMarker {
    constructor(severity) {
      super()
      this.severity = severity
    }
    toDOM() {
      const span = document.createElement('span')
      span.className = `cm-dot cm-dot-${this.severity}`
      span.textContent = '●'
      return span
    }
  }

  const setMarks = StateEffect.define()
  const marks = StateField.define({
    create: () => RangeSet.empty,
    update(value, tr) {
      for (const effect of tr.effects) if (effect.is(setMarks)) return effect.value
      return value.map(tr.changes)
    },
  })

  function markers(doc, list) {
    const seen = new Map()
    for (const d of list) {
      if (d.line < 1 || d.line > doc.lines) continue
      // An error on a line outranks a warning on the same line.
      if (seen.get(d.line) === 'error') continue
      seen.set(d.line, d.severity)
    }
    return RangeSet.of(
      [...seen.entries()]
        .sort((a, b) => a[0] - b[0])
        .map(([line, severity]) => new Dot(severity).range(doc.line(line).from)),
      true,
    )
  }

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: text,
        extensions: [
          lineNumbers(),
          highlightActiveLine(),
          highlightActiveLineGutter(),
          history(),
          bracketMatching(),
          indentUnit.of('  '),
          keymap.of([...defaultKeymap, ...historyKeymap, indentWithTab]),
          bgraph(),
          marks,
          gutter({ class: 'cm-dots', markers: (v) => v.state.field(marks) }),
          EditorView.lineWrapping,
          skin.of(look(dark)),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) text = update.state.doc.toString()
          }),
        ],
      }),
    })
  })

  onDestroy(() => view?.destroy())

  // Set from outside — picking a genre replaces the whole document, and doing
  // that through a `dispatch` rather than a new state keeps the undo history.
  export function replace(next) {
    if (!view) return
    view.dispatch({
      changes: { from: 0, to: view.state.doc.length, insert: next },
    })
  }

  $effect(() => {
    if (!view) return
    const list = diagnostics
    view.dispatch({ effects: setMarks.of(markers(view.state.doc, list)) })
  })

  $effect(() => {
    if (!view) return
    view.dispatch({ effects: skin.reconfigure(look(dark)) })
  })
</script>

<div class="editor" bind:this={host}></div>

<style>
  .editor {
    min-height: 0;
    overflow: hidden;
    background: var(--surface);
  }

  :global(.cm-editor) {
    height: 100%;
    font: 13px/1.6 ui-monospace, 'SF Mono', Menlo, monospace;
    color: var(--ink);
  }
  :global(.cm-editor.cm-focused) { outline: none; }
  :global(.cm-scroller) { font-family: inherit; }
  :global(.cm-dots) { min-width: 14px; }
  :global(.cm-dot) { font-size: 9px; line-height: 1.6; }
  :global(.cm-dot-error) { color: var(--bad); }
  :global(.cm-dot-warning) { color: var(--warn); }
</style>
