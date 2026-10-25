'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { load } = require('./load');

const P = load('protocol.js');
const FIX = path.resolve(__dirname, '../../fixtures');
const line = (tag, payload) => JSON.stringify({ _tag: tag, payload });
const plain = (v) => JSON.parse(JSON.stringify(v));

test('client messages carry _tag and payload in the step-1 shapes', () => {
  assert.deepEqual(JSON.parse(P.hello()), { _tag: 'hello', payload: { version: 1 } });
  const target = { busName: 'org.mpris.MediaPlayer2.mpv', owner: ':1.4', extra: 'dropped' };
  assert.deepEqual(JSON.parse(P.command('c1', target, P.action('transport.seek', { positionMs: 5000, trackId: '/t/1' }))), {
    _tag: 'command', payload: { id: 'c1', target: { busName: 'org.mpris.MediaPlayer2.mpv', owner: ':1.4' }, action: { _tag: 'transport.seek', payload: { positionMs: 5000, trackId: '/t/1' } } },
  });
  assert.deepEqual(JSON.parse(P.command('c2', target, P.action('transport.next'))).payload.action, { _tag: 'transport.next', payload: {} });
});

test('every fixture state and visualizer frame decodes', () => {
  for (const f of fs.readdirSync(FIX)) {
    const fx = JSON.parse(fs.readFileSync(path.join(FIX, f), 'utf8'));
    const s = P.decodeServer(line('state', fx.state));
    assert.equal(s.ok, true, `${f}: ${s.reason}`);
    for (const frame of fx.visualizer) assert.equal(P.decodeServer(line('visualizer', frame)).ok, true, f);
  }
});

test('following state keeps absent metadata absent', () => {
  const fx = JSON.parse(fs.readFileSync(path.join(FIX, 'following-limited.json'), 'utf8'));
  const m = P.decodeServer(line('state', fx.state)).msg;
  assert.equal(m.player._tag, 'following');
  assert.equal('durationMs' in m.player.track, false);
  assert.equal('channels' in m.player.track, false);
  assert.equal('volume' in m.player, false);
  assert.equal(m.player.capabilities.seek, false);
});

test('ready, result and error decode to tagged values', () => {
  assert.deepEqual(plain(P.decodeServer(line('ready', { version: 1, instanceId: 'i-1' }))), { ok: true, msg: { _tag: 'ready', instanceId: 'i-1' } });
  assert.deepEqual(plain(P.decodeServer(line('result', { id: 'c1', outcome: { _tag: 'accepted', payload: {} } }))).msg.outcome, { _tag: 'accepted' });
  assert.deepEqual(plain(P.decodeServer(line('result', { id: 'c1', outcome: { _tag: 'rejected', payload: { code: 'staleTarget', message: 'owner changed' } } }))).msg.outcome,
    { _tag: 'rejected', code: 'staleTarget', message: 'owner changed' });
  assert.deepEqual(plain(P.decodeServer(line('error', { code: 'unsupportedVersion', message: 'v2 only' }))).msg, { _tag: 'error', code: 'unsupportedVersion', message: 'v2 only' });
});

test('malformed lines are refused with a reason, never thrown', () => {
  const bad = [
    'not json', '[]', '{"_tag":"ready"}', line('ready', { version: 2, instanceId: 'x' }), line('nope', {}),
    line('state', { revision: -1, player: { _tag: 'idle', payload: {} }, library: { _tag: 'noSources' }, queue: [] }),
    line('state', { revision: 1, player: { _tag: 'idle', payload: {} }, library: { _tag: 'noSources' }, queue: [{ id: 'x' }] }),
    line('state', { revision: 1, player: { _tag: 'following', payload: { identity: 'x' } }, library: { _tag: 'noSources' }, queue: [] }),
    line('visualizer', { sequence: 1, status: 'ready', columns: [1, 2], waveform: [] }),
    line('visualizer', { sequence: 1, status: 'ready', columns: Array(75).fill(256), waveform: Array(75).fill(128) }),
    line('result', { id: 'x', outcome: { _tag: 'maybe', payload: {} } }),
  ];
  for (const b of bad) assert.equal(P.decodeServer(b).ok, false, b.slice(0, 60));
  assert.equal(P.decodeServer('x'.repeat(P.MAX_LINE + 1)).ok, false);
});

// A valid following state to break one field at a time.
function followingState() {
  const fx = JSON.parse(fs.readFileSync(path.join(FIX, 'following-full.json'), 'utf8'));
  return JSON.parse(JSON.stringify(fx.state));
}
function refused(mutate) {
  const s = followingState(); mutate(s);
  return P.decodeServer(line('state', s));
}
const track = (s) => s.player.payload.track;

