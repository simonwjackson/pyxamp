'use strict';
// Fit, placement, visualizer physics, preferences, key precedence and text layout.
const test = require('node:test');
const assert = require('node:assert/strict');
const { load } = require('./load');

const F = load('fit.js'), VIS = load('vis.js'), PR = load('prefs.js'), K = load('keys.js'), T = load('text.js'), S = load('sprites.js');
const plain = (v) => JSON.parse(JSON.stringify(v));
const want = (o = {}) => ({ dbl: false, eq: true, pl: true, shade: false, plN: 3, ...o });

test('scale is the monitor scale rounded, doubled by Ctrl+D, and counted once', () => {
  const fhd = { width: 1920, height: 1080, scale: 1 };
  assert.deepEqual([F.plan(fhd, want()).k, F.plan(fhd, want()).physW, F.plan(fhd, want()).physH], [1, 275, 435]);
  const d = F.plan(fhd, want({ dbl: true }));
  assert.deepEqual([d.k, d.physW, d.physH, d.refused, d.plN], [2, 550, 870, false, 3]);
  const myoko = { width: 2880, height: 1800, scale: 2 };
  const m = F.plan(myoko, want());
  assert.deepEqual([m.k, m.physW, m.physH, m.logicalW, m.logicalH, m.contentScale], [2, 550, 870, 275, 435, 1]);
  const frac = F.plan({ width: 2880, height: 1800, scale: 1.5 }, want());
  assert.equal(frac.k, 2); assert.equal(frac.physW, 550); assert.equal(frac.logicalW, 367);
  assert.ok(Math.abs(frac.contentScale * 1.5 - 2) < 1e-9);
});

test('a short monitor shortens the playlist first, then refuses Ctrl+D (decision 22)', () => {
  // Myoko with Ctrl+D: k = 4, the playlist loses one step (mock: 1100x1624).
  const m4 = F.plan({ width: 2880, height: 1800, scale: 2 }, want({ dbl: true }));
  assert.deepEqual([m4.k, m4.plN, m4.physH, m4.refused, m4.fits], [4, 2, 1624, false, true]);
  // 700 px tall at scale 1: Ctrl+D (k = 2) cannot fit even with no playlist rows (696 > 668).
  const short = F.plan({ width: 1366, height: 700, scale: 1 }, want({ dbl: true }));
  assert.equal(short.refused, true); assert.equal(short.k, 1); assert.equal(short.plN, 3);
  // Boundary: exactly fits with 2*16 px margin, and one px less does not.
  const need = 435 + 32;
  assert.equal(F.plan({ width: 1920, height: need, scale: 1 }, want()).plN, 3);
  assert.equal(F.plan({ width: 1920, height: need - 1, scale: 1 }, want()).plN, 2);
  // Nothing becomes unreachable: when even plN 0 cannot fit, the plan still says so.
  const tiny = F.plan({ width: 1920, height: 200, scale: 1 }, want());
  assert.equal(tiny.fits, false); assert.equal(tiny.plN, 0);
  // Shade and folds change the height only.
  assert.equal(F.plan({ width: 1920, height: 1080, scale: 1 }, want({ shade: true, eq: false, pl: false })).physH, 14);
});

test('a remembered position is kept on the monitor', () => {
  assert.deepEqual(plain(F.place(undefined, { w: 275, h: 435 }, { width: 1920, height: 1080 })), { x: 16, y: 16 });
  assert.deepEqual(plain(F.place({ x: 1800, y: 900 }, { w: 275, h: 435 }, { width: 1920, height: 1080 })), { x: 1645, y: 645 });
  assert.deepEqual(plain(F.place({ x: -50, y: 30 }, { w: 275, h: 435 }, { width: 1920, height: 1080 })), { x: 0, y: 30 });
});

test('visualizer bands, falling bars and peaks', () => {
  const cols = Array(75).fill(0); cols[0] = 255; cols[74] = 128;
  const b = VIS.bands(cols);
  assert.equal(b.length, 19); assert.equal(b[0], 16); assert.ok(Math.abs(b[18] - 128 / 255 * 16) < 1e-9);
  let s = VIS.step(VIS.empty(), b);
  assert.equal(s.bars[0], 16); assert.equal(s.peaks[0], 16);
  s = VIS.step(s, null);
  assert.equal(s.bars[0], 14.75); assert.equal(s.peaks[0], 16, 'peak holds');
  for (let i = 0; i < 6; i++) s = VIS.step(s, null);
  assert.ok(s.peaks[0] < 16, 'then falls');
  for (let i = 0; i < 200; i++) s = VIS.step(s, null);
  assert.equal(VIS.isQuiet(s), true, 'silence empties the display');
  const rows = VIS.scopeRows(Array(75).fill(128));
  assert.deepEqual(plain(rows.slice(0, 3)), [8, 8, 8]);
  assert.deepEqual(plain(VIS.scopeRows([0, 255]).slice(0, 2)), [0, 15]);
  assert.deepEqual(plain(VIS.scopeSegments([8, 12])[1]), { x: 1, top: 8, height: 5 });
});

