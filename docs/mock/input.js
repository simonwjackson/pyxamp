// Keyboard and pointer for the live surface (index.html). Needs pyxamp.js.
'use strict';

function hitAt(x, y) { for (let i = HITS.length - 1; i >= 0; i--) { const h = HITS[i]; if (x >= h.x && x < h.x + h.w && y >= h.y && y < h.y + h.h) return h; } return null; }
function menuHitAt(S, x, y) { const hs = S._menuHits || []; for (let i = hs.length - 1; i >= 0; i--) { const h = hs[i]; if (x >= h.x && x < h.x + h.w && y >= h.y && y < h.y + h.h) return h; } return null; }
function openMenu(S, kind, h, up) { // anchor below (or above) a hit, in surface coordinates
  const items = menuItems(S, kind), g = menuGeom(items);
  S.menu = { kind, x: h.x - S.at.x, y: h.y - S.at.y + h.h, up: up ? 1 : 0, hover: null, sub: null };
}
const PRESS_IDS = new Set(['pl-album', 'prev', 'play', 'pause', 'stop', 'next', 'eject', 'shuf', 'rep', 'eqbtn', 'plbtn', 'logo', 'eqon', 'eqauto', 'pl-lib', 'cl-A', 'cl-I', 'cl-D', 'min', 'shade', 'close', 'eq-close', 'eq-shade', 'pl-close', 'pl-shade']);
const ACT_OF = { 'pl-lib': 'lib', 'pl-album': 'album' };
const MENU_OF = { opt: 'O', 'cl-O': 'O', 'cl-V': 'V', eqpre: 'presets', 'pl-add': 'pl-add', 'pl-rem': 'pl-rem', 'pl-sel': 'pl-sel', 'pl-misc': 'pl-misc' };

function dragUpdate(S, d, x, y, t) {
  const sx = x - S.at.x, sy = y - S.at.y, sw = spoken(S);
  if (d.id === 'vol') { S.vol = Math.max(0, Math.min(1, (sx - 114) / 54)); flash(S, `VOLUME: ${Math.round(S.vol * 100)}%`, t, 1); }
  if (d.id === 'bal') {
    const f = Math.max(0, Math.min(1, (sx - 184) / 24));
    if (sw) { S.speed = Math.round((0.5 + f * 2.5) * 10) / 10; flash(S, `SPEED: ${S.speed.toFixed(1)}X`, t, 1); }
    else { let b = f * 2 - 1; if (Math.abs(b) < 0.12) b = 0; S.bal = b; flash(S, b === 0 ? 'BALANCE: CENTER' : `BALANCE: ${Math.round(Math.abs(b) * 100)}% ${b < 0 ? 'LEFT' : 'RIGHT'}`, t, 1); }
  }
  if (d.id === 'pos') { const it = curItem(S); const len = S.follow ? S.follow.len : it ? it.len : 0; d.v = Math.max(0, Math.min(1, (sx - 30) / 219)); flash(S, `SEEK TO: ${fmt(d.v * len)}/${fmt(len)} (${Math.round(d.v * 100)}%)`, t, 1); }
  if (d.id === 'preamp' || d.id.startsWith('band-')) {
    const top = d.hy - S.at.y; const db = Math.max(-12, Math.min(12, Math.round(12 - (sy - top - 5) / 52 * 24)));
    if (d.id === 'preamp') { S.eq.pre = db; flash(S, `EQ: PREAMP: ${db >= 0 ? '+' : ''}${db.toFixed(1)} DB`, t, 1); }
    else { const i = +d.id.slice(5); S.eq.bands[i] = db; flash(S, `EQ: ${BANDS[i]}HZ: ${db >= 0 ? '+' : ''}${db.toFixed(1)} DB`, t, 1); }
  }
  if (d.id === 'pl-scroll') { const m = plRows(S); const L = layout(S, t); const lh = plFullH(S) - 58 - (m.head ? ROW : 0) - (m.foot ? ROW : 0); const full = Math.floor(lh / ROW); const max = Math.max(0, m.rows.length - full); const f = Math.max(0, Math.min(1, (sy - (L.pl.y + 20) - 9) / (plFullH(S) - 58 - 18))); S.scroll = Math.round(f * max); }
}
function dragEnd(S, d, t) { if (d.id === 'pos' && d.v != null) { const it = curItem(S); if (S.follow) S.follow.pos = d.v * S.follow.len; else if (it) setPos(S, d.v * it.len, t); } }

