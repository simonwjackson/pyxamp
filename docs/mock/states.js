// State catalogue. Each state: group, id, label, set(S) on a fresh defaults(), hold time t (s), crop for the sheets.
// crop: main | eq | pl | full | bbox (bbox = surface plus menus, tooltips and desktop props)
'use strict';
const STATES = [];
const GROUPS = {
  1: 'Music from Pyxis, as host', 2: 'Pyxis as console', 3: 'Spoken word from Ossicle', 4: 'Jellyfin music and video hand-off',
  5: 'Followed foreign player', 10: 'Pass 2: album mode, Pyxis playlists, prompt line, stations, Ossicle conflict, sponsor skip, failed track', 6: 'Playlist section: queue, library view, jump box, item info', 7: 'EQ section', 8: 'Edge states',
};
function st(g, id, label, set, o = {}) { STATES.push({ g, id, label, set: set || (() => {}), t: o.t != null ? o.t : 4, crop: o.crop || 'main', k: o.k, keep: !!o.keep }); }
const at = (S, t) => { S.posT = 0; return S; };
const ossicle = (S, cur = 1) => { S.src = 'ossicle'; S.queue = OSSICLE_QUEUE.map(x => ({ ...x })); S.cur = cur; S.pos0 = S.queue[cur].pos || 0; S.speed = 1.5; S.sel = [cur]; S.eq.bands = [-6, -4, -1, 2, 4, 4, 3, 1, -2, -4]; S.eq.auto = true; };
const jelly = (S) => { S.src = 'jellyfin'; S.queue = JELLYFIN_QUEUE.map(x => ({ ...x })); S.cur = 0; S.pos0 = 217; S.sel = [0]; };
const follow = (S, waited = 34) => { S.follow = { ...FOREIGN, waited, t0: 0, playing: true }; S.play = 'pause'; S.pos0 = 151; S.props = [{ id: 'ff', x: S.at.x + W + 14, y: S.at.y, w: 330, h: 230, title: 'Khruangbin: Tiny Desk Concert - YouTube - Mozilla Firefox', video: ['#3a2a1a', '#120a06'], caption: 'YouTube, playing in a Firefox tab', bar: '#2b2a33' }]; };
const flashAt = (S, text, t) => { S.flash = { text, t0: t - 0.3, until: t + 1 }; };

// 1. Pyxis host
st(1, 'h-playing', 'Playing, time elapsed, spectrum, 320 kbps 44 kHz stereo, repeat all, shuffle off');
st(1, 'h-paused', 'Paused: the clock blinks (lit frame shown)', S => { S.play = 'pause'; S.pos0 = 97; }, { t: 4 });
st(1, 'h-paused-dark', 'Paused: the clock blinks (dark frame)', S => { S.play = 'pause'; S.pos0 = 97; }, { t: 4.6 });
st(1, 'h-stopped', 'Stopped: no clock, no position thumb, no fidelity', S => { S.play = 'stop'; });
st(1, 'h-long-0', 'Long title: the marquee at its start', S => { S.queue[2] = { ...LONG_TRACK }; S.marqueeT0 = 0; }, { t: 0.05, keep: true });
st(1, 'h-long-1', 'Long title: the marquee 5.5 s later', S => { S.queue[2] = { ...LONG_TRACK }; S.marqueeT0 = 0; }, { t: 5.5, keep: true });
st(1, 'h-long-2', 'Long title: the "  ***  " separator passing', S => { S.queue[2] = { ...LONG_TRACK }; S.marqueeT0 = 0; }, { t: 16.7, keep: true });
st(1, 'h-remaining', 'Time remaining (Ctrl+T or click the clock)', S => { S.timeMode = 'remaining'; });
st(1, 'h-lossless', 'Lossless: the kbps box shows the bit depth ("24 bit 96 kHz")', S => { S.queue[2] = { ...LOSSLESS_TRACK }; });
st(1, 'h-osc', 'Oscilloscope', S => { S.vis = 'osc'; });
st(1, 'h-visoff', 'Visualizer off', S => { S.vis = 'off'; });
st(1, 'h-rep-off', 'Repeat off', S => { S.repeat = 'off'; });
st(1, 'h-rep-all', 'Repeat all', S => { S.repeat = 'all'; });
st(1, 'h-rep-one', 'Repeat one track (new third state: "1" in the loop)', S => { S.repeat = 'one'; });
st(1, 'h-shuf-on', 'Shuffle on', S => { S.shuffle = true; });
st(1, 'h-shuf-off', 'Shuffle off, repeat off', S => { S.shuffle = false; S.repeat = 'off'; });
st(1, 'h-repflash', 'R pressed: the marquee confirms the mode', S => { S.repeat = 'one'; flashAt(S, 'REPEAT: ONE TRACK', 4); });
st(1, 'h-shade', 'Windowshade (Alt+W)', S => { S.shade = true; }, { crop: 'main' });
st(1, 'h-full', 'The whole surface: main, EQ, playlist', null, { crop: 'full' });

