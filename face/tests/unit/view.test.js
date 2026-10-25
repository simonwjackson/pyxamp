'use strict';
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { load } = require('./load');

const P = load('protocol.js');
const V = load('view.js');
const FIX = path.resolve(__dirname, '../../fixtures');
const plain = (v) => JSON.parse(JSON.stringify(v));

function fixture(name) {
  const fx = JSON.parse(fs.readFileSync(path.join(FIX, name + '.json'), 'utf8'));
  const player = P.decodeServer(JSON.stringify({ _tag: 'state', payload: fx.state })).msg.player;
  const frames = fx.visualizer.map((f) => P.decodeServer(JSON.stringify({ _tag: 'visualizer', payload: f })).msg);
  return { player, frames };
}
const connected = { _tag: 'connected', instanceId: 'i' };
const input = (player, extra = {}) => ({ link: connected, player, observedAt: 1000, now: 1000, timeMode: 'elapsed', flash: null, seekDrag: null, ...extra });

test('following is amber; idle, connecting and disconnected are green', () => {
  assert.equal(V.main(input(fixture('following-playing').player)).lcd, 'amber');
  assert.equal(V.main(input(fixture('idle').player)).lcd, 'green');
  assert.equal(V.main(input(null, { link: { _tag: 'connecting' } })).lcd, 'green');
  assert.equal(V.main(input(fixture('following-playing').player, { link: { _tag: 'disconnected', reason: 'x' } })).lcd, 'green');
});

test('daemon loss drops control authority and says so', () => {
  const m = V.main(input(fixture('following-full').player, { link: { _tag: 'disconnected', reason: 'socket closed' } }));
  assert.equal(m.target, null);
  assert.equal(m.marquee.text, 'DAEMON DISCONNECTED');
  assert.equal(m.status, 'error');
  assert.equal(m.clock, null);
  assert.equal(m.position, null);
  for (const c of Object.values(m.controls)) { assert.equal(c.enabled, false); assert.match(c.reason, /disconnected/); }
});

test('position moves on only while playing, at the playback rate, clamped to the length', () => {
  const play = fixture('following-full').player; // 83 s of 171 s
  assert.equal(V.positionMs(input(play, { now: 3000 })), 85000);
  const paused = fixture('following-paused').player;
  assert.equal(V.positionMs(input(paused, { now: 60000 })), 254000);
  const fast = fixture('following-mono-long').player; // rate 1.5
  assert.equal(V.positionMs(input(fast, { now: 3000 })), 21763000);
  assert.equal(V.positionMs(input(play, { now: 1e9 })), 171000);
});

test('clock: elapsed, remaining, and hours past 99:59', () => {
  const full = fixture('following-full').player;
  assert.deepEqual(plain(V.clock(input(full))), { minus: false, digits: [0, 1, 2, 3], hours: false });
  assert.deepEqual(plain(V.clock(input(full, { timeMode: 'remaining' }))), { minus: true, digits: [0, 1, 2, 8], hours: false });
  assert.deepEqual(plain(V.clock(input(fixture('following-mono-long').player))), { minus: false, digits: [0, 6, 0, 2], hours: true });
  assert.equal(V.clock(input(fixture('following-stopped').player)), null);
  assert.equal(V.clock(input(fixture('idle').player)), null);
});

test('a missing length makes remaining time and seek unsupported', () => {
  const lim = fixture('following-limited').player;
  assert.equal(V.clock(input(lim, { timeMode: 'remaining' })).minus, false);
  const m = V.main(input(lim));
  assert.equal(m.controls.seek.enabled, false);
  assert.equal(m.position, null);
  assert.equal(m.controls.next.enabled, false);
  assert.match(m.controls.next.reason, /Chromium does not offer this/);
  assert.equal(m.controls.play.enabled, true);
  assert.equal(m.volume, null);
});

test('format boxes show only what the player reports', () => {
  assert.deepEqual(plain(V.format(input(fixture('following-full').player))), { left: '320', leftUnit: 'kbps', khz: '44', channels: 2 });
  assert.deepEqual(plain(V.format(input(fixture('following-lossless').player))), { left: '24', leftUnit: 'bit', khz: '96', channels: 2 });
  assert.deepEqual(plain(V.format(input(fixture('following-mono-long').player))), { left: '64', leftUnit: 'kbps', khz: '22', channels: 1 });
  assert.deepEqual(plain(V.format(input(fixture('following-playing').player))), { left: null, leftUnit: 'kbps', khz: null, channels: 0 });
});

