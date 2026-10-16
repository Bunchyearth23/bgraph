import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import { join, resolve } from 'node:path';

const platforms = new Map([
  ['ubuntu-latest', { os: 'linux', arch: ['x86_64'] }],
  ['ubuntu-24.04-arm', { os: 'linux', arch: ['aarch64'] }],
  ['windows-latest', { os: 'windows', arch: ['x86_64'] }],
  ['windows-11-arm', { os: 'windows', arch: ['aarch64'] }],
  ['macos-latest', { os: 'macos', arch: ['aarch64'] }],
]);
const requiredFixtures = [
  'declared', 'group', 'pie', 'components', 'frame-grid', 'underconstrained',
  'fractional', 'frame-nested', 'frame-external', 'overlap', 'missing-anchor',
  'conflict', 'minimum-conflict', 'managed-layout', 'unknown-component', 'unicode-error',
  'size-chain-conflict', 'frame-inflation-conflict', 'bpmn-dependent',
  'bpmn-attach-conflict', 'bpmn-cycle', 'bpmn-cycle-conflict',
];
const defaultFeatures = { cli: true, pdf: true, json: true, csv: true, completions: true, png: false, wasm: false, typst: false };
const hex = value => typeof value === 'string' && /^[0-9a-f]{16}$/.test(value);
const tuple = (value, length) => Array.isArray(value) && value.length === length && value.every(hex);

function validateGeometry(geometry, id) {
  assert.ok(geometry && typeof geometry === 'object', `${id}: geometry missing`);
  for (const field of ['nodes', 'groups', 'edges', 'guides', 'polylines', 'annotations', 'routes']) {
    assert.ok(Array.isArray(geometry[field]), `${id}: ${field} missing`);
  }
  assert.ok(tuple(geometry.canvas, 4), `${id}: canvas must contain exact f64 bits`);
  for (const box of [...geometry.nodes, ...geometry.groups]) {
    assert.ok(tuple(box.rect, 4), `${id}: rect must contain exact f64 bits`);
    assert.ok(box.styleGeometry && typeof box.styleGeometry === 'object', `${id}: style geometry missing`);
  }
  for (const edge of geometry.edges) {
    assert.ok(Array.isArray(edge.via) && edge.via.every(point => tuple(point, 2)), `${id}: via bits missing`);
  }
  for (const guide of geometry.guides) {
    assert.ok(tuple(guide.from, 2) && tuple(guide.to, 2) && hex(guide.width), `${id}: guide bits missing`);
  }
  for (const line of geometry.polylines) {
    assert.ok(Array.isArray(line.points) && line.points.every(point => tuple(point, 2)) && hex(line.width), `${id}: polyline bits missing`);
  }
  for (const text of geometry.annotations) {
    assert.ok(tuple(text.position, 2) && hex(text.size), `${id}: annotation bits missing`);
  }
  for (const route of geometry.routes) {
    assert.ok(Array.isArray(route.points) && route.points.every(point => tuple(point, 2)), `${id}: route bits missing`);
    if (route.label !== null) assert.ok(tuple(route.label?.rect, 4), `${id}: route label bits missing`);
  }
  assert.ok(geometry.optionsGeometry && hex(geometry.optionsGeometry.padding) && hex(geometry.optionsGeometry.gap), `${id}: option bits missing`);
  assert.equal(typeof geometry.chart?.kind, 'string', `${id}: chart geometry missing`);
  if (geometry.chart.kind === 'pie') {
    const pie = geometry.chart;
    assert.ok(tuple(pie.center, 2) && hex(pie.radius) && hex(pie.inner), `${id}: raw pie geometry missing`);
    assert.ok(Array.isArray(pie.slices) && pie.slices.every(slice => hex(slice.start) && hex(slice.end)), `${id}: raw pie slice bits missing`);
  }
}

