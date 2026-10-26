'use strict';
// The Platinum face's pure parts: which face the desktop asks for, sentence
// case, the fit and the library outline.
const test = require('node:test');
const assert = require('node:assert/strict');
const { load } = require('./load');

const DT = load('desktop-theme.js'), C = load('lcdcase.js'), F = load('fit.js'), O = load('outline.js');
const plain = (v) => JSON.parse(JSON.stringify(v));

// ---------- which face ----------
const REGISTRY = JSON.stringify([
  { id: 'platinum', label: 'Mac OS 8 Platinum', pyxampFace: 'platinum' },
  { id: 'win98', label: 'Windows 98', pyxampFace: 'winamp' },
  { id: 'beos', label: 'BeOS' },                                  // no pyxampFace
  { id: 'amiga', label: 'Workbench', pyxampFace: 'workbench' },   // unknown face
  { id: 'next', pyxampFace: 'winamp' },                           // a new theme that picks a face by data
]);

test('registry and state paths follow the environment, with the defaults', () => {
  assert.deepEqual(plain(DT.paths({ HOME: '/h' })), {
    registry: '/h/.config/quickshell/mountainous/themes.json',
    name: '/h/.local/state/mountainous/theme-name', mode: '/h/.local/state/mountainous/theme', extra: '/h/.local/state/mountainous/theme-extra',
  });
  const p = DT.paths({ HOME: '/h', XDG_CONFIG_HOME: '/c', XDG_STATE_HOME: '/s' });
  assert.equal(p.registry, '/c/quickshell/mountainous/themes.json');
  assert.equal(p.name, '/s/mountainous/theme-name');
  assert.equal(DT.paths({ HOME: '/h', XDG_CONFIG_HOME: '/c', PYXAMP_DESKTOP_THEMES: '/x/themes.json' }).registry, '/x/themes.json');
});

test('the face is the entry\'s pyxampFace', () => {
  assert.equal(DT.face('platinum', REGISTRY), 'platinum');
  assert.equal(DT.face('win98', REGISTRY), 'winamp');
  assert.equal(DT.face('next', REGISTRY), 'winamp');
  assert.equal(DT.face('  win98\n', REGISTRY), 'winamp', 'the name is trimmed');
});

test('a missing theme-name file, or a name not in the registry, resolves to platinum (Theme.qml)', () => {
  assert.equal(DT.themeName(null, DT.parseRegistry(REGISTRY)), 'platinum');
  assert.equal(DT.face(null, REGISTRY), 'platinum');
  assert.equal(DT.themeName('solaris', DT.parseRegistry(REGISTRY)), 'platinum');
  assert.equal(DT.face('solaris', REGISTRY), 'platinum');
  assert.equal(DT.face('', REGISTRY), 'platinum');
  assert.equal(DT.face('WIN98', REGISTRY), 'platinum', 'ids are matched exactly');
  // A registry without a platinum entry: the name still resolves to platinum, which has no entry.
  assert.equal(DT.face('solaris', JSON.stringify([{ id: 'win98', pyxampFace: 'winamp' }])), 'platinum');
});

test('an entry with no or an unknown pyxampFace gives the Platinum face', () => {
  assert.equal(DT.face('beos', REGISTRY), 'platinum');
  assert.equal(DT.face('amiga', REGISTRY), 'platinum');
  // The registry as mountainous ships it before the field lands: win98 has no pyxampFace.
  const before = JSON.stringify([{ id: 'platinum' }, { id: 'win98' }]);
  assert.equal(DT.face('win98', before), 'platinum');
});

test('a registry that cannot be read falls back to the id: win98 is Winamp, anything else Platinum', () => {
  for (const bad of [null, undefined, '', 'not json', '{"id":"win98"}', '42']) {
    assert.equal(DT.parseRegistry(bad), null, String(bad));
    assert.equal(DT.face('win98', bad), 'winamp', String(bad));
    assert.equal(DT.face('platinum', bad), 'platinum', String(bad));
    assert.equal(DT.face('solaris', bad), 'platinum', String(bad));
    assert.equal(DT.face(null, bad), 'platinum', String(bad));
  }
  assert.deepEqual(plain(DT.parseRegistry('[1, null, {"id":"win98","pyxampFace":"winamp"}, []]')), [{ id: 'win98', pyxampFace: 'winamp' }]);
});

