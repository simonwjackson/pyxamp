// Pyxamp skin: every sprite is drawn here, from pixel maps and primitives, at 1 skin pixel (sp).
// Nothing is loaded from any Winamp bitmap. Colours are my own values in Winamp's colour roles.
'use strict';

const C = {
  desk: '#3b6896',
  edge: '#0b0b14', hi: '#66668a', lo: '#10101c', mid: '#46466a',
  lcd: '#000000', dot: '#191926', lcdHi: '#5c5c7c', lcdLo: '#08080e',
  green: '#22e422', greenDim: '#0d5c0d', greenMid: '#16a016',
  amber: '#ffb020', amberDim: '#5e3f06', amberMid: '#b07812',
  red: '#ff3a24', redDim: '#6a1408',
  white: '#f2f2f8',
  bFace: '#bcc5d6', bHi: '#eef2f9', bLo: '#7a8299', bEdge: '#262a3a', bGlyph: '#2e3654', bPress: '#9aa3b8', bDim: '#8d95aa',
  ledOn: '#30f030', ledOff: '#1e3a1e', ledEdge: '#2e3654',
  stA: '#efe2b4', stB: '#a88c4a', stD: '#3a301a', stIA: '#8a8aa0', stIB: '#4e4e64',
  ttl: '#e4e4f0', ttlSh: '#14141e', ttlI: '#9a9ab0',
  label: '#d4d4e4', labelDim: '#5a5a74',
  gold: '#e8b830', goldDim: '#7a6420',
  plNormal: '#22e422', plCurrent: '#ffffff', plBg: '#000000', plSel: '#1c2fbf', plDim: '#0f7a0f', plHead: '#0b3a0b', plErr: '#ff5a40',
  menu: '#c0c0c0', menuHi: '#ffffff', menuLo: '#808080', menuDk: '#000000', menuSel: '#000080', menuTxt: '#000000', menuDis: '#808080', menuDisSh: '#ffffff',
  tip: '#ffffe1',
};
// Rows of the chassis gradient (one colour per row, so the art stays flat pixels).
function chassisRow(y, h) { const t = Math.min(1, y / Math.max(1, h - 1)); return mix('#34344e', '#22223a', t); }
// My own viscolor.txt: 0 bg, 1 dots, 2..17 spectrum top..bottom, 18..22 scope, 23 peaks.
const VIS = ['#000000', '#191926',
  '#ff2a1a', '#ff4c14', '#ff6e10', '#ff900c', '#ffb008', '#f0c808', '#d8dc10', '#b8e418',
  '#98e820', '#78e828', '#58e030', '#40d834', '#30cc34', '#28bc30', '#20aa2c', '#189828',
  '#ffffff', '#d8e0d8', '#b0c8b0', '#88b088', '#609860', '#e8e8f4'];

