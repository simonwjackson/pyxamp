// Fake data only. No live calls.
'use strict';
const T = (m, s) => m * 60 + s;
const H = (h, m, s) => h * 3600 + m * 60 + s;

const PYXIS_QUEUE = [
  { a: 'Boards of Canada', t: 'Roygbiv', len: T(2, 31) },
  { a: 'Broadcast', t: 'Tears in the Typing Pool', len: T(2, 56) },
  { a: 'Stereolab', t: "Cybele's Reverie", len: T(4, 10), kbps: 320, khz: 44, ch: 2 },
  { a: 'Air', t: "La femme d'argent", len: T(7, 11) },
  { a: 'Khruangbin', t: 'María También', len: T(3, 5) },
  { a: 'Portishead', t: 'Roads', len: T(5, 5) },
  { a: 'Cocteau Twins', t: 'Heaven or Las Vegas', len: T(4, 57) },
  { a: 'Massive Attack', t: 'Teardrop', len: T(5, 29) },
  { a: 'Mazzy Star', t: 'Fade into You', len: T(4, 55) },
  { a: 'Beach House', t: 'Space Song', len: T(5, 20) },
  { a: 'Grouper', t: "Heavy Water/I'd Rather Be Sleeping", len: T(3, 1) },
  { a: 'Slowdive', t: 'When the Sun Hits', len: T(4, 45) },
  { a: 'Low', t: 'Lullaby', len: T(9, 47) },
  { a: 'Yo La Tengo', t: 'Autumn Sweater', len: T(6, 19) },
];
const LONG_TRACK = { a: 'Godspeed You! Black Emperor', t: 'Storm (Lift Yr. Skinny Fists Like Antennas to Heaven)', len: T(22, 32), kbps: 256, khz: 44, ch: 2 };
const LOSSLESS_TRACK = { a: 'Floating Points, Pharoah Sanders & LSO', t: 'Movement 6', len: T(4, 19), lossless: true, bits: 24, khz: 96, ch: 2 };

const JELLYFIN_QUEUE = [
  { a: 'Talk Talk', t: 'New Grass', len: T(9, 40), lossless: true, bits: 16, khz: 44, ch: 2 },
  { a: 'Talk Talk', t: 'Myrrhman', len: T(5, 33) },
  { a: 'Talk Talk', t: 'Ascension Day', len: T(6, 0) },
  { a: 'Talk Talk', t: 'After the Flood', len: T(9, 39) },
  { a: 'Talk Talk', t: 'Taphead', len: T(7, 39) },
  { a: 'Talk Talk', t: 'Runeii', len: T(4, 58) },
];

// Ossicle: spoken word. pos = synced resume position, played flag, kind.
const OSSICLE_QUEUE = [
  { kind: 'ep', show: 'The Talk Show', t: '412: The Fall Event, With John Siracusa', len: H(2, 11, 4), played: true },
  { kind: 'ep', show: 'Hardcore History', t: '70: Mania for Subjugation III', len: H(5, 32, 10), pos: H(1, 18, 22), kbps: 64, khz: 44, ch: 1 },
  { kind: 'ep', show: '99% Invisible', t: '580: The Smell of Rain', len: T(41, 12) },
  { kind: 'book', show: 'Andy Weir, read by Ray Porter', t: 'Project Hail Mary', len: H(16, 10, 35), pos: H(6, 2, 40), chapter: 'Chapter 12', kbps: 64, khz: 22, ch: 1 },
  { kind: 'ep', show: 'Radiolab', t: 'The Wubi Effect', len: T(52, 3), played: true },
  { kind: 'ep', show: 'Decoder', t: 'Why every app is becoming a bank', len: T(58, 40) },
];
const OSSICLE_INTERVALS = { back: 15, fwd: 30 };

const DEVICES = [
  { id: 'here', name: 'This computer', kind: 'Pyxamp (host)' },
  { id: 'living', name: 'Living room', kind: 'Sonos Five, stereo pair' },
  { id: 'kitchen', name: 'Kitchen', kind: 'Sonos One' },
  { id: 'office', name: 'Office Mac', kind: 'Pyxis web' },
  { id: 'phone', name: "Simon's phone", kind: 'Pyxis app' },
];

const FOREIGN = { app: 'Firefox', site: 'YouTube', title: 'Khruangbin: Tiny Desk Concert', len: T(18, 32), pos: T(4, 12) };

