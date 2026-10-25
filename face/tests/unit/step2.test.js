'use strict';
// Step 2: the wire's new snapshot fields and pages, own playback in the main
// section, the playlist section's model, keys, EQ and preferences.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { load } = require('./load');

const P = load('protocol.js'), V = load('view.js'), PL = load('playlist.js'), K = load('keys.js'), EQ = load('eq.js'), PR = load('prefs.js');
const FIX = path.resolve(__dirname, '../../fixtures');
const plain = (v) => JSON.parse(JSON.stringify(v));
const raw = (name) => JSON.parse(fs.readFileSync(path.join(FIX, name + '.json'), 'utf8'));
const T0 = 1793000000000;
function snap(name) {
  const d = P.decodeServer(JSON.stringify({ _tag: 'state', payload: raw(name).state }));
  assert.equal(d.ok, true, d.reason);
  return d.msg;
}
const connected = { _tag: 'connected', instanceId: 'i' };
function input(name, extra = {}) {
  const s = snap(name);
  return { link: connected, player: s.player, snap: s, observedAt: 1000, now: 1000, wallNow: T0 - 86000, timeMode: 'elapsed', flash: null, seekDrag: null, ...extra };
}
function ctx(name, extra = {}) {
  const inp = input(name, extra.input || {});
  return {
    snap: inp.snap, main: V.main(inp), view: 'queue', albumMode: false, albumOpen: '', albumSel: 0, sel: '',
    lib: { path: [], sel: 0 }, pages: {}, jump: { text: '', sel: 0, status: 'idle', items: [] }, prompt: null, conflictPick: 0,
    info: V.itemInfo(inp), caret: '_', ...extra,
  };
}
const ev = (key, o = {}) => ({ key, text: o.text !== undefined ? o.text : key.length === 1 ? key : '', ctrl: false, alt: false, shift: false, meta: false, ...o });

test('protocol: own, library, queue, outputs, conflicts and engine decode; absent stays absent', () => {
  const s = snap('own-ossicle-conflict');
  assert.equal(s.own.source, 'ossicle');
  assert.equal(s.own.speed, 1.5);
  assert.deepEqual(plain(s.own.skip), { backS: 15, forwardS: 30 });
  assert.equal('repeat' in s.own, false, 'Ossicle reports no repeat');
  assert.equal(s.conflicts[0].positions[1].device, 'Pixel');
  assert.equal(s.library._tag, 'sources');
  assert.equal(s.library.sources[2].status._tag, 'unavailable');
  assert.equal(s.library.sources[2].status.message, 'jellyfin.local did not answer');
  assert.equal(s.queue[0].item.progress.played, true);
  assert.equal('positionMs' in s.queue[0].item.progress, false);
  assert.equal(s.engine._tag, 'ready');
  const idle = snap('idle');
  assert.equal(idle.own, null); assert.deepEqual(plain(idle.outputs), []); assert.equal(idle.engine, null);
  assert.equal(snap('own-pyxis-console').own.output.local, false);
  assert.equal(snap('own-yield-stays').own.yielded.resumeByMs, undefined);
});

test('protocol: a page decodes with its id; every fixture page is valid wire', () => {
  for (const f of fs.readdirSync(FIX)) for (const pg of raw(f.replace('.json', '')).pages || []) {
    const d = P.decodeServer(JSON.stringify({ _tag: 'page', payload: pg }));
    assert.equal(d.ok, true, `${f}: ${d.reason}`);
    assert.equal(d.msg._tag, 'page'); assert.equal(d.msg.id, pg.id);
  }
  const d = P.decodeServer(JSON.stringify({ _tag: 'page', payload: { id: 'r1', items: [{ source: 'pyxis', id: 'a', kind: 'album', title: 'X', openable: true, playable: true }] } }));
  assert.equal('year' in d.msg.items[0], false);
  assert.equal(P.decodeServer(JSON.stringify({ _tag: 'page', payload: { id: 'r1', items: [{ id: 'a' }] } })).ok, false);
});

