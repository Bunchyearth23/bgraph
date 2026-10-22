const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const vm = require('node:vm');
const { createRequire } = require('node:module');

const entry = path.join(__dirname, '..', 'src', 'extension.js');
const extensionRequire = createRequire(entry);

function event() {
  const listeners = new Set();
  return {
    subscribe(listener) {
      listeners.add(listener);
      return { dispose: () => listeners.delete(listener) };
    },
    fire(value) { for (const listener of listeners) listener(value); },
  };
}

function extension({ missingRenderer = false } = {}) {
  const changed = event(), closed = event();
  const commands = new Map(), documents = new Map(), panels = [], timers = new Map();
  const notices = [];
  let timerId = 0;
  const window = {
    showWarningMessage: text => notices.push(text),
    showInformationMessage: text => notices.push(text),
    createWebviewPanel(viewType, title, column, options) {
      const disposed = event(), received = event();
      const panel = {
        viewType, title, column, options, revealed: 0,
        webview: {
          html: '', messages: [],
          onDidReceiveMessage: received.subscribe,
          postMessage(message) { this.messages.push(message); return Promise.resolve(true); },
        },
        ready() { received.fire({ type: 'ready' }); },
        onDidDispose: disposed.subscribe,
        reveal() { this.revealed++; },
        dispose() {
          if (this.disposed) return;
          this.disposed = true;
          disposed.fire();
        },
      };
      panels.push(panel);
      return panel;
    },
  };
  const vscode = {
    window,
    ViewColumn: { Beside: 2 },
    commands: {
      registerCommand(id, handler) {
        commands.set(id, handler);
        return { dispose: () => commands.delete(id) };
      },
    },
    workspace: {
      getConfiguration: () => ({ get: (_key, fallback) => fallback }),
      onDidChangeTextDocument: changed.subscribe,
      onDidCloseTextDocument: closed.subscribe,
      openTextDocument: async uri => documents.get(uri.toString()),
    },
  };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(entry, 'utf8'), {
    module, __dirname: path.dirname(entry),
    require(id) {
      if (id === 'vscode') return vscode;
      if (id === 'vscode-languageclient/node') throw new Error('No language client');
      if (id === 'fs' && missingRenderer) return {
        readFileSync() { throw new Error('No bundled renderer'); },
      };
      return extensionRequire(id);
    },
    setTimeout(fn, delay) {
      assert.equal(delay, 150);
      timers.set(++timerId, fn);
      return timerId;
    },
    clearTimeout(id) { timers.delete(id); },
    TextEncoder, TextDecoder, Uint8Array, WebAssembly,
  }, { filename: entry });
  const context = { subscriptions: [] };
  module.exports.activate(context);
  return {
    panels, timers, notices, changed, closed, window,
    api: module.exports,
    document(uri, text, languageId = 'bgraph') {
      const document = {
        uri: { toString: () => uri }, fileName: uri.split('/').pop(),
        languageId, text, getText() { return this.text; },
      };
      documents.set(uri, document);
      window.activeTextEditor = { document };
      return document;
    },
    async preview(uri, ready = true) {
      await commands.get('bgraph.showPreview')(uri);
      if (ready) panels.at(-1)?.ready();
    },
    flush() {
      for (const callback of [...timers.values()]) callback();
      assert.equal(timers.size, 0);
    },
    dispose() {
      module.exports.deactivate();
      for (const disposable of context.subscriptions) disposable.dispose();
    },
  };
}

test('preview uses unsaved buffer text without the language client, with a scoped shortcut', async () => {
  const harness = extension();
  const document = harness.document('file:///tmp/diagram.bd', 'node a "Unsaved buffer"\n');
  await harness.preview();
  const panel = harness.panels[0];
  assert.match(panel.webview.messages.at(-1).svg, /<svg\b/);
  assert.match(panel.webview.messages.at(-1).svg, /Unsaved buffer/);
  assert.equal(panel.title, 'Preview: diagram.bd');
  assert.equal(panel.viewType, 'bgraph.preview');
  assert.equal(panel.options.enableScripts, true);
  assert.equal(panel.options.localResourceRoots.length, 0);
  assert.match(panel.webview.html, /default-src 'none'/);
  const nonce = panel.webview.html.match(/<script nonce="([a-f0-9]+)">/)[1];
  assert.ok(panel.webview.html.includes(`script-src 'nonce-${nonce}'`));
  assert.equal((panel.webview.html.match(/<script\b/g) || []).length, 1);
  await harness.preview(document.uri);
  assert.equal(harness.panels.length, 1);
  assert.equal(panel.revealed, 1);
  const manifest = require('../package.json');
  const shortcut = manifest.contributes.keybindings.find(key => key.command === 'bgraph.showPreview');
  assert.equal(shortcut.key, 'ctrl+shift+v');
  assert.equal(shortcut.mac, 'cmd+shift+v');
  assert.equal(shortcut.when, 'editorLangId == bgraph && editorTextFocus && resourceExtname != .bds');
  assert.equal(manifest.contributes.commands[0].icon, '$(open-preview)');
  harness.dispose();
});

