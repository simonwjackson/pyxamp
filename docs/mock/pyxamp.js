// Pyxamp mock: state, layout, renderer (1 sp buffer), menus, input. Needs skin.js and data.js.
'use strict';

// ---------- one clock ----------
const CLOCK = { held: null, t0: performance.now() };
function now() { return CLOCK.held !== null ? CLOCK.held : (performance.now() - CLOCK.t0) / 1000; }

const W = 275, MAIN_H = 116, EQ_H = 116, SHADE_H = 14, PL_MIN = 116, PL_STEP = 29, ROW = 13;
// Folds, windowshade and SUPER+Z are instant, as in Winamp 2 (approved change to decision 20).
const fmt = (s) => { s = Math.max(0, Math.floor(s)); const h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60; return h ? `${h}:${String(m).padStart(2, '0')}:${String(x).padStart(2, '0')}` : `${m}:${String(x).padStart(2, '0')}`; };

function defaults() {
  return {
    src: 'pyxis', role: 'host', device: 'here', queue: PYXIS_QUEUE.map(x => ({ ...x })), cur: 2, play: 'play', pos0: 83, posT: 0,
    timeMode: 'elapsed', vis: 'spec', repeat: 'all', shuffle: false, vol: 0.72, bal: 0, speed: 1.0,
    eqOpen: true, plOpen: true, shade: false, mon: 2, dbl: false, onTop: true, shown: true,
    eq: { on: true, auto: false, pre: 0, bands: [3, 4, 2, 0, -1, -2, 0, 2, 4, 5] },
    plN: 3, plView: 'queue', lib: [], libSel: 0, libPyxis: 'pyxis', jump: null, jumpSel: 1, info: false, sel: [2], scroll: 0,
    albumMode: false, albumOpen: null, albumSel: 0, prompt: null, conflict: null, fail: null, sponsor: null,
    follow: null, handback: null, flash: null, err: null, buffering: null, unreachable: null, handoff: null,
    menu: null, tip: null, pressed: null, drag: null, marqueeT0: 0, props: [], reduced: false, visForced: false, at: { x: 16, y: 16 },
  };
}
const spoken = (S) => S.src === 'ossicle';
const curItem = (S) => S.queue[S.cur];
function posNow(S, t) { const it = curItem(S); if (!it) return 0; let p = S.pos0; if (S.play === 'play' && !S.err && !S.follow) p += (t - S.posT) * (spoken(S) ? S.speed : 1); return Math.min(p, it.len); }
function setPos(S, p, t) { S.pos0 = Math.max(0, p); S.posT = t; }
function flash(S, text, t, dur = 1.6) { S.flash = { text, t0: t, until: t + dur }; }

// ---------- layout ----------
function foldFrac(S, which) { return (which === 'eq' ? S.eqOpen : S.plOpen) ? 1 : 0; } // 1 = open, 0 = folded; no in-between
const plFullH = (S) => PL_MIN + PL_STEP * S.plN;
function layout(S, t) {
  const mh = S.shade ? SHADE_H : MAIN_H, eh = Math.round(EQ_H * foldFrac(S, 'eq', t)), ph = Math.round(plFullH(S) * foldFrac(S, 'pl', t));
  return { main: { y: 0, h: mh }, eq: { y: mh, h: eh }, pl: { y: mh + eh, h: ph }, H: mh + eh + ph };
}
function surfaceAlpha(S) { return S.shown ? 1 : 0; } // SUPER+Z: instant, no fade

// ---------- intrinsic plan: surface size as a pure function of the monitor and the user's choices ----------
// Thresholds are guesses until measured on Myoko: MARGIN_LP logical px kept free above and below.
const MARGIN_LP = 16;
function plan(mon, want) { // mon: {w, h} physical px, scale. want: {dbl, eq, pl, shade, plN}
  const base = Math.max(1, Math.round(mon.scale)), margin = 2 * MARGIN_LP * mon.scale;
  const hFor = (k, n) => k * ((want.shade ? SHADE_H : MAIN_H) + (want.eq ? EQ_H : 0) + (want.pl ? PL_MIN + PL_STEP * n : 0));
  const fit = (k) => { let n = want.plN; while (n > 0 && hFor(k, n) > mon.h - margin) n--; return { n, ok: hFor(k, n) <= mon.h - margin && W * k <= mon.w }; };
  let k = base * (want.dbl ? 2 : 1), f = fit(k), refused = false;
  if (!f.ok && k > base) { k = base; f = fit(k); refused = true; } // 2. Ctrl+D is refused on this monitor (D stays unlit, tooltip says why)
  return { k, plN: f.n, rowsYielded: want.plN - f.n, refused, w: W * k, h: hFor(k, f.n), fits: f.ok };
}

// ---------- hit regions ----------
let HITS = [];
function hit(id, x, y, w, h, d) { HITS.push({ id, x, y, w, h, d }); }

// ---------- visualizer: one simulation per state object, seekable from the clock ----------
const hrand = (a, b = 0, c = 0) => { const v = Math.sin(a * 127.1 + b * 311.7 + c * 74.7) * 43758.5453; return v - Math.floor(v); };
function specTarget(i, n, seed) {
  const x = i / 18, slow = hrand(seed, i, Math.floor(n / 4)), fast = hrand(seed + 9, i, n);
  const beat = (n % 15) < 2 ? 1 : (n % 15) < 4 ? 0.55 : 0, hat = (n % 8) === 0 ? 1 : 0;
  let v = 10.5 * Math.pow(1 - x, 0.55) * (0.5 + 0.5 * slow) + beat * 6.5 * Math.pow(1 - x, 2) + hat * 6 * Math.pow(x, 1.3) * fast + 2.2 * fast + 1.5 * Math.sin(n * 0.21 + i);
  return Math.max(0, Math.min(16, v));
}
function visStep(V, playing) {
  const n = V.tick;
  for (let i = 0; i < 19; i++) {
    const tg = playing ? specTarget(i, n, V.seed) : 0;
    V.bars[i] = Math.max(tg, V.bars[i] - 1.25);
    if (V.bars[i] >= V.peaks[i]) { V.peaks[i] = V.bars[i]; V.vel[i] = 0.12; V.hold[i] = 6; }
    else if (V.hold[i] > 0) V.hold[i]--;
    else { V.peaks[i] = Math.max(0, V.peaks[i] - V.vel[i]); V.vel[i] *= 1.12; }
  }
  V.tick++;
}
function visState(S, t, playing) {
  const want = Math.floor(t * 30), seed = S.src === 'ossicle' ? 3 : S.follow ? 7 : 1;
  let V = S._vis;
  if (!V || V.seed !== seed || want < V.tick || want - V.tick > 300) { V = S._vis = { seed, tick: Math.max(0, want - 120), bars: Array(19).fill(0), peaks: Array(19).fill(0), vel: Array(19).fill(0), hold: Array(19).fill(0) }; }
  while (V.tick < want) visStep(V, playing);
  return V;
}

// ---------- text for the marquee ----------
function marqueeText(S, t) {
  if (S.flash && t < S.flash.until) return { text: S.flash.text, fixed: true };
  if (S.conflict) return { text: 'POSITION CONFLICT', fixed: true };
  if (S.fail && !S.fail.done) { const d = t - S.fail.t0; return { text: d < 2 ? `CANNOT PLAY: RETRY 1 IN ${Math.max(1, Math.ceil(2 - d))} S` : `CANNOT PLAY: RETRY 2 IN ${Math.max(1, Math.ceil(12 - d))} S`, fixed: true }; }
  if (S.err) return { text: S.err };
  if (S.buffering != null) return { text: `[BUFFER: ${S.buffering}%] ${trackLine(S)}` };
  if (S.follow) {
    const f = S.follow, waited = f.waited || 0;
    const tail = waited > 120 ? 'PYXAMP STAYS PAUSED (OVER 2:00)' : `PYXAMP PAUSED, RESUMES IF THIS STOPS BY ${fmt(120 - waited)}`;
    return { text: `${f.app}: ${f.title} (${f.site})  ***  ${tail}` };
  }
  if (!curItem(S)) return { text: `PYXAMP  ***  NOTHING QUEUED IN ${srcName(S).toUpperCase()}. PRESS J TO FIND SOMETHING` };
  const line = trackLine(S);
  if (S.role === 'console') return { text: `${devName(S)}: ${line}` };
  return { text: line };
}
const srcName = (S) => ({ pyxis: 'Pyxis', jellyfin: 'Jellyfin', ossicle: 'Ossicle' })[S.src];
const devName = (S) => (DEVICES.find(d => d.id === S.device) || DEVICES[0]).name.toUpperCase();
function trackLine(S) { const it = curItem(S); if (!it) return ''; const n = S.cur + 1; if (spoken(S)) return it.kind === 'book' ? `${n}. ${it.t}, ${it.chapter} - ${it.show} (${fmt(it.len)})` : `${n}. ${it.show} - ${it.t} (${fmt(it.len)})`; return `${n}. ${it.a} - ${it.t} (${fmt(it.len)})`; }