test('malformed numbers are refused by the generated schema validator', () => {
  const cases = {
    'fractional bitDepth': (s) => { track(s).bitDepth = 24.5; },
    'fractional channels': (s) => { track(s).channels = 1.5; },
    'negative channels': (s) => { track(s).channels = -1; },
    'fractional durationMs': (s) => { track(s).durationMs = 171000.5; },
    'negative positionMs': (s) => { s.player.payload.positionMs = -1; },
    'fractional positionMs': (s) => { s.player.payload.positionMs = 0.25; },
    'revision past 2^53': (s) => { s.revision = 2 ** 53; },
    'fractional revision': (s) => { s.revision = 1.5; },
    'string volume': (s) => { s.player.payload.volume = '0.5'; },
    'bitDepth past uint32': (s) => { track(s).bitDepth = 2 ** 32; },
  };
  for (const [name, mutate] of Object.entries(cases)) {
    const d = refused(mutate);
    assert.equal(d.ok, false, name);
    assert.match(d.reason, /^#\/payload\//, `${name}: ${d.reason}`);
  }
  assert.match(refused((s) => { track(s).bitDepth = 24.5; }).reason, /bitDepth: expected integer/);
  const frame = (mutate) => { const f = { sequence: 1, status: 'ready', columns: Array(75).fill(0), waveform: Array(75).fill(128) }; mutate(f); return P.decodeServer(line('visualizer', f)); };
  assert.equal(frame((f) => { f.columns[3] = 256; }).ok, false, 'column above 255');
  assert.equal(frame((f) => { f.columns[3] = 1.5; }).ok, false, 'fractional column');
  assert.equal(frame((f) => { f.waveform.pop(); }).ok, false, '74 waveform samples');
  assert.equal(frame((f) => { f.sequence = -2; }).ok, false, 'negative sequence');
  assert.equal(frame(() => {}).ok, true, 'the unbroken frame is valid');
});

test('the schema is strict about shape: unknown and missing fields are refused', () => {
  assert.equal(refused((s) => { track(s).genre = 'rock'; }).ok, false);
  assert.match(refused((s) => { track(s).genre = 'rock'; }).reason, /unexpected property genre/);
  assert.equal(refused((s) => { delete s.player.payload.capabilities.seek; }).ok, false);
  assert.equal(refused((s) => { s.player.payload.transport = 'rewinding'; }).ok, false);
  assert.equal(refused((s) => { s.library = { _tag: 'pyxis' }; }).ok, false);
});

test('optional fields may be null on the wire and become absent', () => {
  const d = P.decodeServer(line('state', (() => { const s = followingState(); track(s).album = null; track(s).bitDepth = null; s.player.payload.repeat = null; s.player.payload.volume = null; return s; })()));
  assert.equal(d.ok, true, d.reason);
  assert.equal('album' in d.msg.player.track, false);
  assert.equal('bitDepth' in d.msg.player.track, false);
  assert.equal('repeat' in d.msg.player, false);
  assert.equal('volume' in d.msg.player, false);
  const v = P.decodeServer(line('visualizer', { target: null, sequence: 0, status: 'idle', columns: Array(75).fill(0), waveform: Array(75).fill(128), reason: null }));
  assert.equal(v.ok, true, v.reason);
  assert.equal(v.msg.target, null);
});

test('every message the face sends is valid under the schema', () => {
  const t = { busName: 'org.mpris.MediaPlayer2.mpv', owner: ':1.4' };
  assert.equal(P.checkClient(P.hello()), null);
  const actions = [P.action('transport.play'), P.action('transport.pause'), P.action('transport.playPause'), P.action('transport.stop'),
    P.action('transport.previous'), P.action('transport.next'), P.action('transport.seek', { positionMs: 5000, trackId: '/t/1' }),
    P.action('volume.set', { value: 0.74 }), P.action('repeat.set', { mode: 'one' }), P.action('shuffle.set', { enabled: false })];
  for (const a of actions) assert.equal(P.checkClient(P.command('face-1-1', t, a)), null, a._tag);
  assert.notEqual(P.checkClient(P.command('face-1-1', t, P.action('volume.set', { value: 1.5 }))), null, 'volume above 1');
  assert.notEqual(P.checkClient(P.command('', t, P.action('transport.play'))), null, 'empty id');
  assert.notEqual(P.checkClient(P.command('x'.repeat(129), t, P.action('transport.play'))), null, 'id over 128');
  assert.notEqual(P.checkClient(P.command('face-1-1', t, P.action('transport.seek', { positionMs: 1.5, trackId: '/t' }))), null, 'fractional seek');
});

test('targets compare by bus name and owner', () => {
  assert.equal(P.sameTarget({ busName: 'a', owner: ':1' }, { busName: 'a', owner: ':1' }), true);
  assert.equal(P.sameTarget({ busName: 'a', owner: ':1' }, { busName: 'a', owner: ':2' }), false);
  assert.equal(P.sameTarget(null, { busName: 'a', owner: ':1' }), false);
});