function libEnter(S, t) {
  const m = libRows(S), row = m.rows[S.libSel]; if (!row) return;
  const p = S.lib;
  if (!p.length) { const key = [S.libPyxis, 'jellyfin', 'ossicle'][S.libSel]; if (S.unreachable === key.replace(/9$/, '')) { flash(S, `${LIB[key].name.toUpperCase()} UNREACHABLE: CONNECTION REFUSED`, t, 2.5); return; } p.push({ key, name: LIB[key].name }); }
  else if (p.length === 1) p.push({ name: LIB[p[0].key].groups[S.libSel] });
  else if (p[0].key === 'pyxisCore' && p.length === 2) {
    if (row.kind === 'station') { startStation(S, row.l, t); return; }
    if (row.kind === 'playlist') p.push({ name: row.l }); else if (row.kind === 'album') p.push({ name: row.l.split(' - ')[1].replace(/ \(\d+\)$/, '') }); else return;
  }
  else if (p.length === 2 && p[0].key.startsWith('pyxis') && p[1].name === 'Albums') p.push({ name: PYXIS_ALBUMS[S.libSel][1] });
  else if (row.video) { handoff(S, JF_VIDEOS[S.libSel][0], t); return; }
  else { flash(S, `PLAYING FROM ${p[0].name.toUpperCase()}`, t, 1.5); return; }
  S.libSel = 0; S.scroll = 0;
}
function libBack(S) { if (S.lib.length) { S.lib.pop(); S.libSel = 0; S.scroll = 0; } else { S.plView = 'queue'; S.scroll = 0; } }
function handoff(S, title, t) {
  S.handoff = title; flash(S, `${title.toUpperCase()}: OPENED IN MPV`, t, 2.5);
  S.props = S.props.filter(p => p.id !== 'mpv').concat([{ id: 'mpv', x: S.at.x + W + 12, y: S.at.y + 20, w: 300, h: 190, title: `${title} - mpv`, video: ['#2a3a4a', '#0a0a10'], caption: 'Video plays here, outside Pyxamp', bar: '#2a2a30' }]);
}
function jumpMove(S, dir) { const m = jumpRows(S); let i = S.jumpSel; for (let k = 0; k < m.rows.length; k++) { i = Math.max(0, Math.min(m.rows.length - 1, i + dir)); if (!m.rows[i].group) break; } S.jumpSel = i; }
function jumpEnter(S, t, queue) {
  const m = jumpRows(S), r = m.rows[S.jumpSel]; if (!r || !r.item) return;
  if (r.item.kind === 'video') { handoff(S, r.item.text.split(' - ')[0], t); S.plView = 'queue'; return; }
  if (queue) { flash(S, `QUEUED IN ${LIB[r.src].name.toUpperCase()}`, t, 1.5); return; }
  flash(S, r.src !== S.src ? `${srcName(S).toUpperCase()} PAUSED. PLAYING FROM ${LIB[r.src].name.toUpperCase()}` : `PLAYING: ${r.item.text.toUpperCase()}`, t, 2.2);
  S.plView = 'queue'; S.jump = null;
}