test('marquee names the app, then the track; idle and failures say what is wrong', () => {
  assert.equal(V.marquee(input(fixture('following-playing').player)).text, 'Firefox: Khruangbin - Tiny Desk Concert (24:31)');
  assert.equal(V.marquee(input(fixture('following-limited').player)).text, 'Chromium: Radio Paradise Main Mix');
  assert.match(V.marquee(input(fixture('idle').player)).text, /NO PLAYER TO FOLLOW\. NO SOURCES YET/);
  assert.equal(V.marquee(input(fixture('unavailable').player)).text, 'PLAYERS UNAVAILABLE: the session bus is not reachable');
  assert.equal(V.status(input(fixture('unavailable').player)), 'error');
  const f = V.marquee(input(fixture('idle').player, { now: 1500, flash: { text: 'VOLUME: 50%', t0: 1400, until: 2400 } }));
  assert.equal(f.text, 'VOLUME: 50%');
  assert.equal(f.fixed, true);
});

test('EQ note only while following', () => {
  assert.equal(V.eqNote(input(fixture('following-playing').player)), 'NO EQ ON FIREFOX');
  assert.equal(V.eqNote(input(fixture('idle').player)), null);
});

test('visualizer: no signal, unavailable capture, a frame for another player, reduced motion', () => {
  const pl = fixture('following-playing'), lim = fixture('following-limited'), idle = fixture('idle'), paused = fixture('following-paused');
  assert.equal(V.visual(input(pl.player), pl.frames[0], 'spec', false)._tag, 'signal');
  assert.equal(V.visual(input(paused.player), paused.frames[0], 'spec', false)._tag, 'silent');
  const un = V.visual(input(lim.player), lim.frames[0], 'spec', false);
  assert.equal(un._tag, 'label'); assert.equal(un.text, 'NO CAPTURE'); assert.match(un.tip, /two Chromium tabs/);
  assert.equal(V.visual(input(lim.player), pl.frames[0], 'spec', false)._tag, 'blank', 'old bars never show for a new player');
  assert.equal(V.visual(input(idle.player), idle.frames[0], 'spec', false)._tag, 'silent');
  assert.equal(V.visual(input(pl.player), pl.frames[0], 'spec', true).text, 'VIS OFF');
  assert.equal(V.visual(input(pl.player), pl.frames[0], 'off', false)._tag, 'blank');
  assert.equal(V.visual(input(pl.player, { link: { _tag: 'disconnected', reason: 'x' } }), pl.frames[0], 'spec', false)._tag, 'blank');
});

test('item info lists actual metadata only', () => {
  const info = V.itemInfo(input(fixture('following-full').player));
  assert.deepEqual(plain(info.rows.map((r) => r[0])), ['Player', 'Title', 'Artist', 'Album', 'Length', 'Bitrate', 'Sample rate', 'Channels', 'State']);
  assert.equal(info.rows.find((r) => r[0] === 'Sample rate')[1], '44.1 kHz');
  assert.deepEqual(plain(V.itemInfo(input(fixture('following-limited').player)).rows.map((r) => r[0])), ['Player', 'Title', 'State']);
  assert.equal(V.itemInfo(input(fixture('idle').player)).empty, 'No player to follow.');
});

test('marquee frames: fits, scrolls 5 sp per 220 ms, or is cut under reduced motion', () => {
  assert.deepEqual(plain(V.marqueeFrame('Daemon disconnected', true, 0, 9999, false)), { text: 'DAEMON DISCONNECTED', offset: 0 });
  const long = 'A'.repeat(40);
  assert.equal(V.marqueeFrame(long, false, 0, 219, false).offset, 0);
  assert.equal(V.marqueeFrame(long, false, 0, 440, false).offset, 10);
  assert.equal(V.marqueeFrame(long, false, 0, 47 * 220, false).offset, 0, 'the loop of text plus separator repeats');
  assert.equal(V.marqueeFrame(long, false, 0, 440, true).text, 'A'.repeat(28) + '...');
  assert.equal(V.toText5('María – café'), 'MARIA - CAFE');
});
