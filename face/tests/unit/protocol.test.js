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

test('targets compare by bus name and owner', () => {
  assert.equal(P.sameTarget({ busName: 'a', owner: ':1' }, { busName: 'a', owner: ':1' }), true);
  assert.equal(P.sameTarget({ busName: 'a', owner: ':1' }, { busName: 'a', owner: ':2' }), false);
  assert.equal(P.sameTarget(null, { busName: 'a', owner: ':1' }), false);
});