// Library: groups are whatever the source reports. Nothing below a source name is fixed.
const LIB = {
  pyxis: { name: 'Pyxis', groups: ['Albums', 'Hot albums', 'Playlists', 'Bookmarks', 'Stations'] },
  pyxis9: { name: 'Pyxis', groups: ['Albums', 'Hot albums', 'Playlists', 'Bookmarks', 'Stations', 'Discovery', 'Archive', 'Late night (signal)', 'Tag: shoegaze'] },
  jellyfin: { name: 'Jellyfin', groups: ['Artists', 'Albums', 'Playlists', 'Videos'] },
  ossicle: { name: 'Ossicle', groups: ['Up next', 'Shows', 'Books'] },
};
const COUNTS = { 'pyxis/Albums': 1284, 'pyxis/Hot albums': 37, 'pyxis/Playlists': 22, 'pyxis/Bookmarks': 61, 'pyxis/Stations': 9, 'pyxis/Discovery': 412, 'pyxis/Archive': 2210, 'pyxis/Late night (signal)': 48, 'pyxis/Tag: shoegaze': 73,
  'jellyfin/Artists': 316, 'jellyfin/Albums': 940, 'jellyfin/Playlists': 8, 'jellyfin/Videos': 211, 'ossicle/Up next': 6, 'ossicle/Shows': 24, 'ossicle/Books': 11 };
const PYXIS_ALBUMS = [
  ['Stereolab', 'Dots and Loops', 1997, 11], ['Broadcast', 'Haha Sound', 2003, 12], ['Boards of Canada', 'Music Has the Right to Children', 1998, 17],
  ['Cocteau Twins', 'Heaven or Las Vegas', 1990, 10], ['Air', 'Moon Safari', 1998, 10], ['Portishead', 'Dummy', 1994, 11], ['Grouper', 'Dragging a Dead Deer Up a Hill', 2008, 13],
  ['Khruangbin', 'Con Todo El Mundo', 2018, 10], ['Low', 'Things We Lost in the Fire', 2001, 13], ['Mazzy Star', 'So Tonight That I Might See', 1993, 10],
  ['Slowdive', 'Souvlaki', 1993, 10], ['Yo La Tengo', 'Electr-O-Pura', 1995, 14], ['Beach House', 'Bloom', 2012, 10], ['Massive Attack', 'Mezzanine', 1998, 11],
];
const DOTS_AND_LOOPS = [['Brakhage', T(5, 2)], ['Miss Modular', T(4, 30)], ['The Flower Called Nowhere', T(5, 0)], ['Diagonals', T(4, 59)], ['Prisoner of Mars', T(4, 25)],
  ['Rainbo Conversation', T(4, 39)], ['Refractions in the Plastic Pulse', T(17, 34)], ['Parsec', T(5, 36)], ['Ticker-tape of the Unconscious', T(4, 21)], ['Contronatura', T(10, 29)], ["Cybele's Reverie (bonus)", T(4, 10)]];
const OSSICLE_BOOKS = [
  ['Project Hail Mary', 'Andy Weir, read by Ray Porter', H(16, 10, 35), 0.37], ['The Hobbit', 'J. R. R. Tolkien, read by Andy Serkis', H(10, 25, 0), 1],
  ['Piranesi', 'Susanna Clarke, read by Chiwetel Ejiofor', H(6, 52, 0), 0], ['Dune', 'Frank Herbert, read by a full cast', H(21, 2, 0), 0.08],
  ['The Remains of the Day', 'Kazuo Ishiguro, read by Simon Prebble', H(7, 41, 0), 0], ['Station Eleven', 'Emily St. John Mandel, read by Kirsten Potter', H(10, 59, 0), 0],
  ['A Wizard of Earthsea', 'Ursula K. Le Guin, read by Rob Inglis', H(6, 34, 0), 1], ['Klara and the Sun', 'Kazuo Ishiguro, read by Sura Siu', H(10, 16, 0), 0],
  ['The Left Hand of Darkness', 'Ursula K. Le Guin, read by George Guidall', H(11, 29, 0), 0], ['Children of Time', 'Adrian Tchaikovsky, read by Mel Hudson', H(16, 31, 0), 0],
  ['Recursion', 'Blake Crouch, read by Jon Lindstrom', H(10, 49, 0), 0]];
const JF_VIDEOS = [['Alien (1979)', H(1, 57, 0)], ['Arrival (2016)', H(1, 56, 0)], ['Stop Making Sense (1984)', H(1, 28, 0)], ['Paris, Texas (1984)', H(2, 25, 0)], ['Koyaanisqatsi (1982)', H(1, 26, 0)], ['Stalker (1979)', H(2, 42, 0)]];