// 2. Console
const consoleSet = (S, dev = 'living') => { S.role = 'console'; S.device = dev; S.queue[2].kbps = 320; };
st(2, 'c-playing', 'Console: plays on "Living room" Sonos. Marquee leads with the device; vis is a flat line', S => consoleSet(S), { t: 1.2 });
st(2, 'c-playing-later', 'Same, 6 s later: the marquee has scrolled', S => consoleSet(S), { t: 6.2, keep: true });
st(2, 'c-kitchen-mono', 'Console on "Kitchen" (a single Sonos One): the mono light is lit', S => { consoleSet(S, 'kitchen'); S.queue[2].ch = 1; }, { t: 1.2 });
st(2, 'c-paused', 'Console, the Sonos session is paused', S => { consoleSet(S); S.play = 'pause'; }, { t: 4 });
st(2, 'c-menu', 'O menu > Play on, with "Living room" chosen', S => { consoleSet(S); S.menu = { kind: 'O', x: 6, y: 13, hover: 0, sub: 0, subHover: 1 }; }, { crop: 'bbox', t: 1.2 });
st(2, 'c-menu-host', 'O menu > Play on from host mode (This computer)', S => { S.menu = { kind: 'O', x: 6, y: 13, hover: 0, sub: 0, subHover: 0 }; }, { crop: 'bbox' });
st(2, 'c-switch', 'Just after picking Living room: the marquee confirms', S => { consoleSet(S); flashAt(S, 'PLAYS ON LIVING ROOM', 1.2); }, { t: 1.2 });
st(2, 'c-full', 'Console, whole surface: EQ says it has no effect on the Sonos', S => consoleSet(S), { crop: 'full', t: 1.2 });

// 3. Spoken word
st(3, 's-podcast', 'Podcast: speed slider replaces balance, skip buttons, shuffle and repeat dimmed, mono', S => ossicle(S, 1), { t: 1 });
st(3, 's-book', 'Narrated book at 6:02:40: clock shows hours:minutes with an H', S => ossicle(S, 3), { t: 1 });
st(3, 's-speed', 'Dragging the speed slider: "SPEED: 1.5X"', S => { ossicle(S, 1); S.drag = { id: 'bal' }; flashAt(S, 'SPEED: 1.5X', 1); }, { t: 1 });
st(3, 's-back', 'Z or the back button: "-15 S"', S => { ossicle(S, 1); S.pressed = 'prev'; flashAt(S, '-15 S', 1); }, { t: 1 });
st(3, 's-fwd', 'B or the forward button: "+30 S"', S => { ossicle(S, 1); S.pressed = 'next'; flashAt(S, '+30 S', 1); }, { t: 1 });
st(3, 's-full', 'Up next: two played items (tick, dimmed), a part-played book shows time left', S => ossicle(S, 1), { crop: 'full', t: 1 });
st(3, 's-rowmenu', 'Right-click an item: "Mark as played"', S => { ossicle(S, 1); S.sel = [2]; S.menu = { kind: 'row', idx: 2, x: 120, y: 2 * 116 + 20 + 2 * 13 + 8, hover: 1 }; }, { crop: 'bbox', t: 1 });
st(3, 's-misc', 'MISC menu: "Mark as played"', S => { ossicle(S, 1); S.menu = { kind: 'pl-misc', x: 101, y: 0, hover: 1, up: 1 }; }, { crop: 'bbox', t: 1 });
st(3, 's-info', 'Item info for the episode, inside the playlist section', S => { ossicle(S, 1); S.info = true; }, { crop: 'full', t: 1 });