test('appearance: light, dark grey, and extra dark only in dark mode', () => {
  assert.equal(DT.appearance('light\n', 'off', '').appearance, 'light');
  assert.equal(DT.appearance('light', 'on', '').appearance, 'light', 'extra dark applies only in dark mode');
  assert.equal(DT.appearance('dark', 'off', '').appearance, 'grey');
  assert.equal(DT.appearance('dark', null, '').appearance, 'grey', 'a missing theme-extra file is off');
  assert.equal(DT.appearance('dark', 'on\n', '').appearance, 'black');
  assert.equal(DT.appearance('dark', 'yes', '').appearance, 'grey');
  assert.equal(DT.appearance(null, null, '').mode, 'dark', 'Theme.qml starts in dark');
  assert.equal(DT.appearance('sepia', null, 'light').mode, 'light', 'an unknown value keeps the mode shown');
  assert.deepEqual(plain(DT.appearance('dark', 'on', '')), { mode: 'dark', extraDark: true, appearance: 'black' });
});

// ---------- sentence case ----------
test('the LCD turns the model\'s upper-case messages into sentence case and keeps titles', () => {
  const names = ['Living Room', 'Firefox'];
  const cases = [
    ['SPEED: 1.5X', 'Speed: 1.5x'],
    ['BACK TO PYXAMP: STAYS PAUSED', 'Back to Pyxamp: stays paused'],
    ['-15 S', '-15 s'],
    ['EQ: 1KHZ: +6.0 DB', 'EQ: 1 kHz: +6.0 dB'],
    ['PLAYS ON LIVING ROOM', 'Plays on Living Room'],
    ['NO EQ ON LIVING ROOM', 'No EQ on Living Room'],
    ['PYXAMP  ***  NOTHING PLAYS. PRESS J OR L TO FIND SOMETHING', 'Pyxamp  ***  Nothing plays. Press J or L to find something'],
    ['Firefox: Khruangbin - Tiny Desk (22:32)  ***  PYXAMP PAUSED, RESUMES IF THIS STOPS BY 1:26', 'Firefox: Khruangbin - Tiny Desk (22:32)  ***  Pyxamp paused, resumes if this stops by 1:26'],
    ['NEXT: Chromium does not offer this', 'Next: Chromium does not offer this'],
    ['PLAY REFUSED: the player changed', 'Play refused: the player changed'],
    ['2. Stereolab - Miss Modular (4:51)', '2. Stereolab - Miss Modular (4:51)'],
    ['', ''],
  ];
  for (const [a, b] of cases) assert.equal(C.sentence(a, names), b);
  assert.equal(C.title('OSSICLE UP NEXT'), 'Ossicle up next');
  assert.equal(C.title('SAVE EQ PRESET'), 'Save EQ preset');
});

// ---------- fit ----------
const want = (o = {}) => ({ dbl: false, eq: true, pl: true, shade: false, plN: 3, ...o });
test('Platinum fit: 300 x 515 sp; folded sections keep their 17-sp header; shorter playlist first', () => {
  const fhd = { width: 1920, height: 1080, scale: 1 };
  const p = F.plan(fhd, want(), 'platinum');
  assert.deepEqual([p.k, p.physW, p.physH, p.plN], [1, 300, 515, 3]);
  assert.equal(F.plan(fhd, want({ eq: false }), 'platinum').spH, 515 - 105, 'EQ folds by exactly 105 sp');
  assert.equal(F.plan(fhd, want({ eq: false, pl: false }), 'platinum').spH, 150 + 17 + 17 + 1);
  assert.equal(F.plan(fhd, want({ shade: true, eq: false, pl: false }), 'platinum').spH, 18 + 17 + 17 + 1);
  const myoko = F.plan({ width: 2880, height: 1800, scale: 1.5 }, want(), 'platinum');
  assert.deepEqual([myoko.k, myoko.physW, myoko.physH], [2, 600, 1030]);
  // The usable height under a 28-px bar: 1080p with Ctrl+D loses one step (brief: 600 x 966, 8 rows).
  const under = F.plan({ width: 1920, height: 1080 - 28, scale: 1 }, want({ dbl: true }), 'platinum');
  assert.deepEqual([under.k, under.physH, 4 + 2 * under.plN], [2, 966, 8]);
  const m4 = F.plan({ width: 2880, height: 1800 - 42, scale: 1.5 }, want({ dbl: true }), 'platinum');
  assert.deepEqual([m4.k, m4.physW, m4.physH, 4 + 2 * m4.plN], [4, 1200, 1676, 4]);
  // The Winamp face is unchanged.
  assert.deepEqual([F.plan(fhd, want()).physW, F.plan(fhd, want()).physH], [275, 435]);
});