function validate(document) {
  assert.equal(document.schemaVersion, 1, 'unknown native snapshot schema');
  assert.ok(typeof document.compilerVersion === 'string' && document.compilerVersion.length > 0, 'compiler version missing');
  assert.ok(typeof document.revision === 'string' && document.revision.length > 0, 'source revision missing');
  if (process.env.GITHUB_SHA) assert.equal(document.revision, process.env.GITHUB_SHA, 'artifact belongs to another revision');
  const expected = platforms.get(document.platform?.runner);
  assert.ok(expected, `unexpected runner: ${document.platform?.runner}`);
  assert.equal(document.platform.os, expected.os, 'runner operating system differs');
  assert.ok(expected.arch.includes(document.platform.arch), 'runner architecture differs');
  assert.equal(document.verification?.inProcessRuns, 3, 'three in-process runs are required');
  assert.equal(document.verification?.freshProcesses, 3, 'three fresh processes are required');
  assert.deepEqual(document.verification?.childInProcessRuns, [3, 3, 3], 'each child must repeat its snapshot three times');
  const { archetypes, cases } = document.snapshot ?? {};
  assert.ok(Array.isArray(archetypes) && archetypes.length === 30 && new Set(archetypes).size === 30, 'all 30 compiler archetypes are required');
  assert.ok(Array.isArray(cases), 'case catalog missing');
  assert.deepEqual(document.snapshot.features, defaultFeatures, 'native proof must use the default features');
  assert.ok(['debug', 'release'].includes(document.snapshot.buildProfile), 'build profile missing');
  const ids = new Set(cases.map(item => item.id));
  assert.equal(ids.size, cases.length, 'duplicate native case');
  assert.equal(cases.filter(item => item.id.startsWith('example/')).length, 30, 'all 30 examples are required');
  for (const genre of archetypes) assert.ok(ids.has(`example/${genre}`), `example/${genre} missing`);
  for (const fixture of requiredFixtures) assert.ok(ids.has(`fixture/${fixture}`), `fixture/${fixture} missing`);
  for (const item of cases) {
    assert.ok(item.id.startsWith('example/') || item.id.startsWith('fixture/'), `unknown case category: ${item.id}`);
    assert.ok(typeof item.source === 'string' && item.source.length > 0, `${item.id}: source missing`);
    assert.equal(item.source.replaceAll('\r\n', '\n'), item.source, `${item.id}: source CRLF was not normalized`);
    assert.equal(typeof item.valid, 'boolean', `${item.id}: validity missing`);
    assert.ok(Array.isArray(item.diagnostics), `${item.id}: diagnostics missing`);
    for (const diagnostic of item.diagnostics) {
      assert.ok(['error', 'warning'].includes(diagnostic.severity), `${item.id}: diagnostic severity missing`);
      assert.equal(typeof diagnostic.message, 'string', `${item.id}: diagnostic message missing`);
      assert.ok(Array.isArray(diagnostic.span) && diagnostic.span.length === 2 && diagnostic.span.every(Number.isInteger), `${item.id}: byte span missing`);
      assert.ok(diagnostic.span[0] >= 0 && diagnostic.span[1] >= diagnostic.span[0] && diagnostic.span[1] <= Buffer.byteLength(item.source), `${item.id}: invalid diagnostic byte span`);
    }
    assert.equal(item.diagnostics.some(diagnostic => diagnostic.severity === 'error'), !item.valid, `${item.id}: validity disagrees with diagnostics`);
    if (item.valid) {
      assert.ok(typeof item.svg === 'string' && item.svg.startsWith('<svg'), `${item.id}: SVG bytes missing`);
      assert.equal(item.renderError, null, `${item.id}: valid render failed`);
      validateGeometry(item.geometry, item.id);
    } else {
      assert.equal(item.svg, null, `${item.id}: invalid input rendered successfully`);
      assert.ok(typeof item.renderError === 'string' && item.renderError.length > 0, `${item.id}: render diagnostics missing`);
      if (item.geometry !== null) validateGeometry(item.geometry, item.id);
    }
  }
}

function firstDifference(left, right, path = '') {
  if (Object.is(left, right)) return null;
  if (typeof left !== typeof right || left === null || right === null) return path;
  if (typeof left !== 'object') return path;
  if (Array.isArray(left) !== Array.isArray(right)) return path;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  if (JSON.stringify(leftKeys) !== JSON.stringify(rightKeys)) return path;
  for (const key of leftKeys) {
    const difference = firstDifference(left[key], right[key], `${path}/${key}`);
    if (difference !== null) return difference;
  }
  return null;
}