function onKey(S, e) {
  const t = now(), k = e.key, lk = k.length === 1 ? k.toLowerCase() : k;
  if ((e.metaKey || e.getModifierState && e.getModifierState('OS')) && lk === 'z') { act(S, 'superz', t); return true; }
  if (!S.shown) return false;
  if (S.menu) { if (k === 'Escape') { if (S.menu.sub != null) S.menu.sub = null; else S.menu = null; return true; } }
  if (S.prompt) { // the prompt line takes every key until Enter or Esc
    if (k === 'Escape') { closePrompt(S, false, t); return true; }
    if (k === 'Enter') { closePrompt(S, true, t); return true; }
    if (k === 'Backspace') { S.prompt.text = S.prompt.text.slice(0, -1); return true; }
    if (k.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) { if (S.prompt.text.length < 40) S.prompt.text += k; return true; }
    return true;
  }
  const queueView = S.plOpen && S.plView === 'queue' && !S.info;
  if (S.conflict && queueView) {
    if (k === 'ArrowLeft' || k === 'ArrowRight') { S.conflict.pick = 1 - S.conflict.pick; return true; }
    if (k === 'Enter') { keepPosition(S, S.conflict.pick, t); return true; }
  }
  if (S.albumMode && queueView && !e.ctrlKey && !e.altKey && !e.metaKey) {
    const groups = queueGroups(S);
    if (S.albumOpen != null) {
      const items = groups[S.albumOpen].items; const j = Math.max(0, items.indexOf(S.sel[0]));
      if (k === 'Escape' || k === 'Backspace') { S.albumSel = S.albumOpen; S.albumOpen = null; S.scroll = 0; return true; }
      if (k === 'ArrowDown') { S.sel = [items[Math.min(items.length - 1, j + 1)]]; return true; }
      if (k === 'ArrowUp') { S.sel = [items[Math.max(0, j - 1)]]; return true; }
      if (k === 'Enter') { S.cur = S.sel[0]; setPos(S, 0, t); S.play = 'play'; S.marqueeT0 = t; return true; }
    } else {
      if (k === 'ArrowDown') { S.albumSel = Math.min(groups.length - 1, S.albumSel + 1); return true; }
      if (k === 'ArrowUp') { S.albumSel = Math.max(0, S.albumSel - 1); return true; }
      if (k === 'Enter') { S.albumOpen = S.albumSel; S.sel = [groups[S.albumSel].items[0]]; S.scroll = 0; return true; }
    }
  }
  if (S.plView === 'jump' && S.plOpen) {
    if (k === 'Escape') { S.plView = 'queue'; S.jump = null; return true; }
    if (k === 'Backspace') { S.jump = (S.jump || '').slice(0, -1); S.jumpSel = 0; return true; }
    if (k === 'ArrowDown') { jumpMove(S, 1); return true; } if (k === 'ArrowUp') { jumpMove(S, -1); return true; }
    if (k === 'Enter') { jumpEnter(S, t, e.shiftKey); return true; } if (k === 'Insert') { jumpEnter(S, t, true); return true; }
    if (k.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) { S.jump = (S.jump || '') + k; S.jumpSel = 0; return true; }
  }
  if (e.ctrlKey && lk === 'd') { act(S, 'dbl', t); return true; }
  if (e.ctrlKey && lk === 't') { act(S, 'time-toggle', t); return true; }
  if (e.altKey && lk === 'w') { act(S, 'shade', t); return true; }
  if (e.altKey && lk === 'g') { act(S, 'eqbtn', t); return true; }
  if (e.altKey && lk === 'e') { act(S, 'plbtn', t); return true; }
  if (e.altKey && k === '3') { act(S, 'info', t); return true; }
  if (e.ctrlKey || e.altKey || e.metaKey) return false;
  const lib = S.plView === 'lib' && S.plOpen && !S.info;
  switch (lk) {
    case 'z': act(S, 'prev', t); return true; case 'x': act(S, 'play', t); return true; case 'c': act(S, 'pause', t); return true;
    case 'v': act(S, 'stop', t); return true; case 'b': act(S, 'next', t); return true;
    case 'g': act(S, 'album', t); return true;
    case 'j': act(S, 'jump', t); return true; case 'r': act(S, 'rep', t); return true; case 's': act(S, 'shuf', t); return true; case 'l': act(S, 'lib', t); return true;
    case 'Escape': if (S.info) { S.info = false; return true; } if (lib) { libBack(S); return true; } return false;
    case 'Backspace': if (lib) { libBack(S); return true; } return false;
    case 'Enter': if (lib) { libEnter(S, t); return true; } return false;
    case 'ArrowDown': if (lib) { S.libSel = Math.min(libRows(S).rows.length - 1, S.libSel + 1); return true; } S.vol = Math.max(0, S.vol - 0.02); flash(S, `VOLUME: ${Math.round(S.vol * 100)}%`, t, 1); return true;
    case 'ArrowUp': if (lib) { S.libSel = Math.max(0, S.libSel - 1); return true; } S.vol = Math.min(1, S.vol + 0.02); flash(S, `VOLUME: ${Math.round(S.vol * 100)}%`, t, 1); return true;
    case 'ArrowRight': if (lib) { libEnter(S, t); return true; } setPos(S, posNow(S, t) + 5, t); return true;
    case 'ArrowLeft': if (lib) { libBack(S); return true; } setPos(S, posNow(S, t) - 5, t); return true;
  }
  return false;
}

