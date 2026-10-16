#!/usr/bin/env node
// Real browser proof; uses an existing Playwright installation and Chromium.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { createHash } from 'node:crypto';

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.BGRAPH_PLAYWRIGHT || '/home/bunchy/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const executablePath = process.env.BGRAPH_CHROMIUM || '/home/bunchy/.cache/ms-playwright/chromium-1234/chrome-linux64/chrome';
const binary = resolve(process.argv[2] || 'target/debug/bgraph');
const evidence = process.env.BGRAPH_INSPECTOR_EVIDENCE || await mkdtemp(join(tmpdir(), 'bgraph-inspector-'));
const file = join(evidence, 'inspection.bd');
const child = join(evidence, 'é.bd');
const fixture = name => readFile(new URL(`./${name}.bd`, import.meta.url), 'utf8');
const reserve = createServer();
await new Promise(resolve => reserve.listen(0, '127.0.0.1', resolve));
const port = reserve.address().port;
await new Promise(resolve => reserve.close(resolve));
await writeFile(file, await fixture('frame-nested'));
const server = spawn(binary, ['watch', file, '--port', String(port)], { stdio: ['ignore', 'pipe', 'pipe'] });
let output = '';
server.stdout.on('data', chunk => { output += chunk; });
server.stderr.on('data', chunk => { output += chunk; });
const base = `http://127.0.0.1:${port}`;
let browser;
const errors = [];
const cases = [];
const request = url => fetch(url, { signal: AbortSignal.timeout(10000) });
try {
  for (let n = 0; ; n++) {
    try { if ((await request(base)).ok) break; } catch (error) { if (n >= 100) throw error; }
    await new Promise(resolve => setTimeout(resolve, 50));
  }
  browser = await chromium.launch({ executablePath, headless: true,
    env: { ...process.env, XDG_CONFIG_HOME: join(evidence, 'browser-config'), XDG_CACHE_HOME: join(evidence, 'browser-cache') } });
  const page = await browser.newPage({ viewport: { width: 1440, height: 1000 } });
  page.setDefaultTimeout(10000);
  page.on('pageerror', error => errors.push(error.message));
  async function open(source) {
    if (source !== undefined) await writeFile(file, source);
    await page.goto(base);
    await page.waitForFunction(() => !document.getElementById('inspect-toggle').disabled);
    if (!await page.locator('#inspector').isVisible()) await page.locator('#inspect-toggle').click();
    await page.locator('#inspector').waitFor({ state: 'visible' });
    return (await request(base + '/inspect')).json();
  }
  console.log('frames');
  const frames = await open();
  assert.equal(frames.geometry_available, true);
  await page.locator('.inspect-hit[data-entity="b"]').click();
  assert.equal(await page.locator('#inspect-entity').inputValue(), 'b');
  assert.match(await page.locator('#inspect-details').innerText(), /Parent: inner/);
  assert.match(await page.locator('#inspect-details').innerText(), /parent `inner`/);
  assert.equal(await page.locator('#inspect-overlay circle').count(), 5);
  await page.locator('#inspect-relations button').filter({ hasText: 'place b at' }).click();
  assert.match(await page.locator('#inspect-source button.active').innerText(), /place b at/);
  await page.locator('#inspect-details button').filter({ hasText: 'Parent: inner' }).click();
  assert.equal(await page.locator('#inspect-entity').inputValue(), 'inner');
  assert.match(await page.locator('#inspect-details').innerText(), /Parent: outer/);
  await page.screenshot({ path: join(evidence, 'frames.png') });
  await page.locator('#inspect-source button').filter({ hasText: 'node a {' }).click();
  assert.equal(await page.locator('#inspect-entity').inputValue(), 'a');
  await page.reload();
  await page.waitForFunction(() => document.getElementById('inspect-entity').value === 'a');
  assert.equal(await page.locator('#inspector').isVisible(), true);
  cases.push('translated frames: box click, five anchors, parent, relation/source navigation, reload selection');

  console.log('include');
  await writeFile(child, 'node a "🦀 <script>alert(1)</script>"\nlayout { place a at (12, 34) }\n');
  const included = await open('include "é.bd" as p\nnode root\nlayout { place root below p.a }\n');
  await page.locator('#inspect-entity').selectOption('p.a');
  assert.match(await page.locator('#inspect-file').inputValue(), /é\.bd$/);
  assert.match(await page.locator('#inspect-source button.active').innerText(), /node a/);
  assert.equal(await page.locator('script[src], img').count(), 0);
  assert.equal(included.entities.find(entity => entity.id === 'p.a').declaration.line, 1);
  await page.locator('#inspect-relations button').filter({ hasText: 'place root below p.a' }).click();
  assert.match(await page.locator('#inspect-file').inputValue(), /inspection\.bd$/);
  assert.match(await page.locator('#inspect-source button.active').innerText(), /place root below p.a/);
  await page.screenshot({ path: join(evidence, 'include.png') });
  const oldVersion = await (await request(base + '/version')).text();
  await writeFile(child, 'node a "Changed 🦀"\nlayout { place a at (13, 34) }\n');
  assert.notEqual(await (await request(base + '/version')).text(), oldVersion);
  await page.waitForFunction(() => document.querySelector('#inspect-source')?.textContent.includes('Changed 🦀'));
  cases.push('namespaced include: child/root source links, UTF8 filename, escaped source, include edit reload');

  console.log('conflict');
  const conflict = await open(await fixture('size-chain-conflict'));
  assert.equal(conflict.geometry_available, false);
  await page.locator('#inspect-entity').selectOption('b');
  assert.match(await page.locator('#inspect-details').innerText(), /No solved bounds/);
  const diagnostic = page.locator('#inspect-conflicts button.error').first();
  await diagnostic.click();
  const shown = conflict.diagnostics.find(item => item.severity === 'error' && (!item.entities.length || item.entities.includes('b')));
  assert.ok((await page.locator('#inspect-source').innerText()).includes(shown.source.text.trim()));
  await page.locator('#inspect-relations button').filter({ hasText: 'equal-width b c' }).click();
  assert.match(await page.locator('#inspect-source button.active').innerText(), /equal-width b c/);
  assert.equal(await page.locator('.inspect-hit').count(), 0);
  assert.ok(conflict.diagnostics.some(item => item.entities.includes('b')));
  await page.screenshot({ path: join(evidence, 'conflict.png') });
  cases.push('unsatisfiable size chain: visible conflict messages, relation/diagnostic source, no invented geometry');

  console.log('bpmn');
  const bpmn = await open('type bpmn\nstyle event { fill: #dbeafe }\nstyle gateway { fill: #dcfce7 }\nstyle data { fill: #fef3c7 }\nstyle task { fill: #ede9fe }\nevent start "Start"\ngateway choice "Choice"\ndata input "Input"\ntask work "Work"\nlayout { row start choice input work gap 40 }\n');
  assert.equal(bpmn.geometry_available, true);
  assert.equal(bpmn.entities.find(entity => entity.id === 'start').bounds.width, 40);
  assert.equal(bpmn.entities.find(entity => entity.id === 'choice').bounds.width, 48);
  for (const [id, theme, color] of [['start','event','#dbeafe'],['choice','gateway','#dcfce7'],['input','data','#fef3c7'],['work','task','#ede9fe']]) {
    await page.locator('#inspect-entity').selectOption(id);
    assert.match(await page.locator('#inspect-details').innerText(), new RegExp(`style ${theme}`));
    assert.ok((await page.locator('#inspect-details').innerText()).includes(color));
  }
  await page.screenshot({ path: join(evidence, 'bpmn-style.png') });
  cases.push('BPMN four written forms: resolved shape, fill and canonical style attribution visible');
  console.log('pie');
  const pie = await open('type pie\nstyle slice { fill: #123456 }\nslice organic "Organic" 4\nslice ads "Ads" 2\n');
  assert.equal(pie.geometry_available, true);
  await page.locator('#inspect-entity').selectOption('organic');
  assert.match(await page.locator('#inspect-details').innerText(), /Geometry is managed by type pie/);
  assert.match(await page.locator('#inspect-details').innerText(), /from `style slice`/);
  await page.locator('#inspect-source button').filter({ hasText: 'slice ads' }).click();
  assert.equal(await page.locator('#inspect-entity').inputValue(), 'ads');
  assert.match(await page.locator('#inspect-details').innerText(), /#123456/);
  assert.equal(pie.entities.find(entity => entity.id === 'ads').bounds, null);
  await page.screenshot({ path: join(evidence, 'pie.png') });
  cases.push('managed pie: source line selects slice and shows canonical slice paint; no invented 1:1 bounds');

  console.log('managed');
  const managed = await open('type packet\nfield bits "Payload" 0 40\n');
  assert.equal(managed.geometry_available, true);
  assert.equal(managed.entities[0].bounds, null);
  assert.match(await page.locator('#inspect-details').innerText(), /archetype manages the drawing/);
  assert.match(await page.locator('#inspect-details').innerText(), /Geometry is managed by type packet/);
  await page.screenshot({ path: join(evidence, 'managed.png') });
  cases.push('managed packet: source entity remains available; absent per-entity geometry is stated explicitly');
  assert.deepEqual(errors, []);
  const result = { binary, sha256: createHash('sha256').update(await readFile(binary)).digest('hex'), executablePath, cases, browserErrors: errors, evidence };
  await writeFile(join(evidence, 'result.json'), JSON.stringify(result, null, 2) + '\n');
  console.log(JSON.stringify(result, null, 2));
} finally {
  if (browser) await browser.close();
  server.kill();
  await writeFile(join(evidence, 'server.log'), output);
}
