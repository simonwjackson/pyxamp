'use strict';
// The volume slider is the system volume (the default audio sink) in every
// state: following a foreign player, own playback, console mode and idle.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { load } = require('./load');

const P = load('protocol.js'), V = load('view.js');
const FIX = path.resolve(__dirname, '../../fixtures');
const plain = (v) => JSON.parse(JSON.stringify(v));
const raw = (name) => JSON.parse(fs.readFileSync(path.join(FIX, name + '.json'), 'utf8')).state;
function decode(state) {
  const d = P.decodeServer(JSON.stringify({ _tag: 'state', payload: state }));
  assert.equal(d.ok, true, d.reason);
  return d.msg;
}
const connected = { _tag: 'connected', instanceId: 'i' };
function input(state, extra = {}) {
  const s = decode(state);
  return { link: connected, player: s.player, snap: s, observedAt: 1000, now: 1000, wallNow: 0, timeMode: 'elapsed', flash: null, seekDrag: null, ...extra };
}

test('protocol: systemVolume decodes; absent is null; the schema refuses a bad one', () => {
  assert.deepEqual(plain(decode(raw('following-lossless')).systemVolume), { volume: 1.2, muted: true });
  assert.equal(decode(raw('unavailable')).systemVolume, null);
  const bad = (sv) => P.decodeServer(JSON.stringify({ _tag: 'state', payload: Object.assign(raw('idle'), { systemVolume: sv }) })).ok;
  assert.equal(bad({ volume: 0.5, muted: false }), true);
  assert.equal(bad({ volume: -0.1, muted: false }), false, 'below 0');
  assert.equal(bad({ volume: 3.5, muted: false }), false, 'above the wire bound');
  assert.equal(bad({ volume: 0.5 }), false, 'muted is required');
  assert.equal(bad({ volume: 0.5, muted: false, extra: 1 }), false);
});

test('protocol: system.volume is a valid request only for 0..1', () => {
  assert.equal(P.checkClient(P.request('face-1', 'system.volume', { value: 0.42 })), null);
  assert.notEqual(P.checkClient(P.request('face-2', 'system.volume', { value: 1.2 })), null);
  assert.notEqual(P.checkClient(P.request('face-3', 'system.volume', {})), null);
});

test('the slider is the system volume in every state, never the player\'s or the source\'s', () => {
  for (const name of ['following-full', 'following-limited', 'own-pyxis-playing', 'own-pyxis-console', 'own-ossicle-playing', 'own-yielded', 'idle', 'library-sources']) {
    const m = V.main(input(raw(name)));
    assert.equal(m.volume, 0.6, name);
    assert.deepEqual(plain(m.controls.volume), { enabled: true, reason: '' }, name);
    assert.equal(m.volumeTip, 'System volume: 60%', name);
  }
  // Chromium offers no volume of its own; the system volume still works.
  assert.equal(V.main(input(raw('following-limited'))).controls.volume.enabled, true);
});

test('unknown system volume: the slider is off and says why', () => {
  const m = V.main(input(raw('unavailable')));
  assert.equal(m.volume, null);
  assert.deepEqual(plain(m.controls.volume), { enabled: false, reason: 'The system volume is unknown (no PipeWire default sink)' });
  assert.equal(m.volumeTip, 'System volume: The system volume is unknown (no PipeWire default sink)');
  const lost = V.main(input(raw('following-full'), { link: { _tag: 'disconnected', reason: 'x' } }));
  assert.equal(lost.volume, null);
  assert.match(lost.controls.volume.reason, /disconnected/);
});

test('boost: the slider shows full, the tooltip the real value; Up does not lower it', () => {
  const inp = input(raw('following-lossless'));
  const m = V.main(inp);
  assert.deepEqual([m.volume, m.volumeReal, m.muted, m.volumeTip], [1, 1.2, true, 'System volume: 120% (muted)']);
  const sys = V.systemVolume(inp);
  assert.equal(V.volumeStep(sys, 0.02), null);
  assert.equal(V.volumeStep(sys, -0.02), 0.98);
});

test('Up and Down step 2 % and stay in 0..1', () => {
  const at = (volume) => ({ volume, real: volume, muted: false });
  assert.equal(V.volumeStep(at(0.6), 0.02), 0.62);
  assert.equal(V.volumeStep(at(0.6), -0.02), 0.58);
  assert.equal(V.volumeStep(at(0.99), 0.02), 1);
  assert.equal(V.volumeStep(at(0.01), -0.02), 0);
  assert.equal(V.volumeStep(at(1), 0.02), null, 'already full');
  assert.equal(V.volumeStep(null, 0.02), null);
});
