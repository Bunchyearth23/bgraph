const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { join } = require('node:path');
const { writeFileSync } = require('node:fs');
const vscode = require('vscode');

const pause = milliseconds => new Promise(resolve => setTimeout(resolve, milliseconds));
const normalize = diagnostics => diagnostics.sort((a, b) =>
  a.line - b.line || a.column - b.column || a.message.localeCompare(b.message));

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

async function inspectPreview(documents) {
  const previews = () => vscode.window.tabGroups.all.flatMap(group => group.tabs)
    .filter(tab => tab.input instanceof vscode.TabInputWebview
      && tab.input.viewType.includes('bgraph.preview'));
  const waitForPreviews = async count => {
    const deadline = Date.now() + 5_000;
    while (previews().length !== count && Date.now() < deadline) await pause(25);
    assert.equal(previews().length, count, 'expected preview tabs');
  };
  assert.equal(previews().length, 0, 'preview starts closed');
  for (const label of ['Unsaved preview A', 'Unsaved preview B']) {
    const document = await vscode.workspace.openTextDocument({
      language: 'bgraph', content: `node a "${label}"\n`,
    });
    documents.push(document);
    await vscode.window.showTextDocument(document, { preview: false });
    await vscode.commands.executeCommand('bgraph.showPreview');
    const count = documents.filter(doc => doc.isUntitled).length;
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
  console.log('PASS preview: unsaved buffers, panel reuse, editor focus, typing and reopening');
}

module.exports.run = async function run() {
  for (const name of ['BGRAPH_RUNTIME_SERVER', 'BGRAPH_RUNTIME_FIXTURES', 'BGRAPH_RUNTIME_CLI']) {
    assert(process.env[name], `${name} must be set`);
  }
  const configuration = vscode.workspace.getConfiguration('bgraph');
  const previousPath = configuration.inspect('server.path')?.globalValue;
  const documents = [];
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
      ['frame-nested', 'valid'], ['size-chain-conflict', 'error'],
    ]) {
      const deadline = Date.now() + 10_000;
      const file = join(process.env.BGRAPH_RUNTIME_FIXTURES, `${name}.bd`);
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
    await inspectPreview(documents);
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
    }
  }
  if (process.env.BGRAPH_RUNTIME_RESULT) {
    writeFileSync(process.env.BGRAPH_RUNTIME_RESULT,
      'VS Code host: 7 fixtures passed; 2 inspection scenarios passed; preview passed\n');
  }
};
