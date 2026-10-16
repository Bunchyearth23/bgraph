import assert from 'node:assert/strict';
import { copyFileSync, readFileSync, writeFileSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:net';
import { createRequire } from 'node:module';
import { resolve, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { WASI } from 'node:wasi';

const root = resolve(import.meta.dirname, '../..');
const scratch = resolve(process.argv[2]);
const native = join(process.env.CARGO_TARGET_DIR, 'debug');
const cli = join(native, 'bgraph');
const fixtures = [
  ['declared', 'valid'], ['group', 'valid'], ['pie', 'valid'],
  ['components', 'valid'], ['frame-grid', 'valid'],
  ['underconstrained', 'valid'], ['fractional', 'valid'],
  ['frame-nested', 'valid'], ['frame-external', 'valid'],
  ['overlap', 'warning'], ['missing-anchor', 'error'],
  ['conflict', 'error'], ['managed-layout', 'error'],
  ['unknown-component', 'error'],
  ['unicode-error', 'error'], ['size-chain-conflict', 'error'],
  ['frame-inflation-conflict', 'error'],
];
const encoder = new TextEncoder();
const decoder = new TextDecoder();

function run(command, args, input) {
  const result = spawnSync(command, args, {
    cwd: root, input, encoding: 'utf8', timeout: 60_000, maxBuffer: 16 * 1024 * 1024,
  });
  assert.ifError(result.error);
  assert.equal(result.signal, null, `${command}: ${result.signal}`);
  return result;
}

function instantiate(name) {
  const module = new WebAssembly.Module(readFileSync(join(scratch, `${name}.wasm`)));
  let wasi;
  let imports = {};
  if (name === 'wasi') {
    wasi = new WASI({ version: 'preview1', args: [], env: {}, preopens: {} });
    imports = wasi.getImportObject();
  }
  const instance = new WebAssembly.Instance(module, imports);
  if (wasi) wasi.initialize(instance);
  const api = instance.exports;
  return (entry, source) => {
    const bytes = encoder.encode(source);
    const pointer = api.bgraph_alloc(bytes.length);
    try {
      new Uint8Array(api.memory.buffer, pointer, bytes.length).set(bytes);
      const output = api[entry](pointer, bytes.length);
      return decoder.decode(new Uint8Array(api.memory.buffer, output, api.bgraph_result_len()));
    } finally {
      api.bgraph_free(pointer, bytes.length);
    }
  };
}

function typstRenderer() {
  let api, input, output;
  const instance = new WebAssembly.Instance(
    new WebAssembly.Module(readFileSync(join(scratch, 'typst.wasm'))),
    { typst_env: {
      wasm_minimal_protocol_write_args_to_buffer(pointer) {
        new Uint8Array(api.memory.buffer, pointer, input.length).set(input);
      },
      wasm_minimal_protocol_send_result_to_host(pointer, length) {
        output = decoder.decode(new Uint8Array(api.memory.buffer, pointer, length));
      },
    } },
  );
  api = instance.exports;
  return source => {
    input = encoder.encode(source);
    const status = api.bgraph_typst_render(input.length);
    return { status, output };
  };
}

const normalize = diagnostics => diagnostics.map(d => ({
  severity: d.severity, line: d.line, column: d.column ?? d.col, message: d.message,
})).sort((a, b) => a.line - b.line || a.column - b.column || a.message.localeCompare(b.message));

function publishedDiagnostics(source, file) {
  const messages = [
    { id: 1, method: 'initialize', params: { capabilities: {} } },
    { method: 'textDocument/didOpen', params: { textDocument: {
      uri: pathToFileURL(file).href, languageId: 'bgraph', version: 1, text: source,
    } } },
    { id: 2, method: 'shutdown', params: null },
    { method: 'exit', params: null },
  ];
  const input = messages.map(message => {
    const body = JSON.stringify({ jsonrpc: '2.0', ...message });
    return `Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`;
  }).join('');
  const result = run(join(native, 'bgraph-lsp'), [], input);
  assert.equal(result.status, 0, result.stderr);
  let buffer = Buffer.from(result.stdout);
  const answers = [];
  while (buffer.length) {
    const end = buffer.indexOf('\r\n\r\n');
    assert(end >= 0, 'incomplete LSP header');
    const length = Number(/Content-Length: (\d+)/i.exec(buffer.subarray(0, end).toString())?.[1]);
    assert(Number.isFinite(length), 'missing LSP content length');
    const start = end + 4;
    assert(buffer.length >= start + length, 'incomplete LSP body');
    answers.push(JSON.parse(buffer.subarray(start, start + length).toString()));
    buffer = buffer.subarray(start + length);
  }
  const published = answers.find(answer => answer.method === 'textDocument/publishDiagnostics');
  assert(published, 'LSP published no diagnostics');
  return published.params.diagnostics;
}

function lspDiagnostics(source, file) {
  const lines = source.split('\n');
  return normalize(publishedDiagnostics(source, file).map(d => ({
    severity: d.severity === 1 ? 'error' : 'warning',
    line: d.range.start.line + 1,
    column: Array.from(lines[d.range.start.line].slice(0, d.range.start.character)).length + 1,
    message: d.message,
  })));
}

function position(source, start) {
  return {
    line: start.line + 1,
    column: Array.from(source.split('\n')[start.line].slice(0, start.character)).length + 1,
  };
}

const escapeHtml = text => text.replace(/[&<>"']/g,
  c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

function assertMessages(answer, diagnostics, label, escape = text => text) {
  const counts = new Map();
  for (const diag of diagnostics) {
    const message = escape(diag.message);
    counts.set(message, (counts.get(message) ?? 0) + 1);
  }
  for (const [message, count] of counts) {
    assert(answer.split(message).length - 1 >= count, `${label} lost diagnostics: ${message}`);
  }
}

async function preview(file, svg, diagnostics) {
  const listener = createServer();
  await new Promise((resolve, reject) => {
    listener.once('error', reject);
    listener.listen(0, '127.0.0.1', resolve);
  });
  const port = listener.address().port;
  await new Promise(resolve => listener.close(resolve));
  const child = spawn(cli, ['watch', file, '--port', String(port)], { cwd: root, stdio: 'ignore' });
  let spawnError;
  child.once('error', error => { spawnError = error; });
  try {
    let response;
    const deadline = Date.now() + 10_000;
    while (!response && Date.now() < deadline) {
      if (spawnError) throw spawnError;
      assert(child.exitCode === null, 'preview exited before accepting a connection');
      try {
        response = await fetch(`http://127.0.0.1:${port}/`, { signal: AbortSignal.timeout(1000) });
      } catch {
        await new Promise(resolve => setTimeout(resolve, 25));
      }
    }
    assert(response, 'preview startup timed out');
    const html = await response.text();
    assertMessages(html, diagnostics, 'preview', escapeHtml);
    const served = await fetch(`http://127.0.0.1:${port}/svg`, { signal: AbortSignal.timeout(5000) });
    const body = await served.text();
    if (svg) assert.equal(body, svg, 'preview SVG differs from CLI');
    else assert(!body.startsWith('<svg'), 'preview rendered an invalid diagram');
  } finally {
    child.kill();
    if (child.exitCode === null && !spawnError) await new Promise(resolve => child.once('exit', resolve));
  }
}

let extension;
if (process.argv[3] === '--extensions') {
  // Load the packaged extension host code with only the VS Code API stubbed.
  // The actual fence renderer and its wasm module run unchanged.
  const require = createRequire(import.meta.url);
  const manifest = require(join(root, 'editors/vscode/package.json'));
  const archive = join(scratch, `editors/vscode/${manifest.name}-${manifest.version}.vsix`);
  const unpacked = run('unzip', ['-q', archive, '-d', join(scratch, 'vsix')]);
  assert.equal(unpacked.status, 0, unpacked.stderr);
  const extensionRoot = join(scratch, 'vsix/extension');
  assert.equal(require.resolve('vscode-languageclient/node', { paths: [extensionRoot] }).length > 0, true);
  const Module = require('node:module');
  const load = Module._load;
  try {
    Module._load = function(name, ...args) {
      return name === 'vscode' ? { workspace: {}, window: {} } : load.call(this, name, ...args);
    };
    extension = require(join(extensionRoot, 'src/extension.js'));
  } finally {
    Module._load = load;
  }
  const zed = new WebAssembly.Module(readFileSync(join(root, 'editors/zed/target/wasm32-wasip1/debug/zed_bgraph.wasm')));
  assert(WebAssembly.Module.exports(zed).some(item => item.name.includes('init-extension')),
    'Zed shim has no extension entry point');
}

const browser = instantiate('browser');
const wasi = instantiate('wasi');
const typst = typstRenderer();
copyFileSync(join(root, 'typst/bgraph.typ'), join(scratch, 'bgraph.typ'));
copyFileSync(join(scratch, 'typst.wasm'), join(scratch, 'bgraph.wasm'));
for (const [name, expectation] of fixtures) {
  const file = join(root, `tests/runtime/${name}.bd`);
  const source = readFileSync(file, 'utf8');
  const checked = run(cli, ['check', file, '--json']);
  assert.equal(checked.status, expectation === 'error' ? 1 : 0, `${name}: ${checked.stderr}`);
  for (let repetition = 0; repetition < 3; repetition++) {
    const again = run(cli, ['check', file, '--json']);
    assert.equal(again.status, checked.status, `${name}: native diagnostic status drifted`);
    assert.equal(again.stdout, checked.stdout, `${name}: native diagnostic serialization drifted`);
  }
  const diagnostics = normalize(JSON.parse(checked.stdout));
  if (expectation === 'valid') assert.equal(diagnostics.length, 0, `${name}: unexpected diagnostics`);
  else assert(diagnostics.some(d => d.severity === expectation), `${name}: missing ${expectation}`);
  assert.deepEqual(lspDiagnostics(source, file), diagnostics, `${name}: LSP differs from CLI`);
  for (const [runtime, render] of [['browser', browser], ['WASI', wasi]]) {
    const checkedWasm = render('bgraph_check', source);
    const human = JSON.parse(checkedWasm).map(d => ({ ...d,
      column: position(source, { line: d.line - 1, character: d.col - 1 }).column,
    }));
    assert.deepEqual(normalize(human), diagnostics,
      `${name}: ${runtime} diagnostics differ from CLI`);
    assert.equal(render('bgraph_check', source), checkedWasm,
      `${name}: ${runtime} diagnostic serialization drifted`);
  }
  const output = join(scratch, `${name}.svg`);
  const rendered = run(cli, [file, '-o', output]);
  const plugin = typst(source);
  const hasErrors = expectation === 'error';
  assert.equal(rendered.status, hasErrors ? 1 : 0, `${name}: ${rendered.stderr}`);
  assert.equal(plugin.status, hasErrors ? 1 : 0, `${name}: ${plugin.output}`);
  let svg;
  if (!hasErrors) {
    svg = readFileSync(output, 'utf8');
    assert.equal(browser('bgraph_render', source), svg, `${name}: browser SVG differs`);
    assert.equal(wasi('bgraph_render', source), svg, `${name}: WASI SVG differs`);
    assert.equal(plugin.output, svg, `${name}: Typst SVG differs`);
    for (let repetition = 0; repetition < 3; repetition++) {
      for (const runtime of ['browser', 'wasi']) {
        assert.equal(instantiate(runtime)('bgraph_render', source), svg,
          `${name}: fresh ${runtime} instance differs`);
      }
      const again = run(cli, [file, '-o', output]);
      assert.equal(again.status, 0, again.stderr);
      assert.equal(readFileSync(output, 'utf8'), svg, `${name}: native process differs`);
    }
  } else {
    const errors = diagnostics.filter(d => d.severity === 'error');
    assertMessages(rendered.stderr, errors, `${name}: CLI render`);
    assertMessages(browser('bgraph_render', source), errors, `${name}: browser`);
    assertMessages(wasi('bgraph_render', source), errors, `${name}: WASI`);
    assertMessages(plugin.output, errors.map(d => ({ ...d,
      message: `${d.line}:${d.column}: ${d.message}`,
    })), `${name}: Typst`);
  }
  writeFileSync(join(scratch, `${name}.bd`), source);
  const document = join(scratch, `${name}.typ`);
  writeFileSync(document, `#import "bgraph.typ": bg-diagram, bg-diagram-file\n#bg-diagram(read("${name}.bd"))\n#pagebreak()\n#bg-diagram-file(path("${name}.bd"))\n`);
  const compiled = run('typst', ['compile', '--root', scratch, document, join(scratch, `${name}.pdf`)]);
  assert.equal(compiled.status, hasErrors ? 1 : 0, `${name}: Typst runtime: ${compiled.stderr}`);
  await preview(file, svg, diagnostics);
  if (extension) {
    const md = { renderer: { rules: { fence: () => 'fallback' } } };
    extension.extendMarkdownIt(md);
    const fence = md.renderer.rules.fence([{ info: 'bgraph', content: source }], 0, {}, {}, {});
    if (svg) assert.equal(fence, `<div class="bgraph">${svg}</div>`, `${name}: VS Code preview differs`);
    else assertMessages(fence, diagnostics.filter(d => d.severity === 'error'),
      `${name}: VS Code`, escapeHtml);
  }
  console.log(`PASS ${name}: CLI, LSP, browser, WASI, preview, Typst${extension ? ', VS Code' : ''}`);
}
console.log(`Runtime gate passed (${fixtures.length} fixtures${extension ? ', extensions included' : ''}).`);

const examples = JSON.parse(browser('bgraph_examples', ''));
assert(examples.length >= 30, 'the exported gallery unexpectedly lost archetypes');
for (const { name, source } of examples) {
  const file = join(scratch, `gallery-${name}.bd`);
  const output = join(scratch, `gallery-${name}.svg`);
  writeFileSync(file, source);
  const rendered = run(cli, [file, '-o', output]);
  assert.equal(rendered.status, 0, `${name}: ${rendered.stderr}`);
  const svg = readFileSync(output, 'utf8');
  assert.equal(browser('bgraph_render', source), svg, `${name}: gallery browser SVG differs`);
  assert.equal(wasi('bgraph_render', source), svg, `${name}: gallery WASI SVG differs`);
  const plugin = typst(source);
  assert.equal(plugin.status, 0, `${name}: ${plugin.output}`);
  assert.equal(plugin.output, svg, `${name}: gallery Typst SVG differs`);
}
console.log(`Gallery runtime parity passed (${examples.length} archetypes).`);

// File contexts resolve includes; browser/Typst callers deliberately have no filesystem.
for (const [name, expected] of [
  ['include-valid', []],
  ['include-errors', [
    ['includes/invalid.bd', 1, 25, 'unknown_child'],
    ['includes/invalid.bd', 2, 13, 'syntax error'],
    ['includes/component.bd', 5, 1, 'unknown component'],
    ['include-errors.bd', 3, 20, 'unknown_root'],
  ]],
  ['include-missing', [
    ['include-missing.bd', 1, 1, 'cannot open'],
    ['include-missing.bd', 2, 20, 'unknown_root'],
  ]],
  ['include-conflict', [
    ['includes/anchored.bd', 2, 10, 'required layout constraints conflict'],
    ['include-conflict.bd', 4, 3, 'required layout constraints conflict'],
    ['include-conflict.bd', 5, 3, 'required layout constraints conflict'],
  ]],
]) {
  const file = join(root, `tests/runtime/${name}.bd`);
  const source = readFileSync(file, 'utf8');
  const checked = run(cli, ['check', file, '--json']);
  assert.equal(checked.status, expected.length ? 1 : 0, `${name}: ${checked.stderr}`);
  const diagnostics = JSON.parse(checked.stdout);
  assert.equal(diagnostics.length, expected.length, `${name}: lost file diagnostics`);
  for (const [relative, line, column, message] of expected) {
    assert(diagnostics.some(d => resolve(d.file) === join(root, 'tests/runtime', relative)
      && d.line === line && d.column === column && d.message.includes(message)),
    `${name}: missing ${relative}:${line}:${column}: ${message}`);
  }
  const published = publishedDiagnostics(source, file);
  assert.equal(published.length, diagnostics.length, `${name}: LSP lost file diagnostics`);
  for (const diagnostic of diagnostics) {
    const actualFile = resolve(diagnostic.file);
    const child = actualFile !== file;
    const located = published.some(d => {
      if (!child) {
        const start = position(source, d.range.start);
        return d.message === diagnostic.message
          && start.line === diagnostic.line && start.column === diagnostic.column;
      }
      return d.relatedInformation?.some(info => {
        const start = position(readFileSync(actualFile, 'utf8'), info.location.range.start);
        return info.location.uri === pathToFileURL(actualFile).href
          && info.message === diagnostic.message
          && start.line === diagnostic.line && start.column === diagnostic.column
          && source.split('\n')[d.range.start.line].startsWith('include ');
      });
    });
    assert(located, `${name}: LSP lost source location for ${JSON.stringify(diagnostic)}`);
  }
  const output = join(scratch, `${name}.svg`);
  const rendered = run(cli, [file, '-o', output]);
  assert.equal(rendered.status, expected.length ? 1 : 0, `${name}: ${rendered.stderr}`);
  const localized = diagnostics.map(d => ({ ...d,
    message: `${resolve(d.file)}:${d.line}:${d.column}: ${d.message}`,
  }));
  if (expected.length) assertMessages(rendered.stderr, localized, `${name}: CLI render`);
  await preview(file, expected.length ? undefined : readFileSync(output, 'utf8'), localized);
  console.log(`PASS ${name}: CLI, LSP related locations, preview`);
}
console.log('File-context gate passed (4 fixtures).');