// ---------- main section ----------
function lcdCols(S) { return S.follow ? { on: C.amber, dim: C.amberDim, mid: C.amberMid } : { on: C.green, dim: C.greenDim, mid: C.greenMid }; }
function chassis(ctx, x, y, w, h) { for (let j = 0; j < h; j++) R(ctx, x, y + j, w, 1, chassisRow(j, h)); }
function drawMain(ctx, S, t, oy) {
  if (S.shade) return drawShade(ctx, S, t, oy);
  const L = lcdCols(S), it = curItem(S), sw = spoken(S), playing = S.follow ? S.follow.playing !== false : S.play === 'play';
  chassis(ctx, 0, oy, W, MAIN_H); frame(ctx, 0, oy, W, MAIN_H, C.hi, C.edge);
  titleBar(ctx, 1, oy + 1, W - 2, 'PYXAMP', true, ['min', 'shade', 'close']);
  hit('opt', 6, oy + 3, 10, 10, { tip: 'Main menu (O)' }); hit('min', 244, oy + 3, 9, 9, { tip: 'Hide (SUPER+Z)' }); hit('shade', 254, oy + 3, 9, 9, { tip: 'Windowshade (Alt+W)' }); hit('close', 264, oy + 3, 9, 9, { tip: 'Hide; playback goes on (SUPER+Z)' });
  frame(ctx, 6, oy + 17, 263, 96, C.lo, C.hi); frame(ctx, 7, oy + 18, 261, 94, C.mid, C.lo);
  // LCD block with clutterbar, status, clock, vis
  lcdBox(ctx, 10, oy + 22, 94, 40, true);
  R(ctx, 19, oy + 22, 1, 40, '#121220');
  const cl = [['O', !!(S.menu && S.menu.kind === 'O')], ['A', S.onTop], ['I', S.info && S.plView !== 'jump'], ['D', S.dbl], ['V', !!(S.menu && S.menu.kind === 'V')]];
  cl.forEach(([ch, on], i) => { const pr = S.pressed === 'cl-' + ch; text5(ctx, ch, 12 + (pr ? 1 : 0), oy + 24 + i * 8 + (pr ? 1 : 0), on ? '#c8c8dc' : '#40405a'); hit('cl-' + ch, 10, oy + 22 + i * 8, 9, 8, { tip: { O: 'Options', A: S.onTop ? 'On top. Click: on the desktop' : 'On the desktop. Click: on top', I: 'Item info', D: 'Double size (Ctrl+D)', V: 'Visualizer' }[ch] }); });
  // status light + indicator
  const st = (S.err || (S.fail && !S.fail.done)) ? 'err' : S.buffering != null ? 'buf' : S.follow ? (playing ? 'play' : 'pause') : S.play;
  R(ctx, 23, oy + 28, 2, 9, st === 'err' ? C.red : st === 'buf' ? (Math.floor(t * 4) % 2 ? C.amber : C.amberDim) : st === 'play' ? L.on : L.dim);
  if (st === 'play' || st === 'buf') map(ctx, ['#....', '##...', '###..', '####.', '#####', '####.', '###..', '##...', '#....'], 27, oy + 28, { '#': L.on });
  if (st === 'pause') { R(ctx, 27, oy + 29, 2, 7, L.on); R(ctx, 31, oy + 29, 2, 7, L.on); }
  if (st === 'stop' || st === 'err') R(ctx, 27, oy + 29, 7, 7, st === 'err' ? C.red : L.on);
  // clock
  let showTime = S.follow ? true : (S.play !== 'stop' && it && !S.err && !(S.fail && !S.fail.done) && !S.conflict);
  if (showTime && S.play === 'pause' && !S.follow && Math.floor(t * 2) % 2 === 1) showTime = false; // Winamp blinks the clock while paused
  hit('time', 34, oy + 24, 66, 16, { tip: 'Elapsed or remaining (Ctrl+T)' });
  if (showTime) {
    const len = S.follow ? S.follow.len : it.len, p = S.follow ? S.follow.pos + (playing ? (t - (S.follow.t0 || 0)) : 0) : posNow(S, t);
    const v = S.timeMode === 'remaining' ? Math.max(0, len - p) : p; const total = Math.floor(v);
    let a, b; if (total >= 6000) { a = Math.floor(total / 3600); b = Math.floor(total / 60) % 60; } else { a = Math.floor(total / 60); b = total % 60; }
    if (S.timeMode === 'remaining') digit9(ctx, '-', 36, oy + 26, L.on);
    digit9(ctx, Math.floor(a / 10) % 10, 48, oy + 26, L.on); digit9(ctx, a % 10, 60, oy + 26, L.on);
    R(ctx, 72, oy + 30, 2, 2, L.on); R(ctx, 72, oy + 35, 2, 2, L.on);
    digit9(ctx, Math.floor(b / 10), 78, oy + 26, L.on); digit9(ctx, b % 10, 90, oy + 26, L.on);
    if (total >= 6000) text5(ctx, 'H', 99, oy + 33, L.mid);
  }
  // visualizer
  drawVis(ctx, S, t, 24, oy + 43, 76, 16, playing);
  hit('vis', 22, oy + 42, 80, 19, { tip: 'Click: spectrum, oscilloscope, off' });
  // marquee
  lcdBox(ctx, 110, oy + 24, 155, 12, false);
  const mq = marqueeText(S, t); let mt = mq.text.toUpperCase();
  if ((mq.fixed && mt.length <= 31) || mt.length <= 31 || S.reduced) { if (mt.length > 31 && S.reduced) mt = mt.slice(0, 28) + '...'; text5(ctx, mt.slice(0, 31), 111, oy + 27, L.on); }
  else {
    const loop = mt + '  ***  ', lw = loop.length * 5, t0 = mq.fixed && S.flash ? S.flash.t0 : S.marqueeT0, off = (Math.floor(Math.max(0, t - t0) / 0.22) * 5) % lw;
    text5(ctx, loop + loop, 111 - off, oy + 27, L.on, { clip: [111, 266] });
  }
  hit('marquee', 110, oy + 24, 155, 12, { tip: mq.text });
  // kbps / kHz / mono / stereo
  const fid = S.follow ? null : (S.play === 'stop' || !it || S.err) ? null : it;
  lcdBox(ctx, 110, oy + 41, 17, 9, false); lcdBox(ctx, 156, oy + 41, 17, 9, false);
  if (fid) {
    const lossless = !!fid.lossless; const k1 = lossless ? String(fid.bits) : String(fid.kbps || (sw ? 64 : 320)); const k2 = String(fid.khz || 44);
    text5(ctx, k1.padStart(3, ' '), 111, oy + 43, L.on); text5(ctx, k2.padStart(3, ' '), 157, oy + 43, L.on);
    label(ctx, lossless ? 'bit' : 'kbps', 131, oy + 42, C.label); label(ctx, 'kHz', 177, oy + 42, C.label);
  } else { label(ctx, 'kbps', 131, oy + 42, C.label); label(ctx, 'kHz', 177, oy + 42, C.label); }
  const ch = S.follow ? 2 : fid ? (fid.ch || 2) : 0;
  label(ctx, 'mono', 212, oy + 42, ch === 1 ? '#c8f8c8' : C.labelDim); label(ctx, 'stereo', 238, oy + 42, ch === 2 ? '#c8f8c8' : C.labelDim);
  // volume
  tube(ctx, 108, oy + 60, 66, volCol(S.vol)); knob(ctx, 107 + Math.round(S.vol * 54), oy + 57, S.drag && S.drag.id === 'vol'); hit('vol', 107, oy + 56, 68, 13, { tip: `Volume ${Math.round(S.vol * 100)}%` });
  // balance, or speed for spoken word
  if (sw) {
    tube(ctx, 178, oy + 60, 36, '#2ab0b8'); R(ctx, 178 + 7, oy + 59, 1, 1, C.label); R(ctx, 178 + 7, oy + 65, 1, 1, C.label); // notch at 1.0x
    const f = (S.speed - 0.5) / 2.5; knob(ctx, 177 + Math.round(f * 24), oy + 57, S.drag && S.drag.id === 'bal'); hit('bal', 177, oy + 56, 38, 13, { tip: `Speed ${S.speed.toFixed(1)}x (synced with Ossicle)` });
  } else {
    tube(ctx, 178, oy + 60, 36, balCol(S.bal)); knob(ctx, 177 + Math.round((S.bal + 1) / 2 * 24), oy + 57, S.drag && S.drag.id === 'bal'); hit('bal', 177, oy + 56, 38, 13, { tip: 'Balance' });
  }
  labelBtn(ctx, 219, oy + 58, 23, 12, 'EQ', { led: S.eqOpen, pressed: S.pressed === 'eqbtn' }); hit('eqbtn', 219, oy + 58, 23, 12, { tip: 'Equalizer (Alt+G)' });
  labelBtn(ctx, 242, oy + 58, 23, 12, 'PL', { led: S.plOpen, pressed: S.pressed === 'plbtn' }); hit('plbtn', 242, oy + 58, 23, 12, { tip: 'Playlist (Alt+E)' });
  // position bar
  R(ctx, 16, oy + 72, 248, 10, '#16162a'); frame(ctx, 16, oy + 72, 248, 10, C.lo, C.hi); R(ctx, 17, oy + 76, 246, 2, '#0e0e1c');
  if (S.sponsor && it && !S.follow) for (const [a, b] of S.sponsor) { const x0 = 17 + Math.round(a / it.len * 246), x1 = 17 + Math.round(b / it.len * 246); R(ctx, x0, oy + 76, Math.max(1, x1 - x0), 2, C.goldDim); }
  const hasPos = S.follow ? !!S.follow.len : (S.play !== 'stop' && it && !S.err && !S.conflict && !(S.fail && !S.fail.done));
  if (hasPos) {
    const len = S.follow ? S.follow.len : it.len, p = S.follow ? S.follow.pos + (playing ? t - (S.follow.t0 || 0) : 0) : (S.drag && S.drag.id === 'pos' ? S.drag.v * len : posNow(S, t));
    const tx = 16 + Math.round(Math.min(1, p / len) * 219), pr = S.drag && S.drag.id === 'pos';
    R(ctx, tx, oy + 72, 29, 10, C.bEdge); R(ctx, tx + 1, oy + 73, 27, 8, pr ? C.bPress : C.bFace); R(ctx, tx + 1, oy + 73, 27, 1, C.bHi); R(ctx, tx + 1, oy + 73, 1, 8, C.bHi); R(ctx, tx + 2, oy + 80, 26, 1, C.bLo);
    for (const gx of [10, 13, 16]) { R(ctx, tx + gx, oy + 75, 1, 4, C.bGlyph); R(ctx, tx + gx + 1, oy + 75, 1, 4, C.bHi); }
  }
  hit('pos', 16, oy + 72, 248, 10, { tip: 'Seek' });
  // transport
  const P_ = (id) => S.pressed === id;
  if (sw) { transportBtn(ctx, 'skipback', 16, oy + 88, 23, 18, P_('prev'), { secs: OSSICLE_INTERVALS.back }); hit('prev', 16, oy + 88, 23, 18, { tip: `Back ${OSSICLE_INTERVALS.back} s (Z)` }); }
  else { transportBtn(ctx, 'prev', 16, oy + 88, 23, 18, P_('prev')); hit('prev', 16, oy + 88, 23, 18, { tip: 'Previous (Z)' }); }
  transportBtn(ctx, 'play', 39, oy + 88, 23, 18, P_('play')); hit('play', 39, oy + 88, 23, 18, { tip: 'Play (X)' });
  transportBtn(ctx, 'pause', 62, oy + 88, 23, 18, P_('pause')); hit('pause', 62, oy + 88, 23, 18, { tip: 'Pause (C)' });
  transportBtn(ctx, 'stop', 85, oy + 88, 23, 18, P_('stop')); hit('stop', 85, oy + 88, 23, 18, { tip: 'Stop (V)' });
  if (sw) { transportBtn(ctx, 'skipfwd', 108, oy + 88, 22, 18, P_('next'), { secs: OSSICLE_INTERVALS.fwd }); hit('next', 108, oy + 88, 22, 18, { tip: `Forward ${OSSICLE_INTERVALS.fwd} s (B)` }); }
  else { transportBtn(ctx, 'next', 108, oy + 88, 22, 18, P_('next')); hit('next', 108, oy + 88, 22, 18, { tip: 'Next (B)' }); }
  transportBtn(ctx, 'eject', 136, oy + 89, 22, 16, P_('eject')); hit('eject', 136, oy + 89, 22, 16, { tip: 'Library (L)' });
  labelBtn(ctx, 164, oy + 89, 47, 15, 'SHUFFLE', { led: !sw && S.shuffle, pressed: P_('shuf'), dim: sw }); hit('shuf', 164, oy + 89, 47, 15, { tip: sw ? 'Shuffle: not for spoken word' : 'Shuffle (S)' });
  bevelBox(ctx, 211, oy + 89, 28, 15, P_('rep')); { const o = P_('rep') ? 1 : 0; led(ctx, 214 + o, oy + 94 + o, !sw && S.repeat !== 'off', sw); repeatIcon(ctx, 222 + o + 1, oy + 94 + o + 1, sw ? C.bFace : C.bHi, !sw && S.repeat === 'one'); repeatIcon(ctx, 222 + o, oy + 94 + o, sw ? C.bDim : C.bGlyph, !sw && S.repeat === 'one'); }
  hit('rep', 211, oy + 89, 28, 15, { tip: sw ? 'Repeat: not for spoken word' : `Repeat: ${S.repeat === 'one' ? 'one track' : S.repeat} (R)` });
  markBig(ctx, 253, oy + 91, P_('logo')); hit('logo', 253, oy + 91, 13, 15, { tip: 'About Pyxamp' });
}
function drawShade(ctx, S, t, oy) {
  const L = lcdCols(S), it = curItem(S), playing = S.follow ? true : S.play === 'play';
  R(ctx, 0, oy, W, SHADE_H, C.edge); titleBar(ctx, 1, oy, W - 2, '', true, ['min', 'unshade', 'close']);
  for (let j = 0; j < SHADE_H; j++) R(ctx, 20, oy + j, 218, 1, mix('#2a2a42', '#1e1e32', j / 13)); R(ctx, 20, oy, 218, 1, C.mid);
  hit('opt', 6, oy + 3, 10, 10, { tip: 'Main menu' }); hit('min', 244, oy + 3, 9, 9, { tip: 'Hide (SUPER+Z)' }); hit('shade', 254, oy + 3, 9, 9, { tip: 'Full size (Alt+W)' }); hit('close', 264, oy + 3, 9, 9, { tip: 'Hide' });
  // mini vis 38x5 and time
  lcdBox(ctx, 22, oy + 4, 40, 7, false); drawVis(ctx, S, t, 23, oy + 5, 38, 5, playing, true);
  lcdBox(ctx, 66, oy + 4, 104, 7, false);
  const mq = marqueeText(S, t); let mt = mq.text.toUpperCase(); if (mt.length > 20) mt = mt.slice(0, 19) + '.';
  text5(ctx, mt, 67, oy + 5, L.on, { clip: [67, 170] });
  if (S.follow || (it && S.play !== 'stop')) { const p = S.follow ? S.follow.pos : posNow(S, t); const s = (S.timeMode === 'remaining' ? '-' : ' ') + fmt(S.timeMode === 'remaining' ? (S.follow ? S.follow.len : it.len) - p : p); lcdBox(ctx, 173, oy + 4, 32, 7, false); text5(ctx, s.slice(-6).padStart(6), 174, oy + 5, L.on); }
  else lcdBox(ctx, 173, oy + 4, 32, 7, false);
  ['prev', 'play', 'pause', 'stop', 'next'].forEach((k, i) => { const x = 208 + i * 6; map(ctx, MINI[k].map(r => r.slice(0, 3)), x, oy + 5, { '#': '#c8c8dc' }); hit(k, x - 1, oy + 3, 6, 9, { tip: k }); });
}
function drawVis(ctx, S, t, x, y, w, h, playing, mini) {
  if (S.role === 'console' && !S.follow) { R(ctx, x, y + Math.floor(h / 2), w, 1, VIS[20]); return; } // flat line: no audio here
  const mode = S.reduced && !S.visForced ? 'off' : S.vis;
  S._visOffLabel = false;
  if (S.reduced && !S.visForced) { // reduced motion: off until a mode is picked, and the vis area says so
    const L = lcdCols(S), s = mini ? 'OFF' : 'VIS OFF', tw = w5(s) - 1; S._visOffLabel = true;
    text5(ctx, s, x + Math.floor((w - tw) / 2), y + Math.floor((h - 5) / 2), L.mid); return;
  }
  if (mode === 'off' || (S.play === 'stop' && !S.follow) || S.err || (S.fail && !S.fail.done)) return;
  if (mode === 'spec') {
    const V = visState(S, t, playing && !S.err);
    if (mini) { for (let i = 0; i < 19; i++) { const bh = Math.round(V.bars[i] / 16 * h); for (let j = 0; j < bh; j++) R(ctx, x + i * 2, y + h - 1 - j, 1, 1, VIS[17 - Math.round(j / h * 15)]); } return; }
    for (let i = 0; i < 19; i++) {
      const bx = x + i * 4, bh = Math.round(V.bars[i]);
      for (let j = 0; j < bh; j++) R(ctx, bx, y + h - 1 - j, 3, 1, VIS[17 - j]);
      const pk = Math.round(V.peaks[i]); if (pk > 0) R(ctx, bx, y + h - pk, 3, 1, VIS[23]);
    }
  } else if (mode === 'osc') {
    if (!playing) { R(ctx, x, y + Math.floor(h / 2), w, 1, VIS[18]); return; }
    const n = Math.floor(t * 30); let prev = null;
    for (let i = 0; i < w - 1; i++) {
      const a = Math.sin(i / 75 * Math.PI * 6 + n * 0.37) * 4.5 + Math.sin(i / 75 * Math.PI * 17 + n * 0.9) * 2 * hrand(n) + (hrand(n, i) - 0.5) * 1.5;
      const yy = Math.max(0, Math.min(h - 1, Math.round(h / 2 + a * (h / 16))));
      const lo = prev == null ? yy : Math.min(prev, yy), hi = prev == null ? yy : Math.max(prev, yy);
      for (let j = lo; j <= hi; j++) R(ctx, x + i, y + j, 1, 1, VIS[18 + Math.min(4, Math.floor(Math.abs(j - h / 2) / 2))]);
      prev = yy;
    }
  }
}

