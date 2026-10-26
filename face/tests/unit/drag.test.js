'use strict';
// Slider drags map from each control's own rectangle, on both faces (ADR 0006).
const test = require('node:test');
const assert = require('node:assert/strict');
const { load } = require('./load');

const D = load('drag.js'), S = load('sliders.js');

// ---------- drags from each control's own rectangle ----------
const sliders = (face) => {
  const out = Object.entries(face === 'winamp' ? S.winamp : S.platinum).filter(([k]) => k !== 'eq').map(([k, r]) => [face + ' ' + k, r]);
  for (let i = 0; i <= 10; i++) out.push([face + ' eq ' + i, S.eqTarget(face, i)]);
  return out;
};

test('a pointer on the drawn thumb maps back to the thumb\'s value, on every slider of both faces', () => {
  for (const face of ['winamp', 'platinum']) for (const [name, r] of sliders(face)) {
    const len = r.axis === 'x' ? r.w : r.h;
    for (const f of [0, 0.1, 0.25, 0.5, 0.73, 1]) {
      const thumb = D.thumbOffset(f, len, r.grip);                     // where the section draws the thumb
      const centre = thumb + Math.floor(r.grip / 2);                   // the pointer on its centre
      const got = D.fraction(centre, len, r.grip);
      assert.ok(Math.abs(got - f) <= 0.5 / (len - r.grip) + 1e-9, `${name} at ${f}: got ${got}`);
    }
    assert.equal(D.fraction(-50, len, r.grip), 0, name + ' clamps low');
    assert.equal(D.fraction(len + 50, len, r.grip), 1, name + ' clamps high');
  }
});

test('the Winamp face keeps its exact mapping (the old constants, now from the rectangles)', () => {
  const v = S.winamp.volume, b = S.winamp.balance, s = S.winamp.seek;
  for (let mx = -3; mx <= 75; mx += 0.5) {
    assert.equal(D.fraction(mx, v.w, v.grip), Math.max(0, Math.min(1, (v.x + mx - 114) / 54)), 'volume ' + mx);
    assert.equal(D.fraction(mx, b.w, b.grip), Math.max(0, Math.min(1, (b.x + mx - 184) / 24)), 'balance ' + mx);
  }
  for (let mx = -3; mx <= 255; mx += 0.5) assert.equal(D.fraction(mx, s.w, s.grip), Math.max(0, Math.min(1, (s.x + mx - 30) / 219)), 'seek ' + mx);
  const oldDb = (y) => Math.max(-12, Math.min(12, Math.round(12 - (y - 5) / 52 * 24)));
  const oldKnob = (db) => 38 + Math.round((12 - db) / 24 * 52);
  const e = S.eqTarget('winamp', 3);
  for (let y = -4; y <= 70; y++) assert.equal(D.dbAt(y, e.h, e.grip, -12, 12), oldDb(y), 'eq ' + y);
  for (let db = -12; db <= 12; db++) assert.equal(e.y + D.thumbOffset(D.dbFraction(db, -12, 12), e.h, e.grip), oldKnob(db), 'knob ' + db);
});

test('the old hard-coded Winamp origins are wrong on the Platinum face (the bug ADR 0006 names)', () => {
  // A click on the Platinum volume thumb at 25 %: the old Winamp formula, given the
  // Platinum target's own x, reads a full slider; the rectangle mapping reads 25 %.
  const v = S.platinum.volume, mx = D.pointerFor(0.25, v.w, v.grip);
  const old = Math.max(0, Math.min(1, (v.x + mx - 114) / 54));
  assert.equal(old, 1);
  assert.ok(Math.abs(D.fraction(mx, v.w, v.grip) - 0.25) < 1e-9);
  const s = S.platinum.seek, sx = D.pointerFor(0.6, s.w, s.grip);
  assert.notEqual(Math.round(Math.max(0, Math.min(1, (s.x + sx - 30) / 219)) * 100), 60);
  assert.ok(Math.abs(D.fraction(sx, s.w, s.grip) - 0.6) < 1e-9);
});

test('Platinum targets match the approved mock (input.js macDragUpdate: (x - h.x - 7) / (h.w - 15))', () => {
  for (const k of ['seek', 'volume', 'balance', 'speed']) {
    const r = S.platinum[k];
    for (let px = r.x; px < r.x + r.w; px += 3) {
      const mock = Math.max(0, Math.min(1, (px - r.x - 7) / (r.w - 15)));
      assert.ok(Math.abs(D.fraction(px - r.x, r.w, r.grip) - mock) < 1e-9, k + ' ' + px);
    }
  }
  const e = S.eqTarget('platinum', 5);
  assert.equal(e.x, 59 + 4 * 23);
  for (let y = 0; y < e.h; y += 2) {
    const mock = Math.max(-12, Math.min(12, Math.round(12 - Math.max(0, Math.min(1, (y - 3) / (e.h - 6))) * 24)));
    assert.equal(D.dbAt(y, e.h, e.grip, -12, 12), mock, 'eq ' + y);
  }
});