function hex(c) { const n = parseInt(c.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; }
function mix(a, b, t) { const A = hex(a), B = hex(b); return '#' + A.map((v, i) => Math.round(v + (B[i] - v) * t).toString(16).padStart(2, '0')).join(''); }

// ---------- primitives (integer sp only) ----------
function R(ctx, x, y, w, h, c) { if (w <= 0 || h <= 0) return; ctx.fillStyle = c; ctx.fillRect(x, y, w, h); }
function P(ctx, x, y, c) { ctx.fillStyle = c; ctx.fillRect(x, y, 1, 1); }
function frame(ctx, x, y, w, h, tl, br) { R(ctx, x, y, w, 1, tl); R(ctx, x, y, 1, h, tl); R(ctx, x, y + h - 1, w, 1, br); R(ctx, x + w - 1, y, 1, h, br); }
function map(ctx, rows, x, y, pal) { // rows: strings; pal: {char: colour}; '.' and ' ' are clear
  for (let j = 0; j < rows.length; j++) for (let i = 0; i < rows[j].length; i++) { const c = pal[rows[j][i]]; if (c) P(ctx, x + i, y + j, c); }
}

// ---------- Pyx 5x6 font (my own). Cell 5x6; most glyphs 4x5, some 3 or 5 wide. ----------
const F5 = {
  A: ['.##.', '#..#', '####', '#..#', '#..#'], B: ['###.', '#..#', '###.', '#..#', '###.'], C: ['.###', '#...', '#...', '#...', '.###'],
  D: ['###.', '#..#', '#..#', '#..#', '###.'], E: ['####', '#...', '###.', '#...', '####'], F: ['####', '#...', '###.', '#...', '#...'],
  G: ['.###', '#...', '#.##', '#..#', '.###'], H: ['#..#', '#..#', '####', '#..#', '#..#'], I: ['###', '.#.', '.#.', '.#.', '###'],
  J: ['..##', '...#', '...#', '#..#', '.##.'], K: ['#..#', '#.#.', '##..', '#.#.', '#..#'], L: ['#...', '#...', '#...', '#...', '####'],
  M: ['#...#', '##.##', '#.#.#', '#...#', '#...#'], N: ['#..#', '##.#', '#.##', '#..#', '#..#'], O: ['.##.', '#..#', '#..#', '#..#', '.##.'],
  P: ['###.', '#..#', '###.', '#...', '#...'], Q: ['.##.', '#..#', '#..#', '#.#.', '.#.#'], R: ['###.', '#..#', '###.', '#.#.', '#..#'],
  S: ['.###', '#...', '.##.', '...#', '###.'], T: ['###', '.#.', '.#.', '.#.', '.#.'], U: ['#..#', '#..#', '#..#', '#..#', '.##.'],
  V: ['#.#', '#.#', '#.#', '#.#', '.#.'], W: ['#...#', '#...#', '#.#.#', '##.##', '#...#'], X: ['#.#', '#.#', '.#.', '#.#', '#.#'],
  Y: ['#.#', '#.#', '.#.', '.#.', '.#.'], Z: ['####', '..#.', '.#..', '#...', '####'],
  0: ['###', '#.#', '#.#', '#.#', '###'], 1: ['.#.', '##.', '.#.', '.#.', '###'], 2: ['###', '..#', '###', '#..', '###'],
  3: ['###', '..#', '.##', '..#', '###'], 4: ['#.#', '#.#', '###', '..#', '..#'], 5: ['###', '#..', '###', '..#', '###'],
  6: ['###', '#..', '###', '#.#', '###'], 7: ['###', '..#', '..#', '.#.', '.#.'], 8: ['###', '#.#', '###', '#.#', '###'], 9: ['###', '#.#', '###', '..#', '###'],
  ' ': [''], '.': ['', '', '', '', '#'], ',': ['', '', '', '', '.#', '#.'], ':': ['', '#', '', '#', ''], ';': ['', '.#', '', '.#', '#.'],
  '-': ['', '', '###', '', ''], '+': ['', '.#.', '###', '.#.', ''], '=': ['', '###', '', '###', ''], '_': ['', '', '', '', '####'],
  "'": ['#', '#', '', '', ''], '"': ['#.#', '#.#', '', '', ''], '!': ['#', '#', '#', '', '#'], '?': ['###', '..#', '.##', '', '.#.'],
  '(': ['.#', '#.', '#.', '#.', '.#'], ')': ['#.', '.#', '.#', '.#', '#.'], '[': ['##', '#.', '#.', '#.', '##'], ']': ['##', '.#', '.#', '.#', '##'],
  '/': ['..#', '..#', '.#.', '#..', '#..'], '\\': ['#..', '#..', '.#.', '..#', '..#'], '*': ['', '#.#', '.#.', '#.#', ''],
  '&': ['.#..', '#.#.', '.#.#', '#.#.', '.#.#'], '%': ['#..#', '..#.', '.#..', '#...', '#..#'], '#': ['.#.#', '####', '.#.#', '####', '.#.#'],
  '$': ['.###', '##..', '.##.', '..##', '###.'], '@': ['.##.', '#.##', '#.##', '#...', '.###'], '<': ['..#', '.#.', '#..', '.#.', '..#'],
  '>': ['#..', '.#.', '..#', '.#.', '#..'], '^': ['.#.', '#.#', '', '', ''], '|': ['#', '#', '#', '#', '#'], '~': ['', '.#.#', '#.#.', '', ''],
  '\u25b6': ['#..', '##.', '###', '##.', '#..'], // ▶ for "PLAY ON ▶ device" style hints
};
function f5glyph(ch) {
  let c = ch.toUpperCase();
  if (!F5[c]) c = c.normalize('NFD').replace(/[\u0300-\u036f]/g, '');
  if (c === '\u2019' || c === '\u2018') c = "'"; if (c === '\u201c' || c === '\u201d') c = '"'; if (c === '\u2013' || c === '\u2014') c = '-'; if (c === '\u00d7') c = 'X';
  return F5[c] || F5['?'];
}
// Draw a string in the 5x6 font. Fixed 5 sp advance (Winamp's text.bmp habit).
function text5(ctx, s, x, y, col, o = {}) {
  const adv = o.adv || 5;
  for (let i = 0; i < s.length; i++) {
    const g = f5glyph(s[i]), gx = x + i * adv;
    if (o.clip && (gx + 5 <= o.clip[0] || gx >= o.clip[1])) continue;
    for (let j = 0; j < g.length; j++) for (let k = 0; k < g[j].length; k++) if (g[j][k] === '#') {
      const px = gx + k; if (o.clip && (px < o.clip[0] || px >= o.clip[1])) continue;
      P(ctx, px, y + j, col); if (o.bold) P(ctx, px + 1, y + j, col);
    }
  }
}
const w5 = (s, adv = 5) => s.length * adv;

// 3x5 digits for small places (mini time, skip amounts) use the same F5 digits.

// ---------- label set (7 rows, proportional) for kbps / kHz / mono / stereo / bit / db ----------
const FL = {
  k: ['#...', '#...', '#..#', '#.#.', '###.', '#..#', '....'], b: ['#...', '#...', '###.', '#..#', '#..#', '###.', '....'],
  p: ['....', '....', '###.', '#..#', '#..#', '###.', '#...'], s: ['...', '...', '.##', '#..', '.##', '##.', '...'],
  H: ['#..#', '#..#', '#..#', '####', '#..#', '#..#', '....'], z: ['...', '...', '###', '..#', '.#.', '###', '...'],
  m: ['.....', '.....', '####.', '#.#.#', '#.#.#', '#.#.#', '.....'], o: ['....', '....', '.##.', '#..#', '#..#', '.##.', '....'],
  n: ['...', '...', '##.', '#.#', '#.#', '#.#', '...'], t: ['.#.', '.#.', '###', '.#.', '.#.', '..#', '...'],
  e: ['....', '....', '.##.', '####', '#...', '.###', '....'], r: ['...', '...', '#.#', '##.', '#..', '#..', '...'],
  i: ['#', '.', '#', '#', '#', '#', '.'], d: ['...#', '...#', '.###', '#..#', '#..#', '.###', '....'], ' ': ['..', '..', '..', '..', '..', '..', '..'],
};
function label(ctx, s, x, y, col) { let cx = x; for (const ch of s) { const g = FL[ch] || FL[' ']; map(ctx, g, cx, y, { '#': col }); cx += g[0].length + 1; } return cx - x - 1; }
function labelW(s) { let w = 0; for (const ch of s) w += (FL[ch] || FL[' '])[0].length + 1; return w - 1; }

// ---------- Pyx 9x13 clock digits (my own seven-segment cut with bevelled ends) ----------
const SEG = {
  a: [[1, 0, 6, 1], [2, 1, 4, 1]], d: [[2, 11, 4, 1], [1, 12, 6, 1]], g: [[2, 6, 4, 1], [1, 6, 1, 1], [6, 6, 1, 1]],
  f: [[0, 1, 1, 5], [1, 2, 1, 3]], b: [[7, 1, 1, 5], [6, 2, 1, 3]], e: [[0, 7, 1, 5], [1, 8, 1, 3]], c: [[7, 7, 1, 5], [6, 8, 1, 3]],
};
const DIGSEG = { 0: 'abcdef', 1: 'bc', 2: 'abged', 3: 'abgcd', 4: 'fgbc', 5: 'afgcd', 6: 'afgedc', 7: 'abc', 8: 'abcdefg', 9: 'abcdfg', '-': 'g' };
function digit9(ctx, d, x, y, col, ghost) {
  if (ghost) for (const s of 'abcdefg') for (const r of SEG[s]) R(ctx, x + r[0], y + r[1], r[2], r[3], ghost);
  const segs = DIGSEG[d]; if (!segs) return;
  for (const s of segs) for (const r of SEG[s]) R(ctx, x + r[0], y + r[1], r[2], r[3], col);
}

// ---------- W95FA Grid, thresholded to one colour (playlist, menus, tooltips) ----------
const SYS = { font: '12.5px "W95FA Grid"', asc: 10, line: 13, cache: new Map(), mctx: null };
function sysCtx() { if (!SYS.mctx) { const c = document.createElement('canvas'); c.width = 4; c.height = 4; SYS.mctx = c.getContext('2d'); SYS.mctx.font = SYS.font; } return SYS.mctx; }
function sysW(s) { return Math.round(sysCtx().measureText(s).width); }
function sysBitmap(s, col) {
  const key = s + '\u0000' + col; let c = SYS.cache.get(key); if (c) return c;
  const w = Math.max(1, sysW(s) + 2), h = SYS.line;
  c = document.createElement('canvas'); c.width = w; c.height = h;
  const x = c.getContext('2d'); x.font = SYS.font; x.fillStyle = '#000'; x.textBaseline = 'alphabetic'; x.fillText(s, 0, SYS.asc);
  const id = x.getImageData(0, 0, w, h), [r, g, b] = hex(col);
  for (let i = 0; i < id.data.length; i += 4) { const on = id.data[i + 3] >= 110; id.data[i] = r; id.data[i + 1] = g; id.data[i + 2] = b; id.data[i + 3] = on ? 255 : 0; }
  x.putImageData(id, 0, 0); SYS.cache.set(key, c); if (SYS.cache.size > 4000) SYS.cache.clear(); return c;
}
function sys(ctx, s, x, y, col) { if (!s) return 0; ctx.drawImage(sysBitmap(s, col), x, y); return sysW(s); }
function sysFit(s, max) { if (sysW(s) <= max) return s; let lo = 0, hi = s.length; while (lo < hi) { const m = (lo + hi + 1) >> 1; if (sysW(s.slice(0, m) + '...') <= max) lo = m; else hi = m - 1; } return s.slice(0, lo).trimEnd() + '...'; }

// ---------- sprites ----------
function bevelBox(ctx, x, y, w, h, pressed, face) { // Winamp-style light button
  R(ctx, x, y, w, h, C.bEdge);
  R(ctx, x + 1, y + 1, w - 2, h - 2, pressed ? C.bPress : (face || C.bFace));
  if (!pressed) { R(ctx, x + 1, y + 1, w - 2, 1, C.bHi); R(ctx, x + 1, y + 1, 1, h - 2, C.bHi); R(ctx, x + 2, y + h - 2, w - 3, 1, C.bLo); R(ctx, x + w - 2, y + 2, 1, h - 3, C.bLo); }
  else { R(ctx, x + 1, y + 1, w - 2, 1, C.bLo); R(ctx, x + 1, y + 1, 1, h - 2, C.bLo); }
}
function led(ctx, x, y, on, dim) { R(ctx, x, y, 5, 4, C.ledEdge); R(ctx, x + 1, y + 1, 3, 2, on ? C.ledOn : C.ledOff); if (on) P(ctx, x + 1, y + 1, '#c8ffc8'); if (dim) R(ctx, x + 1, y + 1, 3, 2, '#3c4258'); }

// Transport glyphs, solid with a 1 sp light emboss below-right. o = offset when pressed.
const GL = {
  prev: ['#.....#', '#....##', '#...###', '#..####', '#...###', '#....##', '#.....#'],
  play: ['#....', '##...', '###..', '####.', '#####', '####.', '###..', '##...', '#....'],
  pause: ['##.##', '##.##', '##.##', '##.##', '##.##', '##.##', '##.##'],
  stop: ['#######', '#######', '#######', '#######', '#######', '#######', '#######'].map(r => r.slice(0, 7)),
  next: ['#.....#', '##....#', '###...#', '####..#', '###...#', '##....#', '#.....#'],
  eject: ['...#...', '..###..', '.#####.', '#######', '.......', '#######'],
  back: ['..#..#', '.##.##', '######', '.##.##', '..#..#'],
  fwd: ['#..#..', '##.##.', '######', '##.##.', '#..#..'],
};
function glyph(ctx, g, x, y, col, emb) { if (emb) map(ctx, g, x + 1, y + 1, { '#': emb }); map(ctx, g, x, y, { '#': col }); }
function transportBtn(ctx, kind, x, y, w, h, pressed, opt = {}) {
  bevelBox(ctx, x, y, w, h, pressed); const o = pressed ? 1 : 0; const col = opt.dim ? C.bDim : C.bGlyph, emb = opt.dim ? null : C.bHi;
  if (kind === 'skipback' || kind === 'skipfwd') { // spoken word: arrows plus the interval in the 5x6 digits
    const g = kind === 'skipback' ? GL.back : GL.fwd; const gx = x + Math.floor((w - 6) / 2) + o; glyph(ctx, g, gx, y + 3 + o, col, emb);
    const s = String(opt.secs || 15); const tw = s.length * 4 - 1; const sx = x + Math.floor((w - tw) / 2) + o;
    if (emb) text5(ctx, s, sx + 1, y + 11 + o, emb, { adv: 4 }); text5(ctx, s, sx, y + 10 + o, col, { adv: 4 }); return;
  }
  const g = GL[kind]; const gw = g[0].length, gh = g.length; glyph(ctx, g, x + Math.floor((w - gw) / 2) + o, y + Math.floor((h - gh) / 2) + o, col, emb);
}

// Title lettering: my own bold capitals, 2 sp stems, 5 rows. Proportional (advance = width + 1).
const TF = {
  A: ['.###.', '##.##', '#####', '##.##', '##.##'], B: ['####.', '##.##', '####.', '##.##', '####.'], C: ['.####', '##...', '##...', '##...', '.####'],
  D: ['####.', '##.##', '##.##', '##.##', '####.'], E: ['#####', '##...', '####.', '##...', '#####'], F: ['#####', '##...', '####.', '##...', '##...'],
  G: ['.####', '##...', '##.##', '##.##', '.####'], H: ['##.##', '##.##', '#####', '##.##', '##.##'], I: ['##', '##', '##', '##', '##'],
  J: ['...##', '...##', '...##', '##.##', '.###.'], K: ['##.##', '####.', '###..', '####.', '##.##'], L: ['##...', '##...', '##...', '##...', '#####'],
  M: ['##...##', '###.###', '##.#.##', '##...##', '##...##'], N: ['##..##', '###.##', '######', '##.###', '##..##'], O: ['.###.', '##.##', '##.##', '##.##', '.###.'],
  P: ['####.', '##.##', '####.', '##...', '##...'], Q: ['.###.', '##.##', '##.##', '##.#.', '.##.#'], R: ['####.', '##.##', '####.', '##.##', '##.##'],
  S: ['.####', '##...', '.###.', '...##', '####.'], T: ['######', '..##..', '..##..', '..##..', '..##..'], U: ['##.##', '##.##', '##.##', '##.##', '.###.'],
  V: ['##..##', '##..##', '##..##', '.####.', '..##..'], W: ['##...##', '##...##', '##.#.##', '###.###', '##...##'], X: ['##..##', '.####.', '..##..', '.####.', '##..##'],
  Y: ['##..##', '##..##', '.####.', '..##..', '..##..'], Z: ['#####', '...##', '.###.', '##...', '#####'], ' ': ['..', '..', '..', '..', '..'],
};
const tfW = (s) => { let w = 0; for (const ch of s) w += ((TF[ch] || TF[' '])[0].length + 1); return Math.max(0, w - 1); };
function titleText(ctx, s, x, y, col) { let cx = x; for (const ch of s) { const g = TF[ch] || TF[' ']; map(ctx, g, cx, y, { '#': col }); cx += g[0].length + 1; } }

// Title bar (14 sp) with stripes, centred text, and optional buttons at the right.
function titleBar(ctx, x, y, w, text, active, btns = ['min', 'shade', 'close'], mark = true) {
  for (let j = 0; j < 14; j++) R(ctx, x, y + j, w, 1, mix('#2a2a42', '#1e1e32', j / 13));
  R(ctx, x, y, w, 1, C.mid);
  const tw = tfW(text), tx = x + Math.floor((w - tw) / 2);
  const left0 = x + (mark ? 20 : 6), right1 = x + w - (btns.length ? 6 + btns.length * 10 + 2 : 6);
  const st = (x0, x1) => { if (x1 - x0 < 4) return; for (let j = 0; j < 7; j++) { const yy = y + 4 + j; const c = active ? [C.stD, C.stA, C.stB, C.stD, C.stA, C.stB, C.stD][j] : [C.lo, C.stIA, C.stIB, C.lo, C.stIA, C.stIB, C.lo][j]; R(ctx, x0, yy, x1 - x0, 1, c); } };
  st(left0, tx - 5); st(tx + tw + 4, right1);
  if (text) { titleText(ctx, text, tx + 1, y + 5, C.ttlSh); titleText(ctx, text, tx, y + 4, active ? C.ttl : C.ttlI); }
  if (mark) markSmall(ctx, x + 6, y + 3, active);
  let bx = x + w - 11; for (let i = btns.length - 1; i >= 0; i--) { titleBtn(ctx, btns[i], bx, y + 3, active); bx -= 10; }
}
function titleBtn(ctx, kind, x, y, active, pressed) {
  R(ctx, x, y, 9, 9, C.edge); R(ctx, x + 1, y + 1, 7, 7, pressed ? '#55556e' : (active ? '#9a9ab4' : '#6a6a82')); R(ctx, x + 1, y + 1, 7, 1, active ? '#d0d0e0' : '#8a8aa0'); R(ctx, x + 1, y + 1, 1, 7, active ? '#d0d0e0' : '#8a8aa0');
  const g = C.edge;
  if (kind === 'min') R(ctx, x + 2, y + 6, 5, 1, g);
  if (kind === 'shade') { R(ctx, x + 2, y + 2, 5, 2, g); }
  if (kind === 'unshade') { R(ctx, x + 2, y + 2, 5, 1, g); R(ctx, x + 2, y + 6, 5, 1, g); }
  if (kind === 'album') map(ctx, ['.###.', '#...#', '#.#.#', '#...#', '.###.'], x + 2, y + 2, { '#': g }); // a record: album mode
  if (kind === 'close') map(ctx, ['#...#', '.#.#.', '..#..', '.#.#.', '#...#'], x + 2, y + 2, { '#': g });
}
// The Pyxamp mark: a mariner's compass star (Pyxis is the compass constellation). 9x9 and 13x15.
const MARK9 = ['....n....', '....n....', '...nNn...', '..g.N.g..', 'gGG.o.GGg', '..g.S.g..', '...sSs...', '....s....', '....s....'];
function markSmall(ctx, x, y, active) { map(ctx, MARK9, x, y, active ? { n: '#fff4c8', N: '#ffffff', g: '#a88c4a', G: '#e0b850', s: '#a88c4a', S: '#e0b850', o: '#3a301a' } : { n: '#b0b0c4', N: '#d0d0e0', g: '#6a6a82', G: '#8a8aa0', s: '#6a6a82', S: '#8a8aa0', o: '#2a2a3a' }); }
const MARK13 = ['......n......', '......n......', '.....nNn.....', '.....nNn.....', '..r..nNn..r..', '...r.nNn.r...', 'ggGGGG.GGGGgg', '...r.sSs.r...', '..r..sSs..r..', '.....sSs.....', '.....sSs.....', '......s......', '......s......', '.............', '.............'];
function markBig(ctx, x, y, pressed) { const o = pressed ? 1 : 0; map(ctx, MARK13, x + 1 + o, y + 1 + o, { n: '#14141e', N: '#14141e', g: '#14141e', G: '#14141e', s: '#14141e', S: '#14141e', r: '#14141e' }); map(ctx, MARK13, x + o, y + o, { n: '#fff4c8', N: '#ffffff', g: '#a88c4a', G: '#e0b850', s: '#a88c4a', S: '#e0b850', r: '#6a6a90' }); }

// Slider knob 14x11 (volume, balance, speed) with two grip lines.
function knob(ctx, x, y, pressed) {
  R(ctx, x, y, 14, 11, C.bEdge); R(ctx, x + 1, y + 1, 12, 9, pressed ? C.bPress : C.bFace); R(ctx, x + 1, y + 1, 12, 1, C.bHi); R(ctx, x + 1, y + 1, 1, 9, C.bHi); R(ctx, x + 2, y + 9, 11, 1, C.bLo); R(ctx, x + 12, y + 2, 1, 8, C.bLo);
  for (const gx of [5, 8]) { R(ctx, x + gx, y + 3, 1, 5, C.bGlyph); R(ctx, x + gx + 1, y + 3, 1, 5, C.bHi); }
}
// A rounded tube track, coloured, inside a recessed groove.
function tube(ctx, x, y, w, col, dim) {
  R(ctx, x + 1, y, w - 2, 1, C.lo); R(ctx, x, y + 1, 1, 3, C.lo); R(ctx, x + w - 1, y + 1, 1, 3, C.lo); R(ctx, x + 1, y + 4, w - 2, 1, C.mid);
  R(ctx, x + 1, y + 1, w - 2, 3, dim ? mix(col, '#22223a', 0.6) : col); R(ctx, x + 2, y + 1, w - 4, 1, mix(col, '#ffffff', dim ? 0.1 : 0.45)); R(ctx, x + 1, y + 3, w - 2, 1, mix(col, '#000000', 0.35));
}
const volCol = (v) => v < 0.5 ? mix('#2aa82a', '#d8c020', v * 2) : mix('#d8c020', '#e85414', (v - 0.5) * 2);
const balCol = (b) => mix('#2aa82a', '#e85414', Math.min(1, Math.abs(b)));
const eqCol = (db) => db < 0 ? mix('#2aa82a', '#d8c020', (db + 12) / 12) : mix('#d8c020', '#e85414', db / 12);

// Recessed LCD box.
function lcdBox(ctx, x, y, w, h, dots) {
  R(ctx, x, y, w, h, C.lcd); frame(ctx, x - 1, y - 1, w + 2, h + 2, C.lcdLo, C.lcdHi);
  if (dots) for (let j = 1; j < h; j += 2) for (let i = 1; i < w; i += 2) P(ctx, x + i, y + j, C.dot);
}
// Small light button with a label in the 5x6 font and an optional LED (EQ, PL, SHUFFLE, LIB, ON, AUTO, PRESETS, ADD...).
function labelBtn(ctx, x, y, w, h, text, o = {}) {
  bevelBox(ctx, x, y, w, h, o.pressed); const p = o.pressed ? 1 : 0; let tx = x + Math.floor((w - w5(text) + 1) / 2) + p;
  if (o.led !== undefined) { led(ctx, x + 3 + p, y + Math.floor((h - 4) / 2) + p, o.led, o.dim); tx = x + 9 + p + Math.floor((w - 10 - (w5(text) - 1)) / 2); }
  const ty = y + Math.floor((h - 5) / 2) + p; text5(ctx, text, tx + 1, ty + 1, o.dim ? C.bFace : C.bHi); text5(ctx, text, tx, ty, o.dim ? C.bDim : C.bGlyph);
}
// Repeat icon: a loop with an arrow; "1" inside for repeat-one.
function repeatIcon(ctx, x, y, col, one) {
  if (!one) { map(ctx, ['.#########.', '#.........#', '#.........#', '#..#......#', '.###.######'], x, y, { '#': col }); return; }
  map(ctx, ['.#######.', '#.......#', '#.......#', '#..#....#', '.###.####'], x - 1, y, { '#': col }); map(ctx, ['.#', '##', '.#', '.#', '.#'], x + 9, y, { '#': col });
}

// Mini transport (playlist bottom, windowshade): 7x7 glyphs, no buttons.
const MINI = { prev: ['#..#', '#.##', '####', '#.##', '#..#'], play: ['#..', '##.', '###', '##.', '#..'], pause: ['#.#', '#.#', '#.#', '#.#', '#.#'], stop: ['###', '###', '###', '###', '###'], next: ['#..#', '##.#', '####', '##.#', '#..#'], eject: ['.#.', '###', '...', '###', '...'] };