// Jump box corpus: what each source returns for a query (fake search).
const CORPUS = [
  { src: 'pyxis', kind: 'track', text: 'Broadcast - Tears in the Typing Pool', len: T(2, 56) },
  { src: 'pyxis', kind: 'album', text: 'Broadcast - Haha Sound (album)', len: T(41, 3) },
  { src: 'pyxis', kind: 'album', text: 'Broadcast - Tender Buttons (album)', len: T(42, 50) },
  { src: 'pyxis', kind: 'station', text: 'Broadcast radio (station)', len: 0 },
  { src: 'jellyfin', kind: 'album', text: 'Broadcast - The Noise Made by People (album)', len: T(44, 2) },
  { src: 'jellyfin', kind: 'track', text: 'Broadcast - Come On Let\'s Go', len: T(3, 25) },
  { src: 'ossicle', kind: 'ep', text: 'Song Exploder - Broadcast: "Pendulum"', len: T(18, 40) },
  { src: 'ossicle', kind: 'ep', text: 'Decoder - Broadcast rights and the streaming wars', len: T(49, 12) },
  { src: 'pyxis', kind: 'artist', text: 'Stereolab (artist)', len: 0 },
  { src: 'pyxis', kind: 'track', text: 'Stereolab - Brakhage', len: T(5, 2) },
  { src: 'pyxis', kind: 'album', text: 'Stereolab - Dots and Loops (album)', len: H(1, 11, 0) },
  { src: 'jellyfin', kind: 'video', text: 'Stop Making Sense (1984) - video, opens in mpv', len: H(1, 28, 0) },
  { src: 'ossicle', kind: 'book', text: 'Project Hail Mary - Andy Weir (book)', len: H(16, 10, 35) },
];

function longQueue(n) { const out = []; for (let i = 0; i < n; i++) { const b = PYXIS_QUEUE[i % PYXIS_QUEUE.length]; out.push({ a: b.a, t: b.t + (i >= PYXIS_QUEUE.length ? ` (${Math.floor(i / PYXIS_QUEUE.length) + 1})` : ''), len: b.len }); } return out; }

// ---------- pass 2 data ----------
// A Pyxis queue that mixes whole albums and single tracks. al = album, yr, an = tracks on the album.
const DNL = [['Brakhage', T(5, 2)], ['Miss Modular', T(4, 30)], ['The Flower Called Nowhere', T(5, 0)], ['Diagonals', T(4, 59)], ['Prisoner of Mars', T(4, 25)],
  ['Rainbo Conversation', T(4, 39)], ['Refractions in the Plastic Pulse', T(17, 34)], ['Parsec', T(5, 36)], ['Ticker-tape of the Unconscious', T(4, 21)], ['Contronatura', T(13, 25)]];
const MHRTC = [['Wildlife Analysis', T(1, 17)], ['An Eagle in Your Mind', T(6, 23)], ['The Color of the Fire', T(1, 45)], ['Telephasic Workshop', T(6, 35)], ['Triangles & Rhombuses', T(1, 50)]];
const HOLV = [['Cherry-coloured Funk', T(3, 13)], ['Pitch the Baby', T(3, 14)], ['Iceblink Luck', T(3, 18)], ['Fifty-fifty Clown', T(3, 10)], ['Heaven or Las Vegas', T(4, 57)],
  ['I Wear Your Ring', T(3, 30)], ['Fotzepolitic', T(3, 30)], ['Wolf in the Breast', T(3, 32)], ['Road, River and Rail', T(3, 21)], ['Frou-frou Foxes in Midsummer Fires', T(5, 37)]];
const PYXIS_MIXED = [].concat(
  DNL.map(([t, len]) => ({ a: 'Stereolab', t, len, al: 'Dots and Loops', yr: 1997, an: 10 })),
  [{ a: 'Broadcast', t: 'Tears in the Typing Pool', len: T(2, 56), al: 'Haha Sound', yr: 2003, an: 12 }],
  MHRTC.map(([t, len]) => ({ a: 'Boards of Canada', t, len, al: 'Music Has the Right to Children', yr: 1998, an: 17 })),
  [{ a: 'Massive Attack', t: 'Teardrop', len: T(5, 29), al: 'Mezzanine', yr: 1998, an: 11 }],
  HOLV.map(([t, len]) => ({ a: 'Cocteau Twins', t, len, al: 'Heaven or Las Vegas', yr: 1990, an: 10 })));
