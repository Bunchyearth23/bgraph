const { randomBytes } = require('node:crypto');

function previewHtml() {
  const nonce = randomBytes(16).toString('hex');
  return `<!DOCTYPE html>
<html lang="en"><head><meta charset="UTF-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-${nonce}'; style-src 'unsafe-inline'; img-src data:;">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>bgraph preview</title>
<style>
* { box-sizing: border-box; }
html, body { width: 100%; height: 100%; margin: 0; overflow: hidden; }
body { display: flex; flex-direction: column; color: var(--vscode-foreground); background: var(--vscode-editor-background); font: var(--vscode-font-size, 13px) var(--vscode-font-family, sans-serif); }
.tools { display: flex; flex-wrap: wrap; align-items: center; gap: 4px; padding: 6px 8px; border-bottom: 1px solid var(--vscode-panel-border, #7775); }
button { border: 1px solid transparent; border-radius: 3px; padding: 4px 7px; font: inherit; color: inherit; background: transparent; cursor: pointer; }
button:hover { background: var(--vscode-toolbar-hoverBackground, #8883); }
button:focus-visible, #viewport:focus-visible { outline: 1px solid var(--vscode-focusBorder, #007acc); outline-offset: -2px; }
button:disabled { opacity: .45; cursor: default; }
button[aria-pressed="true"] { background: var(--vscode-toolbar-activeBackground, #8884); }
#scale { min-width: 4em; text-align: center; font-variant-numeric: tabular-nums; }
#viewport { position: relative; flex: 1; min-height: 0; overflow: hidden; cursor: grab; touch-action: none; }
#viewport.dragging { cursor: grabbing; }
#diagram { position: absolute; top: 0; left: 0; transform-origin: 0 0; }
#diagram svg { display: block; width: 100%; height: 100%; max-width: none; user-select: none; }
#error { margin: 0; padding: 16px; overflow: auto; white-space: pre-wrap; overflow-wrap: anywhere; color: var(--vscode-errorForeground); }
#notice { margin: 0; padding: 8px 12px; border-bottom: 1px solid var(--vscode-panel-border, #7775); color: var(--vscode-descriptionForeground); background: var(--vscode-textCodeBlock-background, #8881); white-space: pre-wrap; overflow-wrap: anywhere; }
#hint { margin: 0; padding: 5px 8px; color: var(--vscode-descriptionForeground); font-size: .85em; }
[hidden] { display: none !important; }
</style></head><body>
<div class="tools" role="toolbar" aria-label="Preview controls">
<button data-action="fit" aria-label="Fit diagram to preview" aria-pressed="true" title="Fit (F or Home)">Fit</button>
<button data-action="actual-size" aria-label="Show diagram at actual size" title="Actual size (0)">100%</button>
<button data-action="zoom-out" aria-label="Zoom out" title="Zoom out (-)">−</button>
<output id="scale" aria-label="Zoom level">—</output>
<button data-action="zoom-in" aria-label="Zoom in" title="Zoom in (+)">+</button>
</div>
<main id="viewport" tabindex="0" role="region" aria-label="Diagram preview" aria-describedby="hint"><div id="diagram"></div></main>
<p id="notice" role="status" hidden></p>
<pre id="error" role="alert" hidden></pre>
<p id="hint">Drag or arrow keys to move · scroll to zoom · F to fit</p>
<script nonce="${nonce}">(${previewMain.toString()})();</script>
</body></html>`;
}