test('protocol: every request the face sends is valid under the canonical schema', () => {
  const reqs = [
    ['library.open', { source: 'pyxis', node: 'placement:collection', cursor: 'c2' }], ['library.open', { source: 'ossicle' }],
    ['search', { query: 'stereo' }], ['item.play', { source: 'jellyfin', item: 'x', mode: 'last' }], ['station.start', { source: 'pyxis', seed: 'track:1' }],
    ['queue.jump', { occurrence: 'occ-1' }], ['queue.remove', { occurrence: 'occ-1' }], ['output.select', { source: 'pyxis', output: 'sonos' }],
    ['playlist.save', { source: 'pyxis', name: 'Sunday' }], ['playlist.add', { source: 'pyxis', playlist: 'p', item: 't' }],
    ['item.markPlayed', { source: 'ossicle', item: 'e', played: true }], ['conflict.resolve', { source: 'ossicle', conflict: 'c', keep: 'phone' }],
    ['own.transport', { action: 'playPause' }], ['own.seek', { positionMs: 5000, occurrence: 'occ-2' }], ['own.volume', { value: 0.5 }],
    ['own.repeat', { mode: 'one' }], ['own.shuffle', { enabled: true }], ['own.speed', { rate: 1.5 }],
    ['eq.set', EQ.wire(EQ.effective(PR.defaults().eq, false))], ['balance.set', { value: -0.5 }],
  ];
  for (const [tag, payload] of reqs) assert.equal(P.checkClient(P.request('face-1', tag, payload)), null, tag);
  assert.notEqual(P.checkClient(P.request('face-1', 'own.speed', { rate: 4 })), null, 'speed is bounded');
  assert.notEqual(P.checkClient(P.request('face-1', 'eq.set', { on: true, preampDb: 0, bandsDb: [0, 0] })), null, 'ten bands');
});

test('main: own playback is green, numbered, and drives own requests; following stays amber', () => {
  const m = V.main(input('own-pyxis-playing'));
  assert.equal(m.mode, 'own'); assert.equal(m.lcd, 'green'); assert.equal(m.status, 'play');
  assert.equal(m.marquee.text, '2. Stereolab - Miss Modular (4:51)');
  assert.deepEqual(plain(m.format), { left: '320', leftUnit: 'kbps', khz: '44', channels: 2 });
  assert.equal(m.controls.seek.enabled, true); assert.equal(m.controls.balance.enabled, true);
  assert.equal(m.eqNote, null, 'the EQ works on own local playback');
  assert.equal(m.target, null, 'no step-1 command target');
  assert.equal(m.occurrence, 'occ-2');
  assert.equal(V.positionMs(input('own-pyxis-playing', { now: 3000 })), 63000);
  assert.equal(V.main(input('own-yielded')).lcd, 'amber');
});

test('main: console mode starts with the device, has a flat visualizer and no EQ', () => {
  const inp = input('own-pyxis-console'), m = V.main(inp);
  assert.equal(m.lcd, 'green');
  assert.equal(m.marquee.text, 'Living Room: 1. Stereolab - Brakhage (4:30)');
  assert.equal(m.eqNote, 'NO EQ ON LIVING ROOM');
  assert.equal(V.visual(inp, null, 'spec', false)._tag, 'flat');
  assert.equal(V.visual(inp, null, 'off', false)._tag, 'blank');
  assert.equal(m.controls.balance.enabled, false); assert.match(m.controls.balance.reason, /Living Room/);
});

test('yield: the amber tail states the resume rule; the hand-back is said for 2 s', () => {
  assert.equal(V.marquee(input('own-yielded')).text, 'Firefox: Khruangbin - Tiny Desk Concert (24:31)  ***  PYXAMP PAUSED, RESUMES IF THIS STOPS BY 1:26');
  const later = V.marquee(input('own-yielded', { wallNow: T0 - 85000 }));
  assert.match(later.text, /BY 1:25$/);
  assert.equal(later.key, V.marquee(input('own-yielded')).key, 'the countdown does not restart the scroll');
  assert.match(V.marquee(input('own-yielded', { wallNow: T0 + 1 })).text, /PYXAMP STAYS PAUSED$/);
  assert.match(V.marquee(input('own-yield-stays')).text, /\*\*\*  PYXAMP STAYS PAUSED$/);
  assert.equal(V.transition(snap('own-yielded'), snap('own-resumed')), 'BACK TO PYXAMP: RESUMING');
  const stays = snap('own-resumed'); stays.own.transport = 'paused';
  assert.equal(V.transition(snap('own-yield-stays'), stays), 'BACK TO PYXAMP: STAYS PAUSED');
  assert.equal(V.transition(snap('own-pyxis-playing'), snap('own-pyxis-playing')), null);
});