// Ossicle up next with runs of the same show, and a book.
const OSSICLE_MIXED = [
  { kind: 'ep', show: 'The Talk Show', t: '412: The Fall Event, With John Siracusa', len: H(2, 11, 4), played: true },
  { kind: 'ep', show: 'Hardcore History', t: '70: Mania for Subjugation III', len: H(5, 32, 10), pos: H(1, 18, 22), kbps: 64, khz: 44, ch: 1 },
  { kind: 'ep', show: 'Hardcore History', t: '71: Mania for Subjugation IV', len: H(5, 51, 0) },
  { kind: 'ep', show: '99% Invisible', t: '580: The Smell of Rain', len: T(41, 12), played: true },
  { kind: 'ep', show: '99% Invisible', t: '581: The Pool and the Stream', len: T(38, 40) },
  { kind: 'ep', show: '99% Invisible', t: '582: Ten Thousand Doors', len: T(44, 5) },
  { kind: 'book', show: 'Andy Weir, read by Ray Porter', t: 'Project Hail Mary', len: H(16, 10, 35), pos: H(6, 2, 40), chapter: 'Chapter 12', kbps: 64, khz: 22, ch: 1 },
  { kind: 'ep', show: 'The Talk Show', t: '413: Vision Pro, a Year On', len: H(1, 58, 30) },
  { kind: 'ep', show: 'Decoder', t: 'Why every app is becoming a bank', len: T(58, 40) },
];
// The groups the Pyxis core reports, in the core's order: album groups (placements, Hot, tags), then Bookmarks, Stations, Playlists.
const PYXIS_CORE = [
  { l: 'Discovery', n: 412, kind: 'placement' }, { l: 'Collection', n: 1284, kind: 'placement' }, { l: 'Archive', n: 2210, kind: 'placement' }, { l: 'Dismissed', n: 118, kind: 'placement' },
  { l: 'Hot', n: 37, kind: 'signal' }, { l: 'shoegaze', n: 73, kind: 'tag' }, { l: 'late night', n: 48, kind: 'tag' },
  { l: 'Bookmarks', n: 61, kind: 'bookmarks' }, { l: 'Stations', n: 9, kind: 'stations' }, { l: 'Playlists', n: 5, kind: 'playlists' },
];
const PYXIS_PLAYLISTS = [['Sunday morning', 14, H(0, 58, 12)], ['Long drive north', 31, H(2, 9, 40)], ['Run 5k', 12, T(41, 3)], ['For Mum', 9, T(37, 50)], ['Rainy Kyoto', 18, H(1, 12, 9)]];
const SUNDAY_MORNING = [['Cocteau Twins', 'Heaven or Las Vegas', T(4, 57)], ['Mazzy Star', 'Fade into You', T(4, 55)], ['Air', "La femme d'argent", T(7, 11)], ['Beach House', 'Space Song', T(5, 20)],
  ['Grouper', "Heavy Water/I'd Rather Be Sleeping", T(3, 1)], ['Low', 'Lullaby', T(9, 47)], ['Yo La Tengo', 'Autumn Sweater', T(6, 19)], ['Slowdive', 'When the Sun Hits', T(4, 45)],
  ['Broadcast', 'Pendulum', T(4, 34)], ['Khruangbin', 'María También', T(3, 5)], ['Stereolab', 'Diagonals', T(4, 59)], ['Portishead', 'Roads', T(5, 5)], ['Massive Attack', 'Teardrop', T(5, 29)], ['Boards of Canada', 'Roygbiv', T(2, 31)]];
const PYXIS_STATIONS = [['Brakhage (song)', 'yesterday'], ['Broadcast (artist)', 'Oct 19'], ['Slowdive (artist)', 'Oct 2'], ['Teardrop (song)', 'Sep 28']];
const STATION_QUEUE = [{ a: 'Stereolab', t: 'Brakhage', len: T(5, 2) }, { a: 'Broadcast', t: 'Come On Let\'s Go', len: T(3, 25) }, { a: 'Pram', t: 'Loco', len: T(4, 2) },
  { a: 'Laetitia Sadier', t: 'One Million Year Trip', len: T(5, 48) }, { a: 'The High Llamas', t: 'Checking In, Checking Out', len: T(4, 1) }, { a: 'Tortoise', t: 'Glass Museum', len: T(5, 26) },
  { a: 'Cornelius', t: 'Count Five or Six', len: T(3, 9) }, { a: 'Seefeel', t: 'Climactic Phase #3', len: T(7, 3) }];
LIB.pyxisCore = { name: 'Pyxis', groups: PYXIS_CORE.map(g => g.l) };
