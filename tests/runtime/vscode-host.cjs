const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { join } = require('node:path');
const { writeFileSync } = require('node:fs');
const Module = require('node:module');
const vscode = require('vscode');

const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const normalize = diagnostics => diagnostics.sort((a, b) =>
  a.line - b.line || a.column - b.column || a.message.localeCompare(b.message));

function instrumentPreviews() {
  const original = vscode.window.createWebviewPanel;
  const originalLoad = Module._load;
  const records = [];
  function createWebviewPanel(...args) {
    const panel = original.apply(this, args);
    if (args[0] !== 'bgraph.preview') return panel;
    const webview = panel.webview;
    const token = `host-${records.length}-${Date.now()}`;
    const pending = new Map();
    let sequence = 0;
    webview.onDidReceiveMessage(message => {
      if (message.type === 'runtime-geometry' && message.token === token) {
        pending.get(message.id)?.(message);
        pending.delete(message.id);
      }
    });
    let owner = webview;
    while (!Object.getOwnPropertyDescriptor(owner, 'html')) owner = Object.getPrototypeOf(owner);
    const descriptor = Object.getOwnPropertyDescriptor(owner, 'html');
    Object.defineProperty(webview, 'html', {
      configurable: true,
      get() { return descriptor.get.call(webview); },
      set(html) {
        const nonce = html.match(/script-src[^;]*'nonce-([^']+)'/)?.[1];
        assert(nonce, 'preview scripts must have a CSP nonce');
        // Only the test host adds this probe; the shipped webview exposes no testing API.
        const probe = `<script nonce="${nonce}">
          (() => {
            const acquire = window.acquireVsCodeApi;
            window.acquireVsCodeApi = () => {
              const api = acquire();
              window.addEventListener('message', ({ data }) => {
                if (data.type !== 'runtime-geometry' || data.token !== ${JSON.stringify(token)}) return;
                if (data.action) document.querySelector('[data-action="' + data.action + '"]')?.click();
                requestAnimationFrame(() => requestAnimationFrame(() => {
                  const rect = element => element?.getBoundingClientRect().toJSON();
                  const viewport = document.querySelector('#viewport');
                  const svg = document.querySelector('#diagram > svg');
                  api.postMessage({ type: data.type, token: data.token, id: data.id,
                    viewport: rect(viewport), svg: rect(svg), text: svg?.textContent,
                    body: { width: document.body.scrollWidth, height: document.body.scrollHeight },
                    window: { width: innerWidth, height: innerHeight } });
                }));
              });
              return api;
            };
          })();
        </script>`;
        descriptor.set.call(webview, html.replace(/<script\b/, `${probe}<script`));
      },
    });
    records.push({ panel, async measure(action) {
      const id = ++sequence;
      const answer = new Promise(resolve => pending.set(id, resolve));
      try {
        await webview.postMessage({ type: 'runtime-geometry', token, id, action });
        return await beforeDeadline(answer, Date.now() + 2_000, 'preview geometry');
      } finally {
        pending.delete(id);
      }
    } });
    return panel;
  }
  // VS Code freezes its public API, so intercept only this extension's import.
  Module._load = function (request, parent, ...args) {
    const loaded = originalLoad.call(this, request, parent, ...args);
    if (request === 'vscode' && parent?.filename.endsWith('/extension/src/extension.js')) {
      return Object.create(loaded, { window: { value: Object.create(loaded.window,
        { createWebviewPanel: { value: createWebviewPanel } }) } });
    }
    return loaded;
  };
  return { records, dispose() { Module._load = originalLoad; } };
}

