const assert = require('node:assert/strict');
const { test } = require('node:test');
const vm = require('node:vm');
const { previewHtml } = require('../src/preview');

function preview(saved) {
  function element() {
    const listeners = new Map();
    return {
      style: {}, hidden: false, dataset: {}, attributes: {},
      classList: { add() {}, remove() {} }, focus() {}, setPointerCapture() {},
      addEventListener(name, callback) { listeners.set(name, callback); },
      fire(name, event = {}) { listeners.get(name)?.({ preventDefault() {}, ...event }); },
      setAttribute(name, value) { this.attributes[name] = value; },
    };
  }
  const elements = Object.fromEntries(['viewport', 'diagram', 'error', 'scale'].map(id => [id, element()]));
  const buttons = ['fit', 'actual-size', 'zoom-out', 'zoom-in'].map(action => {
    const button = element(); button.dataset.action = action; return button;
  });
  const window = element();
  let state = saved, resize, dimensions;
  const messages = [];
  elements.viewport.clientWidth = 800; elements.viewport.clientHeight = 400;
  elements.viewport.getBoundingClientRect = () => ({ left: 0, top: 0 });
  elements.diagram.querySelector = () => ({ viewBox: { baseVal: dimensions }, getAttribute: () => null });
  vm.runInNewContext(previewHtml().match(/<script nonce="[^"]+">([\s\S]*?)<\/script>/)[1], {
    window,
    document: { getElementById: id => elements[id], querySelectorAll: () => buttons },
    acquireVsCodeApi: () => ({ getState: () => state, setState(value) { state = { ...value }; },
      postMessage: message => messages.push(message) }),
    ResizeObserver: class { constructor(callback) { resize = callback; } observe() {} },
  });
  return {
    ...elements, buttons, messages,
    get state() { return state; },
    render(width, height) { dimensions = { width, height }; window.fire('message', { data: { type: 'render', svg: '<svg/>' } }); },
    showError(text) { window.fire('message', { data: { type: 'render', error: text } }); },
    resize(width, height) { elements.viewport.clientWidth = width; elements.viewport.clientHeight = height; resize(); },
    click(action) { buttons.find(button => button.dataset.action === action).fire('click'); },
  };
}

test('fit keeps tall and wide diagrams inside both dimensions after resize', () => {
  const p = preview();
  assert.equal(p.messages[0].type, 'ready');
  for (const [width, height] of [[100, 2000], [3000, 100], [500, 500]]) {
    p.render(width, height);
    for (const [vw, vh] of [[800, 400], [220, 600], [1200, 250]]) {
      p.resize(vw, vh);
      assert.ok(width * p.state.zoom <= vw - 32 + 1e-8);
      assert.ok(height * p.state.zoom <= vh - 32 + 1e-8);
      assert.equal(p.state.x, width / 2);
      assert.equal(p.state.y, height / 2);
    }
  }
  p.render(80, 40);
  assert.equal(p.state.zoom, 1, 'small diagrams retain their natural size');
});

test('manual zoom and pan survive edits, resize and webview restoration; Fit resets them', () => {
  const p = preview();
  p.render(600, 900);
  p.click('actual-size');
  p.viewport.fire('pointerdown', { button: 0, pointerId: 1, clientX: 100, clientY: 100 });
  p.viewport.fire('pointermove', { pointerId: 1, clientX: 140, clientY: 160 });
  p.viewport.fire('pointerup');
  const camera = { ...p.state };
  assert.equal(camera.zoom, 1);
  assert.equal(camera.x, 260);
  assert.equal(camera.y, 390);
  assert.equal(camera.fit, false);
  p.render(600, 1000);
  p.resize(250, 300);
  assert.deepEqual(p.state, camera);
  const restored = preview(camera);
  restored.render(600, 1000);
  assert.deepEqual(restored.state, camera);
  p.viewport.fire('keydown', { key: 'f' });
  assert.equal(p.state.fit, true);
  assert.ok(1000 * p.state.zoom <= 268);
  assert.equal(p.buttons[0].attributes['aria-pressed'], 'true');
});

test('wheel zoom is cursor anchored and independent of platform delta magnitude', () => {
  const cameras = [-1, -15, -120].map(deltaY => {
    const p = preview(); p.render(400, 200);
    const before = p.state;
    const worldX = before.x + (100 - 400) / before.zoom;
    p.viewport.fire('wheel', { deltaY, clientX: 100, clientY: 100 });
    assert.ok(Math.abs(p.state.x + (100 - 400) / p.state.zoom - worldX) < 1e-8);
    return p.state;
  });
  assert.deepEqual(cameras[0], cameras[1]);
  assert.deepEqual(cameras[0], cameras[2]);
});

test('errors are text, disable controls, and recover without changing the camera', () => {
  const p = preview(); p.render(400, 200); p.click('zoom-in');
  const camera = { ...p.state };
  p.showError('<script>alert(1)</script>');
  assert.equal(p.error.textContent, '<script>alert(1)</script>');
  assert.equal(p.viewport.hidden, true);
  assert.ok(p.buttons.every(button => button.disabled));
  p.render(400, 200);
  assert.equal(p.viewport.hidden, false);
  assert.deepEqual(p.state, camera);
  assert.ok(p.buttons.every(button => !button.disabled));
  p.render(0, 200);
  assert.equal(p.viewport.hidden, true);
});