// 4. Jellyfin
st(4, 'j-music', 'Jellyfin music: lossless 16 bit 44 kHz', S => jelly(S), { crop: 'full', t: 2 });
st(4, 'j-videos', 'Library: Jellyfin > Videos. Each row says it opens in mpv', S => { jelly(S); S.plView = 'lib'; S.lib = [{ key: 'jellyfin', name: 'Jellyfin' }, { name: 'Videos' }]; S.libSel = 0; S.tip = { x: 120, y: 2 * 116 + 20 + 13 * 2 + 4, text: 'Opens in mpv, outside Pyxamp' }; }, { crop: 'bbox', t: 2 });
st(4, 'j-handoff', 'Enter on "Alien (1979)": mpv opens beside Pyxamp; Pyxamp follows it like any player', S => { jelly(S); S.plView = 'lib'; S.lib = [{ key: 'jellyfin', name: 'Jellyfin' }, { name: 'Videos' }]; handoff(S, 'Alien (1979)', 0); S.follow = { app: 'MPV', site: 'JELLYFIN VIDEO', title: 'Alien (1979)', len: 7020, pos: 2, waited: 2, t0: 0, playing: true }; S.play = 'pause'; S.flash = { text: 'ALIEN (1979): OPENED IN MPV', t0: 0, until: 3 }; }, { crop: 'bbox', t: 1 });

// 5. Followed foreign player
st(5, 'f-follow', 'Following Firefox (YouTube). Amber LCD = not Pyxamp\'s own audio. Pyxamp yielded (paused)', S => follow(S), { crop: 'bbox', t: 2 });
st(5, 'f-follow-scroll', 'The marquee scrolls on to say Pyxamp is paused and when it resumes', S => follow(S), { crop: 'main', t: 13, keep: true });
st(5, 'f-follow-long', 'After more than 2:00 the marquee says Pyxamp stays paused', S => follow(S, 150), { crop: 'main', t: 13, keep: true });
st(5, 'f-shade', 'Following, in windowshade: the time is amber too', S => { follow(S); S.shade = true; }, { crop: 'main', t: 2 });
st(5, 'f-eq', 'Following: the EQ says it has no effect on Firefox', S => { follow(S); S.plOpen = false; }, { crop: 'full', t: 2 });
st(5, 'f-back-1', 'Hand-back 1/3: Firefox still plays (amber)', S => { follow(S, 70); }, { crop: 'main', t: 2 });
st(5, 'f-back-2', 'Hand-back 2/3: Firefox paused after 1:12, within 2:00. LCD turns green, marquee says so', S => { S.play = 'play'; S.pos0 = 151; S.posT = 2; S.flash = { text: 'BACK TO PYXAMP: RESUMING', t0: 2, until: 4 }; }, { crop: 'main', t: 2.1 });
st(5, 'f-back-3', 'Hand-back 3/3: Pyxamp plays on from where it yielded', S => { S.play = 'play'; S.pos0 = 151; S.posT = 2; S.marqueeT0 = 4; }, { crop: 'main', t: 4.5 });
st(5, 'f-back-stay', 'Hand-back after a long interruption (over 2:00): green, stays paused', S => { S.play = 'pause'; S.pos0 = 151; S.flash = { text: 'BACK TO PYXAMP: STAYS PAUSED', t0: 2, until: 4 }; }, { crop: 'main', t: 2.1 });