// ---------- EQ section ----------
const BANDS = ['60', '170', '310', '600', '1K', '3K', '6K', '12K', '14K', '16K'];
function eqNote(S) { if (S.role === 'console') return `NO EQ ON ${devName(S)}`.slice(0, 22); if (S.follow) return `NO EQ ON ${S.follow.app.toUpperCase()}`.slice(0, 22); return null; }
function drawEQ(ctx, S, t, oy) {
  chassis(ctx, 0, oy, W, EQ_H); frame(ctx, 0, oy, W, EQ_H, C.hi, C.edge);
  titleBar(ctx, 1, oy + 1, W - 2, 'PYXAMP EQUALIZER', true, ['shade', 'close'], false);
  hit('eq-close', 264, oy + 3, 9, 9, { tip: 'Fold the equalizer (Alt+G)' }); hit('eq-shade', 254, oy + 3, 9, 9, { tip: 'Fold the equalizer' });
  frame(ctx, 8, oy + 15, 259, 98, C.lo, C.hi);
  labelBtn(ctx, 14, oy + 18, 26, 12, 'ON', { led: S.eq.on, pressed: S.pressed === 'eqon' }); hit('eqon', 14, oy + 18, 26, 12, { tip: 'Equalizer on or off' });
  labelBtn(ctx, 40, oy + 18, 32, 12, 'AUTO', { led: S.eq.auto, pressed: S.pressed === 'eqauto' }); hit('eqauto', 40, oy + 18, 32, 12, { tip: 'Auto: a preset per source (music or spoken word)' });
  hit('eqgraph', 86, oy + 17, 113, 19, { tip: "The equalizer changes Pyxamp's own audio only" });
  labelBtn(ctx, 217, oy + 18, 44, 12, 'PRESETS', { pressed: S.pressed === 'eqpre' || (S.menu && S.menu.kind === 'presets') }); hit('eqpre', 217, oy + 18, 44, 12, { tip: 'Presets' });
  // curve graph 113x19
  const gx = 86, gy = oy + 17, note = eqNote(S), off = !S.eq.on || !!note;
  R(ctx, gx, gy, 113, 19, '#14142a'); frame(ctx, gx - 1, gy - 1, 115, 21, C.lo, C.hi);
  for (let i = 0; i < 10; i++) R(ctx, gx + 6 + i * 11, gy, 1, 19, '#2a2a48');
  const pre = Math.round(9 - S.eq.pre / 12 * 9); R(ctx, gx, gy + pre, 113, 1, off ? '#3a3a58' : '#6a6a90');
  const pts = S.eq.bands.map((v, i) => [6 + i * 11, v]);
  for (let x = 0; x < 113; x++) {
    let v; if (x <= pts[0][0]) v = pts[0][1]; else if (x >= pts[9][0]) v = pts[9][1]; else { const i = Math.floor((x - 6) / 11), p = (x - pts[i][0]) / 11, a = pts[Math.max(0, i - 1)][1], b = pts[i][1], c = pts[i + 1][1], d = pts[Math.min(9, i + 2)][1]; v = b + 0.5 * p * (c - a + p * (2 * a - 5 * b + 4 * c - d + p * (3 * (b - c) + d - a))); }
    const yy = Math.max(0, Math.min(18, Math.round(9 - v / 12 * 9))); const col = yy < 6 ? mix('#e85414', '#d8c020', yy / 6) : mix('#d8c020', '#2aa82a', (yy - 6) / 12);
    P(ctx, gx + x, gy + yy, off ? mix(col, '#14142a', 0.65) : col);
  }
  if (note) { const tw = w5(note); R(ctx, gx + Math.floor((113 - tw) / 2) - 2, gy + 6, tw + 3, 8, '#14142a'); text5(ctx, note, gx + Math.floor((113 - tw) / 2), gy + 7, C.gold); }
  // sliders
  const slider = (x, db, id) => {
    for (const ty of [0, 26, 52]) { R(ctx, x - 5, oy + 43 + ty, 3, 1, '#9a9ab4'); R(ctx, x + 16, oy + 43 + ty, 3, 1, '#9a9ab4'); }
    R(ctx, x + 4, oy + 38, 6, 63, C.lo); R(ctx, x + 5, oy + 39, 4, 61, off ? mix(eqCol(db), '#22223a', 0.55) : eqCol(db)); R(ctx, x + 5, oy + 39, 1, 61, mix(eqCol(db), '#ffffff', off ? 0.05 : 0.4)); R(ctx, x + 8, oy + 39, 1, 61, mix(eqCol(db), '#000000', 0.35));
    const ky = oy + 38 + Math.round((12 - db) / 24 * 52), pr = S.drag && S.drag.id === id;
    R(ctx, x + 1, ky, 12, 11, C.bEdge); R(ctx, x + 2, ky + 1, 10, 9, pr ? C.bPress : C.bFace); R(ctx, x + 2, ky + 1, 10, 1, C.bHi); R(ctx, x + 2, ky + 1, 1, 9, C.bHi); R(ctx, x + 3, ky + 9, 9, 1, C.bLo); R(ctx, x + 11, ky + 2, 1, 8, C.bLo);
    R(ctx, x + 4, ky + 4, 6, 1, C.bGlyph); R(ctx, x + 4, ky + 6, 6, 1, C.bGlyph);
    hit(id, x, oy + 38, 14, 63, { tip: id === 'preamp' ? 'Preamp' : `${BANDS[+id.slice(5)]} Hz`.replace('K Hz', ' kHz') });
  };
  slider(21, S.eq.pre, 'preamp'); S.eq.bands.forEach((v, i) => slider(78 + i * 18, v, 'band-' + i));
  const dbl = (s, x, y) => { text5(ctx, s, x, y, C.gold); label(ctx, 'db', x + w5(s) + 1, y - 1, C.gold); };
  dbl('+12', 42, oy + 41); dbl('+0', 47, oy + 67); dbl('-12', 42, oy + 93);
  text5(ctx, 'PREAMP', 13, oy + 104, C.label);
  BANDS.forEach((b, i) => { const x = 78 + i * 18 + 7 - Math.floor((w5(b) - 1) / 2); text5(ctx, b, x, oy + 104, C.label); });
}

