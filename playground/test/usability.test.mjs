import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'
import vm from 'node:vm'

// Execute the component's own handlers with browser boundaries stubbed; Vite
// separately compiles the markup and Svelte's reactive wiring.
function component(file, globals) {
  const source = readFileSync(new URL(`../src/${file}`, import.meta.url), 'utf8')
    .match(/<script>([\s\S]*?)<\/script>/)[1]
    .replace(/^  import .*$/gm, '')
  const context = vm.createContext({
    $state: value => value, $derived: value => value,
    $effect() {}, $props: () => ({}), onMount() {}, tick: () => Promise.resolve(),
    setTimeout, clearTimeout, ...globals,
  })
  vm.runInContext(source, context)
  return code => vm.runInContext(code, context)
}

function app({ hash = '', draft = null, readError = false, writeError = false,
  render = () => '<svg id="drawn"></svg>', check = () => [] } = {}) {
  let resolveLoad
  const loading = new Promise(resolve => { resolveLoad = resolve })
  const downloads = []
  let blob
  const run = component('App.svelte', {
    wasm: {
      load: () => loading, examples: () => [{ name: 'free', source: 'node default\n' }],
      ready: () => true, render, check,
    },
    matchMedia: () => ({ matches: false }), location: { hash }, atob, btoa, Blob,
    localStorage: {
      getItem() { if (readError) throw Error('blocked'); return draft },
      setItem(_key, text) { if (writeError) throw Error('full'); draft = text },
    },
    URL: { createObjectURL(value) { blob = value; return 'blob:download' }, revokeObjectURL() {} },
    document: { createElement() { return { click() { downloads.push({ filename: this.download, blob }) } } } },
  })
  return { run, resolveLoad, downloads, draft: () => draft }
}

test('typing while WASM loads is preserved, and a fresh page gets the free example', async () => {
  const edited = app()
  const pending = edited.run('initialize()')
  edited.run('text = "node typed\\n"')
  edited.resolveLoad()
  await pending
  assert.equal(edited.run('text'), 'node typed\n')
  const fresh = app()
  const ready = fresh.run('initialize()')
  fresh.resolveLoad()
  await ready
  assert.equal(fresh.run('text'), 'node default\n')
})

test('explicit links including an empty document win over a saved draft', async () => {
  for (const shared of ['node shared "é"\n', '']) {
    const hash = `#s=${encodeURIComponent(btoa(unescape(encodeURIComponent(shared))))}`
    const page = app({ hash, draft: 'node draft\n' })
    const pending = page.run('initialize()')
    assert.equal(page.run('text'), shared)
    page.resolveLoad()
    await pending
    assert.equal(page.run('text'), shared)
  }
})

test('draft restores before WASM loads, malformed links fall back, and storage errors preserve source', async () => {
  const restored = app({ hash: '#s=%invalid', draft: 'node saved\n' })
  const pending = restored.run('initialize()')
  assert.equal(restored.run('text'), 'node saved\n')
  restored.run('text = "node changed\\n"; saveDraft()')
  assert.equal(restored.draft(), 'node changed\n')
  assert.match(restored.run('draftStatus'), /saved/)
  restored.resolveLoad()
  await pending
  for (const error of [{ readError: true }, { writeError: true }]) {
    const page = app({ draft: 'node original\n', ...error })
    const ready = page.run('initialize()')
    page.run('text = "node kept\\n"; saveDraft()')
    assert.equal(page.run('text'), 'node kept\n')
    assert.equal(page.draft(), 'node original\n')
    assert.match(page.run('draftStatus'), /unavailable/)
    page.resolveLoad()
    await ready
    assert.equal(page.run('text'), 'node kept\n')
  }
})

test('source download works before renderer loads and contains the current document', async () => {
  const page = app()
  page.run('text = "node unsaved \\\"é\\\"\\n"; download(true)')
  assert.equal(page.downloads[0].filename, 'diagram.bd')
  assert.equal(await page.downloads[0].blob.text(), 'node unsaved "é"\n')
})

test('a diagram that reads another file says so instead of drawing a wrong picture', () => {
  // The playground renders in memory. A pie whose slices come from a table is
  // drawn from what the document declares alone — right about everything it
  // has, silent about the file it cannot open. The picture is kept; the
  // caveat is shown with it.
  const page = app()
  page.run('text = "type pie { from: \\"sales.csv\\" }\\nslice a \\"A\\" 10\\n"; draw()')
  assert.equal(page.run('svg'), '<svg id="drawn"></svg>')
  assert.match(page.run('notice'), /reads its data from another file/)
  assert.equal(page.run('error'), '')

  // A document that has no such file needs no caveat.
  const plain = app()
  plain.run('text = "node a\\n"; draw()')
  assert.equal(plain.run('notice'), '')
})

test('an include is refused in words a reader can act on', () => {
  // The renderer's own wording names a Rust function, which nobody in a
  // browser can call.
  const page = app({
    render: () => 'error: `include "other.bd"` needs a file to resolve it against, ' +
      'and `render_source` is given text — use `bgraph::render_file`',
  })
  page.run('text = "include \\"other.bd\\"\\n"; draw()')
  assert.match(page.run('error'), /bgraph watch/)
  assert.doesNotMatch(page.run('error'), /bgraph::render_file/)
  assert.equal(page.run('notice'), '')
})

test('keyboard panning and fitting keep tall diagrams inside the preview', () => {
  const run = component('lib/Viewer.svelte', {})
  run('stage = { clientWidth: 800, clientHeight: 400 }; sheet = { offsetWidth: 200, offsetHeight: 1000 }; fit()')
  assert.equal(run('zoom'), 0.336)
  run('keydown({key: "ArrowRight", shiftKey: false, preventDefault() {}})')
  assert.equal(run('pan.x'), 40)
  run('keydown({key: "ArrowDown", shiftKey: true, preventDefault() {}})')
  assert.equal(run('pan.y'), 100)
  run('keydown({key: "Home", preventDefault() {}})')
  assert.equal(run('pan.x'), 0)
  assert.equal(run('pan.y'), 0)
  assert.equal(run('zoom'), 0.336)
  run('sheet.offsetHeight = 10000; fit(); keydown({key: "-", preventDefault() {}})')
  assert.ok(run('zoom') < 0.0336)
})
