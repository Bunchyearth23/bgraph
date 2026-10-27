<script>
  import { tick } from 'svelte'
  let { svg = '', error = '', notice = '' } = $props()

  let zoom = $state(1)
  let pan = $state({ x: 0, y: 0 })
  let dragging = $state(null)
  let stage = $state(null)
  let sheet = $state(null)
  let fitted = false

  function fit() {
    if (!stage || !sheet?.offsetWidth || !sheet.offsetHeight) return
    zoom = Math.max(0.01, Math.min(1, (stage.clientWidth - 32) / sheet.offsetWidth, (stage.clientHeight - 64) / sheet.offsetHeight))
    pan = { x: 0, y: 0 }
  }

  $effect(() => {
    if (svg && !fitted) {
      fitted = true
      tick().then(fit)
    }
  })

  function keydown(event) {
    const step = event.shiftKey ? 100 : 40
    switch (event.key) {
      case 'ArrowLeft': pan = { ...pan, x: pan.x - step }; break
      case 'ArrowRight': pan = { ...pan, x: pan.x + step }; break
      case 'ArrowUp': pan = { ...pan, y: pan.y - step }; break
      case 'ArrowDown': pan = { ...pan, y: pan.y + step }; break
      case '+': case '=': zoom = Math.min(8, zoom * 1.25); break
      case '-': zoom = Math.max(0.01, zoom / 1.25); break
      case 'Home': case '0': fit(); break
      default: return
    }
    event.preventDefault()
  }

  function wheel(event) {
    if (!event.ctrlKey && !event.metaKey) return
    // Ctrl-wheel only. A bare wheel scrolls, because a preview you cannot
    // scroll past is worse than one you have to hold a key to zoom.
    event.preventDefault()
    const next = zoom * (event.deltaY < 0 ? 1.1 : 1 / 1.1)
    zoom = Math.min(8, Math.max(0.01, next))
  }

  function down(event) {
    if (event.button !== 0) return
    dragging = { x: event.clientX - pan.x, y: event.clientY - pan.y }
    event.currentTarget.setPointerCapture(event.pointerId)
  }

  function move(event) {
    if (!dragging) return
    pan = { x: event.clientX - dragging.x, y: event.clientY - dragging.y }
  }

  function up() {
    dragging = null
  }
</script>

<div class="viewer">
  <div class="tools">
    <button onclick={() => (zoom = Math.min(8, zoom * 1.25))} aria-label="Zoom in" title="Zoom in">+</button>
    <button onclick={() => (zoom = Math.max(0.01, zoom / 1.25))} aria-label="Zoom out" title="Zoom out">−</button>
    <button onclick={fit} aria-label="Fit diagram to preview" title="Fit diagram (Home)">Fit · {Math.round(zoom * 100)}%</button>
  </div>

  {#if error}
    <pre class="error">{error}</pre>
  {:else}
    <!-- A caveat about the picture, kept beside it: the diagram is right
         about everything the document declares and silent about the file it
         cannot read. -->
    {#if notice}
      <p class="notice" role="status">{notice}</p>
    {/if}
    <!-- The application role is scoped to this keyboard-operated diagram. -->
    <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
    <div
      class="stage"
      bind:this={stage}
      role="application"
      tabindex="0"
      aria-label="Diagram preview. Arrow keys move, plus and minus zoom, Home fits the diagram."
      class:grabbing={dragging}
      onkeydown={keydown}
      onwheel={wheel}
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
    >
      <div
        class="sheet"
        bind:this={sheet}
        style="transform: translate({pan.x}px, {pan.y}px) scale({zoom})"
      >
        <!-- The renderer's own output, and the only thing here that is not
             written in this repository's JavaScript. -->
        {@html svg}
      </div>
    </div>
  {/if}
</div>

<style>
  .viewer {
    position: relative;
    min-height: 0;
    overflow: hidden;
    background: var(--stage);
  }

  .tools {
    position: absolute;
    top: 10px;
    right: 12px;
    z-index: 2;
    display: flex;
    gap: 4px;
  }
  .tools button {
    border: 1px solid var(--line);
    background: var(--surface);
    color: var(--muted);
    border-radius: 6px;
    min-width: 30px;
    height: 26px;
    font: inherit;
    font-size: 12px;
    cursor: pointer;
  }
  .tools button:hover { color: var(--ink); border-color: var(--faint); }

  .stage {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    place-items: center;
    cursor: grab;
    overflow: hidden;
    touch-action: none;
  }
  .stage.grabbing { cursor: grabbing; }
  .stage:focus-visible { outline: 2px solid var(--ink); outline-offset: -3px; }

  .sheet {
    width: max-content;
    transform-origin: center;
    line-height: 0;
  }
  .sheet :global(svg) {
    max-width: none;
    height: auto;
    user-select: none;
  }

  .error {
    margin: 0;
    padding: 20px;
    height: 100%;
    overflow: auto;
    color: var(--bad);
    font: 13px/1.6 ui-monospace, monospace;
    white-space: pre-wrap;
  }

  .notice {
    position: absolute;
    top: 10px;
    left: 12px;
    right: 170px;
    z-index: 2;
    margin: 0;
    padding: 8px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    background: var(--surface);
    color: var(--muted);
    font: 12px/1.5 system-ui, sans-serif;
  }
</style>