// ---------- library outline ----------
const page = (items) => ({ status: 'ready', items });
function libCtx(path, sel, pages) {
  const sources = [
    { source: 'pyxis', label: 'Pyxis', status: { _tag: 'ready' }, groups: [{ id: 'albums', label: 'Albums', kind: 'albums', count: 1284 }, { id: 'hot', label: 'Hot albums', kind: 'albums', count: 12 }] },
    { source: 'jellyfin', label: 'Jellyfin', status: { _tag: 'unavailable', message: 'connection refused' }, groups: [] },
    { source: 'ossicle', label: 'Ossicle', status: { _tag: 'ready' }, groups: [{ id: 'books', label: 'Books', kind: 'books', count: 3 }] },
  ];
  return {
    snap: { own: null, queue: [], conflicts: [], library: { _tag: 'sources', sources } }, main: { mode: 'none' }, view: 'library',
    albumMode: false, albumOpen: '', albumSel: 0, sel: '', lib: { path, sel }, pages: pages || {}, jump: { text: '', sel: 0, status: 'idle', items: [] },
    prompt: null, conflictPick: 0, info: { empty: '', rows: [] }, caret: '_',
  };
}

test('outline: the root shows the three sources; only the deepest level carries model indices', () => {
  const o = O.rows(libCtx([], 0));
  assert.deepEqual(plain(o.map((r) => [r.row.l, r.row.r, r.tri, r.at])), [['Pyxis', '2 groups', 'closed', 0], ['Jellyfin', 'unreachable', null, 1], ['Ossicle', '1 group', 'closed', 2]]);
});

test('outline: every open level stays visible, with the deepest level\'s rows in model order', () => {
  const P = load('playlist.js');
  const albums = [{ source: 'pyxis', id: 'a1', kind: 'album', title: 'Dots and Loops', subtitle: 'Stereolab', year: 1997, openable: true, playable: true },
                  { source: 'pyxis', id: 'a2', kind: 'album', title: 'Emperor Tomato Ketchup', subtitle: 'Stereolab', openable: true, playable: true }];
  const path = [{ source: 'pyxis', label: 'Pyxis' }, { source: 'pyxis', node: 'albums', label: 'Albums', kind: 'albums' }];
  const ctx = libCtx(path, 1, { [P.pageKey('pyxis', 'albums')]: page(albums) });
  const o = O.rows(ctx);
  assert.deepEqual(plain(o.map((r) => [r.depth, r.row.l, r.tri, r.at])), [
    [0, 'Pyxis', 'open', -1],
    [1, 'Albums', 'open', -1],
    [2, 'Stereolab - Dots and Loops (1997)', 'closed', 0],
    [2, 'Stereolab - Emperor Tomato Ketchup', 'closed', 1],
    [1, 'Hot albums', 'closed', -1],
    [0, 'Jellyfin', null, -1],
    [0, 'Ossicle', 'closed', -1],
  ]);
  const m = P.model(ctx);
  assert.deepEqual(plain(o.filter((r) => r.at >= 0).map((r) => r.row.l)), plain(m.rows.map((r) => r.l)), 'the live level is the model\'s rows');
  assert.equal(O.selIndex(o, m.sel), 3, 'the model\'s selection, as drawn');
  assert.deepEqual(plain([o[4].level, o[4].idx]), [1, 1], 'a higher row knows its level and index (a click folds below it)');
});

test('outline: a level opened from elsewhere (the jump box) keeps an open row', () => {
  const P = load('playlist.js');
  const path = [{ source: 'pyxis', label: 'Pyxis' }, { source: 'pyxis', node: 'a9', label: 'Mars Audiac Quintet', kind: 'album' }];
  const ctx = libCtx(path, 0, { [P.pageKey('pyxis', 'a9')]: page([{ source: 'pyxis', id: 't1', kind: 'track', title: 'Three-Dee Melodie', playable: true }]) });
  const o = O.rows(ctx);
  assert.deepEqual(plain(o.map((r) => [r.depth, r.row.l, r.tri, r.at]).slice(0, 5)), [
    [0, 'Pyxis', 'open', -1], [1, 'Albums', 'closed', -1], [1, 'Hot albums', 'closed', -1], [1, 'Mars Audiac Quintet', 'open', -1], [2, '1. Three-Dee Melodie', null, 0],
  ]);
});