test('preferences survive damaged files and keep monitor positions', () => {
  assert.deepEqual(plain(PR.sanitize(null)), plain(PR.defaults()));
  const p = PR.sanitize({ dbl: true, plN: 99, vis: 'milkdrop', timeMode: 'remaining', eq: { pre: 40, bands: [1, 2] }, positions: { 'DP-1': { x: 10.4, y: 20 }, bad: { x: 'a' } } });
  assert.equal(p.dbl, true); assert.equal(p.plN, 6); assert.equal(p.vis, 'spec'); assert.equal(p.timeMode, 'remaining');
  assert.equal(p.eq.pre, 12); assert.deepEqual(plain(p.eq.bands), Array(10).fill(0));
  assert.deepEqual(plain(p.positions), { 'DP-1': { x: 10, y: 20 } });
  const q = PR.withChanges(p, { onTop: false });
  assert.equal(q.onTop, false); assert.equal(p.onTop, true, 'immutable');
});

const ev = (key, o = {}) => ({ key, text: o.text !== undefined ? o.text : key.length === 1 ? key : '', ctrl: false, alt: false, shift: false, meta: false, ...o });
const queue = { view: 'queue', plOpen: true };

test('keys: Winamp transport, chords, and nothing leaks through the jump box', () => {
  assert.deepEqual(plain(K.decide(ev('x'), queue)), { _tag: 'transport', which: 'play' });
  assert.deepEqual(plain(K.decide(ev('d', { ctrl: true }), queue)), { _tag: 'double' });
  assert.deepEqual(plain(K.decide(ev('w', { alt: true }), queue)), { _tag: 'shade' });
  assert.deepEqual(plain(K.decide(ev('3', { alt: true }), queue)), { _tag: 'info' });
  assert.equal(K.decide(ev('x', { ctrl: true }), queue), null);
  const jump = { view: 'jump', plOpen: true };
  for (const k of ['z', 'x', 'c', 'v', 'b', 'r', 's', 'l', 'j']) assert.deepEqual(plain(K.decide(ev(k), jump)), { _tag: 'jumpType', text: k }, k);
  assert.deepEqual(plain(K.decide(ev('Escape'), jump)), { _tag: 'jumpClose' });
  assert.deepEqual(plain(K.decide(ev('d', { ctrl: true }), jump)), { _tag: 'double' }, 'chords still work');
  assert.equal(K.decide(ev('Escape'), queue), null);
  assert.deepEqual(plain(K.decide(ev('Escape'), { view: 'library', plOpen: true })), { _tag: 'libBack' });
  assert.deepEqual(plain(K.decide(ev('ArrowUp'), queue)), { _tag: 'volume', delta: 0.02 });
  assert.deepEqual(plain(K.decide(ev('ArrowLeft'), queue)), { _tag: 'seek', deltaMs: -5000 });
  assert.deepEqual(['off', 'all', 'one'].map(K.nextRepeat), ['all', 'one', 'off']);
});

test('keys: no plain key leaks from the jump box into seek, volume or transport', () => {
  const jump = { view: 'jump', plOpen: true };
  const leaks = (a) => a !== null && ['transport', 'seek', 'volume', 'repeat', 'shuffle', 'library', 'jump', 'info', 'viewBack'].includes(a._tag);
  for (const k of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Tab', 'Insert', 'Enter']) {
    const a = K.decide(ev(k, { text: '' }), jump);
    assert.equal(leaks(a), false, `${k} -> ${JSON.stringify(a)}`);
    assert.notEqual(a, null, `${k} is consumed by the jump box`);
  }
  // A letter whose event carries no text (an input method, a dead key) is still the box's.
  for (const k of ['z', 'x', 'c', 'v', 'b', 'r', 's', 'l', 'j']) {
    const a = K.decide(ev(k, { text: '' }), jump);
    assert.equal(leaks(a), false, `${k} without text -> ${JSON.stringify(a)}`);
  }
  // Shift+arrow and other unmodified-by-Ctrl/Alt keys stay in the box too.
  assert.equal(leaks(K.decide(ev('ArrowLeft', { text: '', shift: true }), jump)), false);
  // Outside the jump box the arrows still seek, as in Winamp.
  assert.deepEqual(plain(K.decide(ev('ArrowLeft', { text: '' }), queue)), { _tag: 'seek', deltaMs: -5000 });
  // The folded playlist has no jump box, so the arrows are transport again.
  assert.deepEqual(plain(K.decide(ev('ArrowRight', { text: '' }), { view: 'jump', plOpen: false })), { _tag: 'seek', deltaMs: 5000 });
});

test('keys from Qt codes', () => {
  assert.deepEqual(plain(K.fromQt(0x44, 'd', 0x04000000)), { key: 'd', text: 'd', ctrl: true, alt: false, shift: false, meta: false });
  assert.equal(K.fromQt(0x01000000, '', 0).key, 'Escape');
  assert.equal(K.fromQt(0x33, '3', 0x08000000).key, '3');
});

test('text layout uses the generated glyph tables', () => {
  assert.equal(T.text5Cells('AB C', 'green').length, 3);
  assert.equal(T.text5Cells('B', 'amber')[0].sy, S.text5.top + S.text5.colours.indexOf('amber') * 6);
  const w = T.sysWidth('No sources yet.');
  assert.ok(w > 40 && w < 120, String(w));
  assert.ok(T.sysWidth(T.sysFit('A very long line that cannot fit in sixty pixels', 60)) <= 60);
  assert.match(T.sysFitLeft('Jump: ', 'abcdefghijklmnopqrstuvwxyz', 80), /^Jump: \.\.\.[a-z]+z$/);
  assert.ok(S.sys.glyphs[0xe9], 'é is in the atlas');
});