// 6. Playlist section
const lib = (S, path, sel = 0) => { S.plView = 'lib'; S.lib = path; S.libSel = sel; };
st(6, 'p-queue', 'Queue (Pyxis): the current item is white, the selection is blue', S => { S.sel = [5]; }, { crop: 'pl' });
st(6, 'p-lib-root', 'Library view, root: the three sources', S => lib(S, []), { crop: 'pl' });
st(6, 'p-lib-pyxis', 'Pyxis: 5 groups (as Pyxis reports today)', S => lib(S, [{ key: 'pyxis', name: 'Pyxis' }]), { crop: 'pl' });
st(6, 'p-lib-pyxis9', 'Pyxis reporting 9 groups (placements, signals, tags): same code, list grows', S => { S.libPyxis = 'pyxis9'; lib(S, [{ key: 'pyxis9', name: 'Pyxis' }], 7); }, { crop: 'pl' });
st(6, 'p-lib-ossicle', 'Ossicle: 3 groups', S => lib(S, [{ key: 'ossicle', name: 'Ossicle' }]), { crop: 'pl' });
st(6, 'p-lib-albums', 'Pyxis > Albums (1,284; scrolls)', S => lib(S, [{ key: 'pyxis', name: 'Pyxis' }, { name: 'Albums' }], 0), { crop: 'pl' });
st(6, 'p-lib-album', 'Pyxis > Albums > Dots and Loops', S => lib(S, [{ key: 'pyxis', name: 'Pyxis' }, { name: 'Albums' }, { name: 'Dots and Loops' }], 6), { crop: 'pl' });
st(6, 'p-lib-books', 'Ossicle > Books: progress per book', S => lib(S, [{ key: 'ossicle', name: 'Ossicle' }, { name: 'Books' }], 0), { crop: 'pl' });
st(6, 'p-jump-open', 'J: the jump box opens over the list', S => { S.plView = 'jump'; S.jump = ''; }, { crop: 'pl', t: 4 });
st(6, 'p-jump', 'Jump box: "broadcast" finds items in all three sources', S => { S.plView = 'jump'; S.jump = 'broadcast'; S.jumpSel = 0; }, { crop: 'pl' });
st(6, 'p-jump-cross', 'A result from another source: the hint says the active source pauses', S => { S.plView = 'jump'; S.jump = 'broadcast'; S.jumpSel = 4; }, { crop: 'pl' });
st(6, 'p-jump-none', 'Jump box: nothing found', S => { S.plView = 'jump'; S.jump = 'zzyzx'; }, { crop: 'pl' });
st(6, 'p-info', 'Item info (I), drawn inside the playlist section', S => { S.info = true; }, { crop: 'pl' });
st(6, 'p-add', 'ADD menu', S => { S.menu = { kind: 'pl-add', x: 14, y: 0, hover: 0, up: 1 }; }, { crop: 'bbox' });

// 7. EQ
st(7, 'e-default', 'EQ on, a user curve, preamp 0', null, { crop: 'eq' });
st(7, 'e-off', 'EQ off: curve and sliders dim', S => { S.eq.on = false; }, { crop: 'eq' });
st(7, 'e-drag', 'Dragging 1K: the main marquee shows the value', S => { S.eq.bands[4] = 6; S.drag = { id: 'band-4' }; flashAt(S, 'EQ: 1KHZ: +6.0 DB', 4); }, { crop: 'full' });
st(7, 'e-presets', 'PRESETS menu', S => { S.menu = { kind: 'presets', x: 217, y: 116 + 30, hover: 4 }; }, { crop: 'bbox' });
st(7, 'e-auto', 'AUTO on with spoken word: the Voice preset', S => { ossicle(S, 1); }, { crop: 'eq', t: 1 });
st(7, 'e-console', 'Console: the EQ has no effect on the Sonos', S => consoleSet(S), { crop: 'eq' });