function onDown(S, x, y, button, dbl) {
  const t = now(); S.tip = null;
  if (S.menu) {
    const mh = menuHitAt(S, x, y);
    if (!mh) { S.menu = null; return; }
    if (mh.it.dis) return;
    if (mh.it.sub) { S.menu.sub = mh.i; return; }
    if (mh.it.act) act(S, mh.it.act, t); S.menu = null; return;
  }
  const h = hitAt(x, y); if (!h) return;
  if (MENU_OF[h.id]) { const up = h.id.startsWith('pl-'); openMenu(S, MENU_OF[h.id], h, up); S.pressed = h.id; return; }
  if (['vol', 'bal', 'pos', 'preamp', 'pl-scroll'].includes(h.id) || h.id.startsWith('band-')) { S.drag = { id: h.id, hy: h.y }; dragUpdate(S, S.drag, x, y, t); return; }
  if (h.id === 'row') {
    let i = h.d.idx;
    if (S.prompt) return;
    if (S.plView === 'lib' && !S.info) { S.libSel = i; if (button === 2) { S.menu = { kind: 'librow', idx: i, x: x - S.at.x, y: y - S.at.y, hover: null, sub: null }; return; } if (dbl) libEnter(S, t); return; }
    if (S.plView === 'jump') { const r = jumpRows(S).rows[i]; if (r && !r.group) { S.jumpSel = i; if (button === 2) { S.menu = { kind: 'jumprow', idx: i, x: x - S.at.x, y: y - S.at.y, hover: null, sub: null }; return; } if (dbl) jumpEnter(S, t); } return; }
    if (S.info) return;
    if (S.albumMode && S.albumOpen == null) { S.albumSel = i; if (dbl) { S.albumOpen = i; S.sel = [queueGroups(S)[i].items[0]]; S.scroll = 0; } return; }
    if (S.albumMode) i = queueGroups(S)[S.albumOpen].items[i];
    S.sel = [i];
    if (button === 2) { S.menu = { kind: 'row', idx: i, x: x - S.at.x, y: y - S.at.y, hover: null, sub: null }; return; }
    if (dbl) { S.cur = i; setPos(S, spoken(S) && S.queue[i].pos ? S.queue[i].pos : 0, t); S.play = 'play'; S.marqueeT0 = t; }
    return;
  }
  if (h.id === 'crumb') { libBack(S); return; }
  if (h.id === 'albumback') { S.albumSel = S.albumOpen; S.albumOpen = null; S.scroll = 0; return; }
  if (h.id === 'chip') { keepPosition(S, h.d.pick, t); return; }
  if (PRESS_IDS.has(h.id)) { S.pressed = h.id; S._pressHit = h; return; }
  act(S, h.id, t);
}
function onMove(S, x, y) {
  const t = now();
  if (S.drag) { dragUpdate(S, S.drag, x, y, t); return; }
  if (S.menu) { const mh = menuHitAt(S, x, y); if (mh) { if (mh.level === 0) { S.menu.hover = mh.i; if (mh.it.sub && !mh.it.dis) S.menu.sub = mh.i; else S.menu.sub = null; S.menu.subHover = null; } else S.menu.subHover = mh.i; } return; }
  const h = hitAt(x, y); const tip = h && h.d && h.d.tip;
  if (!tip) { S.tip = null; S._tipFor = null; clearTimeout(S._tipTimer); return; }
  if (S._tipFor === h.id + (h.d.idx || '')) return;
  S._tipFor = h.id + (h.d.idx || ''); S.tip = null; clearTimeout(S._tipTimer);
  S._tipTimer = setTimeout(() => { S.tip = { x: Math.min(x - S.at.x + 6, W), y: y - S.at.y + 16, text: tip }; }, 550);
}
function onUp(S, x, y) {
  const t = now();
  if (S.drag) { dragEnd(S, S.drag, t); S.drag = null; return; }
  if (S.pressed && S._pressHit) { const h = hitAt(x, y); const id = S.pressed; S.pressed = null; S._pressHit = null; if (h && h.id === id) act(S, ACT_OF[id] || id, t); return; }
  if (S.pressed && !S.menu) S.pressed = null;
  if (S.menu && S.pressed) S.pressed = null;
}
