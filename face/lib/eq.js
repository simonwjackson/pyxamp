.pragma library
// The EQ curve graph (113 x 19 sp), slider geometry and the built-in presets,
// ported from the mock's drawEQ. Pure. The EQ applies to Pyxamp's own audio
// only (the engine, through eq.set); while it does nothing (a foreign player is
// followed, or Pyxis plays on another device) the graph says so.

var GRAPH_W = 113, GRAPH_H = 19;
var BAND_LABELS = ["60", "170", "310", "600", "1K", "3K", "6K", "12K", "14K", "16K"];

// The one list of built-in presets: the PRESETS menu, the preference defaults
// and AUTO (spoken word -> Voice, decision 19) all read it.
var PRESETS = [
    { id: "flat", label: "Flat", pre: 0, bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { id: "voice", label: "Voice", pre: 0, bands: [-6, -4, -1, 2, 4, 4, 3, 1, -2, -4] },
    { id: "small", label: "Small speakers", pre: 0, bands: [6, 5, 3, 1, 0, 0, 1, 2, 3, 3] },
    { id: "night", label: "Night", pre: 0, bands: [-3, -2, 0, 1, 1, 0, -1, -3, -5, -6] },
];
function preset(id) {
    for (var i = 0; i < PRESETS.length; i++) if (PRESETS[i].id === id) return { id: id, label: PRESETS[i].label, pre: PRESETS[i].pre, bands: PRESETS[i].bands.slice() };
    return null;
}
var NAME_MAX = 40, USER_PRESETS_MAX = 20;

// What the engine gets: AUTO picks Voice for spoken word, else the user's bands.
// eq: the preference record { on, auto, pre, bands }.
function effective(eq, spokenWord) {
    if (eq.auto && spokenWord) { var v = preset("voice"); return { on: eq.on, pre: v.pre, bands: v.bands, auto: true }; }
    return { on: eq.on, pre: eq.pre, bands: eq.bands.slice(), auto: false };
}
// The eq.set payload (docs/seam-step2.md).
function wire(e) { return { on: e.on, preampDb: e.pre, bandsDb: e.bands.slice() }; }

function hex(c) { var n = parseInt(c.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255]; }
function mix(a, b, t) {
    var A = hex(a), B = hex(b), out = "#";
    for (var i = 0; i < 3; i++) { var v = Math.round(A[i] + (B[i] - A[i]) * t).toString(16); out += v.length < 2 ? "0" + v : v; }
    return out;
}

// One point per column: { y, color }. off: the EQ is off or does nothing here.
function curve(bands, off) {
    var pts = [];
    for (var i = 0; i < 10; i++) pts.push([6 + i * 11, bands[i]]);
    var out = [];
    for (var x = 0; x < GRAPH_W; x++) {
        var v;
        if (x <= pts[0][0]) v = pts[0][1];
        else if (x >= pts[9][0]) v = pts[9][1];
        else {
            var j = Math.floor((x - 6) / 11), p = (x - pts[j][0]) / 11;
            var a = pts[Math.max(0, j - 1)][1], b = pts[j][1], c = pts[j + 1][1], d = pts[Math.min(9, j + 2)][1];
            v = b + 0.5 * p * (c - a + p * (2 * a - 5 * b + 4 * c - d + p * (3 * (b - c) + d - a)));
        }
        var yy = Math.max(0, Math.min(18, Math.round(9 - v / 12 * 9)));
        var col = yy < 6 ? mix("#e85414", "#d8c020", yy / 6) : mix("#d8c020", "#2aa82a", (yy - 6) / 12);
        out.push({ y: yy, color: off ? mix(col, "#14142a", 0.65) : col });
    }
    return out;
}

function preampRow(pre) { return Math.round(9 - pre / 12 * 9); }

// The sliders map from their own rectangles (lib/drag.js), on both faces.
