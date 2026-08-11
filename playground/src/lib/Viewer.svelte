<script>
  let { svg = '', error = '' } = $props()

  let zoom = $state(1)
  let pan = $state({ x: 0, y: 0 })
  let dragging = $state(null)

  function fit() {
    zoom = 1
    pan = { x: 0, y: 0 }
  }

  function wheel(event) {
    if (!event.ctrlKey && !event.metaKey) return
    // Ctrl-wheel only. A bare wheel scrolls, because a preview you cannot
    // scroll past is worse than one you have to hold a key to zoom.
    event.preventDefault()
    const next = zoom * (event.deltaY < 0 ? 1.1 : 1 / 1.1)
    zoom = Math.min(8, Math.max(0.1, next))
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
    <button onclick={() => (zoom = Math.min(8, zoom * 1.25))} title="Zoom in">+</button>
    <button onclick={() => (zoom = Math.max(0.1, zoom / 1.25))} title="Zoom out">−</button>
    <button onclick={fit} title="Reset">{Math.round(zoom * 100)}%</button>
  </div>

  {#if error}
    <pre class="error">{error}</pre>
  {:else}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="stage"
      class:grabbing={dragging}
      onwheel={wheel}
      onpointerdown={down}
      onpointermove={move}
      onpointerup={up}
      onpointercancel={up}
    >
      <div
        class="sheet"
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
    place-items: center;
    cursor: grab;
    overflow: hidden;
    touch-action: none;
  }
  .stage.grabbing { cursor: grabbing; }

  .sheet {
    transform-origin: center;
    line-height: 0;
  }
  .sheet :global(svg) {
    max-width: min(100%, 1400px);
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
</style>
