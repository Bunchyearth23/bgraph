<script>
  import { onMount } from 'svelte'
  import Editor from './lib/Editor.svelte'
  import Viewer from './lib/Viewer.svelte'
  import * as wasm from './lib/bgraph.js'

  const FALLBACK = `graph { title: "Try it" background: #ffffff }

style card { fill: #eef3ff stroke: #2563eb 1.3 radius: 6 }

node ui  "Browser"  .card
node api "API"      .card
node db  "Database" .card

layout {
  place api right-of ui gap 60
  place db below api gap 50
}

ui  -> api "HTTPS"
api -> db  "reads"
`

  let status = $state('loading')
  let fault = $state('')
  let genres = $state([])
  let genre = $state('')
  let text = $state(FALLBACK)
  let svg = $state('')
  let error = $state('')
  let diagnostics = $state([])
  let dark = $state(false)
  let editor = $state(null)
  let timer

  // The source travels in the fragment, not the query: a fragment is never
  // sent to a server, and this page has no server to send it to.
  function fromLink() {
    const hash = location.hash.replace(/^#/, '')
    if (!hash.startsWith('s=')) return null
    try {
      return decodeURIComponent(escape(atob(decodeURIComponent(hash.slice(2)))))
    } catch {
      return null
    }
  }

  function share() {
    const encoded = btoa(unescape(encodeURIComponent(text)))
    location.hash = `s=${encodeURIComponent(encoded)}`
    navigator.clipboard?.writeText(location.href)
  }

  function download() {
    const blob = new Blob([svg], { type: 'image/svg+xml' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `${genre || 'diagram'}.svg`
    a.click()
    URL.revokeObjectURL(url)
  }

  function pick(name) {
    const found = genres.find((g) => g.name === name)
    if (!found) return
    genre = name
    editor?.replace(found.source)
  }

  function draw() {
    if (!wasm.ready()) return
    const answer = wasm.render(text)
    // One channel, so there is no second call to forget: an error announces
    // itself in the text rather than in a status nobody checks.
    if (answer.startsWith('error:')) {
      error = answer
      svg = ''
    } else {
      error = ''
      svg = answer
    }
    diagnostics = wasm.check(text)
  }

  onMount(async () => {
    dark = matchMedia('(prefers-color-scheme: dark)').matches
    try {
      await wasm.load()
      genres = wasm.examples()
      status = 'ready'
    } catch (e) {
      status = 'broken'
      fault = `${e}`
      return
    }

    const shared = fromLink()
    if (shared) {
      editor?.replace(shared)
      text = shared
    } else {
      genre = 'free'
      const start = genres.find((g) => g.name === 'free')?.source
      if (start) {
        editor?.replace(start)
        text = start
      }
    }
  })

  // Debounced, because rendering is fast but not free and a keystroke is not a
  // question. Sixty milliseconds is under what a typist notices and over what
  // a held key produces.
  $effect(() => {
    const _ = text
    if (status !== 'ready') return
    clearTimeout(timer)
    timer = setTimeout(draw, 60)
  })

  $effect(() => {
    document.documentElement.dataset.theme = dark ? 'dark' : 'light'
  })

  const errors = $derived(diagnostics.filter((d) => d.severity === 'error').length)
  const warnings = $derived(diagnostics.filter((d) => d.severity === 'warning').length)
  const lines = $derived(text.split('\n'))

  function humanColumn(diagnostic) {
    return Array.from((lines[diagnostic.line - 1] ?? '').slice(0, diagnostic.col - 1)).length + 1
  }
</script>

<header>
  <strong>bgraph</strong>
  <span class="tag">diagrams as text, where you decide the layout</span>

  <div class="spacer"></div>

  {#if status === 'ready'}
    <select bind:value={genre} onchange={(e) => pick(e.currentTarget.value)}>
      {#each genres as g (g.name)}
        <option value={g.name}>{g.name}</option>
      {/each}
    </select>
    <button onclick={share} title="Copy a link that carries the source">Share</button>
    <button onclick={download} disabled={!svg}>SVG</button>
  {/if}
  <button class="theme" onclick={() => (dark = !dark)} title="Light or dark">
    {dark ? '☾' : '☀'}
  </button>
</header>

<!-- The editor is up before the module is. A megabyte of wasm is a fraction
     of a second on a warm cache and noticeable on a cold one, and blanking the
     whole page for it would make the one thing you came to do — type — wait on
     the one thing you did not. Only the picture waits. -->
<main>
  <Editor bind:this={editor} bind:text {diagnostics} {dark} />

  {#if status === 'ready'}
    <Viewer {svg} {error} />
  {:else}
    <aside class="pending">
      {#if status === 'broken'}
        <pre>could not load bgraph.wasm — build it with

    ./build.sh

then reload. `pnpm dev` serves public/ at the site root.

{fault}</pre>
      {:else}
        <p>loading the renderer…</p>
      {/if}
    </aside>
  {/if}
</main>

<footer class:bad={errors > 0} class:warn={errors === 0 && warnings > 0}>
  {#if status !== 'ready'}
    <span class="ok">…</span>
  {:else if diagnostics.length === 0}
    <span class="ok">no problems found</span>
  {:else}
    <ul>
      {#each diagnostics as d, i (i)}
        <li class={d.severity}>
          <span class="where">{d.line}:{humanColumn(d)}</span>
          {d.message}
        </li>
      {/each}
    </ul>
  {/if}
</footer>

<style>
  header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 14px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
  }
  header strong { font-weight: 650; letter-spacing: -0.01em; }
  .tag { color: var(--muted); font-size: 13px; }
  .spacer { flex: 1; }

  select, header button {
    border: 1px solid var(--line);
    background: var(--raised);
    color: var(--ink);
    border-radius: 6px;
    padding: 4px 9px;
    font: inherit;
    font-size: 13px;
    cursor: pointer;
  }
  select:hover, header button:hover:not(:disabled) { border-color: var(--faint); }
  header button:disabled { opacity: 0.45; cursor: default; }
  .theme { min-width: 32px; }

  main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.15fr);
    min-height: 0;
    border-bottom: 1px solid var(--line);
  }
  .pending {
    display: grid;
    place-items: center;
    background: var(--stage);
    min-height: 0;
    overflow: auto;
  }
  .pending pre { margin: 0; }

  footer {
    max-height: 27vh;
    overflow: auto;
    padding: 7px 14px;
    background: var(--surface);
    font: 12px/1.65 ui-monospace, monospace;
  }
  footer.bad { box-shadow: inset 3px 0 0 var(--bad); }
  footer.warn { box-shadow: inset 3px 0 0 var(--warn); }
  .ok { color: var(--muted); }
  footer ul { margin: 0; padding: 0; list-style: none; }
  footer li { display: flex; gap: 10px; }
  li.error { color: var(--bad); }
  li.warning { color: var(--warn); }
  .where { color: var(--faint); min-width: 46px; text-align: right; }

  .pending p, .pending pre {
    color: var(--muted);
    font: 13px/1.7 ui-monospace, monospace;
    white-space: pre-wrap;
    padding: 24px;
  }

  @media (max-width: 800px) {
    main { grid-template-columns: 1fr; grid-template-rows: 1fr 1fr; }
    .tag { display: none; }
  }
</style>
