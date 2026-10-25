'use strict';
// The schema compiler: it implements every keyword it meets, or stops.
const test = require('node:test');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const vm = require('node:vm');

const GEN = path.resolve(__dirname, '../../tools/gen-wire-validators');
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'pyx-gen-'));
test.after(() => fs.rmSync(tmp, { recursive: true, force: true }));

function run(schema, extra = []) {
  const file = path.join(tmp, `s${Math.random().toString(36).slice(2)}.json`), out = file + '.js';
  fs.writeFileSync(file, JSON.stringify(schema));
  const r = spawnSync(process.execPath, [GEN, '--schema', file, '--out', out, ...extra], { encoding: 'utf8' });
  return { status: r.status, stderr: r.stderr, out };
}
const base = (defs) => ({
  $schema: 'https://json-schema.org/draft/2020-12/schema', title: 'WireMessage',
  anyOf: [{ $ref: '#/$defs/ClientMessage' }, { $ref: '#/$defs/ServerMessage' }],
  $defs: { ClientMessage: { type: 'object' }, ServerMessage: { type: 'object' }, ...defs },
});

test('an unsupported keyword stops generation and names its place', () => {
  const r = run(base({ Name: { type: 'string', pattern: '^a' } }));
  assert.equal(r.status, 1);
  assert.match(r.stderr, /unsupported keyword pattern at #\/\$defs\/Name/);
  assert.equal(fs.existsSync(r.out), false, 'nothing is written');
});

test('unsupported formats, refs and open objects stop generation too', () => {
  assert.match(run(base({ N: { type: 'integer', format: 'int128' } })).stderr, /unsupported format "int128"/);
  assert.match(run(base({ N: { $ref: 'other.json#/x' } })).stderr, /unsupported or unknown \$ref/);
  assert.match(run(base({ N: { type: 'object', additionalProperties: { type: 'string' } } })).stderr, /additionalProperties other than false/);
  assert.match(run({ ...base({}), $id: 'x' }).stderr, /unsupported root keyword \$id/);
  assert.match(run({ ...base({}), $schema: 'http://json-schema.org/draft-07/schema#' }).stderr, /unsupported \$schema/);
});

test('generated validators implement types, formats and bounds', () => {
  const r = run(base({ ServerMessage: { type: 'object', properties: { n: { type: ['integer', 'null'], format: 'uint8' }, s: { type: 'string', minLength: 1, maxLength: 3 } }, required: ['n'], additionalProperties: false } }));
  assert.equal(r.status, 0, r.stderr);
  const ctx = {}; vm.createContext(ctx);
  vm.runInContext(fs.readFileSync(r.out, 'utf8').replace(/^\.pragma library\s*$/m, ''), ctx);
  const v = (x) => ctx.validateServerMessage(x);
  assert.equal(v({ n: 3 }), null);
  assert.equal(v({ n: null, s: 'abc' }), null);
  assert.match(v({ n: 3.5 }), /#\/n: expected integer or null/);
  assert.match(v({ n: 256 }), /#\/n: out of uint8 range/);
  assert.match(v({ n: 1, s: '' }), /shorter than 1/);
  assert.match(v({ n: 1, s: 'abcd' }), /longer than 3/);
  assert.match(v({}), /missing "?n"?/);
  assert.match(v({ n: 1, x: 1 }), /unexpected property x/);
});

test('--check reports a stale output', () => {
  const schema = base({});
  const r = run(schema);
  assert.equal(r.status, 0, r.stderr);
  fs.appendFileSync(r.out, '// edited\n');
  const file = path.join(tmp, 'check.json'); fs.writeFileSync(file, JSON.stringify(schema));
  const c = spawnSync(process.execPath, [GEN, '--schema', file, '--out', r.out, '--check'], { encoding: 'utf8' });
  assert.equal(c.status, 1);
  assert.match(c.stderr, /stale/);
});