test('retry, failure and engine errors show in the title with a red light', () => {
  const r = input('own-retry-first', { wallNow: T0 });
  assert.equal(V.marquee(r).text, 'CANNOT PLAY: RETRY 1 IN 2 S');
  assert.equal(V.status(r), 'error'); assert.equal(V.clock(r), null); assert.equal(V.positionFraction(r), null);
  assert.equal(V.marquee(input('own-retry-failed', { wallNow: T0 + 3000 })).text, 'CANNOT PLAY: RETRY 2 IN 7 S');
  assert.equal(V.marquee(input('own-retry-failed', { wallNow: T0 + 20000 })).text, 'CANNOT PLAY: RETRYING');
  const before = snap('own-retry-first'), after = snap('own-retry-first'); after.queue[1].status = 'failed';
  assert.equal(V.transition(before, after), 'SKIPPED: TRACK CANNOT PLAY');
  const e = input('own-engine-unavailable');
  assert.equal(V.marquee(e).text, 'PLAYBACK UNAVAILABLE: mpv is not installed'); assert.equal(V.status(e), 'error');
});

test('spoken word: speed slider, skip intervals, shuffle and repeat dimmed, conflict holds the clock', () => {
  const m = V.main(input('own-ossicle-playing'));
  assert.equal(m.spoken, true); assert.equal(m.speed, 1.5); assert.deepEqual(plain(m.skip), { backS: 15, forwardS: 30 });
  assert.equal(m.controls.speed.enabled, true); assert.equal(m.controls.balance.enabled, false);
  assert.equal(m.controls.shuffle.enabled, false); assert.match(m.controls.repeat.reason, /not for spoken word/);
  assert.equal(m.repeat, null); assert.equal(m.shuffle, null);
  assert.equal(V.positionMs(input('own-ossicle-playing', { now: 3000 })), 643000, 'position moves at 1.5x');
  assert.equal(V.speedAt(0), 0.5); assert.equal(V.speedAt(1), 3); assert.equal(V.speedAt(0.4), 1.5);
  assert.equal(V.speedFraction(1), 0.2);
  const c = input('own-ossicle-conflict'), mc = V.main(c);
  assert.equal(mc.marquee.text, 'POSITION CONFLICT'); assert.equal(mc.clock, null); assert.equal(mc.position, null);
  assert.equal(mc.controls.seek.enabled, false); assert.equal(mc.conflict, true);
  assert.deepEqual(plain(V.itemInfo(input('own-ossicle-playing')).rows.map((r) => r[0])),
    ['Source', 'Plays on', 'Title', 'Show', 'Length', 'Bitrate', 'Sample rate', 'Channels', 'Position', 'Speed', 'Skip', 'Played', 'State']);
});

test('balance: snaps to the centre and says which side', () => {
  assert.equal(V.balanceAt(0.5), 0); assert.equal(V.balanceAt(0.55), 0); assert.equal(V.balanceAt(1), 1);
  assert.equal(V.balanceText(-0.5), 'BALANCE: 50% LEFT'); assert.equal(V.balanceText(0), 'BALANCE: CENTER');
});

test('nothing plays with sources: says so; no sources keeps the step-1 text', () => {
  const s = input('library-sources');
  assert.equal(V.main(s).mode, 'none');
  assert.equal(V.marquee(s).text, 'PYXAMP  ***  NOTHING PLAYS. PRESS J OR L TO FIND SOMETHING');
  assert.match(V.main(s).controls.play.reason, /Press J or L/);
  assert.match(V.marquee(input('idle')).text, /NO SOURCES YET/);
});

test('playlist: the title names what it shows', () => {
  assert.equal(PL.title(ctx('own-pyxis-playing')), 'PYXIS QUEUE');
  assert.equal(PL.title(ctx('own-pyxis-playing', { albumMode: true })), 'PYXIS QUEUE BY ALBUM');
  assert.equal(PL.title(ctx('own-ossicle-playing', { albumMode: true })), 'OSSICLE UP NEXT BY SHOW');
  assert.equal(PL.title(ctx('idle')), 'QUEUE');
  assert.equal(PL.title(ctx('own-pyxis-playing', { view: 'library' })), 'LIBRARY');
  assert.equal(PL.title(ctx('own-pyxis-playing', { prompt: { title: 'SAVE AS PLAYLIST', label: 'Name', text: '' } })), 'SAVE AS PLAYLIST');
  for (const t of PL.TITLES) assert.match(PL.titleSprite(t), /^pl-title-[a-z-]+$/);
});