// ---------- playlist section ----------
function plTitle(S) {
  if (S.prompt) return S.prompt.title;
  if (S.plView === 'jump') return 'JUMP TO ITEM';
  if (S.info) return 'ITEM INFO';
  if (S.plView === 'lib') return 'LIBRARY';
  if (S.albumMode) return { pyxis: 'PYXIS QUEUE BY ALBUM', jellyfin: 'JELLYFIN QUEUE BY ALBUM', ossicle: 'OSSICLE UP NEXT BY SHOW' }[S.src];
  return { pyxis: 'PYXIS QUEUE', jellyfin: 'JELLYFIN QUEUE', ossicle: 'OSSICLE UP NEXT' }[S.src];
}
function plRows(S) { // returns { head?, rows[], foot?, sel, cur }
  if (S.prompt) { // prompt line over the playlist section: same pattern as the jump box; the list below stays as it was
    const P = S.prompt, under = plRowsBase(S), cursor = (Math.floor(now() * 2) % 2 === 0 || CLOCK.held !== null) ? '_' : ' ';
    return { ...under, head: { l: `${P.label}: ${P.text}${cursor}`, fitLeft: `${P.label}: `, c: C.plCurrent, bg: '#10204a', r: P.right || '', id: 'promptline' }, foot: { l: P.hint || 'Enter: save   Esc: cancel', c: C.plDim, bg: '#0a0a14' }, noSel: true };
  }
  return plRowsBase(S);
}
function plRowsBase(S) {
  if (S.plView === 'jump') return jumpRows(S);
  if (S.info) return infoRows(S);
  if (S.plView === 'lib') return libRows(S);
  if (!S.queue.length) return { rows: [{ l: '' }, { l: `Nothing queued in ${srcName(S)}.`, c: C.plDim, center: true }, { l: 'Press J to find something,', c: C.plDim, center: true }, { l: 'or L to open the library.', c: C.plDim, center: true }], noSel: true };
  if (S.albumMode) return albumRows(S);
  const sw = spoken(S);
  const rows = S.queue.map((it, i) => decorateRow(S, it, i, queueRow(S, it, i)));
  const m = { rows, sel: S.sel, scroll: true };
  if (S.conflict) m.foot = { l: 'Left, Right: pick a position   Enter: keep it', c: C.plDim, bg: '#0a0a14' };
  return m;
}
function queueRow(S, it, i) {
  const sw = spoken(S);
  return [it].map((it) => {
    const cur = i === S.cur, base = sw ? (it.kind === 'book' ? `${i + 1}. ${it.t} - ${it.show}` : `${i + 1}. ${it.show} - ${it.t}`) : `${i + 1}. ${it.a} - ${it.t}`;
    if (cur && S.follow) return { l: base, r: 'paused', c: C.plCurrent };
    if (sw && it.played) return { l: base, r: 'played', c: C.plDim, tick: true };
    if (sw && it.pos && !cur) return { l: base, r: '-' + fmt(it.len - it.pos), c: cur ? C.plCurrent : C.plNormal };
    return { l: base, r: fmt(it.len), c: cur ? C.plCurrent : C.plNormal };
  })[0];
}
// Pass 2: failed tracks are red; a position conflict turns the item's row into a two-way choice.
function decorateRow(S, it, i, row) {
  if (it.failed) return { ...row, c: C.plErr, r: 'cannot play', rc: C.plErr };
  if (S.fail && !S.fail.done && S.fail.idx === i) return { ...row, r: 'retrying', rc: C.plErr };
  if (S.conflict && S.conflict.idx === i) { const c = S.conflict; return { ...row, c: C.plCurrent, bg: '#2e2208', r: null, chips: c.opts.map(o => `${o.dev} ${fmt(o.pos)}`), pick: c.pick }; }
  return row;
}
// Album mode: consecutive queue items of one album (or one show) become one row. A book stays one row.
function queueGroups(S) {
  const sw = spoken(S), g = [];
  S.queue.forEach((it, i) => {
    const key = sw ? (it.kind === 'book' ? 'book:' + i : 'show:' + it.show) : (it.al ? `al:${it.a}:${it.al}` : 'one:' + i);
    const last = g[g.length - 1];
    if (last && last.key === key) last.items.push(i); else g.push({ key, items: [i] });
  });
  return g;
}
function albumRows(S) {
  const sw = spoken(S), groups = queueGroups(S);
  if (S.albumOpen != null && groups[S.albumOpen]) {
    const gr = groups[S.albumOpen], f = S.queue[gr.items[0]];
    const name = sw ? f.show : `${f.a} - ${f.al} (${f.yr})`;
    const rows = gr.items.map(i => decorateRow(S, S.queue[i], i, queueRow(S, S.queue[i], i)));
    return { head: { l: `< ${name}`, alt: sw ? null : `< ${f.a} - ${f.al}`, c: C.plNormal, bg: C.plHead, r: sw ? `${gr.items.length} episodes` : gr.items.length === f.an ? `${f.an} tracks` : `${gr.items.length} of ${f.an} tracks`, id: 'albumback' }, rows, sel: [gr.items.indexOf(S.sel[0])], scroll: true, groupItems: gr.items };
  }
  const rows = groups.map((gr, gi) => {
    const its = gr.items.map(i => S.queue[i]), f = its[0], n = its.length, tot = its.reduce((a, b) => a + b.len, 0);
    const playing = gr.items.includes(S.cur), c = playing ? C.plCurrent : C.plNormal, num = `${gi + 1}. `;
    if (n === 1 && !(sw && f.kind === 'book')) { const row = queueRow(S, f, gr.items[0]); return { ...decorateRow(S, f, gr.items[0], row), l: num + row.l.replace(/^\d+\. /, ''), play: playing }; }
    if (sw && f.kind === 'book') return { l: `${num}${f.t} - ${f.show}`, r: f.pos ? `book  -${fmt(f.len - f.pos)}` : `book  ${fmt(f.len)}`, c, play: playing };
    if (sw) { const pl = its.filter(x => x.played).length; return { l: `${num}${f.show}`, r: `${n} episodes${pl ? `, ${pl} played` : ''}  ${fmt(tot)}`, c, play: playing }; }
    const which = playing ? `now ${gr.items.indexOf(S.cur) + 1}/${n}` : n === f.an ? `${n} tracks` : `${n} of ${f.an} tracks`;
    return { l: `${num}${f.a} - ${f.al} (${f.yr})`, alt: `${num}${f.a} - ${f.al}`, r: `${which}  ${fmt(tot)}`, c, play: playing };
  });
  return { rows, sel: [S.albumSel], scroll: true };
}
function libPathName(S) { return ['Library'].concat(S.lib.map(p => p.name)).join(' > '); }
function libRows(S) {
  const path = S.lib, head = { l: (path.length ? '< ' : '') + libPathName(S), c: C.plNormal, bg: C.plHead, id: 'crumb' };
  let rows = [];
  if (!path.length) {
    for (const key of [S.libPyxis, 'jellyfin', 'ossicle']) {
      const s = LIB[key], id = key.replace(/(9|Core)$/, '');
      if (S.unreachable === id) rows.push({ l: s.name, r: 'unreachable', c: C.plDim, rc: C.plErr });
      else rows.push({ l: s.name, r: `${s.groups.length} groups >`, c: id === S.src ? C.plCurrent : C.plNormal });
    }
  } else if (path[0].key === 'pyxisCore') { rows = coreRows(S);
  } else if (path.length === 1) {
    const s = LIB[path[0].key]; rows = s.groups.map(g => ({ l: g, r: `${(COUNTS[path[0].key.replace(/9$/, '') + '/' + g] || 0).toLocaleString('en-US')} >`, c: C.plNormal }));
  } else if (path[1].name === 'Albums' && path[0].key.startsWith('pyxis') && path.length === 2) {
    rows = PYXIS_ALBUMS.map(([a, b, y, n]) => ({ l: `${a} - ${b} (${y})`, r: `${n} >`, c: C.plNormal }));
  } else if (path.length === 3 && path[0].key.startsWith('pyxis')) {
    rows = DOTS_AND_LOOPS.map(([n, l], i) => ({ l: `${i + 1}. ${n}`, r: fmt(l), c: C.plNormal }));
  } else if (path[0].key === 'ossicle' && path[1].name === 'Books') {
    rows = OSSICLE_BOOKS.map(([b, a, l, p]) => ({ l: `${b} - ${a}`, r: p >= 1 ? 'finished' : p > 0 ? `${Math.round(p * 100)}%` : fmt(l), c: p >= 1 ? C.plDim : C.plNormal }));
  } else if (path[0].key === 'jellyfin' && path[1].name === 'Videos') {
    rows = JF_VIDEOS.map(([v, l]) => ({ l: v, r: fmt(l), c: C.plNormal, video: true }));
  } else rows = [{ l: '(fake data ends here)', c: C.plDim }];
  return { head, rows, sel: [S.libSel], scroll: true };
}
// Pass 2: the Pyxis groups as the core reports them (label, count, kind), in the core's order.
function coreRows(S) {
  const p = S.lib;
  if (p.length === 1) return PYXIS_CORE.map((g, i) => ({ l: g.kind === 'tag' ? `Tag: ${g.l}` : g.l, r: `${g.n.toLocaleString('en-US')} >`, c: C.plNormal, divTop: i > 0 && ['placement', 'signal', 'tag'].includes(PYXIS_CORE[i - 1].kind) && !['placement', 'signal', 'tag'].includes(g.kind), kind: g.kind }));
  const grp = PYXIS_CORE.find(g => g.l === p[1].name || `Tag: ${g.l}` === p[1].name);
  if (p.length === 2 && grp && grp.kind === 'playlists') return PYXIS_PLAYLISTS.map(([n, k, l]) => ({ l: n, r: `${k} tracks  ${fmt(l)} >`, c: C.plNormal, kind: 'playlist' }));
  if (p.length === 2 && grp && grp.kind === 'stations') return PYXIS_STATIONS.map(([n, d]) => ({ l: n, r: `started ${d}`, c: C.plNormal, kind: 'station' }));
  if (p.length === 2 && grp && grp.kind === 'bookmarks') return [{ l: '(fake data ends here)', c: C.plDim }];
  if (p.length === 2) return PYXIS_ALBUMS.map(([a, b, y, n]) => ({ l: `${a} - ${b} (${y})`, r: `${n} >`, c: C.plNormal, kind: 'album', artist: a }));
  if (p.length === 3 && p[1].name === 'Playlists') return SUNDAY_MORNING.map(([a, t, l], i) => ({ l: `${i + 1}. ${a} - ${t}`, r: fmt(l), c: C.plNormal, kind: 'track', seed: t }));
  if (p.length === 3) return DOTS_AND_LOOPS.map(([n, l], i) => ({ l: `${i + 1}. ${n}`, r: fmt(l), c: C.plNormal, kind: 'track', seed: n }));
  return [{ l: '(fake data ends here)', c: C.plDim }];
}
function jumpResults(S) {
  const q = (S.jump || '').toLowerCase(); if (!q) return [];
  return CORPUS.filter(r => r.text.toLowerCase().includes(q) && !(S.unreachable && r.src === S.unreachable));
}
const TAG = { pyxis: 'PYX', jellyfin: 'JF', ossicle: 'OSS' };
function jumpRows(S) {
  const res = jumpResults(S), rows = [], order = ['pyxis', 'jellyfin', 'ossicle'];
  for (const src of order) {
    if (S.unreachable === src && S.jump) { rows.push({ l: `${LIB[src].name} is unreachable; no results from it`, c: C.plDim, tag: TAG[src], tagErr: true, group: true }); continue; }
    for (const r of res.filter(x => x.src === src)) rows.push({ l: r.text, r: r.len ? fmt(r.len) : r.kind === 'artist' ? 'artist' : 'live', c: C.plNormal, src, item: r, tag: TAG[src], video: r.kind === 'video' });
  }
  if (S.jump && !res.length) rows.push({ l: `Nothing found for "${S.jump}".`, c: C.plDim, group: true });
  if (!S.jump) rows.push({ l: 'Type to search Pyxis, Jellyfin and Ossicle.', c: C.plDim, group: true });
  const cursor = (Math.floor(now() * 2) % 2 === 0 || CLOCK.held !== null) ? '_' : ' ';
  const nsrc = new Set(res.map(r => r.src)).size;
  const head = { l: `Jump: ${S.jump || ''}${cursor}`, c: C.plCurrent, bg: '#10204a', r: !S.jump ? 'all sources' : res.length ? `${res.length} in ${nsrc} source${nsrc === 1 ? '' : 's'}` : '0 found', id: 'jumpline' };
  if (S.jumpSel >= rows.length || (rows[S.jumpSel] && rows[S.jumpSel].group)) S.jumpSel = Math.max(0, rows.findIndex(r => !r.group));
  const selRow = rows[S.jumpSel] || null; let hint = 'Enter: play   Ins: queue   Esc: close';
  if (selRow && selRow.src && selRow.src !== S.src) hint = `Enter: play from ${LIB[selRow.src].name}; ${srcName(S)} pauses`;
  if (selRow && selRow.item && selRow.item.kind === 'video') hint = 'Enter: open in mpv, outside Pyxamp';
  return { head, rows, sel: [S.jumpSel], foot: { l: hint, c: C.plDim, bg: '#0a0a14' }, scroll: true };
}
function infoRows(S) {
  const it = curItem(S); const kv = [];
  if (!it) return { head: { l: 'Item info', bg: C.plHead, c: C.plNormal, r: 'Esc: close' }, rows: [{ l: 'Nothing selected.', c: C.plDim }] };
  if (spoken(S)) {
    kv.push(['Show', it.show], [it.kind === 'book' ? 'Book' : 'Episode', it.t], ['Source', 'Ossicle (spoken word)'], ['Length', fmt(it.len)], ['Position', `${fmt(posNow(S, now()))} (synced)`], ['Speed', `${S.speed.toFixed(1)}x (synced)`], ['Skip', `back ${OSSICLE_INTERVALS.back} s, fwd ${OSSICLE_INTERVALS.fwd} s`], ['Played', it.played ? 'yes' : 'no'], ['Format', `Opus ${it.kbps || 64} kbps, ${it.ch === 1 ? 'mono' : 'stereo'}`]);
  } else {
    kv.push(['Title', it.t], ['Artist', it.a], ['Album', S.src === 'jellyfin' ? 'Spirit of Eden' : 'Dots and Loops'], ['Source', S.src === 'pyxis' ? `Pyxis (${S.role === 'console' ? 'console, ' + devName(S).toLowerCase() : 'host'})` : 'Jellyfin'], ['Length', fmt(it.len)],
      ['Format', it.lossless ? `FLAC ${it.bits} bit, ${it.khz} kHz, stereo` : `MP3 ${it.kbps || 320} kbps, ${it.khz || 44}.1 kHz, stereo`]);
    if (S.src === 'pyxis') kv.push(['Placement', 'Collection'], ['Hot', 'yes, since Oct 12'], ['Plays', '14']);
    else kv.push(['Plays', '3 (reported to Jellyfin)']);
  }
  return { head: { l: 'Item info', bg: C.plHead, c: C.plNormal, r: 'Esc: close' }, rows: kv.map(([k, v]) => ({ k, l: v, c: C.plNormal })), noSel: true };
}
function drawPL(ctx, S, t, oy, h) {
  chassis(ctx, 0, oy, W, h); frame(ctx, 0, oy, W, h, C.hi, C.edge);
  R(ctx, 1, oy + 1, W - 2, 2, '#22223a');
  titleBar(ctx, 1, oy + 3, W - 2, plTitle(S), true, ['album', 'shade', 'close'], false);
  if (S.albumMode) titleBtn(ctx, 'album', 244, oy + 6, true, true);
  hit('pl-album', 244, oy + 6, 9, 9, { tip: S.albumMode ? 'Album mode on: one row per album (G)' : 'Album mode: one row per album (G)' });
  hit('pl-close', 264, oy + 6, 9, 9, { tip: 'Fold the playlist (Alt+E)' }); hit('pl-shade', 254, oy + 6, 9, 9, { tip: 'Fold the playlist' });
  R(ctx, 1, oy + 17, W - 2, 3, '#22223a');
  const lx = 12, ly = oy + 20, lw = 243, lh = h - 58;
  // side borders with ridges
  for (const x of [3, 7]) { R(ctx, x, ly, 1, lh, C.hi); R(ctx, x + 1, ly, 1, lh, C.lo); }
  R(ctx, lx - 1, ly - 1, lw + 2, lh + 2, C.lo); R(ctx, lx, ly, lw, lh, C.plBg);
  // scrollbar groove
  R(ctx, 259, ly, 10, lh, C.lo); R(ctx, 260, ly, 8, lh, '#1a1a2e');
  const model = plRows(S); const rowsTop = ly + (model.head ? ROW : 0), rowsH = lh - (model.head ? ROW : 0) - (model.foot ? ROW : 0), vis = Math.ceil(rowsH / ROW), full = Math.floor(rowsH / ROW);
  const n = model.rows.length; const maxScroll = Math.max(0, n - full); if (S.scroll > maxScroll) S.scroll = maxScroll;
  const sc = model.scroll ? S.scroll : 0;
  ctx.save(); ctx.beginPath(); ctx.rect(lx, ly, lw, lh); ctx.clip();
  const drawRow = (r, y, sel, idx) => {
    if (r.bg || sel) R(ctx, lx, y, lw, ROW, sel ? C.plSel : r.bg);
    if (r.divTop) R(ctx, lx + 2, y, lw - 4, 1, C.plHead);
    let x = lx + 2;
    if (r.play) { map(ctx, ['#..', '##.', '###', '##.', '#..'], x, y + 4, { '#': r.c || C.plCurrent }); x += 6; }
    if (r.k) { sys(ctx, r.k, x, y, C.plDim); x = lx + 62; }
    if (r.tag) { R(ctx, x, y + 2, 17, 9, r.tagErr ? '#4a0e08' : '#0b3a0b'); text5(ctx, r.tag, x + 2, y + 4, r.tagErr ? C.plErr : C.plNormal); x += 21; }
    if (r.tick) { map(ctx, ['......#', '.....#.', '#...#..', '.#.#...', '..#....'], x, y + 4, { '#': C.plDim }); x += 9; }
    let chipW = 0; if (r.chips) chipW = r.chips.reduce((a, c) => a + sysW(c) + 9, 0);
    const rw = r.r ? sysW(r.r) : 0, max = lx + lw - 3 - x - (rw ? rw + 8 : 0) - (r.video ? sysW('mpv') + 9 : 0) - chipW;
    const lt = r.alt && sysW(r.l) > max ? r.alt : r.l;
    if (r.center) sys(ctx, r.l, lx + Math.floor((lw - sysW(r.l)) / 2), y, r.c || C.plNormal);
    else if (r.fitLeft && sysW(lt) > max) { let k = 0; while (k < lt.length && sysW(r.fitLeft + '...' + lt.slice(r.fitLeft.length + k)) > max) k++; sys(ctx, r.fitLeft + '...' + lt.slice(r.fitLeft.length + k), x, y, r.c || C.plNormal); }
    else sys(ctx, sysFit(lt, max), x, y, r.c || C.plNormal);
    let rx = lx + lw - 3;
    if (r.video) { const bw = sysW('mpv') + 4; rx -= bw; R(ctx, rx + 1, y + 1, bw - 1, 11, sel ? '#4a5ad8' : '#1f3a1f'); sys(ctx, 'mpv', rx + 3, y, sel ? '#ffffff' : C.plNormal); rx -= 5; }
    if (r.r) sys(ctx, r.r, rx - rw, y, r.rc || r.c || C.plNormal);
    if (r.chips) { let cx = lx + lw - 3 - chipW + 4; r.chips.forEach((c, ci) => { const cw = sysW(c) + 5, on = ci === r.pick; R(ctx, cx, y + 1, cw, 11, on ? C.plSel : '#000'); frame(ctx, cx, y + 1, cw, 11, on ? '#8090ff' : C.amberMid, on ? '#8090ff' : C.amberMid); sys(ctx, c, cx + 3, y, on ? '#ffffff' : C.amber); hit('chip', cx, y + 1, cw, 11, { pick: ci, tip: 'Enter keeps the picked position' }); cx += cw + 4; }); }
  };
  if (model.head) { drawRow(model.head, ly, false); hit(model.head.id || 'plhead', lx, ly, lw, ROW, { tip: model.head.id === 'crumb' ? 'Back (Backspace)' : null }); }
  ctx.save(); ctx.beginPath(); ctx.rect(lx, rowsTop, lw, rowsH); ctx.clip();
  for (let i = 0; i < vis && sc + i < n; i++) {
    const idx = sc + i, r = model.rows[idx], y = rowsTop + i * ROW;
    hit('row', lx, y, lw, Math.min(ROW, rowsTop + rowsH - y), { idx, tip: null });
    drawRow(r, y, !model.noSel && model.sel && model.sel.includes(idx) && !r.group && !r.chips, idx);
  }
  ctx.restore();
  if (model.foot) drawRow(model.foot, ly + lh - ROW, false);
  ctx.restore();
  // scrollbar thumb
  const tH = 18, track = lh - tH, ty = ly + (maxScroll ? Math.round(sc / maxScroll * track) : 0);
  R(ctx, 260, ty, 8, tH, C.bEdge); R(ctx, 261, ty + 1, 6, tH - 2, S.drag && S.drag.id === 'pl-scroll' ? C.bPress : C.bFace); R(ctx, 261, ty + 1, 6, 1, C.bHi); R(ctx, 261, ty + 1, 1, tH - 2, C.bHi); for (const gy of [6, 8, 10]) R(ctx, 263, ty + gy, 3, 1, C.bGlyph);
  hit('pl-scroll', 259, ly, 10, lh, { tip: n > full ? `${n.toLocaleString('en-US')} items` : null });
  // bottom bar
  const by = oy + h - 38;
  [['ADD', 14], ['REM', 43], ['SEL', 72], ['MISC', 101]].forEach(([s, x]) => { const id = 'pl-' + s.toLowerCase(); labelBtn(ctx, x, by + 8, 25, 18, s, { pressed: S.pressed === id || (S.menu && S.menu.kind === id) }); hit(id, x, by + 8, 25, 18, { tip: { ADD: 'Add to the queue', REM: 'Remove from the queue', SEL: 'Select', MISC: 'Item info, mark as played, sort' }[s] }); });
  lcdBox(ctx, 132, by + 5, 86, 9, false);
  const tot = S.queue.reduce((a, b) => a + b.len, 0), selLen = (S.sel || []).reduce((a, i) => a + (S.queue[i] ? S.queue[i].len : 0), 0);
  text5(ctx, `${fmt(selLen)}/${fmt(tot)}`.slice(0, 17), 133, by + 7, C.green);
  ['prev', 'play', 'pause', 'stop', 'next', 'eject'].forEach((k, i) => { const x = 133 + i * 8; map(ctx, MINI[k], x, by + 20, { '#': '#c8c8dc' }); hit(k, x - 1, by + 18, 8, 9, { tip: k }); });
  lcdBox(ctx, 186, by + 19, 32, 8, false);
  if (curItem(S) && S.play !== 'stop' && !S.conflict && !(S.fail && !S.fail.done)) { const s = fmt(posNow(S, t)); text5(ctx, s.padStart(6), 187, by + 20, C.green); }
  labelBtn(ctx, 222, by + 8, 30, 18, 'LIB', { led: S.plView === 'lib', pressed: S.pressed === 'pl-lib' }); hit('pl-lib', 222, by + 8, 30, 18, { tip: 'Queue or library (L)' });
  for (let i = 0; i < 4; i++) for (let j = 0; j <= i; j++) { P(ctx, 270 - j * 3 + i * 0, by + 34 - (i - j) * 3, C.hi); }
}