// 8. Edge
st(8, 'x-unreach-lib', 'Jellyfin unreachable: the library root says so', S => { S.unreachable = 'jellyfin'; lib(S, [], 1); }, { crop: 'pl' });
st(8, 'x-unreach-play', 'Jellyfin unreachable while it is the active source', S => { jelly(S); S.unreachable = 'jellyfin'; S.err = 'JELLYFIN UNREACHABLE: CONNECTION REFUSED. RETRYING IN 30 S'; }, { crop: 'main', t: 1 });
st(8, 'x-unreach-jump', 'Jump box with Jellyfin unreachable', S => { S.unreachable = 'jellyfin'; S.plView = 'jump'; S.jump = 'broadcast'; S.jumpSel = 0; }, { crop: 'pl' });
st(8, 'x-empty', 'Nothing queued', S => { S.queue = []; S.cur = 0; S.play = 'stop'; S.sel = []; }, { crop: 'full', t: 1 });
st(8, 'x-long', 'A queue of 1,240 items, scrolled to the current one', S => { S.queue = longQueue(1240); S.cur = 612; S.sel = [612]; S.scroll = 606; S.pos0 = 40; }, { crop: 'pl' });
st(8, 'x-stream-err', 'Stream error, the Winamp way: in the marquee, red status light', S => { S.err = 'STREAM ERROR: PYXIS RETURNED HTTP 503. X TO RETRY'; }, { crop: 'main', t: 1 });
st(8, 'x-stream-err-scroll', 'The error text scrolls like a title', S => { S.err = 'STREAM ERROR: PYXIS RETURNED HTTP 503. X TO RETRY'; }, { crop: 'main', t: 6, keep: true });
st(8, 'x-buffering', 'Buffering a slow stream', S => { S.buffering = 45; }, { crop: 'main', t: 1 });
st(8, 'x-desktop', 'A: "on the desktop": a normal window covers Pyxamp', S => { S.onTop = false; S.props = [{ id: 'term', overlap: true, x: S.at.x + 150, y: S.at.y + 60, w: 280, h: 150, title: 'ghostty: ~/code', lines: ['$ nixos-rebuild switch --flake .#myoko', 'building the system configuration...', 'activating the configuration...'], bg: '#101418', fg: '#b8d0b8' }]; }, { crop: 'bbox' });
st(8, 'x-ontop', 'A: "on top": the same window, under Pyxamp (A lit)', S => { S.onTop = true; S.props = [{ id: 'term', overlap: true, x: S.at.x + 150, y: S.at.y + 60, w: 280, h: 150, title: 'ghostty: ~/code', lines: ['$ nixos-rebuild switch --flake .#myoko', 'building the system configuration...', 'activating the configuration...'], bg: '#101418', fg: '#b8d0b8' }]; }, { crop: 'bbox' });