test('playlist: queue rows show played, part-played, failed, retrying, paused and the conflict', () => {
  const o = PL.model(ctx('own-ossicle-playing')).rows;
  assert.equal(o[0].r, 'played'); assert.equal(o[0].tick, true); assert.equal(o[0].colour, 'dim');
  assert.equal(o[1].colour, 'current'); assert.equal(o[1].r, '35:40', 'the current item shows its length');
  assert.equal(o[3].l, '4. Dune - Frank Herbert'); assert.equal(o[3].r, '-10:05:00', 'a part-played book shows the time left');
  const f = PL.model(ctx('own-retry-failed')).rows;
  assert.equal(f[1].colour, 'err'); assert.equal(f[1].r, 'cannot play');
  assert.equal(f[2].r, 'retrying'); assert.equal(f[2].rcolour, 'err');
  const y = PL.model(ctx('own-yielded')).rows;
  assert.equal(y[1].r, 'paused', "own playback's row says paused while following");
  const c = PL.model(ctx('own-ossicle-conflict', { conflictPick: 1 }));
  assert.deepEqual(plain(c.rows[1].chips), ['Myoko 10:40', 'Pixel 20:10']);
  assert.equal(c.rows[1].pick, 1);
  assert.match(c.foot.l, /Left, Right: pick a position/);
});

test('album mode: consecutive runs fold; the playing album says now k/n; Enter opens it', () => {
  const rows = PL.model(ctx('library-mixed', { albumMode: true })).rows;
  assert.equal(rows.length, 3);
  assert.equal(rows[0].l, '1. Stereolab - Dots and Loops (1997)'); assert.equal(rows[0].r, 'now 4/4  19:16'); assert.equal(rows[0].play, true);
  assert.equal(rows[1].l, '2. Talk Talk - I Believe in You', 'a single track stays a track row');
  assert.equal(rows[2].r, '3 tracks  13:49');
  const open = PL.model(ctx('library-mixed', { albumMode: true, albumOpen: 'occ-1' }));
  assert.equal(open.head.l, '< Stereolab - Dots and Loops (1997)'); assert.equal(open.head.r, '4 tracks');
  assert.equal(open.rows[3].l, '4. Stereolab - Diagonals');
  const sw = PL.model(ctx('own-ossicle-playing', { albumMode: true })).rows;
  assert.deepEqual(plain(sw.map((r) => [r.l, r.r])), [
    ['1. The Allusionist', '2 episodes, 1 played  1:06:10'], ['2. Song Exploder - Mixtapes', '25:00'],
    ['3. Dune - Frank Herbert', 'book  -10:05:00'], ['4. The Allusionist - Old radio', '32:00']]);
  assert.equal(PL.groups(snap('own-pyxis-long').queue).length, 24);
});