// ---------- surface ----------
function drawSurface(S, t) {
  const Lo = layout(S, t); const c = S._surf || (S._surf = document.createElement('canvas'));
  if (c.width !== W || c.height !== Math.max(1, Lo.H)) { c.width = W; c.height = Math.max(1, Lo.H); }
  const ctx = c.getContext('2d'); ctx.imageSmoothingEnabled = false; ctx.clearRect(0, 0, c.width, c.height);
  settle(S, t);
  HITS = [];
  drawMain(ctx, S, t, 0);
  if (Lo.eq.h > 0) { ctx.save(); ctx.beginPath(); ctx.rect(0, Lo.eq.y, W, Lo.eq.h); ctx.clip(); drawEQ(ctx, S, t, Lo.eq.y); ctx.restore(); if (Lo.eq.h < EQ_H) HITS = HITS.filter(h => !(h.y >= Lo.eq.y && h.y < Lo.eq.y + EQ_H) || h.y < Lo.eq.y); }
  if (Lo.pl.h > 0) {
    ctx.save(); ctx.beginPath(); ctx.rect(0, Lo.pl.y, W, Lo.pl.h); ctx.clip();
    drawPL(ctx, S, t, Lo.pl.y, plFullH(S)); ctx.restore();
    HITS = HITS.filter(h => h.y < Lo.pl.y + Lo.pl.h);
  }
  return { canvas: c, L: Lo };
}