function compare(documents) {
  assert.equal(documents.length, platforms.size, 'exactly five native artifacts are required');
  const seen = new Set();
  for (const document of documents) {
    validate(document);
    assert.ok(!seen.has(document.platform.runner), `duplicate runner: ${document.platform.runner}`);
    seen.add(document.platform.runner);
  }
  for (const runner of platforms.keys()) assert.ok(seen.has(runner), `native artifact missing: ${runner}`);
  const baseline = documents.find(document => document.platform.runner === 'ubuntu-latest');
  for (const document of documents) {
    assert.equal(document.compilerVersion, baseline.compilerVersion, 'compiler versions differ');
    assert.equal(document.revision, baseline.revision, 'source revisions differ');
    const difference = firstDifference(baseline.snapshot, document.snapshot);
    assert.equal(difference, null, `${document.platform.runner} differs from ubuntu-latest at /snapshot${difference}`);
  }
  return baseline.snapshot.cases.length;
}

function selfTest() {
  const zero = '0000000000000000';
  const geometry = {
    nodes: [], groups: [], edges: [], guides: [], polylines: [], annotations: [], routes: [],
    canvas: [zero, zero, zero, zero], optionsGeometry: { padding: zero, gap: zero }, chart: { kind: 'free' },
  };
  const archetypes = Array.from({ length: 30 }, (_, index) => `genre-${index}`);
  const cases = [...archetypes.map(name => `example/${name}`), ...requiredFixtures.map(name => `fixture/${name}`)]
    .map(id => ({ id, source: 'node a\n', valid: true, svg: '<svg/>', renderError: null, diagnostics: [], geometry }));
  const documents = [...platforms].map(([runner, platform]) => ({
    schemaVersion: 1, compilerVersion: 'test', revision: process.env.GITHUB_SHA || 'test-revision',
    platform: { runner, os: platform.os, arch: platform.arch[0] },
    verification: { inProcessRuns: 3, freshProcesses: 3, childInProcessRuns: [3, 3, 3] },
    snapshot: structuredClone({ archetypes, buildProfile: 'debug', features: defaultFeatures, cases }),
  }));
  assert.equal(compare(documents), archetypes.length + requiredFixtures.length);
  let rejected = 0;
  for (const mutate of [
    docs => docs.pop(),
    docs => docs.push(structuredClone(docs[0])),
    docs => { docs[1].platform.runner = docs[0].platform.runner; },
    docs => { docs[1].platform.arch = 'wrong'; },
    docs => { docs[1].verification.freshProcesses = 0; },
    docs => { docs[1].verification.childInProcessRuns = [3, 3]; },
    docs => { docs[1].schemaVersion = 2; },
    docs => { docs[1].compilerVersion = 'different'; },
    docs => { docs[1].revision = 'different'; },
    docs => { docs[1].snapshot.cases.pop(); },
    docs => { docs[1].snapshot.features.png = true; },
    docs => { docs[1].snapshot.buildProfile = 'release'; },
    docs => { docs[1].snapshot.cases[0].svg += ' '; },
    docs => { docs[1].snapshot.cases[0].geometry.canvas[0] = '8000000000000000'; },
    docs => { docs[1].snapshot.cases[0].source += '// different\n'; },
    docs => { docs[1].snapshot.cases[0].source = 'node a\r\n'; },
    docs => { delete docs[1].snapshot.cases[0].geometry.canvas; },
    docs => { docs[1].snapshot.cases[0].diagnostics = [{ severity: 'warning', message: 'different', span: [0, 4] }]; },
  ]) {
    const changed = structuredClone(documents);
    mutate(changed);
    assert.throws(() => compare(changed));
    rejected++;
  }
  console.log(`Native comparator self-test passed: complete five-platform proof accepted, ${rejected} incomplete or changed proofs rejected.`);
}

function jsonFiles(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? jsonFiles(path) : entry.isFile() && entry.name.endsWith('.json') ? [path] : [];
  });
}

if (process.argv[2] === '--self-test') {
  selfTest();
} else {
  assert.equal(process.argv.length, 3, 'usage: node tests/runtime/compare-native.mjs <artifact-directory>');
  const files = jsonFiles(resolve(process.argv[2]));
  const documents = files.map(path => JSON.parse(readFileSync(path, 'utf8')));
  const count = compare(documents);
  console.log(`Native parity passed: five runners, ${count} identical sources/SVG/geometry-bit/diagnostic snapshots, 3 local runs and 3 fresh processes per runner.`);
}