test('library: root, groups as reported with a rule, pages with more, errors', () => {
  const root = PL.model(ctx('library-mixed', { view: 'library' }));
  assert.equal(root.head.l, 'Library');
  assert.deepEqual(plain(root.rows.map((r) => [r.l, r.r])), [['Pyxis', '10 groups >'], ['Ossicle', 'connecting'], ['Jellyfin', '4 groups >']]);
  assert.equal(root.rows[0].colour, 'current', 'the active source is white');
  const down = PL.model(ctx('library-sources', { view: 'library' })).rows[2];
  assert.equal(down.r, 'unreachable'); assert.equal(down.rcolour, 'err'); assert.match(down.tip, /did not answer/);
  const groups = PL.model(ctx('library-mixed', { view: 'library', lib: { path: [{ source: 'pyxis', label: 'Pyxis' }], sel: 0 } })).rows;
  assert.equal(groups.length, 10); assert.equal(groups[1].r, '412 >'); assert.equal(groups[2].r, '1,288 >');
  assert.deepEqual(plain(groups.map((g, i) => (g.divTop ? i : -1)).filter((i) => i >= 0)), [7], 'one rule before Bookmarks');
  const at = { path: [{ source: 'pyxis', label: 'Pyxis' }, { source: 'pyxis', node: 'placement:collection', label: 'Collection', kind: 'albums' }], sel: 0 };
  assert.equal(PL.model(ctx('library-mixed', { view: 'library', lib: at })).rows[0].l, 'Loading...');
  const pg = raw('library-mixed').pages[0];
  const pages = {}; pages[PL.pageKey('pyxis', 'placement:collection')] = { status: 'ready', items: P.decodeServer(JSON.stringify({ _tag: 'page', payload: pg })).msg.items, nextCursor: 'collection-2', total: 412 };
  const m = PL.model(ctx('library-mixed', { view: 'library', lib: at, pages }));
  assert.equal(m.head.l, '< Library > Pyxis > Collection'); assert.equal(m.head.r, '412');
  assert.equal(m.rows[0].l, 'Stereolab - Dots and Loops (1997)'); assert.equal(m.rows[0].r, '10 >');
  assert.equal(m.rows[7].r, '>', 'an unknown count stays unknown');
  assert.equal(m.rows[m.rows.length - 1].ref.type, 'more'); assert.equal(m.rows[m.rows.length - 1].r, '8 of 412');
  pages[PL.pageKey('pyxis', 'placement:collection')] = { status: 'error', items: [], message: 'timeout' };
  assert.equal(PL.model(ctx('library-mixed', { view: 'library', lib: at, pages })).rows[0].l, 'Pyxis did not answer: timeout');
});

test('jump box: source tags, unreachable sources, and which source pauses', () => {
  const items = raw('library-mixed').pages.find((p) => p.query === 'stereo').items.map((i) => P.decodeServer(JSON.stringify({ _tag: 'page', payload: { id: 'x', items: [i] } })).msg.items[0]);
  const j = (sel, extra) => PL.model(ctx('library-mixed', { view: 'jump', jump: { text: 'stereo', sel, status: 'ready', items }, ...extra }));
  const m = j(0);
  assert.deepEqual(plain(m.rows.map((r) => r.tag)), ['PYX', 'PYX', 'PYX', 'OSS', 'JF', 'JF']);
  assert.equal(m.head.r, '6 in 3 sources');
  assert.equal(m.foot.l, 'Enter: play   Ins: queue   Esc: close');
  assert.equal(j(3).foot.l, 'Enter: play from Ossicle; Pyxis pauses');
  assert.equal(j(5).foot.l, 'Enter: open in mpv, outside Pyxamp');
  const down = PL.model(ctx('library-sources', { view: 'jump', jump: { text: 'zz', sel: 0, status: 'ready', items: [] } })).rows;
  assert.equal(down[0].l, 'Nothing found for "zz".');
  assert.equal(down[1].tag, 'JF'); assert.equal(down[1].tagErr, true);
  const empty = PL.model(ctx('library-mixed', { view: 'jump' }));
  assert.equal(empty.rows[0].l, 'Type to search Pyxis, Ossicle and Jellyfin.');
  assert.equal(empty.head.l, 'Jump: _');
});

test('prompt line: names the action and makes the list inert', () => {
  const m = PL.model(ctx('own-pyxis-playing', { prompt: { kind: 'savepl', title: 'SAVE AS PLAYLIST', label: 'Name', text: 'Sun', right: '5 tracks', hint: 'Enter: save in Pyxis   Esc: cancel' } }));
  assert.equal(m.head.l, 'Name: Sun_'); assert.equal(m.head.r, '5 tracks'); assert.equal(m.inert, true); assert.equal(m.sel, -1);
  assert.equal(m.foot.l, 'Enter: save in Pyxis   Esc: cancel');
  assert.equal(m.rows.length, 5, 'the queue stays visible below');
});

test('keys: the prompt line and the jump box own every plain key', () => {
  const pr = { view: 'queue', plOpen: true, prompt: true };
  for (const k of ['z', 'x', 'c', 'v', 'b', 'r', 's', 'l', 'j', 'g', 'i']) assert.deepEqual(plain(K.decide(ev(k), pr)), { _tag: 'promptType', text: k }, k);
  for (const k of ['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Tab', 'Insert', 'Delete']) assert.deepEqual(plain(K.decide(ev(k, { text: '' }), pr)), { _tag: 'none' }, k);
  assert.deepEqual(plain(K.decide(ev('Enter'), pr)), { _tag: 'promptEnter' });
  assert.deepEqual(plain(K.decide(ev('Escape'), pr)), { _tag: 'promptCancel' });
  assert.deepEqual(plain(K.decide(ev('d', { ctrl: true }), pr)), { _tag: 'double' });
  const jump = { view: 'jump', plOpen: true };
  assert.deepEqual(plain(K.decide(ev('ArrowDown', { text: '' }), jump)), { _tag: 'jumpMove', delta: 1 });
  assert.deepEqual(plain(K.decide(ev('Enter', { shift: true }), jump)), { _tag: 'jumpEnter', queue: true });
  assert.deepEqual(plain(K.decide(ev('Insert'), jump)), { _tag: 'jumpEnter', queue: true });
});