// ---------- menus (Win9x style, drawn on the same grid; may sit outside the surface) ----------
function menuItems(S, kind) {
  const sw = spoken(S);
  if (kind === 'O') return [
    { l: 'Play on', sub: 'playon', dis: S.src !== 'pyxis', key: S.src !== 'pyxis' ? 'Pyxis only' : '' },
    { sep: 1 }, { l: 'Time elapsed', radio: S.timeMode === 'elapsed', act: 'time-e', key: 'Ctrl+T' }, { l: 'Time remaining', radio: S.timeMode === 'remaining', act: 'time-r' },
    { sep: 1 }, { l: 'Repeat', sub: 'repeat', dis: sw }, { l: 'Shuffle', check: S.shuffle && !sw, act: 'shuf', key: 'S', dis: sw },
    { sep: 1 }, { l: 'On top', radio: S.onTop, act: 'ontop', key: 'A' }, { l: 'On the desktop', radio: !S.onTop, act: 'ondesk' },
    { l: 'Double size', check: S.dbl, act: 'dbl', key: 'Ctrl+D' }, { l: 'Windowshade', check: S.shade, act: 'shade', key: 'Alt+W' },
    { sep: 1 }, { l: 'Equalizer', check: S.eqOpen, act: 'eqbtn', key: 'Alt+G' }, { l: 'Playlist', check: S.plOpen, act: 'plbtn', key: 'Alt+E' },
    { l: 'Library', act: 'lib', key: 'L' }, { l: 'Jump to item...', act: 'jump', key: 'J' },
    { sep: 1 }, { l: 'Hide Pyxamp', act: 'hide', key: 'Super+Z' }];
  if (kind === 'playon') return DEVICES.map(d => ({ l: d.name, key: d.kind, radio: S.device === d.id, act: 'dev:' + d.id }));
  if (kind === 'repeat') return [{ l: 'Off', radio: S.repeat === 'off', act: 'rep:off' }, { l: 'All', radio: S.repeat === 'all', act: 'rep:all' }, { l: 'One track', radio: S.repeat === 'one', act: 'rep:one' }];
  if (kind === 'V') return [{ l: 'Spectrum analyzer', radio: S.vis === 'spec', act: 'vis:spec' }, { l: 'Oscilloscope', radio: S.vis === 'osc', act: 'vis:osc' }, { l: 'Off', radio: S.vis === 'off', act: 'vis:off' }, { sep: 1 }, { l: 'Thick bands', radio: true }, { l: 'Thin bands', radio: false }, { l: 'Show peaks', check: true }, { sep: 1 }, { l: 'MilkDrop (later)', dis: true }];
  if (kind === 'presets') return [{ l: 'Flat', act: 'eqp:flat' }, { l: 'Voice', act: 'eqp:voice', key: 'auto: spoken word' }, { l: 'Small speakers', act: 'eqp:small' }, { l: 'Night', act: 'eqp:night' }, { l: 'Simon 1', radio: true, act: 'eqp:simon', key: 'auto: music' }, { sep: 1 }, { l: 'Save as new preset...', act: 'prompt:eq' }, { l: 'Delete preset', sub: 'eqdel' }];
  if (kind === 'eqdel') return [{ l: 'Simon 1', act: 'noop' }];
  if (kind === 'pl-add') return [{ l: 'From the library', act: 'lib', key: 'L' }, { l: 'With the jump box', act: 'jump', key: 'J' }, { l: 'Pyxis station', act: 'lib', dis: S.src !== 'pyxis' }];
  if (kind === 'pl-rem') return [{ l: 'Remove selected', act: 'noop', key: 'Del' }, { l: 'Crop selection', act: 'noop' }, { l: `Clear the ${srcName(S)} queue`, act: 'noop', key: 'Ctrl+N' }];
  if (kind === 'pl-sel') return [{ l: 'Select all', act: 'noop', key: 'Ctrl+A' }, { l: 'Select none', act: 'noop' }, { l: 'Invert selection', act: 'noop', key: 'Ctrl+I' }];
  if (kind === 'addpl') return PYXIS_PLAYLISTS.map(([n], i) => ({ l: n, act: 'addpl:' + i }));
  if (kind === 'librow') { const m = libRows(S), r = m.rows[S.menu.idx] || {}; const st = r.kind === 'album' ? { l: `Start station from ${r.artist}`, act: 'station:' + r.artist + ' (artist)' } : r.seed ? { l: 'Start station', act: 'station:' + r.seed + ' (song)' } : null;
    return [{ l: 'Play', act: 'noop', bold: true }, { l: 'Queue', act: 'noop', key: 'Ins' }].concat(r.kind === 'track' ? [{ sep: 1 }, { l: 'Add to playlist', sub: 'addpl' }] : []).concat(st ? [{ sep: 1 }, st] : []); }
  if (kind === 'jumprow') { const r = jumpRows(S).rows[S.menu.idx] || {}; const it = r.item || {}; const ok = r.src === 'pyxis' && (it.kind === 'track' || it.kind === 'artist');
    return [{ l: 'Play', act: 'jumpplay', bold: true }, { l: 'Queue', act: 'jumpqueue', key: 'Ins' }, { sep: 1 }, { l: 'Start station', act: 'station:' + (it.text || '').replace(/^.* - /, ''), dis: !ok }]; }
  if (kind === 'pl-misc') return [{ l: 'Item info', act: 'info', key: 'Alt+3' }, { l: 'Mark as played', act: 'played', dis: !sw }, { sep: 1 }, { l: 'Sort by title', act: 'noop', dis: sw }, { l: 'Sort by artist', act: 'noop', dis: sw }, { l: 'Reverse list', act: 'noop' }]
    .concat(S.src === 'pyxis' ? [{ sep: 1 }, { l: 'Album mode', check: S.albumMode, act: 'album', key: 'G' }, { l: 'Save queue as playlist...', act: 'prompt:savepl' }] : []);
  if (kind === 'row') { const r = S.queue[S.menu && S.menu.idx]; return sw ? [{ l: 'Play', act: 'playrow', bold: true }, { l: 'Mark as played', act: 'played', dis: r && r.played }, { l: 'Mark as unplayed', act: 'unplayed', dis: !(r && r.played) }, { sep: 1 }, { l: 'Item info', act: 'info', key: 'Alt+3' }, { l: 'Remove from up next', act: 'noop', key: 'Del' }] : [{ l: 'Play', act: 'playrow', bold: true }, { l: 'Play next', act: 'noop' }, { sep: 1 }].concat(S.src === 'pyxis' ? [{ l: 'Add to playlist', sub: 'addpl' }, { l: 'Start station', act: 'station:' + (r ? r.t : '') + ' (song)' }, { sep: 1 }] : []).concat([{ l: 'Item info', act: 'info', key: 'Alt+3' }, { l: 'Remove', act: 'noop', key: 'Del' }]); }
  return [];
}
function menuGeom(items) { let w = 0, kw = 0; for (const it of items) if (!it.sep) { w = Math.max(w, sysW(it.l)); if (it.key) kw = Math.max(kw, sysW(it.key)); } const W_ = 22 + w + (kw ? 18 + kw : 0) + 20, H_ = items.reduce((a, it) => a + (it.sep ? 9 : 17), 0) + 6; return { w: W_, h: H_, kx: 22 + w + 18 }; }
function drawMenu(ctx, items, x, y, hover, out) {
  const g = menuGeom(items); R(ctx, x, y, g.w, g.h, C.menu);
  R(ctx, x, y, g.w, 1, C.menu); R(ctx, x, y, 1, g.h, C.menu); R(ctx, x + 1, y + 1, g.w - 2, 1, C.menuHi); R(ctx, x + 1, y + 1, 1, g.h - 2, C.menuHi);
  R(ctx, x, y + g.h - 1, g.w, 1, C.menuDk); R(ctx, x + g.w - 1, y, 1, g.h, C.menuDk); R(ctx, x + 1, y + g.h - 2, g.w - 2, 1, C.menuLo); R(ctx, x + g.w - 2, y + 1, 1, g.h - 2, C.menuLo);
  let yy = y + 3;
  items.forEach((it, i) => {
    if (it.sep) { R(ctx, x + 3, yy + 3, g.w - 6, 1, C.menuLo); R(ctx, x + 3, yy + 4, g.w - 6, 1, C.menuHi); yy += 9; return; }
    const hv = hover === i && !it.dis; if (hv) R(ctx, x + 3, yy, g.w - 6, 17, C.menuSel);
    const col = it.dis ? C.menuDis : hv ? '#ffffff' : C.menuTxt;
    const txt = (s, tx) => { if (it.dis) sys(ctx, s, tx + 1, yy + 3, C.menuDisSh); sys(ctx, s, tx, yy + 2, col); if (it.bold) sys(ctx, s, tx + 1, yy + 2, col); };
    txt(it.l, x + 22); if (it.key) txt(it.key, x + g.kx);
    if (it.check) map(ctx, ['......#', '.....##', '#...###', '##.###.', '#####..', '.###...', '..#....'], x + 8, yy + 5, { '#': col });
    if (it.radio) map(ctx, ['.##.', '####', '####', '.##.'], x + 9, yy + 7, { '#': col });
    if (it.sub) map(ctx, ['#...', '##..', '###.', '####', '###.', '##..', '#...'], x + g.w - 12, yy + 5, { '#': col });
    if (out) out.push({ i, x: x + 3, y: yy, w: g.w - 6, h: 17, it });
    yy += 17;
  });
  return g;
}