// 10. Pass 2 (decided after pass 1)
const mixed = (S) => { S.queue = PYXIS_MIXED.map(x => ({ ...x })); S.cur = 3; S.sel = [3]; S.pos0 = 83; };
const ossMixed = (S, cur = 1) => { ossicle(S); S.queue = OSSICLE_MIXED.map(x => ({ ...x })); S.cur = cur; S.pos0 = S.queue[cur].pos || 0; S.sel = [cur]; };
const core = (S, path, sel = 0) => { S.libPyxis = 'pyxisCore'; S.plView = 'lib'; S.lib = [{ key: 'pyxisCore', name: 'Pyxis' }].concat(path.map(name => ({ name }))); S.libSel = sel; };
const plTop = 116 * 2 + 20; // y of the first list row when main and EQ are open
st(10, 'am-tracks', 'Album mode off: the mixed Pyxis queue, one row per track (27 tracks)', S => mixed(S), { crop: 'pl' });
st(10, 'am-pyxis', 'Album mode on (title bar record button, G, or MISC): one row per album; the playing album is white with a play mark; single tracks stay single', S => { mixed(S); S.albumMode = true; S.albumSel = 0; }, { crop: 'pl' });
st(10, 'am-pyxis-sel', 'Album mode: Down selects the next album; partial albums say "5 of 17 tracks"', S => { mixed(S); S.albumMode = true; S.albumSel = 2; }, { crop: 'pl' });
st(10, 'am-pyxis-open', 'Enter opens an album: its queued tracks, numbered by queue position; Esc goes back', S => { mixed(S); S.albumMode = true; S.albumOpen = 0; S.sel = [3]; }, { crop: 'pl' });
st(10, 'am-full', 'Album mode, whole surface: the title bar button looks pressed', S => { mixed(S); S.albumMode = true; }, { crop: 'full' });
st(10, 'am-ossicle', 'Ossicle up next by show: runs of one show become one row; the book stays one row', S => { ossMixed(S); S.albumMode = true; S.albumSel = 1; }, { crop: 'pl', t: 1 });
st(10, 'am-ossicle-open', 'Enter on "99% Invisible": its three episodes (one played)', S => { ossMixed(S); S.albumMode = true; S.albumOpen = 2; S.sel = [4]; }, { crop: 'pl', t: 1 });
st(10, 'am-misc', 'MISC menu for Pyxis: "Album mode" and "Save queue as playlist..."', S => { mixed(S); S.albumMode = true; S.menu = { kind: 'pl-misc', x: 101, y: 0, hover: 7, up: 1 }; }, { crop: 'bbox' });
st(10, 'pl-groups', 'Pyxis groups as the core reports them: album groups first (placements, Hot, tags), a rule, then Bookmarks, Stations, Playlists', S => core(S, []), { crop: 'pl' });
st(10, 'pl-root', 'Library root: Pyxis reports 10 groups (the pass-1 boards keep the old 5-group list)', S => { S.libPyxis = 'pyxisCore'; S.plView = 'lib'; S.lib = []; }, { crop: 'pl' });
st(10, 'pl-playlists', 'Pyxis > Playlists (last group)', S => core(S, ['Playlists'], 0), { crop: 'pl' });
st(10, 'pl-open', 'A playlist opened in the library view: Enter plays from it; right-click > Queue queues', S => core(S, ['Playlists', 'Sunday morning'], 0), { crop: 'pl' });
st(10, 'pl-save-prompt', 'MISC > "Save queue as playlist...": the prompt line over the playlist section', S => { mixed(S); S.prompt = { kind: 'savepl', title: 'SAVE AS PLAYLIST', label: 'Name', right: '27 tracks', hint: 'Enter: save in Pyxis   Esc: cancel', text: 'Sunday afternoon' }; }, { crop: 'pl' });
st(10, 'pl-saved', 'Enter: saved; the marquee confirms', S => { mixed(S); S.flash = { text: 'SAVED AS PLAYLIST: SUNDAY AFTERNOON', t0: 3.99, until: 7 }; }, { crop: 'main' });
st(10, 'pl-addto', 'Right-click a track > "Add to playlist" > the Pyxis playlists', S => { mixed(S); S.sel = [5]; S.menu = { kind: 'row', idx: 5, x: 110, y: plTop + 5 * 13 + 8, hover: 3, sub: 3, subHover: 0 }; }, { crop: 'bbox' });
st(10, 'pl-addto-lib', 'The same from a track in the library view', S => { core(S, ['Playlists', 'Sunday morning'], 2); S.menu = { kind: 'librow', idx: 2, x: 110, y: plTop + 13 + 2 * 13 + 8, hover: 3, sub: 3, subHover: 4 }; }, { crop: 'bbox' });
st(10, 'pr-eq', 'Presets > "Save as new preset...": the same prompt line, over the playlist section', S => { S.prompt = { kind: 'eq', title: 'SAVE EQ PRESET', label: 'Name', right: '10 bands + preamp', hint: 'Enter: save   Esc: cancel', text: 'Late night' }; }, { crop: 'full' });
st(10, 'pr-eq-folded', 'Same with the playlist folded: the prompt unfolds it, and it folds again after Enter or Esc', S => { S.plOpen = true; S.prompt = { kind: 'eq', title: 'SAVE EQ PRESET', label: 'Name', right: '10 bands + preamp', hint: 'Enter: save   Esc: cancel (folds again)', text: 'Lat', refold: true }; }, { crop: 'full' });
st(10, 'st-song', 'Right-click a song > "Start station" (the song is the seed)', S => { S.sel = [0]; S.menu = { kind: 'row', idx: 0, x: 110, y: plTop + 8, hover: 4 }; }, { crop: 'bbox' });
st(10, 'st-artist', 'Right-click an artist in the jump box > "Start station"', S => { S.plView = 'jump'; S.jump = 'stereolab'; S.jumpSel = 0; S.menu = { kind: 'jumprow', idx: 0, x: 110, y: plTop + 13 + 8, hover: 3 }; }, { crop: 'bbox' });
st(10, 'st-album', 'Right-click an album in the library > "Start station from Stereolab"', S => { core(S, ['Collection'], 0); S.menu = { kind: 'librow', idx: 0, x: 110, y: plTop + 13 + 8, hover: 3 }; }, { crop: 'bbox' });
st(10, 'st-started', 'The station plays: the queue is the station\'s, the marquee says the seed', S => { startStation(S, 'Brakhage (song)', 3.6); }, { crop: 'full' });
st(10, 'cf-row', 'Ossicle position conflict: one row with both positions and devices; the display says POSITION CONFLICT', S => { ossMixed(S, 1); S.play = 'pause'; S.pos0 = 750; S.conflict = { idx: 1, pick: 0, opts: [{ dev: 'This laptop', pos: 750 }, { dev: 'Pixel', pos: 2402 }] }; }, { crop: 'full' });
st(10, 'cf-pick', 'Right arrow picks Pixel; Enter keeps it', S => { ossMixed(S, 1); S.play = 'pause'; S.pos0 = 750; S.conflict = { idx: 1, pick: 1, opts: [{ dev: 'This laptop', pos: 750 }, { dev: 'Pixel', pos: 2402 }] }; }, { crop: 'pl' });
st(10, 'cf-kept', 'Kept: playback starts at Pixel\'s 40:02 and the choice syncs', S => { ossMixed(S, 1); S.pos0 = 2402; S.posT = 3.8; flashAt(S, 'KEPT PIXEL 40:02 (SYNCED)', 4); }, { crop: 'main' });
st(10, 'sp-skip', 'Sponsor segment skipped (Ossicle sponsorSkip on): "SKIPPED SPONSOR"; segments are dim gold marks on the position bar', S => { ossMixed(S, 4); S.pos0 = 151; S.posT = 3.8; S.sponsor = [[60, 150], [1180, 1270], [2010, 2060]]; flashAt(S, 'SKIPPED SPONSOR', 4); }, { crop: 'main' });
st(10, 'ft-retry1', 'Failed track: retry 1 in 2 s (red status light, row says "retrying")', S => { mixed(S); S.fail = { idx: 3, t0: 0 }; }, { crop: 'full', t: 0.4 });
st(10, 'ft-retry2', 'Retry 1 failed: retry 2 in 10 s', S => { mixed(S); S.fail = { idx: 3, t0: 0 }; }, { crop: 'main', t: 5.2 });
st(10, 'ft-skipped', 'After the second retry: skipped, the next track plays, the failed row is red', S => { mixed(S); S.fail = { idx: 3, t0: 0 }; }, { crop: 'full', t: 12.5 });

function applyState(S, id) { const s = STATES.find(x => x.id === id); if (!s) return null; s.set(S); return s; }