test('keys: conflict, album mode, library and Winamp keys', () => {
  const q = { view: 'queue', plOpen: true };
  assert.deepEqual(plain(K.decide(ev('ArrowRight', { text: '' }), { ...q, conflict: true })), { _tag: 'conflictPick', delta: 1 });
  assert.deepEqual(plain(K.decide(ev('Enter'), { ...q, conflict: true })), { _tag: 'conflictKeep' });
  assert.deepEqual(plain(K.decide(ev('ArrowRight', { text: '' }), q)), { _tag: 'seek', deltaMs: 5000 });
  assert.deepEqual(plain(K.decide(ev('ArrowDown', { text: '' }), { ...q, albumMode: true })), { _tag: 'rowMove', delta: 1 });
  assert.deepEqual(plain(K.decide(ev('ArrowDown', { text: '' }), q)), { _tag: 'volume', delta: -0.02 }, 'the track queue keeps volume (decision 29)');
  assert.deepEqual(plain(K.decide(ev('Escape'), { ...q, albumMode: true, albumOpen: true })), { _tag: 'albumBack' });
  const lib = { view: 'library', plOpen: true };
  assert.deepEqual(plain(K.decide(ev('Enter'), lib)), { _tag: 'rowEnter', queue: false });
  assert.deepEqual(plain(K.decide(ev('Insert'), lib)), { _tag: 'rowEnter', queue: true });
  assert.deepEqual(plain(K.decide(ev('Backspace'), lib)), { _tag: 'libBack' });
  assert.deepEqual(plain(K.decide(ev('g'), q)), { _tag: 'albumMode' });
  assert.deepEqual(plain(K.decide(ev('i'), q)), { _tag: 'info' });
  assert.deepEqual(plain(K.decide(ev('Delete', { text: '' }), q)), { _tag: 'rowRemove' });
  assert.equal(K.fromQt(0x01000007, '', 0).key, 'Delete');
});

test('EQ: one preset list; AUTO picks Voice for spoken word; the wire payload', () => {
  assert.deepEqual(plain(PR.defaults().eq.bands), plain(EQ.preset('flat').bands));
  assert.deepEqual(plain(EQ.PRESETS.map((p) => p.label)), ['Flat', 'Voice', 'Small speakers', 'Night']);
  const user = { on: true, auto: true, pre: 2, bands: [1, 1, 1, 1, 1, 1, 1, 1, 1, 1] };
  assert.deepEqual(plain(EQ.effective(user, true).bands), plain(EQ.preset('voice').bands));
  assert.deepEqual(plain(EQ.effective(user, false).bands), plain(user.bands));
  assert.deepEqual(plain(EQ.wire(EQ.effective(user, false))), { on: true, preampDb: 2, bandsDb: user.bands });
  const a = EQ.preset('voice'); a.bands[0] = 9;
  assert.equal(EQ.preset('voice').bands[0], -6, 'presets are copied, never shared');
});

test('preferences: balance, album mode and user EQ presets are kept and bounded', () => {
  const p = PR.sanitize({ balance: 7, albumMode: true, eqPresets: [{ name: '  Mine  ', pre: 40, bands: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] }, { name: '', bands: [] }, 'x'] });
  assert.equal(p.balance, 1); assert.equal(p.albumMode, true);
  assert.deepEqual(plain(p.eqPresets), [{ name: 'Mine', pre: 12, bands: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] }]);
  assert.equal(PR.defaults().balance, 0);
});

test('totals: selected and total length of what is known', () => {
  assert.equal(PL.totals(ctx('own-pyxis-playing', { sel: 'occ-2' })), '4:51/25:10');
  assert.equal(PL.totals(ctx('idle')), '0:00/0:00');
});