// ---------- scene: desktop, props, surface, menus, tooltip ----------
function drawProp(ctx, p) {
  const { x, y, w, h } = p; R(ctx, x - 1, y - 1, w + 2, h + 2, '#101018'); R(ctx, x, y, w, h, p.bg || '#1e1e24'); R(ctx, x, y, w, 15, p.bar || '#3a3a48');
  sys(ctx, sysFit(p.title, w - 10), x + 5, y + 1, '#e8e8f0');
  if (p.video) { R(ctx, x + 4, y + 19, w - 8, h - 23, '#000'); for (let j = 0; j < h - 23; j += 1) R(ctx, x + 4, y + 19 + j, w - 8, 1, mix(p.video[0], p.video[1], j / (h - 23))); if (p.caption) sys(ctx, p.caption, x + 10, y + h - 18, '#ffffff'); }
  (p.lines || []).forEach((l, i) => { if (y + 20 + i * 13 < y + h - 12) sys(ctx, sysFit(l, w - 14), x + 7, y + 20 + i * 13, p.fg || '#c8d0c8'); });
}
function drawScene(ctx, S, t, o = {}) {
  const sw = ctx.canvas.width, sh = ctx.canvas.height;
  if (!o.noBg) R(ctx, 0, 0, sw, sh, o.bg || C.desk);
  const below = S.props.filter(p => !(p.above || (p.overlap && !S.onTop))), above = S.props.filter(p => p.above || (p.overlap && !S.onTop));
  below.forEach(p => drawProp(ctx, p));
  const surf = drawSurface(S, t), a = surfaceAlpha(S, t), ox = S.at.x, oy = S.at.y;
  if (a > 0) { ctx.save(); ctx.globalAlpha = a; ctx.drawImage(surf.canvas, ox, oy); ctx.restore(); }
  HITS.forEach(h => { h.x += ox; h.y += oy; });
  if (a < 1) HITS = [];
  above.forEach(p => drawProp(ctx, p));
  // menus
  S._menuHits = [];
  let bbox = { x0: ox, y0: oy, x1: ox + W, y1: oy + surf.L.H };
  const grow = (x, y, w, h) => { bbox.x0 = Math.min(bbox.x0, x); bbox.y0 = Math.min(bbox.y0, y); bbox.x1 = Math.max(bbox.x1, x + w); bbox.y1 = Math.max(bbox.y1, y + h); };
  S.props.forEach(p => { if (!p.noBox) grow(p.x - 1, p.y - 1, p.w + 2, p.h + 2); });
  if (S.menu && a > 0) {
    const m = S.menu, items = menuItems(S, m.kind), out = [];
    const mx = ox + m.x, my = m.up ? oy + surf.L.pl.y + plFullH(S) - 30 - menuGeom(items).h : oy + m.y;
    const g = drawMenu(ctx, items, mx, my, m.hover, out); grow(mx, my, g.w, g.h); out.forEach(r => S._menuHits.push({ ...r, level: 0 }));
    if (m.sub != null) { const it = items[m.sub]; if (it && it.sub) { const r = out.find(q => q.i === m.sub); const si = menuItems(S, it.sub), sx = mx + g.w - 3, sy = r.y - 3, o2 = []; const g2 = drawMenu(ctx, si, sx, sy, m.subHover, o2); grow(sx, sy, g2.w, g2.h); o2.forEach(r2 => S._menuHits.push({ ...r2, level: 1 })); } }
  }
  if (S.tip && a > 0) { const tw = sysW(S.tip.text) + 8, tx = ox + S.tip.x, ty = oy + S.tip.y; R(ctx, tx, ty, tw, 16, '#000'); R(ctx, tx + 1, ty + 1, tw - 2, 14, C.tip); sys(ctx, S.tip.text, tx + 4, ty + 1, '#000'); grow(tx, ty, tw, 16); }
  return { bbox, L: surf.L };
}

