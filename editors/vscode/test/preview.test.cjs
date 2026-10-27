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

function extension({ missingRenderer = false, withClient = false } = {}) {
  const changed = event(), closed = event();
  const commands = new Map(), documents = new Map(), panels = [], timers = new Map();
  const notices = [];
  const clients = [];
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
      createFileSystemWatcher: () => ({ dispose() {} }),
    },
  };
  const module = { exports: {} };
  vm.runInNewContext(fs.readFileSync(entry, 'utf8'), {
    module, __dirname: path.dirname(entry),
    require(id) {
      if (id === 'vscode') return vscode;
      if (id === 'vscode-languageclient/node') {
        // The real module is not needed to know what the extension asks of it:
        // the constructor's arguments are the whole contract.
        if (!withClient) throw new Error('No language client');
        return {
          TransportKind: { stdio: 0 },
          LanguageClient: class {
            constructor(id, name, serverOptions, clientOptions) {
              clients.push({ id, name, serverOptions, clientOptions });
            }
            start() { return Promise.resolve(); }
            stop() { return Promise.resolve(); }
          },
        };
      }
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
    panels, timers, notices, clients, changed, closed, window,
    api: module.exports,
    document(uri, text, languageId = 'bgraph') {
      const document = {
        uri: { toString: () => uri, scheme: uri.slice(0, uri.indexOf(':')) },
        fileName: uri.split('/').pop(),
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

test('a diagram that reads other files says so, and names the command that works', async () => {
  const harness = extension();

  // `include` is refused by the renderer, and the refusal names a Rust
  // function. The panel must say what the reader can actually do.
  harness.document('file:///tmp/composed.bd', 'include "child.bd"\nnode a "Child"\n');
  await harness.preview();
  const refused = harness.panels[0].webview.messages.at(-1);
  assert.match(refused.error, /^error:/);
  assert.match(refused.error, /`child\.bd`/);
  assert.match(refused.error, /bgraph watch composed\.bd/);
  assert.doesNotMatch(refused.error, /render_file/);
  assert.equal(refused.notice, undefined);

  // A pie that reads a CSV is refused by nothing: the renderer ignores the
  // file and draws what the buffer declares. The picture stays, with the
  // caveat beside it.
  harness.document('file:///tmp/sales.bd', 'type pie { from: "sales.csv" }\nslice a "A" 1\n');
  await harness.preview();
  const data = harness.panels[1].webview.messages.at(-1);
  assert.match(data.svg, /<svg\b/);
  assert.equal(data.error, undefined);
  assert.match(data.notice, /bgraph watch sales\.bd/);

  // A buffer that was never saved has no file to watch, so the command names
  // a placeholder rather than a name that does not exist.
  harness.document('untitled:Untitled-1', 'include "child.bd"\n');
  await harness.preview();
  const unsaved = harness.panels[2].webview.messages.at(-1);
  assert.match(unsaved.error, /Save it as a file, then run `bgraph watch <file>`/);
  assert.doesNotMatch(unsaved.error, /Untitled-1/);

  harness.dispose();
});

test('the caveat is not raised for a diagram that reads no file', async () => {
  const harness = extension();
  // `from:` on an edge names a side, and a label may say anything.
  harness.document('file:///tmp/plain.bd', 'node a "A"\nnode b "B"\na -> b { from: right }\n');
  await harness.preview();
  assert.equal(harness.panels[0].webview.messages.at(-1).notice, undefined);
  assert.match(harness.panels[0].webview.messages.at(-1).svg, /<svg\b/);

  harness.document('file:///tmp/prose.bd', 'node a "include child.bd"\n');
  await harness.preview();
  assert.equal(harness.panels[1].webview.messages.at(-1).notice, undefined);
  harness.dispose();
});

test('the language client is given the untitled scheme, so an unsaved buffer is diagnosed too', () => {
  const harness = extension({ withClient: true });
  assert.equal(harness.clients.length, 1);
  const selector = harness.clients[0].clientOptions.documentSelector;
  assert.ok(selector.some(entry => entry.scheme === 'file' && entry.language === 'bgraph'));
  assert.ok(selector.some(entry => entry.scheme === 'untitled' && entry.language === 'bgraph'),
    'an untitled buffer must reach the server: it is a document like any other');
  harness.dispose();
});

test('a build without the language client still previews and says what it lost', () => {
  const harness = extension();
  assert.equal(harness.clients.length, 0);
  assert.match(harness.notices.at(-1), /no language client/);
  harness.document('file:///tmp/still.bd', 'node a "Still"\n');
  return harness.preview().then(() => {
    assert.match(harness.panels[0].webview.messages.at(-1).svg, /Still/);
    harness.dispose();
  });
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