async function beforeDeadline(promise, deadline, label) {
  let timer;
  try {
    return await Promise.race([
      promise,
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new Error(`${label}: timed out`)),
          Math.max(1, deadline - Date.now()));
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

function flattenSymbols(symbols) {
  return (symbols ?? []).flatMap(symbol => [symbol, ...flattenSymbols(symbol.children)]);
}

async function inspectDocument(document, name, deadline) {
  const text = document.getText();
  const at = (needle, skip = 0) => {
    const offset = text.indexOf(needle);
    assert(offset >= 0, `${name}: missing fixture text ${needle}`);
    return document.positionAt(offset + skip);
  };
  const definition = async position => {
    const locations = await beforeDeadline(vscode.commands.executeCommand(
      'vscode.executeDefinitionProvider', document.uri, position), deadline, `${name}: definition`);
    assert.equal(locations?.length, 1, `${name}: one controlling source location`);
    const location = locations[0];
    assert.equal((location.uri ?? location.targetUri).toString(), document.uri.toString());
    return location.range ?? location.targetSelectionRange ?? location.targetRange;
  };
  const declaration = name === 'declared' ? at('node api', 5) : at('group inner', 6);
  const placement = name === 'declared' ? at('place api at', 6) : at('place inner at', 6);
  assert((await definition(declaration)).start.isEqual(placement),
    `${name}: declaration must navigate to its controlling layout relation`);
  assert((await definition(placement)).start.isEqual(declaration),
    `${name}: layout target must navigate back to its declaration`);

  const references = await beforeDeadline(vscode.commands.executeCommand(
    'vscode.executeReferenceProvider', document.uri, declaration), deadline, `${name}: references`);
  const positions = (references ?? []).map(location => {
    assert.equal(location.uri.toString(), document.uri.toString());
    return `${location.range.start.line}:${location.range.start.character}`;
  }).sort();
  const expectedReferences = name === 'declared'
    ? ['0:5', '3:0', '5:8', '6:6', '7:14', '8:12']
    : ['3:7', '10:7'];
  assert.deepEqual(positions, expectedReferences.sort(), `${name}: source references`);

  const hovers = await beforeDeadline(vscode.commands.executeCommand(
    'vscode.executeHoverProvider', document.uri, declaration), deadline, `${name}: hover`);
  const content = (hovers ?? []).flatMap(hover => hover.contents)
    .map(part => typeof part === 'string' ? part : part.value).join('\n');
  assert(content.includes('Bounds'), `${name}: hover must explain actual bounds`);
  assert(content.includes('file:'), `${name}: hover must link to its source`);
  if (name === 'frame-nested') {
    assert(content.includes('Parent frame') && content.includes('outer'),
      `${name}: hover must identify the enclosing frame`);
  }
  const symbols = flattenSymbols(await beforeDeadline(vscode.commands.executeCommand(
    'vscode.executeDocumentSymbolProvider', document.uri), deadline, `${name}: outline`));
  assert(symbols.some(symbol => symbol.name.toLowerCase().includes('layout')),
    `${name}: outline must expose the layout block: ${JSON.stringify(symbols)}`);
  for (const id of name === 'declared' ? ['api', 'database', 'cache'] : ['outer', 'inner', 'a', 'b']) {
    assert(symbols.some(symbol => symbol.name === id), `${name}: outline is missing ${id}`);
  }
  console.log(`PASS ${name}: VS Code definition, references, hover and scoped outline`);
}

async function inspectPreview(documents, instrumentation) {
  const previews = () => vscode.window.tabGroups.all.flatMap(group => group.tabs)
    .filter(tab => tab.input instanceof vscode.TabInputWebview
      && tab.input.viewType.includes('bgraph.preview'));
  const waitForPreviews = async count => {
    const deadline = Date.now() + 5_000;
    while (previews().length !== count && Date.now() < deadline) await pause(25);
    assert.equal(previews().length, count, 'expected preview tabs');
  };
  assert.equal(previews().length, 0, 'preview starts closed');
  let opened = 0;
  for (const label of ['Unsaved preview A', 'Unsaved preview B']) {
    const document = await vscode.workspace.openTextDocument({
      language: 'bgraph', content: `node a "${label}"\n`,
    });
    documents.push(document);
    await vscode.window.showTextDocument(document, { preview: false });
    await vscode.commands.executeCommand('bgraph.showPreview');
    const count = ++opened;
    await waitForPreviews(count);
    await vscode.commands.executeCommand('bgraph.showPreview');
    assert.equal(previews().length, count, 'reopening a preview reuses its panel');
    assert.equal(vscode.window.activeTextEditor?.document.uri.toString(), document.uri.toString(),
      'preview keeps focus in its source editor');

    const edit = new vscode.WorkspaceEdit();
    edit.insert(document.uri, document.positionAt(document.getText().length), 'node b "Typed update"\n');
    assert(await vscode.workspace.applyEdit(edit));
    await pause(250);
    assert.equal(previews().length, count, 'typing keeps the same preview open');
    if (count === 1 && process.env.BGRAPH_RUNTIME_SCREENSHOT) {
      // The webview paints asynchronously after the extension has sent its HTML.
      await pause(1_000);
      const screenshot = spawnSync('import', ['-window', 'root', process.env.BGRAPH_RUNTIME_SCREENSHOT],
        { timeout: 5_000 });
      assert.ifError(screenshot.error);
      assert.equal(screenshot.status, 0, 'preview screenshot');
    }
  }
  for (const tab of previews()) {
    assert(await vscode.window.tabGroups.close(tab), 'preview closes cleanly');
  }
  await waitForPreviews(0);
  await vscode.commands.executeCommand('bgraph.showPreview');
  await waitForPreviews(1);
  assert(await vscode.window.tabGroups.close(previews()[0]));
  const geometryDocument = await vscode.workspace.openTextDocument({ language: 'bgraph', content:
    'node a "Tall preview"\nnode b "Bottom"\nlayout {\n place a at (0, 0)\n place b at (0, 1800)\n}\n' });
  documents.push(geometryDocument);
  await vscode.window.showTextDocument(geometryDocument, { preview: false });
  await vscode.commands.executeCommand('bgraph.showPreview');
  await waitForPreviews(1);
  const record = instrumentation.records.at(-1);
  const measure = async label => {
    const deadline = Date.now() + 5_000;
    let result;
    do {
      result = await record.measure();
      if (result.text?.includes(label)) return result;
      await pause(50);
    } while (Date.now() < deadline);
    assert.fail(`preview did not render ${label}: ${JSON.stringify(result)}`);
  };
  const assertFits = (geometry, label) => {
    const { viewport, svg, body, window } = geometry;
    assert(viewport?.width > 50 && viewport?.height > 50, `${label}: usable viewport`);
    assert(svg?.width > 0 && svg?.height > 0, `${label}: visible SVG`);
    assert(svg.left >= viewport.left - 1 && svg.right <= viewport.right + 1
      && svg.top >= viewport.top - 1 && svg.bottom <= viewport.bottom + 1,
    `${label}: diagram must fit both dimensions: ${JSON.stringify(geometry)}`);
    assert(body.width <= window.width + 1 && body.height <= window.height + 1,
      `${label}: webview itself must not scroll`);
  };
  const narrow = await measure('Tall preview');
  assertFits(narrow, 'tall diagram');
  const commands = await vscode.commands.getCommands(true);
  if (commands.includes('workbench.action.closeAuxiliaryBar')) {
    await vscode.commands.executeCommand('workbench.action.closeAuxiliaryBar');
  }
  await vscode.commands.executeCommand('workbench.action.focusSecondEditorGroup');
  await vscode.commands.executeCommand('workbench.action.increaseViewSize');
  await pause(250);
  const resized = await record.measure();
  assert.notEqual(resized.viewport.width, narrow.viewport.width, 'editor split actually resized');
  assertFits(resized, 'resized editor split');
  const edit = new vscode.WorkspaceEdit();
  edit.replace(geometryDocument.uri, new vscode.Range(geometryDocument.positionAt(0),
    geometryDocument.positionAt(geometryDocument.getText().length)),
  'node a "Wide preview"\nnode b "Right"\nlayout {\n place a at (0, 0)\n place b at (2400, 0)\n}\n');
  assert(await vscode.workspace.applyEdit(edit));
  const fitted = await measure('Wide preview');
  assertFits(fitted, 'wide diagram after typing');
  const actual = await record.measure('actual-size');
  assert(actual.svg.width > fitted.svg.width * 2, '100% restores the actual drawing size');
  const zoomed = await record.measure('zoom-in');
  assert(zoomed.svg.width > actual.svg.width, 'zoom increases drawing size');
  const restored = await record.measure('fit');
  assertFits(restored, 'Fit after manual zoom');
  if (process.env.BGRAPH_RUNTIME_SCREENSHOT) {
    const screenshot = spawnSync('import', ['-window', 'root',
      process.env.BGRAPH_RUNTIME_SCREENSHOT.replace(/\.png$/, '-geometry.png')], { timeout: 5_000 });
    assert.ifError(screenshot.error);
    assert.equal(screenshot.status, 0, 'geometry screenshot');
  }
  assert(await vscode.window.tabGroups.close(previews()[0]));
  console.log('PASS preview: unsaved buffers, panel reuse, editor focus, typing and reopening');
  console.log('PASS preview geometry: tall/wide diagrams, split resize, zoom and Fit in real webview');
}

module.exports.run = async function run() {
  for (const name of ['BGRAPH_RUNTIME_SERVER', 'BGRAPH_RUNTIME_FIXTURES', 'BGRAPH_RUNTIME_CLI']) {
    assert(process.env[name], `${name} must be set`);
  }
  const configuration = vscode.workspace.getConfiguration('bgraph');
  const previousPath = configuration.inspect('server.path')?.globalValue;
  const documents = [];
  const instrumentation = instrumentPreviews();
  if (process.env.BGRAPH_RUNTIME_RESULT) {
    writeFileSync(process.env.BGRAPH_RUNTIME_RESULT, 'VS Code host: assertions started\n');
  }
  try {
    await configuration.update('server.path', process.env.BGRAPH_RUNTIME_SERVER,
      vscode.ConfigurationTarget.Global);
    const extension = vscode.extensions.getExtension('bunchyearth23.bgraph');
    assert(extension, 'The bgraph development extension was not discovered');
    await beforeDeadline(extension.activate(), Date.now() + 10_000, 'bgraph activation');

    for (const [name, expectation] of [
      ['declared', 'valid'], ['overlap', 'warning'], ['conflict', 'error'],
      ['unknown-component', 'error'], ['unicode-error', 'error'],
      ['frame-nested', 'valid'], ['size-chain-conflict', 'error'], ['style-library', 'valid'],
    ]) {
      const deadline = Date.now() + 10_000;
      const file = join(process.env.BGRAPH_RUNTIME_FIXTURES,
        `${name}.${name === 'style-library' ? 'bds' : 'bd'}`);
      const checked = spawnSync(process.env.BGRAPH_RUNTIME_CLI, ['check', file, '--json'], {
        encoding: 'utf8', timeout: 5_000, maxBuffer: 1024 * 1024,
      });
      assert.ifError(checked.error);
      assert.equal(checked.status, expectation === 'error' ? 1 : 0, checked.stderr);
      const expected = normalize(JSON.parse(checked.stdout).map(diagnostic => ({
        severity: diagnostic.severity,
        line: diagnostic.line,
        column: diagnostic.column ?? diagnostic.col,
        message: diagnostic.message,
      })));
      if (expectation === 'valid') assert.equal(expected.length, 0);
      else assert(expected.some(diagnostic =>
        diagnostic.severity === expectation));

      const document = await beforeDeadline(vscode.workspace.openTextDocument(file), deadline, name);
      documents.push(document);
      await beforeDeadline(vscode.window.showTextDocument(document, { preview: false }), deadline, name);
      assert.equal(document.languageId, 'bgraph', `${name}: language registration is missing`);

      // Empty diagnostics alone cannot prove that the language server started.
      if (name === 'declared') {
        let symbols;
        while (Date.now() < deadline) {
          symbols = await beforeDeadline(vscode.commands.executeCommand(
            'vscode.executeDocumentSymbolProvider', document.uri), deadline, name);
          if (Array.isArray(symbols) && symbols.length) break;
          await pause(50);
        }
        assert(Array.isArray(symbols) && symbols.length,
          `${name}: bgraph LSP did not register its document-symbol provider`);
      }

      let mismatch;
      do {
        const actual = normalize(vscode.languages.getDiagnostics(document.uri).map(diagnostic => ({
          severity: diagnostic.severity === vscode.DiagnosticSeverity.Error ? 'error' : 'warning',
          line: diagnostic.range.start.line + 1,
          column: Array.from(document.lineAt(diagnostic.range.start.line).text
            .slice(0, diagnostic.range.start.character)).length + 1,
          message: diagnostic.message,
        })));
        try {
          assert.deepEqual(actual, expected, `${name}: VS Code diagnostics differ from CLI`);
          mismatch = undefined;
          break;
        } catch (error) {
          mismatch = error;
        }
        await pause(50);
      } while (Date.now() < deadline);
      if (mismatch) throw mismatch;
      console.log(`PASS ${name}: VS Code extension host diagnostics match CLI`);
      if (name === 'declared' || name === 'frame-nested') {
        await inspectDocument(document, name, Date.now() + 10_000);
      }
    }
    const unsaved = await vscode.workspace.openTextDocument({ language: 'bgraph',
      content: 'node a "Unsaved diagnostics" { shape: unknown_shape }\n' });
    documents.push(unsaved);
    await vscode.window.showTextDocument(unsaved, { preview: false });
    const deadline = Date.now() + 5_000;
    while (!vscode.languages.getDiagnostics(unsaved.uri).some(diagnostic =>
      diagnostic.message.includes('unknown_shape')) && Date.now() < deadline) await pause(50);
    assert(vscode.languages.getDiagnostics(unsaved.uri).some(diagnostic =>
      diagnostic.message.includes('unknown_shape')), 'untitled buffers receive LSP diagnostics');
    console.log('PASS untitled: diagnostics before first save');
    await inspectPreview(documents, instrumentation);
  } catch (error) {
    if (process.env.BGRAPH_RUNTIME_RESULT) {
      writeFileSync(process.env.BGRAPH_RUNTIME_RESULT, `${error.stack}\n`);
    }
    throw error;
  } finally {
    try {
      for (const document of documents) {
        await vscode.window.showTextDocument(document, { preview: false });
        await vscode.commands.executeCommand(document.isUntitled
          ? 'workbench.action.revertAndCloseActiveEditor' : 'workbench.action.closeActiveEditor');
      }
    } finally {
      await configuration.update('server.path', previousPath, vscode.ConfigurationTarget.Global);
      instrumentation.dispose();
    }
  }
  if (process.env.BGRAPH_RUNTIME_RESULT) {
    writeFileSync(process.env.BGRAPH_RUNTIME_RESULT,
      'VS Code host: 8 fixtures passed; 2 inspection scenarios passed; untitled diagnostics passed; preview geometry passed\n');
  }
};
