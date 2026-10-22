const assert = require('node:assert/strict');
const { execFileSync, spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { test } = require('node:test');
const manifest = require('../package.json');

test('exported extension packages its supplied WASM and refuses a missing renderer', () => {
  const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'bgraph-vscode-package-'));
  try {
    const extension = path.join(scratch, 'editors', 'vscode');
    fs.mkdirSync(extension, { recursive: true });
    for (const file of ['package.sh', 'package.json', 'language-configuration.json', 'README.md',
      'src', 'syntaxes', 'bgraph.wasm']) {
      fs.cpSync(path.join(__dirname, '..', file), path.join(extension, file), { recursive: true });
    }
    fs.copyFileSync(path.join(__dirname, '..', '..', '..', 'LICENSE'), path.join(scratch, 'LICENSE'));
    const result = spawnSync('bash', ['package.sh', '--no-server'], { cwd: extension, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const archive = path.join(extension, `${manifest.name}-${manifest.version}.vsix`);
    assert.deepEqual(execFileSync('unzip', ['-p', archive, 'extension/src/preview.js']),
      fs.readFileSync(path.join(extension, 'src', 'preview.js')));
    const supplied = fs.readFileSync(path.join(extension, 'bgraph.wasm'));
    assert.deepEqual(execFileSync('unzip', ['-p', archive, 'extension/bgraph.wasm'],
      { maxBuffer: supplied.length + 1024 }), supplied);
    const before = fs.readFileSync(archive);
    fs.unlinkSync(path.join(extension, 'bgraph.wasm'));
    const missing = spawnSync('bash', ['package.sh', '--no-server'], { cwd: extension, encoding: 'utf8' });
    assert.notEqual(missing.status, 0);
    assert.match(missing.stderr, /missing bgraph\.wasm/);
    assert.deepEqual(fs.readFileSync(archive), before, 'failed packaging preserves the existing artifact');
  } finally {
    fs.rmSync(scratch, { recursive: true, force: true });
  }
});