// ---------- actions ----------
function act(S, a, t = now()) {
  const sw = spoken(S), it = curItem(S);
  const toQueueStart = () => { setPos(S, sw && it && it.pos ? it.pos : 0, t); S.marqueeT0 = t; };
  switch (a) {
    case 'prev': if (S.follow) { flash(S, `${S.follow.app}: PREVIOUS`, t); break; } if (sw) { setPos(S, posNow(S, t) - OSSICLE_INTERVALS.back, t); flash(S, `-${OSSICLE_INTERVALS.back} S`, t, 1.2); } else { S.cur = Math.max(0, S.cur - 1); setPos(S, 0, t); S.marqueeT0 = t; S.sel = [S.cur]; } break;
    case 'next': if (S.follow) { flash(S, `${S.follow.app}: NEXT`, t); break; } if (sw) { setPos(S, posNow(S, t) + OSSICLE_INTERVALS.fwd, t); flash(S, `+${OSSICLE_INTERVALS.fwd} S`, t, 1.2); } else { S.cur = Math.min(S.queue.length - 1, S.cur + 1); setPos(S, 0, t); S.marqueeT0 = t; S.sel = [S.cur]; } break;
    case 'play': if (S.follow) { S.follow.playing = true; break; } if (!it) { flash(S, 'NOTHING QUEUED. PRESS J', t); break; } if (S.play === 'stop') toQueueStart(); else if (S.play === 'play' && !sw) setPos(S, 0, t); else setPos(S, posNow(S, t), t); S.play = 'play'; break;
    case 'pause': if (S.follow) { S.follow.playing = !(S.follow.playing !== false); break; } if (S.play === 'play') { setPos(S, posNow(S, t), t); S.play = 'pause'; } else if (S.play === 'pause') { S.posT = t; S.play = 'play'; } break;
    case 'stop': if (S.follow) { flash(S, `${S.follow.app}: STOP`, t); break; } setPos(S, 0, t); S.play = 'stop'; break;
    case 'eject': case 'lib': S.plView = S.plView === 'lib' ? 'queue' : 'lib'; S.info = false; S.scroll = 0; if (!S.plOpen) act(S, 'plbtn', t); break;
    case 'shuf': if (sw) break; S.shuffle = !S.shuffle; flash(S, `SHUFFLE: ${S.shuffle ? 'ON' : 'OFF'}`, t, 1); break;
    case 'rep': if (sw) break; S.repeat = { off: 'all', all: 'one', one: 'off' }[S.repeat]; flash(S, `REPEAT: ${{ off: 'OFF', all: 'ALL', one: 'ONE TRACK' }[S.repeat]}`, t, 1); break;
    case 'eqbtn': S.eqOpen = !S.eqOpen; break;
    case 'plbtn': case 'pl-close': case 'pl-shade': S.plOpen = !S.plOpen; break;
    case 'eq-close': case 'eq-shade': S.eqOpen = false; break;
    case 'shade': S.shade = !S.shade; break;
    case 'dbl': case 'cl-D': S.dbl = !S.dbl; break;
    case 'cl-A': case 'ontop-toggle': S.onTop = !S.onTop; flash(S, S.onTop ? 'ON TOP' : 'ON THE DESKTOP', t, 1.2); break;
    case 'ontop': S.onTop = true; break; case 'ondesk': S.onTop = false; break;
    case 'cl-I': case 'info': S.info = !S.info; if (S.plView === 'jump') S.plView = 'queue'; if (!S.plOpen) act(S, 'plbtn', t); break;
    case 'time': case 'time-toggle': S.timeMode = S.timeMode === 'elapsed' ? 'remaining' : 'elapsed'; break;
    case 'time-e': S.timeMode = 'elapsed'; break; case 'time-r': S.timeMode = 'remaining'; break;
    case 'vis': S.vis = { spec: 'osc', osc: 'off', off: 'spec' }[S.vis]; S.visForced = true; break;
    case 'jump': S.plView = 'jump'; S.info = false; S.jump = S.jump || ''; S.jumpSel = 0; S.scroll = 0; if (!S.plOpen) act(S, 'plbtn', t); break;
    case 'hide': case 'min': case 'close': case 'superz': S.shown = !S.shown; S.menu = null; S.tip = null; break;
    case 'logo': flash(S, 'PYXAMP: PYXIS, JELLYFIN, OSSICLE', t, 2); break;
    case 'eqon': S.eq.on = !S.eq.on; flash(S, `EQ: ${S.eq.on ? 'ON' : 'OFF'}`, t, 1); break;
    case 'eqauto': S.eq.auto = !S.eq.auto; break;
    case 'played': { const i = S.menu && S.menu.idx != null ? S.menu.idx : S.cur; if (S.queue[i]) S.queue[i].played = true; flash(S, 'MARKED AS PLAYED (SYNCED)', t, 1.5); break; }
    case 'unplayed': { const i = S.menu && S.menu.idx; if (S.queue[i]) S.queue[i].played = false; break; }
    case 'playrow': { const i = S.menu && S.menu.idx; if (i != null) { S.cur = i; toQueueStart(); S.play = 'play'; } break; }
    case 'album': S.albumMode = !S.albumMode; S.albumOpen = null; S.albumSel = Math.max(0, queueGroups(S).findIndex(g => g.items.includes(S.cur))); S.scroll = 0; if (S.plView !== 'queue') { S.plView = 'queue'; S.info = false; } if (!S.plOpen) S.plOpen = true; break;
    case 'prompt:savepl': openPrompt(S, { kind: 'savepl', title: 'SAVE AS PLAYLIST', label: 'Name', right: `${S.queue.length} tracks`, hint: 'Enter: save in Pyxis   Esc: cancel' }); break;
    case 'prompt:eq': openPrompt(S, { kind: 'eq', title: 'SAVE EQ PRESET', label: 'Name', right: '10 bands + preamp', hint: 'Enter: save   Esc: cancel' }); break;
    case 'jumpplay': jumpEnter(S, t, false); break; case 'jumpqueue': jumpEnter(S, t, true); break;
    default:
      if (a.startsWith('addpl:')) { const pl = PYXIS_PLAYLISTS[+a.slice(6)]; flash(S, `ADDED TO PLAYLIST: ${pl[0].toUpperCase()}`, t, 1.8); }
      else if (a.startsWith('station:')) startStation(S, a.slice(8), t);
      else if (a.startsWith('keep:')) keepPosition(S, +a.slice(5), t);
      else if (a.startsWith('dev:')) { S.device = a.slice(4); S.role = S.device === 'here' ? 'host' : 'console'; flash(S, S.role === 'console' ? `PLAYS ON ${devName(S)}` : 'PLAYS HERE', t, 1.5); }
      else if (a.startsWith('rep:')) S.repeat = a.slice(4);
      else if (a.startsWith('vis:')) { S.vis = a.slice(4); S.visForced = true; }
      else if (a.startsWith('eqp:')) { const P_ = { flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], voice: [-6, -4, -1, 2, 4, 4, 3, 1, -2, -4], small: [6, 5, 3, 1, 0, 0, 1, 2, 3, 3], night: [-3, -2, 0, 1, 1, 0, -1, -3, -5, -6], simon: [3, 4, 2, 0, -1, -2, 0, 2, 4, 5] }; S.eq.bands = P_[a.slice(4)].slice(); flash(S, `EQ PRESET: ${a.slice(4).toUpperCase()}`, t, 1.2); }
  }
}

// ---------- pass 2 helpers ----------
function openPrompt(S, P) { S.prompt = { ...P, text: P.text || '' }; S.menu = null; if (!S.plOpen) { S.plOpen = true; S.prompt.refold = true; } }
function closePrompt(S, ok, t) {
  const P = S.prompt; if (!P) return; S.prompt = null; if (P.refold) S.plOpen = false;
  if (!ok) return;
  const name = P.text.trim(); if (!name) { flash(S, 'NO NAME: NOTHING SAVED', t, 1.5); return; }
  if (P.kind === 'savepl') flash(S, `SAVED AS PLAYLIST: ${name.toUpperCase()}`, t, 3.5);
  if (P.kind === 'eq') flash(S, `EQ PRESET SAVED: ${name.toUpperCase()}`, t, 2);
}
function startStation(S, seed, t) {
  S.src = 'pyxis'; S.queue = STATION_QUEUE.map(x => ({ ...x })); S.cur = 0; S.sel = [0]; setPos(S, 0, t); S.play = 'play'; S.marqueeT0 = t + 2.2;
  S.plView = 'queue'; S.jump = null; S.albumMode = false; S.albumOpen = null;
  flash(S, `STATION FROM ${seed.toUpperCase()}`, t, 2.2);
}
function keepPosition(S, pick, t) {
  const c = S.conflict; if (!c) return; const o = c.opts[pick]; S.conflict = null; S.cur = c.idx; S.sel = [c.idx]; setPos(S, o.pos, t); S.play = 'play';
  flash(S, `KEPT ${o.dev.toUpperCase()} ${fmt(o.pos)} (SYNCED)`, t, 2);
}
// State that moves on with the clock: a failed track is retried after 2 s and 10 s, then skipped; sponsor segments are skipped.
function settle(S, t) {
  const F = S.fail;
  if (F && !F.done && t - F.t0 >= 12) {
    F.done = true; S.queue[F.idx].failed = true; S.cur = Math.min(S.queue.length - 1, F.idx + 1); S.sel = [S.cur]; setPos(S, 0, F.t0 + 12); S.play = 'play'; S.marqueeT0 = F.t0 + 14;
    S.flash = { text: 'SKIPPED: TRACK CANNOT PLAY', t0: F.t0 + 12, until: F.t0 + 14 };
  }
  if (S.sponsor && spoken(S) && S.play === 'play' && !S.follow) {
    const p = posNow(S, t); for (const [a, b] of S.sponsor) if (p >= a && p < b) { setPos(S, b, t); S.flash = { text: 'SKIPPED SPONSOR', t0: t, until: t + 2 }; }
  }
}
