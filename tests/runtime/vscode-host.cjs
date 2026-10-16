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
      if (expected.length === 0) {
        let symbols;
        while (Date.now() < deadline) {
          symbols = await beforeDeadline(vscode.commands.executeCommand(
            'vscode.executeDocumentSymbolProvider', document.uri), deadline, name);
          // After the first fixture proves provider registration, a null/empty
          // outline is valid; the completed request orders us after didOpen.
          if (name !== 'declared' || (Array.isArray(symbols) && symbols.length)) break;
          await pause(50);
        }
        if (name === 'declared') {
          assert(Array.isArray(symbols) && symbols.length,
            `${name}: bgraph LSP did not register its document-symbol provider`);
        }
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
    }
  } catch (error) {
    if (process.env.BGRAPH_RUNTIME_RESULT) {
      writeFileSync(process.env.BGRAPH_RUNTIME_RESULT, `${error.stack}\n`);
    }
    throw error;
  } finally {
    try {
      for (const document of documents) {
        await vscode.window.showTextDocument(document, { preview: false });
        await vscode.commands.executeCommand('workbench.action.closeActiveEditor');
      }
    } finally {
      await configuration.update('server.path', previousPath, vscode.ConfigurationTarget.Global);
    }
  }
  if (process.env.BGRAPH_RUNTIME_RESULT) {
    writeFileSync(process.env.BGRAPH_RUNTIME_RESULT, 'VS Code host: 7 fixtures passed\n');
  }
};