test('edits debounce per document, stay bound to its panel and clean up on close', async () => {
  const harness = extension();
  const first = harness.document('file:///tmp/first.bd', 'node a "First"\n');
  await harness.preview();
  const second = harness.document('untitled:Untitled-1', 'node a "Second"\n');
  await harness.preview();
  first.text = 'node a "Intermediate"\n';
  const html = harness.panels[0].webview.html;
  harness.changed.fire({ document: first });
  first.text = 'node a "Final buffer"\nnode b "Added while typing"\n';
  harness.changed.fire({ document: first });
  assert.equal(harness.timers.size, 1);
  assert.doesNotMatch(harness.panels[0].webview.messages.at(-1).svg, /Final buffer/);
  harness.flush();
  assert.equal(harness.panels[0].webview.html, html, 'edits preserve the webview and its camera');
  assert.match(harness.panels[0].webview.messages.at(-1).svg, /Final buffer/);
  assert.match(harness.panels[0].webview.messages.at(-1).svg, /Added while typing/);
  assert.match(harness.panels[1].webview.messages.at(-1).svg, /Second/);
  assert.doesNotMatch(harness.panels[1].webview.messages.at(-1).svg, /Final buffer/);
  harness.changed.fire({ document: first });
  harness.panels[0].dispose();
  assert.equal(harness.timers.size, 0);
  await harness.preview(first.uri);
  assert.equal(harness.panels.length, 3);
  harness.closed.fire(second);
  assert.equal(harness.panels[1].disposed, true);
  harness.changed.fire({ document: first });
  harness.dispose();
  assert.equal(harness.timers.size, 0);
  assert.ok(harness.panels.every(panel => panel.disposed));
});

test('syntax errors and hostile labels are escaped, and valid edits recover', async () => {
  const harness = extension();
  const document = harness.document('file:///tmp/hostile.bd', 'node a "<script>alert(1)</script> & text"\n');
  await harness.preview();
  const panel = harness.panels[0];
  assert.match(panel.webview.messages.at(-1).svg, /&lt;script&gt;alert\(1\)&lt;\/script&gt;/);
  assert.doesNotMatch(panel.webview.messages.at(-1).svg, /<script\b/);
  document.text = 'node a @\n';
  harness.changed.fire({ document });
  harness.flush();
  assert.match(panel.webview.messages.at(-1).error, /^error:/);
  assert.equal(panel.webview.messages.at(-1).svg, undefined);
  document.getText = () => { throw new Error('<script>alert(1)</script>'); };
  harness.changed.fire({ document });
  harness.flush();
  assert.match(panel.webview.messages.at(-1).error, /<script>alert\(1\)<\/script>/);
  assert.doesNotMatch(panel.webview.html, /alert\(1\)/);
  document.text = 'node a "Recovered"\n';
  document.getText = () => document.text;
  harness.changed.fire({ document });
  harness.flush();
  assert.match(panel.webview.messages.at(-1).svg, /<svg\b/);
  assert.match(panel.webview.messages.at(-1).svg, /Recovered/);
  harness.dispose();
});

test('Markdown still renders, and WASM file access limits are reported', async () => {
  const harness = extension();
  const md = { renderer: { rules: { fence: () => '<pre>fallback</pre>' } } };
  harness.api.extendMarkdownIt(md);
  const fence = (info, content) => md.renderer.rules.fence([{ info, content }], 0);
  assert.match(fence('bgraph', 'node a "Markdown"\n'), /<svg\b/);
  assert.equal(fence('rust', 'fn main() {}'), '<pre>fallback</pre>');
  assert.match(fence('bgraph', 'node a { bogus: 1 }'), /bgraph-error/);
  harness.document('file:///tmp/include.bd', 'include "child.bd"\n');
  await harness.preview();
  assert.match(harness.panels[0].webview.messages.at(-1).error, /^error:/);
  assert.equal(harness.panels[0].webview.messages.at(-1).svg, undefined);
  harness.dispose();
});

test('missing WASM gives a useful panel error and preserves Markdown fallback', async () => {
  const harness = extension({ missingRenderer: true });
  harness.document('file:///tmp/missing.bd', 'node a\n');
  await harness.preview();
  assert.match(harness.panels[0].webview.messages.at(-1).error, /renderer is missing or could not load/);
  const md = { renderer: { rules: { fence: () => 'fallback' } } };
  harness.api.extendMarkdownIt(md);
  assert.equal(md.renderer.rules.fence([{ info: 'bgraph', content: 'node a' }], 0), 'fallback');
  harness.window.activeTextEditor = undefined;
  await harness.preview();
  assert.match(harness.notices.at(-1), /open a \.bd file/);
  harness.dispose();
});

test('a newly ready or restored webview receives the most recent edit', async () => {
  const harness = extension();
  const document = harness.document('untitled:Untitled-1', 'node a "First"\n');
  await harness.preview(undefined, false);
  const panel = harness.panels[0];
  assert.equal(panel.webview.messages.length, 0);
  document.text = 'node a "Latest"\n';
  harness.changed.fire({ document });
  harness.flush();
  panel.ready();
  assert.match(panel.webview.messages.at(-1).svg, /Latest/);
  panel.ready();
  assert.match(panel.webview.messages.at(-1).svg, /Latest/);
  harness.dispose();
});

test('stylesheets use the language plugin without offering a diagram preview', async () => {
  const harness = extension();
  harness.document('file:///tmp/common.bds', 'style node { fill: #fff }\n');
  await harness.preview();
  assert.equal(harness.panels.length, 0);
  assert.match(harness.notices.at(-1), /shared stylesheet/);
  assert.ok(require('../package.json').contributes.languages[0].extensions.includes('.bds'));
  harness.dispose();
});