// This function runs in the webview; only rendered SVG crosses from the host.
function previewMain() {
  const vscode = acquireVsCodeApi();
  const viewport = document.getElementById('viewport');
  const diagram = document.getElementById('diagram');
  const error = document.getElementById('error');
  const notice = document.getElementById('notice');
  const scale = document.getElementById('scale');
  const buttons = [...document.querySelectorAll('button[data-action]')];
  const saved = vscode.getState();
  let view = saved && [saved.zoom, saved.x, saved.y].every(Number.isFinite) && saved.zoom > 0 ?
    { zoom: saved.zoom, x: saved.x, y: saved.y, fit: saved.fit !== false } :
    { zoom: 1, x: 0, y: 0, fit: true };
  let width = 0, height = 0, dragging;

  function fitScale() {
    return Math.min(1, Math.max(1, viewport.clientWidth - 32) / width,
      Math.max(1, viewport.clientHeight - 32) / height);
  }

  function paint() {
    if (!width || !height || viewport.hidden) return;
    if (view.fit) view = { zoom: fitScale(), x: width / 2, y: height / 2, fit: true };
    diagram.style.transform = `translate(${viewport.clientWidth / 2 - view.x * view.zoom}px, ${viewport.clientHeight / 2 - view.y * view.zoom}px) scale(${view.zoom})`;
    scale.textContent = `${Math.round(view.zoom * 1000) / 10}%`;
    buttons[0].setAttribute('aria-pressed', String(view.fit));
    vscode.setState(view);
  }

  function zoom(factor, px = viewport.clientWidth / 2, py = viewport.clientHeight / 2) {
    if (!width || viewport.hidden) return;
    const next = Math.min(8, Math.max(Math.min(.05, fitScale()), view.zoom * factor));
    const dx = px - viewport.clientWidth / 2, dy = py - viewport.clientHeight / 2;
    view = { zoom: next, x: view.x + dx / view.zoom - dx / next,
      y: view.y + dy / view.zoom - dy / next, fit: false };
    paint();
  }

  function action(name) {
    if (!width || viewport.hidden) return;
    if (name === 'zoom-in') zoom(1.25);
    else if (name === 'zoom-out') zoom(1 / 1.25);
    else {
      view = { zoom: 1, x: width / 2, y: height / 2, fit: name === 'fit' };
      paint();
    }
  }
  buttons.forEach(button => {
    button.disabled = true;
    button.addEventListener('click', () => action(button.dataset.action));
  });
  viewport.addEventListener('keydown', event => {
    const step = (event.shiftKey ? 100 : 40) / view.zoom;
    switch (event.key) {
      case 'f': case 'F': case 'Home': action('fit'); break;
      case '0': action('actual-size'); break;
      case '+': case '=': action('zoom-in'); break;
      case '-': action('zoom-out'); break;
      case 'ArrowLeft': view.x -= step; view.fit = false; paint(); break;
      case 'ArrowRight': view.x += step; view.fit = false; paint(); break;
      case 'ArrowUp': view.y -= step; view.fit = false; paint(); break;
      case 'ArrowDown': view.y += step; view.fit = false; paint(); break;
      default: return;
    }
    event.preventDefault();
  });
  viewport.addEventListener('wheel', event => {
    if (!event.deltaY) return;
    event.preventDefault();
    const bounds = viewport.getBoundingClientRect();
    zoom(event.deltaY < 0 ? 1.1 : 1 / 1.1, event.clientX - bounds.left, event.clientY - bounds.top);
  }, { passive: false });
  viewport.addEventListener('pointerdown', event => {
    if (event.button !== 0 || !width) return;
    dragging = { id: event.pointerId, x: event.clientX, y: event.clientY };
    viewport.setPointerCapture(event.pointerId);
    viewport.classList.add('dragging');
    viewport.focus();
    event.preventDefault();
  });
  viewport.addEventListener('pointermove', event => {
    if (!dragging || dragging.id !== event.pointerId) return;
    const dx = event.clientX - dragging.x, dy = event.clientY - dragging.y;
    if (!dx && !dy) return;
    view.x -= dx / view.zoom; view.y -= dy / view.zoom; view.fit = false;
    dragging.x = event.clientX; dragging.y = event.clientY;
    paint();
  });
  function stopDrag() { dragging = undefined; viewport.classList.remove('dragging'); }
  viewport.addEventListener('lostpointercapture', stopDrag);
  viewport.addEventListener('pointerup', stopDrag);
  viewport.addEventListener('pointercancel', stopDrag);
  new ResizeObserver(paint).observe(viewport);
  window.addEventListener('message', ({ data }) => {
    if (data.type !== 'render') return;
    stopDrag();
    error.hidden = !data.error;
    error.textContent = data.error || '';
    // A caveat about the picture, kept beside it: the diagram is right about
    // everything the buffer declares and silent about the file it cannot read.
    notice.hidden = !data.notice;
    notice.textContent = data.notice || '';
    viewport.hidden = Boolean(data.error);
    if (!data.error) {
      diagram.innerHTML = data.svg;
      const svg = diagram.querySelector('svg');
      const box = svg?.viewBox.baseVal;
      width = box?.width || Number(svg?.getAttribute('width'));
      height = box?.height || Number(svg?.getAttribute('height'));
      if (![width, height].every(value => Number.isFinite(value) && value > 0)) {
        width = height = 0;
        viewport.hidden = true;
        error.hidden = false;
        error.textContent = 'The diagram has no usable dimensions.';
      } else {
        diagram.style.width = `${width}px`; diagram.style.height = `${height}px`;
        paint();
      }
    }
    buttons.forEach(button => { button.disabled = viewport.hidden; });
  });
  vscode.postMessage({ type: 'ready' });
}

module.exports = { previewHtml };
